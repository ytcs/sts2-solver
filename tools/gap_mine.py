#!/usr/bin/env python3
"""Gap mining along exact human replays (plan item 4, E140): at every human decision state on the exact prefix of act-3 boss fights (community
replays, tools/codex_replay.py; a stop is admitted as in tools/round_roots.py) the live search (E136 `live`: cover K=32, exact turn, HP cap,
reserve 0, linear objective) decides from R seeds, aggregated as `Engine.decide` (mean q per action, argmax). A human move = the play / potion /
end turn plus the picks of the selection it opens (replayed on a resampled copy by card name + upgrade; when the options differ the search
resolves it, `hsel` counts); a selection state (the human's first pick after an agreed move) is decided too. Where the search's move differs
from the human's (same card instance up to identical copies, same target), both continuations are scored by N paired live play-outs (one
determinized copy and one job seed per pair). V(s0): live play-outs from each fight's first decision (the E136 measurement on this sample).
usage: tools/gap_mine.py prep --out target/gapmine/fights.json            (CPU; needs target/codex/replays + target/review/replay_fidelity.jsonl)
       tools/gap_mine.py run --fights F --out DIR [--rounds 8 --n 32 --fights-max 0]   (GPU; resumable per part)
       tools/gap_mine.py rescore --fights F --out DIR --only top.jsonl [--n 128 --seed 1]   (GPU; fresh pairs at the costliest disagreements)
       tools/gap_mine.py report --fights F --out DIR --data target/gapmine/gapmine.jsonl.gz --summary evals/gapmine/summary.json"""
import argparse, collections, glob, gzip, hashlib, json, os, re, sys, time

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
BOSSES = ("TEST_SUBJECT_BOSS", "QUEEN_BOSS", "AEONGLASS_BOSS")
QUOTA = dict(IRONCLAD=10_000, SILENT=5, DEFECT=4, NECROBINDER=3, REGENT=2)  # per boss; Ironclad all
TIE = 1e-4


def holdout(run, listed):
    return run.endswith("0") or run in listed


def prep(a):
    import codex_replay as cr
    from round_decks import _main_target
    from round_roots import admitted, outcome
    T = _main_target()
    idx = {x["run_hash"]: x for x in map(json.loads, open(os.path.join(T, "codex", "replays", "index.jsonl"), encoding="utf-8"))}
    rows = [json.loads(l) for l in open(os.path.join(T, "review", "replay_fidelity.jsonl"), encoding="utf-8")]
    listed = set(json.load(open(os.path.join(ROOT, "data", "bench", "contrast_holdout.json")))["runs"])
    last, act3 = collections.defaultdict(int), collections.defaultdict(list)
    for r in rows:
        last[r["run"]] = max(last[r["run"]], r["k"])
        if r["enc"] in BOSSES:
            act3[r["run"]].append(r["k"])
    pick = collections.defaultdict(list)
    for r in rows:
        if r["enc"] in BOSSES and "error" not in r and admitted(r) > 0:
            pick[(r["char"], r["enc"])].append(r)
    out = []
    for (ch, enc), rs in sorted(pick.items()):
        rs = sorted(rs, key=lambda r: hashlib.md5(f"{r['run']}:{r['k']}".encode()).hexdigest())[:QUOTA[ch]]
        for r in rs:
            _, (pre, ops) = cr.chain(os.path.join(T, "codex", "replays", r["run"] + ".jsonl.gz"), keep=r["k"])
            out.append(dict(id=f"{r['run']}:{r['k']}", run=r["run"], k=r["k"], char=ch, enc=enc, n=admitted(r), actions=r["actions"],
                            won=outcome(r, idx[r["run"]], last[r["run"]]) == "won", strong=bool(idx[r["run"]].get("strong")),
                            pair_pos=1 + sum(k < r["k"] for k in act3[r["run"]]), split="holdout" if holdout(r["run"], listed) else "train",
                            pre=pre, ops=[op for op in ops if op[0] != "expect"]))
    out.sort(key=lambda f: (f["char"] != "IRONCLAD", hashlib.md5(f["id"].encode()).hexdigest()))
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    json.dump(out, open(a.out, "w"))
    print(collections.Counter((f["char"], f["enc"]) for f in out), len(out), "fights,", sum(f["n"] for f in out), "admitted stops ->", a.out)


# ---- state helpers ----

def name_of(text):
    m = re.match(r"pick \d+ \((.+)\)$", text)
    return m.group(1) if m else text


def key_of(sim, a, sn=None):
    """the move up to identical copies: card (snapshot dict) + target, potion id + target, pick name + upgrade"""
    j = json.loads(sim.action_json(a))
    if "play" in j:
        sn = sn or json.loads(sim.snapshot())
        return "play " + json.dumps(sn["hand"][j["play"]["hand_pos"]], sort_keys=True) + f" -> {j['play'].get('target')}"
    if "use_potion" in j:
        sn = sn or json.loads(sim.snapshot())
        pid = next((p["id"] for p in sn["potions"] if p["slot"] == j["use_potion"]["slot"]), "?")
        return f"potion {pid} -> {j['use_potion'].get('target')}"
    if "pick" in j:
        return f"pick {name_of(dict(sim.legal()).get(a, ''))} +{sim.pick_upgrade(a)}"
    return next(iter(j))


def match_pick(sim, key):
    for a, t in sim.legal():
        if t in ("confirm",) or t.startswith("pick"):
            if key_of(sim, a) == key:
                return a
    return None


def tags(sim, a):
    """card id / type of a move and whether it kills an enemy or ends a Test Subject phase on the exact state (tag only)"""
    import codex_replay as cr
    sn = json.loads(sim.snapshot())
    j = json.loads(sim.action_json(a))
    card = sn["hand"][j["play"]["hand_pos"]]["id"] if "play" in j else None
    c = sim.copy()
    c.step(a)
    s2 = json.loads(c.snapshot())
    hp0 = [e["hp"] for e in sn["enemies"]]
    kill = any(h > 0 and i < len(s2["enemies"]) and s2["enemies"][i]["hp"] <= 0 for i, h in enumerate(hp0)) or c.outcome() == 1
    return dict(card=card, type=cr.card_type(card) if card else None, kill=bool(kill))


def phase(sn, enc):
    if enc == "TEST_SUBJECT_BOSS":
        e = next((e for e in sn["enemies"] if e["id"] == "TEST_SUBJECT"), None)
        return "-" if e is None else str(1 if e["max_hp"] < 150 else 2 if e["max_hp"] < 260 else 3)
    return "-"


def states(f):
    """(stop, root, info) per admitted human decision at a play stage; root = the exact replay state (copy), info: human move + features; plus
    the selection state after the human's play (`sel`: exact state before the first pick)"""
    import sts2
    sim, n = sts2.Sim.exact(json.dumps(f["pre"])), 0
    ops = f["ops"]
    for i, op in enumerate(ops):
        if op[0] == "apply":
            if n >= f["n"]:
                return
            if sim.stage() == "play" and sim.outcome() == 0:
                want = {k: v for k, v in op[1].items()}
                want = json.dumps({k: {x: y for x, y in v.items() if y is not None} if isinstance(v, dict) else v for k, v in want.items()}, sort_keys=True)
                act = next((a for a, _t in sim.legal() if json.dumps(json.loads(sim.action_json(a)), sort_keys=True) == want), None)
                if act is not None:
                    sn = json.loads(sim.snapshot())
                    root = sim.copy()
                    after = sim.copy()
                    after.apply(json.dumps(op[1]))
                    sel, picks = (after.copy() if after.stage() == "choice" else None), []
                    for op2 in ops[i + 1:]:
                        if op2[0] == "apply":
                            break
                        if op2[0] == "step" and after.stage() == "choice":
                            picks.append(key_of(after, op2[1]))
                            after.step(op2[1])
                    info = dict(stop=n, act=act, key=key_of(root, act, sn), text=dict(root.legal())[act], picks=picks, turn=sn.get("round"),
                                hp=sn["player"]["hp"], max_hp=sn["player"]["max_hp"], energy=sn.get("energy"), hand=len(sn["hand"]),
                                pots=len(sn["potions"]), phase=phase(sn, f["enc"]), enemy_hp=sum(max(e["hp"], 0) for e in sn["enemies"]))
                    yield root, sel, info
            n += 1
            sim.apply(json.dumps(op[1]))
        elif op[0] == "step":
            sim.step(op[1])
        elif op[0] == "sync":
            real = dict(json.loads(sim.snapshot()), **op[1])
            real.pop("relics", None)
            sim.sync(json.dumps(real))


# ---- search ----

def make_fs(a):
    from agent.engine import Engine
    from fastsearch import FastSearch
    eng = Engine()
    kw = dict(eng._fs_kw, pot_cost=0.0)
    fs = FastSearch(eng.solver.net, eng.fs.M, a.K or eng.fs.K, roots=a.roots, **kw)
    fs.warm()
    return fs


def decide_batch(fs, items, rounds, seed0):
    """items: [(scenario, sim)] -> per item {action: [q...]}, first-round record, per-round argmax actions (as Engine.decide, no potion gate)"""
    scen, starts, seeds = [], [], []
    for i, (sc, s) in enumerate(items):
        for r in range(rounds):
            scen.append(sc); starts.append(s); seeds.append(seed0 + 1_000_003 * i + r)
    old = (fs.max_steps, fs.record)
    fs.max_steps, fs.record = 1, True
    try:
        fs.run(scen, np.arange(len(scen), dtype=np.uint32), np.asarray(seeds, np.uint64), starts=starts)
        got = {}
        for idx, eng in fs._runs:
            for jl, j in enumerate(idx):
                acts, searched, opts, p, q, legal, _ex = eng.moves(jl)
                got[int(j)] = (int(acts[0]), bool(searched[0]), opts[0, :fs.M].tolist(), p[0, :fs.M].tolist(), q[0, :fs.M].tolist(), legal[0, :fs.M].astype(bool).tolist())
    finally:
        fs.max_steps, fs.record = old
    out = []
    for i, (sc, s) in enumerate(items):
        skip = {a for a, t in s.legal() if t.startswith("discard potion")}
        acc, firsts, searched_any, p0 = {}, [], False, {}
        for r in range(rounds):
            act, searched, opts, p, q, legal = got[i * rounds + r]
            firsts.append(act)
            searched_any |= searched
            for o, pp, qq, ok in zip(opts, p, q, legal):
                if ok and o not in skip and not np.isnan(qq):
                    acc.setdefault(o, []).append(float(qq))
                if ok and r == 0:
                    p0[o] = float(pp)
        best = max(acc, key=lambda o: np.mean(acc[o])) if acc else firsts[0]
        out.append(dict(action=int(best), searched=searched_any, q={int(o): [round(float(np.mean(v)), 4), len(v)] for o, v in acc.items()},
                        p={int(o): round(v, 3) for o, v in p0.items()}, firsts=[int(x) for x in firsts]))
    return out


def run_batch(fs, scen, starts, seeds):
    rows = fs.run(scen, np.arange(len(scen), dtype=np.uint32), np.asarray(seeds, np.uint64), starts=starts)
    return [(int(r[1] == 1), float(r[6]) if r[1] == 1 else 0.0) for r in rows]


def wpart(path, rows):
    tmp = path + ".tmp"
    with gzip.open(tmp, "wt") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")
    os.replace(tmp, path)


def rparts(pat):
    return [json.loads(l) for p in sorted(glob.glob(pat)) for l in gzip.open(p, "rt")]


def playouts(fs, a, st, rows, F):
    """paired play-outs at the disagreements in rows (one determinized copy and one job seed per pair) and V(s0) for the fights F
    -> {(n, kind): [[h win, h hp, s win, s hp, human pick unmatched]...]}, [(win, hp)] per s0 play-out"""
    # job specs only: a determinized Sim is large, so each batch's starts are built just before its run
    played = {n for n, kind, _sa, row in rows if kind == "play" and row["agree"]}
    # a selection is scored only after the search agreed on the play itself (else the play's pair already holds it)
    specs = [(n, kind, sa, row, j) for n, kind, sa, row in rows if not (row["agree"] or (kind == "sel" and n not in played)) for j in range(a.n)]
    specs += [(None, "s0", None, f, j) for f in F if any(x[0] is f for x in st) for j in range(a.n)]

    def build(spec):
        n, kind, sa, row, j = spec
        if kind == "s0":
            b = next(x for x in st if x[0] is row)[1].copy()
            assert b.determinize(5_555 + 1_000_003 * j)
            return [(row["pre"], b, a.seed + 99_991 + j)], None
        f, r, s, i = st[n]
        dseed = 1_000_003 * (j + 1) + 31 * i["stop"] + 17 + 7_777_777 * a.seed
        base = r.copy()
        assert base.determinize(dseed)
        base.step(i["act"])
        h = base.copy()
        if kind == "play":
            x = r.copy(); x.determinize(dseed); x.step(sa)
        else:
            x = base.copy()
            m = match_pick(x, row["s_key"])
            if m is None:
                return [], None
            x.step(m)
        hsel = 0
        for pk in i["picks"]:
            if h.stage() != "choice":
                break
            m = match_pick(h, pk)
            if m is None:
                hsel = 1
                break
            h.step(m)
        sd = a.seed + 77_777 + 1_000_003 * j + n
        return [(f["pre"], h, sd), (f["pre"], x, sd)], hsel

    po, tail, per = collections.defaultdict(list), [], max(1, a.batch // 2)
    for b0 in range(0, len(specs), per):
        jobs, owners = [], []
        for spec in specs[b0:b0 + per]:
            js, hsel = build(spec)
            if js:
                owners.append((spec[0], spec[1], hsel, len(jobs)))
                jobs += js
        res = run_batch(fs, [x[0] for x in jobs], [x[1] for x in jobs], [x[2] for x in jobs]) if jobs else []
        for n, kind, hsel, o in owners:
            if kind == "s0":
                tail.append(res[o])
            else:
                po[(n, kind)].append([res[o][0], res[o][1], res[o + 1][0], res[o + 1][1], hsel])
    return po, tail


def run(a):
    fights = json.load(open(a.fights))
    if a.fights_max:
        fights = fights[:a.fights_max]
    os.makedirs(a.out, exist_ok=True)
    fs = make_fs(a)
    t0 = time.time()
    for c0 in range(0, len(fights), a.chunk):
        part = os.path.join(a.out, f"part_{c0 // a.chunk:03d}.jsonl.gz")
        if os.path.exists(part):
            continue
        tc = time.time()
        F = fights[c0:c0 + a.chunk]
        st = []  # (fight, root, sel, info)
        for f in F:
            st += [(f, r, s, i) for r, s, i in states(f)]
        # decide at every play state (determinized root) and every selection state
        items, ref = [], []
        for n, (f, r, s, i) in enumerate(st):
            d = r.copy()
            assert d.determinize(7_919 + i["stop"])
            items.append((f["pre"], d)); ref.append((n, "play"))
            if s is not None and i["picks"]:
                items.append((f["pre"], s)); ref.append((n, "sel"))
        dec = decide_batch(fs, items, a.rounds, a.seed)
        rows = []
        for (n, kind), d in zip(ref, dec):
            f, r, s, i = st[n]
            sim = r if kind == "play" else s
            hk = i["key"] if kind == "play" else i["picks"][0]
            sk = key_of(sim, d["action"])
            texts = dict(sim.legal())
            row = dict(fight=f["id"], run=f["run"], k=f["k"], char=f["char"], enc=f["enc"], split=f["split"], pair_pos=f["pair_pos"], won=f["won"],
                       kind=kind, **{x: y for x, y in i.items() if x not in ("act",)},
                       h_key=hk, s_key=sk, s_text=texts.get(d["action"]), s_action=d["action"], agree=hk == sk,
                       q=d["q"], p=d["p"], firsts_agree=float(np.mean([key_of(sim, x) == hk for x in d["firsts"]])),
                       h_in_opts=any(key_of(sim, o) == hk for o in d["q"]))
            hq = [v[0] for o, v in d["q"].items() if key_of(sim, o) == hk]
            row["q_margin"] = round(d["q"][d["action"]][0] - max(hq), 4) if hq and d["action"] in d["q"] else None
            # an exact tie (moves of one turn in another order, the exact-turn solve) is no disagreement: argmax broke it by option order
            row["tie"] = not row["agree"] and row["q_margin"] is not None and row["q_margin"] <= TIE
            row["agree"] = row["agree"] or row["tie"]
            if kind == "play":
                row["h_tags"], row["s_tags"] = tags(r, i["act"]), tags(r, d["action"])
            rows.append((n, kind, d["action"], row))
        po, tail = playouts(fs, a, st, rows, F)
        out = []
        for n, kind, sa, row in rows:
            if (n, kind) in po:
                row["pairs"] = po[(n, kind)]
            out.append(row)
        k = 0
        for f in F:
            if any(x[0] is f for x in st):
                out.append(dict(kind="s0", fight=f["id"], run=f["run"], k=f["k"], char=f["char"], enc=f["enc"], split=f["split"], won=f["won"],
                                pair_pos=f["pair_pos"], n_stops=f["n"], actions=f["actions"], playouts=[list(x) for x in tail[k:k + a.n]]))
                k += a.n
        wpart(part, out)
        nd = sum(1 for _n, _k, _s, r_ in rows if not r_["agree"])
        print(f"part {c0 // a.chunk}: {len(F)} fights, {len(st)} states, {len(rows)} decisions, {nd} disagreements, {2 * sum(len(v) for v in po.values()) + len(tail)} play-outs; "
              f"{time.time() - tc:.0f}s (total {(time.time() - t0) / 60:.1f} min)", flush=True)
    print("done", flush=True)


def rescore(a):
    """fresh paired play-outs (seed offset a.seed, a.n pairs) at listed disagreements ({fight, stop, kind, s_key, s_action} per line):
    the costliest patterns re-measured without the selection bias of ranking them on their first estimate"""
    want = [json.loads(l) for l in open(a.only)]
    fights = {f["id"]: f for f in json.load(open(a.fights))}
    fs = make_fs(a)
    out, t0 = [], time.time()
    for fid in sorted({w["fight"] for w in want}):
        f = fights[fid]
        by = {i["stop"]: (r, s, i) for r, s, i in states(f)}
        st, rows = [], []
        for w in (w for w in want if w["fight"] == fid):
            r, s, i = by[w["stop"]]
            st.append((f, r, s, i))
            rows.append((len(st) - 1, w["kind"], w["s_action"], dict(w, agree=False)))
            if w["kind"] == "sel":  # the play agreed: mark it so the selection is scored
                rows.append((len(st) - 1, "play", None, dict(agree=True)))
        po, _ = playouts(fs, a, st, rows, [])
        for n, kind, _sa, row in rows:
            if (n, kind) in po:
                out.append(dict(row, pairs=po[(n, kind)]))
    wpart(os.path.join(a.out, f"rescore_{a.seed}.jsonl.gz"), out)
    print(f"rescored {len(out)} of {len(want)} in {(time.time() - t0) / 60:.1f} min", flush=True)


# ---- report ----

def cls(r):
    if r["kind"] == "sel":
        return "selection"
    h, s = r["h_key"], r["s_key"]
    if h.startswith("potion") or s.startswith("potion"):
        return ("potion: both, different" if h.startswith("potion") and s.startswith("potion") else
                "potion: human drinks" if h.startswith("potion") else "potion: search drinks")
    if h == "end_turn" or s == "end_turn":
        return "end turn: human ends" if h == "end_turn" else "end turn: search ends"
    if h.rsplit(" -> ", 1)[0] == s.rsplit(" -> ", 1)[0]:
        return "target"
    ht, st = r["h_tags"]["type"], r["s_tags"]["type"]
    if ht == st:
        return f"card order: {str(ht).lower()} vs {str(st).lower()}"
    if "Power" in (ht, st):
        return "power timing"
    return f"card type: human {str(ht).lower()} / search {str(st).lower()}"


def pdiff(pairs):
    p = np.array(pairs, float)
    dw, dh = p[:, 0] - p[:, 2], p[:, 1] - p[:, 3]
    se = lambda d: float(d.std(ddof=1) / np.sqrt(len(d))) if len(d) > 1 else float("nan")
    return float(dw.mean()), se(dw), float(dh.mean()), se(dh)


def short(k):
    if k.startswith("play "):
        c, t = k[5:].rsplit(" -> ", 1)
        c = json.loads(c)
        return f"{c['id']}{'+' if c.get('upgrade') else ''}" + ("" if t == "None" else f"->e{t}")
    return k


def report(a):
    fights = {f["id"]: f for f in json.load(open(a.fights))}
    rows = rparts(os.path.join(a.out, "part_*.jsonl.gz"))
    s0 = {r["fight"]: r for r in rows if r["kind"] == "s0"}
    dec = [r for r in rows if r["kind"] != "s0"]
    for r in dec:
        if r.get("pairs"):
            r["dwin"], r["dwin_se"], r["dhp"], r["dhp_se"] = pdiff(r["pairs"])
            r["class"] = cls(r)
        elif not r["agree"]:
            r["class"] = cls(r)
    os.makedirs(os.path.dirname(os.path.abspath(a.data)), exist_ok=True)
    with gzip.open(a.data, "wt") as fh:
        for r in rows:
            if r["kind"] != "s0":
                r = dict(r, recipe=dict(fights_file=os.path.basename(a.fights), fight=r["fight"], stop=r["stop"]))
            fh.write(json.dumps(r) + "\n")
    se = lambda d: float(np.std(d, ddof=1) / np.sqrt(len(d))) if len(d) > 1 else float("nan")

    def gap(fs_):
        h = np.array([s0[x]["won"] for x in fs_], float)
        v = np.array([np.mean([p[0] for p in s0[x]["playouts"]]) for x in fs_])
        return dict(n=len(fs_), human=round(h.mean(), 3), live=round(v.mean(), 3), gap=round((h - v).mean(), 3), se=round(se(h - v), 3))

    groups = dict(all=list(s0), ironclad=[x for x in s0 if s0[x]["char"] == "IRONCLAD"], others=[x for x in s0 if s0[x]["char"] != "IRONCLAD"])
    summary = dict(doc="tools/gap_mine.py report: live search vs human moves along exact act-3 boss replays (E140). dwin = P(win | human move, live "
                       "play-on) - P(win | search move, live play-on), paired; per-fight = sum over the class's disagreements / fights.",
                   fights=len(s0), states=sum(r["kind"] == "play" for r in dec), selection_states=sum(r["kind"] == "sel" for r in dec),
                   gap={g: gap(v) for g, v in groups.items() if v})
    for b in BOSSES:
        v = [x for x in s0 if s0[x]["enc"] == b]
        if v:
            summary["gap"][b] = gap(v)
    summary["gap"]["ironclad_exact_to_end"] = gap([x for x in groups["ironclad"] if s0[x]["n_stops"] == s0[x]["actions"]])

    def table(sel, nf, by):
        T = collections.defaultdict(list)
        for r in sel:
            T[by(r)].append(r)
        out = []
        for k, v in sorted(T.items(), key=lambda kv: -sum(r.get("dwin", 0) for r in kv[1])):
            sc = [r for r in v if "dwin" in r]
            d = np.array([r["dwin"] for r in sc]) if sc else np.zeros(0)
            hp = np.array([r["dhp"] for r in sc]) if sc else np.zeros(0)
            out.append(dict(key=k, n=len(v), scored=len(sc), mean_dwin=round(float(d.mean()), 4) if len(d) else None, se=round(se(d), 4) if len(d) > 1 else None,
                            mean_dhp=round(float(hp.mean()), 2) if len(hp) else None, per_fight=round(float(d.sum()) / nf, 4),
                            human_better=int(sum(r["dwin"] > 2 * r["dwin_se"] for r in sc)), search_better=int(sum(r["dwin"] < -2 * r["dwin_se"] for r in sc))))
        return out

    for g, fl in groups.items():
        if not fl:
            continue
        fl = set(fl)
        sel = [r for r in dec if r["fight"] in fl]
        dis = [r for r in sel if not r["agree"]]
        nf = len(fl)
        d = np.array([r["dwin"] for r in dis if "dwin" in r])
        summary[g] = dict(fights=nf, decisions=len(sel), disagreements=len(dis), agree_rate=round(1 - len(dis) / max(len(sel), 1), 3),
                          human_not_in_search_opts=sum(not r["h_in_opts"] for r in dis),
                          explained_per_fight=round(float(d.sum()) / nf, 4),
                          by_class=table(dis, nf, lambda r: r["class"]),
                          by_boss=table(dis, nf, lambda r: r["enc"]),
                          by_phase=table(dis, nf, lambda r: (f"TS{r['phase']} " if r["enc"] == "TEST_SUBJECT_BOSS" else "") +
                                         f"turn {'1-2' if (r['turn'] or 1) <= 2 else '3-5' if r['turn'] <= 5 else '6-9' if r['turn'] <= 9 else '10+'}"),
                          by_kill=table(dis, nf, lambda r: "a move kills / ends a phase" if (r.get("h_tags") or {}).get("kill") or (r.get("s_tags") or {}).get("kill")
                                        else "no kill"),
                          by_hp=table(dis, nf, lambda r: f"hp {min(int(4 * r['hp'] / max(r['max_hp'], 1)), 3)}/4"),
                          by_margin=table(dis, nf, lambda r: "near tie (search q margin < 0.02)" if (r["q_margin"] is not None and r["q_margin"] < 0.02)
                                          else "human move not proposed" if not r["h_in_opts"] else "clear (margin >= 0.02)"))
    pat = collections.defaultdict(list)
    for r in dec:
        if "dwin" in r:
            pat[(r["class"], short(r["h_key"]), short(r["s_key"]), r["enc"])].append(r)
    top = sorted(pat.items(), key=lambda kv: -sum(r["dwin"] for r in kv[1]))
    pk = lambda kv: dict(class_=kv[0][0], human=kv[0][1], search=kv[0][2], boss=kv[0][3], n=len(kv[1]), sum_dwin=round(sum(r["dwin"] for r in kv[1]), 3),
                         mean_dhp=round(float(np.mean([r["dhp"] for r in kv[1]])), 1),
                         example=max(kv[1], key=lambda r: r["dwin"])["fight"] + f":{max(kv[1], key=lambda r: r['dwin'])['stop']}")
    summary["top_human_better"] = [pk(kv) for kv in top[:15]]
    summary["top_search_better"] = [pk(kv) for kv in top[::-1][:10]]
    os.makedirs(os.path.dirname(os.path.abspath(a.summary)), exist_ok=True)
    json.dump(summary, open(a.summary, "w"), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k in ("fights", "states", "selection_states", "gap")}, indent=1))
    for g in ("ironclad", "all"):
        if g not in summary:
            continue
        s = summary[g]
        print(f"\n== {g}: {s['fights']} fights, {s['decisions']} decisions, {s['disagreements']} disagreements (agree {s['agree_rate']}), "
              f"human move not proposed {s['human_not_in_search_opts']}, explained per fight {s['explained_per_fight']:+.4f}")
        for t in ("by_class", "by_boss", "by_phase", "by_kill", "by_hp", "by_margin"):
            print(f"-- {t}")
            for x in s[t]:
                print(f"  {str(x['key']):42s} n {x['n']:5d} scored {x['scored']:5d} dwin {x['mean_dwin'] if x['mean_dwin'] is not None else '-':>8} "
                      f"+- {x['se'] if x['se'] is not None else '-':>7} dhp {x['mean_dhp'] if x['mean_dhp'] is not None else '-':>6} per-fight {x['per_fight']:+.4f} "
                      f"H>S {x['human_better']} S>H {x['search_better']}")
    print("\n-- top patterns (human better)")
    for x in summary["top_human_better"][:10]:
        print(" ", x)
    print("-- top patterns (search better)")
    for x in summary["top_search_better"][:5]:
        print(" ", x)


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    p = sp.add_parser("prep"); p.add_argument("--out", required=True)
    p = sp.add_parser("run")
    p.add_argument("--fights", required=True); p.add_argument("--out", required=True)
    p.add_argument("--rounds", type=int, default=8); p.add_argument("--n", type=int, default=32)
    p.add_argument("--fights-max", type=int, default=0); p.add_argument("--chunk", type=int, default=10)
    p.add_argument("--batch", type=int, default=2048); p.add_argument("--roots", type=int, default=2048)
    p.add_argument("--K", type=int, default=0); p.add_argument("--seed", type=int, default=0)
    p = sp.add_parser("rescore")
    p.add_argument("--fights", required=True); p.add_argument("--out", required=True); p.add_argument("--only", required=True)
    p.add_argument("--n", type=int, default=128); p.add_argument("--seed", type=int, default=1)
    p.add_argument("--batch", type=int, default=2048); p.add_argument("--roots", type=int, default=2048); p.add_argument("--K", type=int, default=0)
    p = sp.add_parser("report")
    p.add_argument("--fights", required=True); p.add_argument("--out", required=True)
    p.add_argument("--data", required=True); p.add_argument("--summary", required=True)
    a = ap.parse_args()
    dict(prep=prep, run=run, rescore=rescore, report=report)[a.cmd](a)


if __name__ == "__main__":
    main()
