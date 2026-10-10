"""Death post-mortem for headless baseline runs (`tools/baseline.py` result rows + per-run events.jsonl under --root).

usage: python tools/postmortem.py TAG [--root DIR] [--seeds S1,S2] [--parts traj,macro,fatal,check] [--n 512] [--attempts 64]
         [--arms strong,full,base] [--k-base 512] [--k-strong 1024] [--k-check 4096] [--roots 128]
       python tools/postmortem.py TAG --cf [--refight] [--k 8] [--top 2] [--worker i --workers P] [--games 1] [--port 15900]
       python tools/postmortem.py TAG --table
Value V of a run state: paired run-model rollouts (BasePolicy, n per state, same seeds across compared states); `act` = P(clear the
act the state is in) ranks, `cont` (price's gate surrogate, late rule) is kept. Rollout rooms = the rooms the run visited after that
point in its act, the act template past the death row; counters at defaults.
- traj: per non-fatal fight, actual end HP vs the predictor's E[end HP | win] at its start; drop = V(actual) - V(expected HP).
  Boss fights: both states enter the next act after the ancient's heal.
- macro: card-reward and rest-site screens re-priced with V (rest site: rest + the smiths price screened), chosen - best.
- fatal: batch search P(win) at the fatal fight's start (cover + exact turn, K futures, one round per decision): base = k-base,
  no potions (the baseline searches without potions; proposals not modelled); strong = k-strong, potions in the search; full =
  strong at max HP. check: k-check on deaths with full < 0.5.
- --cf: the run's own agent on the headless server (ORACLE_DLL, STS2_PCK), restored by re-sending the logged commands, continued
  until it dies or passes the floor the run died on. Suspects = up to --top leak fights with a significant drop; K continuations
  at the actual HP and K after `x heal` to the expectation (fresh agent seeds, same game seed). --refight: K replays of the
  fatal fight from its start (same game RNG: P(win) on the realized draw, which the run lost).
Labels: combat HP leak (cf edited - actual pass > 2 se), combat loss at the fatal fight (strong or refight P(win) >= 0.5), macro
decision (priced drop >= 0.1), else unresolved (deck flag when full < 0.5). Cache: evals/baseline/<TAG>_postmortem*.jsonl
(PM_CACHE = file to append to).
"""
import argparse, json, math, os, re, sys, time, zlib

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))

from agent import pools, proposal, runmodel as R, screen as scr, tracker  # noqa: E402
from agent import price as PR  # noqa: E402

MAP_T = {"Monster": "M", "Elite": "E", "Unknown": "?", "RestSite": "R", "Rest": "R", "Shop": "$", "Merchant": "$", "Treasure": "T"}
HDR = re.compile(r"A(\d+) F(\d+) \w+ A\d+ HP (-?\d+)/(\d+) G(\d+)")


def se(x):
    x = np.asarray(x, float)
    return float(x.std(ddof=1) / math.sqrt(len(x))) if len(x) > 1 else float("nan")


def act_name(act, encs):
    names = pools.act_names(act)
    return next((n for n in names if any(e in pools.pool(n, k) for e in encs for k in ("weak", "regular", "elite", "boss"))), names[0])


class Run:
    def __init__(self, row, events_path):
        self.row, self.seed = row, row["seed"]
        self.ev = [json.loads(l) for l in open(events_path, encoding="utf-8")]
        self.fights, self.rooms, self.screens = [], {}, []
        cur, row_, act = None, None, 0
        used = {}
        for i, e in enumerate(self.ev):
            k = e["kind"]
            if k == "macro" and e.get("screen") == "MAP":
                m = HDR.search(e["state"])
                act = int(m.group(1)) - 1 if m else act
                lab = dict(scr.options(e["state"])).get(str(e.get("choice", "")).split(" ")[0], "")
                mm = re.match(r"^(\w+) r(\d+)c\d+", lab)
                if mm and mm.group(1) in MAP_T:
                    row_ = int(mm.group(2))
                    self.rooms.setdefault(act, []).append((row_, MAP_T[mm.group(1)]))
                elif mm and mm.group(1) == "Boss":
                    row_ = 99
            elif k == "fight_start":
                sc = e["scenario"]
                cur = dict(id=e["id"], i=i, sc=sc, enc=e["encounter"], hp0=sc["hp"], act=sc.get("act") or 0, row=row_, floor=e.get("floor"), hp1=None,
                           seen=[x["enc"] for x in self.fights if x["act"] == (sc.get("act") or 0)])
                self.fights.append(cur)
            elif k == "potion_commit" and cur is not None:
                used.setdefault(cur["id"], []).append(e.get("id"))
            elif k == "fight_end" and cur is not None and e["id"] == cur["id"]:
                cur["hp1"] = (e.get("hp") or [0])[0]
                cur["max1"] = (e.get("hp") or [0, cur["sc"]["max_hp"]])[1]
            elif k == "macro" and e.get("screen") in ("CARD_REWARD", "RESTSITE"):
                self.screens.append(dict(i=i, e=e, fight=len(self.fights) - 1, row=row_, act=act))
        for f in self.fights:
            f["used"] = used.get(f["id"], [])
        self.dead = row["result"] != "win" and bool(self.fights) and self.fights[-1]["hp1"] is not None and self.fights[-1]["hp1"] <= 0
        self.fatal = self.fights[-1] if self.dead else None
        self.price = [e for e in self.ev if e["kind"] == "price"]

    def act_encs(self, act, upto=None):
        return [f["enc"] for f in self.fights[:upto] if f["act"] == act]

    def bosses(self, act):
        return [f["enc"] for f in self.fights if f["act"] == act and f["enc"].endswith("_BOSS")]

    def future(self, act, after_row):
        seen = [(r, t) for r, t in self.rooms.get(act, []) if after_row is None or r > after_row]
        last = max([r for r, _ in self.rooms.get(act, [])] + [after_row or 0])
        reached_boss = bool(self.bosses(act))
        tpl = R.TEMPLATE.get(act, R.TEMPLATE[1]).split()
        tail = [] if reached_boss else [(r, tpl[r - 1]) for r in range(last + 1, len(tpl) + 1)]
        return [t for _, t in seen + tail]

    def state(self, fi, hp, deck=None, gold=None, potions=None, row=None):
        """run state right after fight fi (index into self.fights) at HP hp, before its rewards"""
        f = self.fights[fi]
        sc, act = f["sc"], f["act"]
        bosses = None
        if f["enc"].endswith("_BOSS") and act < R.LAST_ACT:
            act += 1
            hp = min(sc["max_hp"], hp + int(R.HEAL_ANCIENT * (sc["max_hp"] - hp)))
            row = 0
            encs = self.act_encs(act)
        elif f["enc"].endswith("_BOSS"):
            bosses = self.bosses(act)[self.bosses(act).index(f["enc"]) + 1:]
            encs, row = self.act_encs(act, fi + 1), 99
        else:
            encs = self.act_encs(act, fi + 1)
            row = f["row"] if row is None else row
        name = act_name(act, encs + self.bosses(act))
        seen = {}
        for e in encs:
            k = PR._kind_of(name, e)
            if k:
                seen.setdefault(k, []).append(e)
        pots = list(potions) if potions is not None else [p["id"] for p in sc["potions"]]
        if potions is None:
            for u in f.get("used", []):
                if u in pots:
                    pots.remove(u)
        st = R.RunState(sc, act, name, int(max(1, hp)), sc["max_hp"], sc.get("gold", 0) if gold is None else gold, deck or sc["deck"],
                        [r["id"] for r in sc["relics"]], pots, sc.get("max_potion_slots", 2),
                        (tracker.POTION_START, tracker.OFFSET_START, dict(tracker.UNKNOWN_BASE), 0), seen,
                        self.bosses(act) if bosses is None else bosses, None, None,
                        len(seen.get("weak", [])) + len(seen.get("regular", [])))
        chain = self.future(act, row)
        st.nodes = {(j, 0): dict(type=t, children=[(j + 1, 0)] if j + 1 < len(chain) else [], visited=False) for j, t in enumerate(chain)}
        st.frontier = [(0, 0)] if chain else []
        return st


def rollout_values(pred, jobs, n, seed=0):
    """jobs: list of (RunState, first-fn or None, pair key); same key -> same rollout seeds. -> per job dict of arrays"""
    ro = R.Rollouts(pred, 4)
    pol = R.BasePolicy()
    pol.gates = True
    states, seeds, firsts, owner = [], [], [], []
    for ji, (st, first, key) in enumerate(jobs):
        for j in range(n):
            states.append(st.copy())
            seeds.append((zlib.crc32(key.encode()) % 100_000) * 100_003 + seed + j)
            firsts.append(first)
            owner.append(ji)
    ro.run(states, seeds, pol=pol, firsts=firsts)
    owner = np.array(owner)
    out = []
    for ji, (st, _f, _k) in enumerate(jobs):
        ss = [s for s, o in zip(states, owner) if o == ji]
        out.append(dict(cont=np.array([PR.arrival(s.gates, "late") if getattr(s, "gates", None) else 0.0 for s in ss]),
                        act=np.array([1.0 if (s.end is None or s.end[0] > st.act or s.end[1] == "won") else 0.0 for s in ss]),
                        floors=np.array([s.floors for s in ss], float)))
    return out


def expected_hp(pred, fights):
    import predictor as PP
    P = pred.fight_start([f["sc"] for f in fights], 8, 1000)
    return PP.p_win(P), PP.end_hp(P)


def part_traj(run, pred, n):
    last = len(run.fights) - 1 if run.dead else len(run.fights)
    if not run.fights:
        return dict(fights=[])
    pw, eh = expected_hp(pred, run.fights)
    jobs, meta = [], []
    for fi, f in enumerate(run.fights[:last]):
        if f["hp1"] is None or f["hp1"] <= 0:
            continue
        exp1 = int(round(min(f["sc"]["max_hp"], eh[fi])))
        meta.append(dict(floor=f["floor"], enc=f["enc"], hp0=f["hp0"], hp1=f["hp1"], exp1=exp1, p_win=round(float(pw[fi]), 3),
                         excess=exp1 - f["hp1"], potions=len(f["used"])))
        if abs(exp1 - f["hp1"]) < 2:
            continue
        key = f"{run.seed}|{f['id']}"
        jobs.append((run.state(fi, f["hp1"]), None, key))
        jobs.append((run.state(fi, exp1), None, key))
        meta[-1]["job"] = len(jobs) - 2
    vals = rollout_values(pred, jobs, n) if jobs else []
    for m in meta:
        j = m.pop("job", None)
        if j is None:
            m.update(d_cont=0.0, se_cont=0.0, d_act=0.0, se_act=0.0)
            continue
        a, b = vals[j], vals[j + 1]
        m.update(v_cont=round(float(b["cont"].mean()), 4), d_cont=float((a["cont"] - b["cont"]).mean()), se_cont=se(a["cont"] - b["cont"]),
                 v_act=round(float(b["act"].mean()), 3), d_act=float((a["act"] - b["act"]).mean()), se_act=se(a["act"] - b["act"]))
    out = dict(fights=meta)
    if run.dead:
        fi = len(run.fights) - 1
        exp1 = int(round(min(run.fatal["sc"]["max_hp"], eh[fi])))
        v = rollout_values(pred, [(run.state(fi, exp1), None, f"{run.seed}|fatal")], n)[0]
        out["fatal"] = dict(exp1=exp1, p_win_pred=round(float(pw[fi]), 3), v_cont=float(v["cont"].mean()), se_cont=se(v["cont"]),
                            v_act=float(v["act"].mean()), se_act=se(v["act"]))
    return out


def _price_before(run, i, screen):
    return next((e for e in reversed(run.ev[:i]) if e["kind"] == "price" and e["screen"] == screen), None)


def part_macro(run, pred, n):
    rows = []
    picks = []
    last_fight = None
    for s in run.screens:
        if s["fight"] < 0:
            continue
        if s["fight"] != last_fight:
            picks, last_fight = [], s["fight"]
        e, state = s["e"], s["e"]["state"]
        m = HDR.search(state)
        labels = dict(scr.options(state))
        choice = labels.get(str(e.get("choice", "")).split(" ")[0], "")
        if not m or any(v.startswith("proceed") for v in labels.values()):
            continue
        f = run.fights[s["fight"]]
        if f["enc"].endswith("_BOSS") and e["screen"] == "RESTSITE":
            continue
        deck = [dict(c) for c in f["sc"]["deck"]] + picks
        belt = [PR._ident(b) for b in scr.belt(state) if b != "-"]
        st = run.state(s["fight"], int(m.group(3)), deck=deck, gold=int(m.group(5)), potions=[p for p in belt if p in PR._ids("potions")], row=s["row"])
        opts = PR.options(st, state)
        if e["screen"] == "RESTSITE":
            pe = _price_before(run, s["i"], "RESTSITE")
            keep = set(pe["options"]) if pe else {"rest"}
            opts = [o for o in opts if o[0] in keep]
            played = "rest" if choice.lower().startswith("rest") else (pe or {}).get("best") if pe and pe["best"] != "rest" else "smith"
        else:
            nm = choice.split("(")[0].strip()
            played = next((lb for lb, _ in opts if lb.lower() == nm.lower()), "skip")
        if e["screen"] == "CARD_REWARD":
            cm = re.match(r"^(.+?)\(", choice)
            cid, up = PR._card_id(cm.group(1)) if cm else (None, 0)
            if cid:
                picks.append({"id": cid, "upgrade": up})
        if len(opts) < 2:
            continue
        key = f"{run.seed}|m{s['i']}"
        vals = rollout_values(pred, [(st, fn, key) for _, fn in opts], n)
        res = {lb: v for (lb, _), v in zip(opts, vals)}
        best = max(res, key=lambda lb: (res[lb]["act"].mean(), res[lb]["cont"].mean()))
        pl = played if played in res else None
        row = dict(floor=f"A{m.group(1)} F{m.group(2)}", screen=e["screen"], hp=int(m.group(3)), options=list(res), played=played, best=best,
                   v={lb: [round(float(v["cont"].mean()), 4), round(float(v["act"].mean()), 3)] for lb, v in res.items()})
        if pl:
            d, da = res[pl]["cont"] - res[best]["cont"], res[pl]["act"] - res[best]["act"]
            row.update(d_cont=float(d.mean()), se_cont=se(d), d_act=float(da.mean()), se_act=se(da))
        rows.append(row)
    return dict(screens=rows)


def search_arm(fights, K, attempts, roots, potions, full=False, max_steps=1000, seed=1):
    from fastsearch import FastSearch
    from model import load
    from solver import DEFAULT_CKPT
    net = load(DEFAULT_CKPT)
    import torch
    cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
    scen, worth = [], []
    for f in fights:
        sc = dict(f["sc"])
        if not potions:
            sc["potions"] = []
        if full:
            sc["hp"] = sc["max_hp"]
        scen.append(sc)
        worth.append(proposal.fight_objective(sc, (), f.get("seen", ()))[0])
    fs = FastSearch(net, 5, K, max_steps=max_steps, roots=roots, groups=1, amp=cuda, cover=True, exact_turn=True)
    S = len(scen)
    js = np.tile(np.arange(S, dtype=np.uint32), attempts)
    jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
    t = time.time()
    r = fs.run(scen, js, jd, worth=worth if any(w is not None for w in worth) else None)
    dt = time.time() - t
    out = r[:, 1].reshape(attempts, S).T
    st = fs.stats
    del fs
    if cuda:
        torch.cuda.empty_cache()
    return [dict(p=float((o == 1).mean()), se=se(o == 1), unfinished=int((o == 0).sum()), n=attempts) for o in out], dt, \
        {k: int(v) for k, v in st.items() if k in ("end_cap", "end_stuck", "end_loop", "ex_triggered", "ex_capped")}


def load_runs(tag, root, seeds=None):
    rows = [json.loads(l) for l in open(os.path.join(root, "evals", "baseline", f"{tag}.jsonl"), encoding="utf-8")]
    out = []
    for r in rows:
        if seeds and r["seed"] not in seeds:
            continue
        p = os.path.join(root, "target", "baseline", tag, r["seed"], "events.jsonl")
        if os.path.exists(p):
            out.append(Run(r, p))
    return out


def cache_path(tag):
    return os.environ.get("PM_CACHE") or os.path.join(ROOT, "evals", "baseline", f"{tag}_postmortem.jsonl")


def read_cache(tag):
    import glob
    c = {}
    paths = [cache_path(tag)] if os.environ.get("PM_CACHE") else glob.glob(os.path.join(ROOT, "evals", "baseline", f"{tag}_postmortem*.jsonl"))
    for p in sorted(set(paths)):
        if os.path.exists(p):
            for l in open(p, encoding="utf-8"):
                x = json.loads(l)
                c[(x["seed"], x["part"])] = x
                if x["part"].startswith("fatal_"):
                    c.setdefault((x["seed"], "fatal"), {})[x["part"][6:]] = x
    return c


def write(tag, seed, part, data):
    with open(cache_path(tag), "a", encoding="utf-8") as f:
        f.write(json.dumps(dict(data, seed=seed, part=part)) + "\n")


def confound(run):
    if not run.dead:
        return None
    tags = set(run.row.get("tags", []))
    start = run.fatal["i"]
    hits = [e["kind"] for e in run.ev[start:] if e["kind"] in ("unplayable", "baseline_error", "baseline_exception")]
    if hits or "fallback" in tags and any(e["kind"] == "baseline_error" for e in run.ev[start:]):
        return f"fatal fight confounded: {', '.join(sorted(set(hits)))}"
    return None


SIG = 2.0
COMBAT_P = 0.5
VAL = "act"
MACRO_D = 0.1


def verdict(run, c, key=VAL):
    """candidate root-cause events, largest value drop first (paired drop < -SIG se); leak suspects carry their counterfactual"""
    t, m, fa = c.get((run.seed, "traj")), c.get((run.seed, "macro")), c.get((run.seed, "fatal"))
    cfs = cf_summary(c)
    cands = []
    for f in (t or {}).get("fights", []):
        d, e = f.get("d_" + key, 0), f.get("se_" + key) or 0
        if d < 0 and d < -SIG * e and f["exp1"] > f["hp1"]:
            x = cfs.get((run.seed, f["floor"]))
            cf = None
            if x and x["actual"] is not None and x["edited"] is not None:
                diff = x["edited"] - x["actual"]
                cf = dict(x, diff=diff, ok=bool(x["se"] and diff > SIG * x["se"]) or (x["se"] == 0 and diff > 0))
            cands.append(dict(d=d, kind="combat HP leak", what=f"{f['floor']} {f['enc'][:18]} {f['hp0']}->{f['hp1']} (exp {f['exp1']})", cf=cf))
    for s in (m or {}).get("screens", []):
        d, e = s.get("d_" + key, 0), s.get("se_" + key) or 0
        if d < 0 and d < -SIG * e:
            cands.append(dict(d=d, kind="macro decision", what=f"{s['screen'].lower()} {s['floor']} {s['played'][:14]} vs {s['best'][:14]}", cf=None))
    if fa and t and t.get("fatal") and fa.get("strong"):
        pb = fa["strong"]["p"]
        v = 1.0 if run.fatal["enc"].endswith("_BOSS") else t["fatal"]["v_" + key]
        cands.append(dict(d=-pb * v, kind="combat loss at the fatal fight", what=f"{run.fatal['floor']} {run.fatal['enc'][:18]} strong {pb:.2f}",
                          cf=None, ok=pb >= COMBAT_P))
    cands.sort(key=lambda x: x["d"])
    return cands


def labels_of(run, c):
    why = confound(run)
    if why:
        return ["unresolved (harness: " + why + ")"], []
    cands = verdict(run, c)
    labs = []
    rf = cf_summary(c).get((run.seed, run.fatal["floor"], "refight"))
    if rf and rf["p"] >= COMBAT_P:
        labs.append("combat loss at the fatal fight")
    for x in cands:
        if x["kind"] == "combat HP leak" and x["cf"] and x["cf"]["ok"]:
            labs.append(f"combat HP leak at {x['what'].split(' (')[0]}")
        elif x["kind"] == "combat loss at the fatal fight" and x.get("ok") and "combat loss at the fatal fight" not in labs:
            labs.append("combat loss at the fatal fight")
        elif x["kind"] == "macro decision" and x["d"] <= -MACRO_D:
            labs.append(f"macro decision {x['what']}")
    if not labs:
        fa = c.get((run.seed, "fatal"), {})
        note = "; strong full-HP P(win) < 0.5 (deck)" if fa.get("full") and fa["full"]["p"] < COMBAT_P else ""
        labs = ["unresolved" + note]
    stuck = [m.group(0) for e in run.ev if e["kind"] == "stuck_option" for m in [re.search(r"A\d+ F\d+", e.get("screen", ""))] if m]
    if stuck:
        labs[-1] += f" (harness caveat: option banned after a server error at {', '.join(stuck)})"
    return labs, cands


def contrib(run, c):
    """gold, deck, belt at death; rest-site choices; summed HP lost beyond the predictor's expectation"""
    m = HDR.search(run.row.get("final_screen") or "")
    belt = re.search(r"pots\[([^\]]*)\]", run.row.get("final_screen") or "")
    held = [p for p in (belt.group(1).split(", ") if belt else []) if p.strip() not in ("-", "")]
    sc = run.fatal["sc"]
    rests, seen = [], set()
    for s in run.screens:
        h = HDR.search(s["e"]["state"])
        if s["e"]["screen"] != "RESTSITE" or not h or h.group(2) in seen:
            continue
        seen.add(h.group(2))
        rest = next((n for n, t in scr.options(s["e"]["state"]) if t.lower().startswith("rest")), None)
        rests.append(("R" if str(s["e"].get("choice", "")).split(" ")[0] == rest else "S") + h.group(3))
    t = c.get((run.seed, "traj"), {}).get("fights", [])
    leak = sum(max(0, f["exp1"] - f["hp1"]) for f in t)
    return (f"G{m.group(5) if m else '?'} deck {len(sc['deck'])} (+{sum(1 for x in sc['deck'] if x.get('upgrade'))}) belt {len(held)} unused, "
            f"fatal start {len(sc['potions'])} pots used {len(run.fatal['used'])}; rests {' '.join(rests) or '-'}; leak {leak} HP over {len(t)} fights")


def kind_of(lab):
    for k in ("combat HP leak", "combat loss at the fatal fight", "macro decision", "unresolved"):
        if lab.startswith(k):
            return k
    return lab


def fmt_table(runs, c):
    out = [f"{'seed':9s} {'death':32s} {'HP':>7s} {'live':>7s} {'(a)':>5s} {'(b)':>5s} {'(b)full':>7s} {'check':>5s} | labels | suspects: drop in P(clear act); cf passed actual/edited (n)"]
    kinds, cfs = {}, cf_summary(c)
    for run in runs:
        if not run.dead:
            lab = "unresolved (no fatal fight: " + str(run.row.get("died_at"))[:24] + ")"
            out.append(f"{run.seed:9s} {str(run.row.get('died_at'))[:32]:32s} | {lab}")
            kinds["unresolved"] = kinds.get("unresolved", 0) + 1
            continue
        fa, ck = c.get((run.seed, "fatal"), {}), c.get((run.seed, "check"), {})
        g = lambda k: f"{fa[k]['p']:.2f}" if fa.get(k) else "-"  # noqa: E731
        rf = cfs.get((run.seed, run.fatal["floor"], "refight"))
        live = f"{rf['p']:.2f}/{rf['n']}" if rf else "-"
        labs, cands = labels_of(run, c)
        for k in {kind_of(x) for x in labs}:
            kinds[k] = kinds.get(k, 0) + 1
        sus = []
        for x in cands[:3]:
            s = f"{x['d']:+.3f} {x['kind'].split()[-1] if x['kind'] != 'combat loss at the fatal fight' else 'fatal'} {x['what']}"
            if x.get("cf"):
                s += f" [cf pass {x['cf']['actual']:.2f}/{x['cf']['edited']:.2f} n{x['cf']['n'][0]}/{x['cf']['n'][1]}, floor {x['cf']['top'][1] - x['cf']['top'][0]:+.1f}+-{x['cf']['dtop_se'] or 0:.1f}]"
            sus.append(s)
        out.append(f"{run.seed:9s} {run.fatal['floor'] + ' ' + run.fatal['enc'][:24]:32s} {run.fatal['hp0']:3d}/{run.fatal['sc']['max_hp']:<3d} "
                   f"{live:>7s} {g('base'):>5s} {g('strong'):>5s} {g('full'):>7s} {(f'{ck['p']:.2f}' if ck.get('p') is not None else '-'):>5s} | "
                   f"{'; '.join(labs)} | {'; '.join(sus)} | {contrib(run, c)}")
    out.append("label kinds (a death can carry several): " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items(), key=lambda x: -x[1])))
    return "\n".join(out)


def _ready():
    from agent import bridge
    s = ""
    for _ in range(500):
        s = bridge.call("s")
        if not scr.busy(s):
            return s
        time.sleep(0.02)
    return s


def _line(s, i=1):
    x = s.split("\n")
    return x[i] if len(x) > i else ""


def replay_prefix(ev, stop):
    """re-send the run's game commands up to event index stop on the current bridge; -> (screen, header mismatches)"""
    from agent import bridge, potions
    sc, bad = None, []
    for e in ev[:stop]:
        k = e["kind"]
        if k == "fight_start":
            sc = e["scenario"]
        elif k == "macro":
            s = _ready()
            if s.split("\n")[:2] != e["state"].split("\n")[:2]:
                bad.append((_line(e["state"])[:60], _line(s)[:60]))
                if len(bad) > 3:
                    break
            bridge.call("a " + e["choice"])
        elif k == "action":
            _ready()
            bridge.call("do " + (json.dumps({"choose": e["picks"]}) if e.get("kind_") == "choose" else potions.to_game_action(sc, e["json"])))
        elif k == "potion_commit":
            _ready()
            bridge.call("do " + potions.to_game_action(sc, e["json"]))
        elif k == "divergence" and "selection, the simulator does not" in str(e.get("what")):
            _ready()
            bridge.call("a 0")
    return _ready(), bad


def cf_jobs(runs, c, top, k):
    """per death: up to `top` leak fights (significant P(clear act) drop, actual HP below expectation), K continuations per arm"""
    jobs = []
    for run in runs:
        if not run.dead or confound(run):
            continue
        fs = [f for f in c.get((run.seed, "traj"), {}).get("fights", []) if f.get("d_act", 0) < -SIG * (f.get("se_act") or 0) and f["exp1"] > f["hp1"]]
        for f in sorted(fs, key=lambda f: f["d_act"])[:top]:
            jobs += [dict(seed=run.seed, floor=f["floor"], target=f["exp1"], arm=arm, k=kk) for kk in range(k) for arm in ("actual", "edited")]
    return sorted(jobs, key=lambda j: j["k"])


def counterfactual(a, runs, jobs):
    """continue the same agent (fresh agent seeds, same game seed) from just after a fight, at the actual HP or the target HP (x heal),
    until it dies or passes the floor the run died on"""
    import baseline as B
    from agent import bridge
    from agent.engine import Engine
    from predictor import Predictor
    by = {r.seed: r for r in runs}
    engine = Engine()
    engine.solver.fs = None
    import gc, torch
    gc.collect()
    torch.cuda.empty_cache()
    pred = Predictor(engine.solver.net)
    slots = [None] * a.games
    try:
        while jobs or any(slots):
            for i in range(a.games):
                if slots[i] is None and jobs:
                    j = jobs.pop(0)
                    run = by[j["seed"]]
                    f = next(x for x in run.fights if x["floor"] == j["floor"])
                    stop = f["i"] if j["arm"] == "refight" else next(n for n, e in enumerate(run.ev) if e["kind"] == "fight_end" and e["id"] == f["id"]) + 1
                    mac = run.row.get("macro", "")
                    price_n = int(re.search(r"price n(\d+)", mac).group(1)) if "price" in mac else 0
                    rounds = int(re.search(r"(\d+) rounds", run.row.get("budget", "16 rounds")).group(1))
                    out = os.path.join(ROOT, "target", "postmortem", a.tag, f"{j['seed']}_{j['floor'].replace(' ', '')}_{j['arm']}{j['k']}")
                    bridge.OVERRIDE = f"127.0.0.1:{a.port + i}"
                    g = B.Game(j["seed"], a.port + i, out, engine, pred, rounds, "ironclad", None, price_n)
                    aseed = f"{j['seed']}#cf{j['k']}"
                    g.h._seed = zlib.crc32(aseed.encode()) * 1000
                    g.macro.seed = aseed
                    bridge.OVERRIDE = g.ep
                    s, bad = replay_prefix(run.ev, stop)
                    m = HDR.search(s)
                    hp = int(m.group(3)) if m else None
                    if j["arm"] == "edited" and hp is not None and j["target"] > hp:
                        bridge.call(f"x heal {j['target'] - hp}")
                        m = HDR.search(_ready())
                    with open(os.path.join(g.dir, "events.jsonl"), "w", encoding="utf-8") as fh:
                        fh.write("".join(json.dumps(e) + "\n" for e in run.ev[:stop]))
                    g.cf = dict(j, replay_hp=hp, start_hp=int(m.group(3)) if m else None, mismatches=len(bad), death_floor=run.row["floor"])
                    g.t_cf, g.top = time.time(), int(m.group(2)) if m else 0
                    slots[i] = g
                g = slots[i]
                if g is None:
                    continue
                bridge.OVERRIDE = g.ep
                try:
                    end = g.step()
                except Exception:  # noqa: BLE001
                    g.errors += 1
                    end = g.errors > 50
                m = HDR.search(g.h.last_state or "")
                g.top = max(g.top, int(m.group(2)) if m else 0)
                passed = g.top > g.cf["death_floor"] or g.macro.won is not None
                if end or passed:
                    r = g.result()
                    row = dict(g.cf, top=g.top, passed=bool(passed), died_at=None if passed else r["died_at"], own_s=r["own_s"],
                               wall_s=round(time.time() - g.t_cf, 1))
                    write(a.tag, row["seed"], f"cfrow|{row['floor']}|{row['arm']}{row['k']}", row)
                    print(f"  {row['seed']} {row['floor']} {row['arm']}{row['k']}: HP {row['replay_hp']}->{row['start_hp']} reached F{row['top']} "
                          f"{'passed' if passed else row['died_at']} ({row['wall_s'] / 60:.1f} min)", flush=True)
                    slots[i] = None
    finally:
        bridge.OVERRIDE = None
        for g in slots:
            if g is not None:
                g.proc.kill()


def cf_summary(c):
    groups = {}
    for (seed, part), x in c.items():
        if part.startswith("cfrow"):
            groups.setdefault((x["seed"], x["floor"]), {}).setdefault(x["arm"], []).append(x)
    out = {}
    for key, arms in groups.items():
        if "refight" in arms:
            r = [float(x["passed"]) for x in arms["refight"]]
            out[key + ("refight",)] = dict(n=len(r), p=float(np.mean(r)), se=se(r))
        a, e = arms.get("actual", []), arms.get("edited", [])
        if not a and not e:
            continue
        pa, pe = [float(x["passed"]) for x in a], [float(x["passed"]) for x in e]
        out[key] = dict(n=(len(a), len(e)), actual=float(np.mean(pa)) if pa else None, edited=float(np.mean(pe)) if pe else None,
                        se=math.sqrt(np.var(pa) / max(1, len(pa) - 1) + np.var(pe) / max(1, len(pe) - 1)) if len(pa) > 1 and len(pe) > 1 else None,
                        top=(float(np.mean([x["top"] for x in a])) if a else None, float(np.mean([x["top"] for x in e])) if e else None),
                        dtop_se=math.sqrt(np.var([x["top"] for x in a]) / max(1, len(a) - 1) + np.var([x["top"] for x in e]) / max(1, len(e) - 1)) if len(a) > 1 and len(e) > 1 else None)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tag")
    ap.add_argument("--root", default=ROOT)
    ap.add_argument("--seeds")
    ap.add_argument("--parts", default="traj,macro,fatal,check")
    ap.add_argument("--n", type=int, default=512)
    ap.add_argument("--attempts", type=int, default=64)
    ap.add_argument("--k-base", type=int, default=512)
    ap.add_argument("--k-strong", type=int, default=1024)
    ap.add_argument("--k-check", type=int, default=4096)
    ap.add_argument("--arms", default="strong,full,base")
    ap.add_argument("--roots", type=int, default=128, help="concurrent fights at K 512 (scaled by 512/K)")
    ap.add_argument("--check-attempts", type=int, default=64)
    ap.add_argument("--table", action="store_true")
    ap.add_argument("--cf", action="store_true", help="counterfactual continuations for the suspects (needs traj in the cache)")
    ap.add_argument("--top", type=int, default=2)
    ap.add_argument("--refight", action="store_true", help="with --cf: K replays of the fatal fight from its start by the run's own agent")
    ap.add_argument("--k", type=int, default=8)
    ap.add_argument("--worker", type=int, default=0)
    ap.add_argument("--workers", type=int, default=1)
    ap.add_argument("--games", type=int, default=4)
    ap.add_argument("--port", type=int, default=15900)
    a = ap.parse_args()
    runs = load_runs(a.tag, a.root, set(a.seeds.split(",")) if a.seeds else None)
    c = read_cache(a.tag)
    if a.cf:
        jobs = [dict(seed=r.seed, floor=r.fatal["floor"], target=0, arm="refight", k=kk) for kk in range(a.k) for r in runs if r.dead and not confound(r)]             if a.refight else cf_jobs(runs, c, a.top, a.k)
        done = {k for k in c if k[1].startswith("cfrow")}
        jobs = [x for x in jobs[a.worker::a.workers] if (x["seed"], f"cfrow|{x['floor']}|{x['arm']}{x['k']}") not in done]
        print(f"worker {a.worker}/{a.workers}: {len(jobs)} continuations", flush=True)
        return counterfactual(a, runs, jobs)
    parts = a.parts.split(",")
    if not a.table:
        pred = None
        if {"traj", "macro"} & set(parts):
            from predictor import Predictor
            from solver import PREDICTOR_CKPT
            pred = Predictor(PREDICTOR_CKPT, batch=1024)
        for part, fn in (("traj", part_traj), ("macro", part_macro)):
            if part not in parts:
                continue
            for run in runs:
                if (run.seed, part) in c:
                    continue
                t = time.time()
                d = fn(run, pred, a.n)
                d["seconds"] = round(time.time() - t, 1)
                write(a.tag, run.seed, part, d)
                c[(run.seed, part)] = d
                print(f"{run.seed} {part} {d['seconds']:.0f}s", flush=True)
        dead = [r for r in runs if r.dead and not confound(r)]
        if "fatal" in parts:
            spec = dict(base=(a.k_base, False, False), strong=(a.k_strong, True, False), full=(a.k_strong, True, True),
                        deep=(a.k_strong, False, False), pots=(a.k_base, True, False))
            for arm in a.arms.split(","):
                K, pots, full = spec[arm]
                todo = [r for r in dead if arm not in c.get((r.seed, "fatal"), {})]
                if not todo:
                    continue
                got, dt, st = search_arm([r.fatal for r in todo], K, a.attempts, max(8, a.roots * 512 // K), pots, full)
                print(f"fatal {arm}: K {K}, {len(todo)} fights x {a.attempts}, {dt:.0f}s, {st}", flush=True)
                for r, g in zip(todo, got):
                    write(a.tag, r.seed, "fatal_" + arm, dict(g, K=K))
                    c.setdefault((r.seed, "fatal"), {})[arm] = dict(g, K=K)
        if "check" in parts:
            todo = [r for r in dead if (r.seed, "check") not in c and c.get((r.seed, "fatal"), {}).get("full", {}).get("p", 1) < COMBAT_P]
            if todo:
                got, dt, st = search_arm([r.fatal for r in todo], a.k_check, a.check_attempts, max(8, a.roots * 512 // a.k_check), True, True)
                print(f"check: K {a.k_check}, {len(todo)} fights, {dt:.0f}s, {st}", flush=True)
                for r, g in zip(todo, got):
                    write(a.tag, r.seed, "check", dict(g, K=a.k_check))
                    c[(r.seed, "check")] = dict(g, K=a.k_check)
    print(fmt_table(runs, c))


if __name__ == "__main__":
    main()
