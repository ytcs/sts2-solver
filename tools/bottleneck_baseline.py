#!/usr/bin/env python3
"""Community reference for the operator's deck bottleneck metrics (agent/bottleneck.py): the human's actions on the clean v7 replays stepped through
Sim.exact (tools/codex_replay.py `chain`), exact prefix only: turns before the turn of the fight's first divergence (diff, refused action, fallback
pick, final outcome mismatch). Fight outcome: the log's victory / loss; none logged (a reload or quit mid-fight) is in neither split. Per turn the live log's encoding (hand cost -1 for Unplayable and X-cost) and bottleneck.py's formulas; energy paid
is the log's cost_paid. Acts 0-based. Shares are pooled ratios, se clustered by run; `ref` holds per-(run, act) percentiles (>= MIN_TURNS turns).
usage: python tools/bottleneck_baseline.py build [--src target/codex/replays] [--out data/bottleneck_baseline.json] [--workers 12]
       python tools/bottleneck_baseline.py show [data/bottleneck_baseline.json] [--char IRONCLAD]
       python tools/bottleneck_baseline.py ours runs/<id> ... [--table data/bottleneck_baseline.json]"""
import argparse, collections, concurrent.futures, json, math, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
METRICS = ("energy_bound", "draw_bound", "wasted", "wasted_on", "basic", "unspent", "played", "hand")
# metric -> (numerator, denominator) of the per-fight sums
RATIO = dict(energy_bound=("poor", "left"), draw_bound=("starved", "turns"), wasted=("waste", "turns"), wasted_on=("waste", "starved"),
             basic=("basic_spent", "spent"), unspent=("en", "turns"), played=("plays", "turns"), hand=("hand", "turns"))
MIN_TURNS, MIN_RUNS = 10, 5


def kinds():
    enc = json.load(open(os.path.join(ROOT, "data", "catalog.json"), encoding="utf-8"))["encounters"]
    return {e["id"]: "event" if e["event"] else dict(Monster="hallway", Elite="elite", Boss="boss")[e["room"]] for e in enc}


def add_turn(s, en, costs, plays):
    """one end of turn, as bottleneck.analyze counts it (cost None: unknown, read as 1)"""
    en = en or 0
    s["turns"] += 1
    s["en"] += en
    s["hand"] += len(costs)
    s["plays"] += plays
    if en >= 1 and not any(c is None or 0 <= c <= en for c in costs):
        s["starved"] += 1
        s["waste"] += en
    for c in costs:
        if c is not None and c < 0:
            continue
        s["left"] += 1
        s["poor"] += en < (1 if c is None else c)


def add_play(s, cid, cost):
    if cost and cost > 0:
        s["spent"] += cost
        s["basic_spent"] += cost if cid.startswith(("STRIKE_", "DEFEND_")) else 0


def one_run(job):
    """fight records of one replay: exact-prefix turns and plays"""
    import codex_replay as C
    path, x = job

    def watch(sim, e, row):
        if "_stop" in row:
            return
        if any(f != "choice.unasked" for f in row["fields"]):
            row["_stop"] = row["actions"]
            return
        if e["t"] == "end_turn":
            sn = json.loads(sim.snapshot())
            row.setdefault("_obs", []).append(("end", row["actions"], sn["energy"], [-1 if "Unplayable" in (c.get("keywords") or []) else c.get("cost") for c in sn["hand"]]))
        elif e["t"] == "play":
            row.setdefault("_obs", []).append(("play", row["actions"], e["id"], e.get("cost_paid", e.get("cost_value"))))

    try:
        rows, _ = C.chain(path, watch=watch)
    except Exception as ex:  # noqa: BLE001
        return dict(run=x["run_hash"], error=repr(ex)[:120])
    out, kind = [], kinds()
    for r in rows:
        if "error" in r:
            out.append(dict(error=r["error"][:60]))
            continue
        obs = r.get("_obs", [])
        if r["failed"]:
            obs = [o for o in obs if o[1] < r["failed"]["step"]]
        elif "_stop" not in r and r["end"]["log"] not in (None, r["end"]["sim"]):
            ends = [i for i, o in enumerate(obs) if o[0] == "end"]
            obs = obs[:ends[-1]] if ends else obs
        turns, plays, n = [], [], 0
        for o in obs:
            if o[0] == "play":
                plays.append((o[2], o[3]))
                n += 1
            else:
                turns.append((o[2], o[3], n))
                n = 0
        out.append(dict(char=r["char"], act=r["act"] - 1, kind=kind.get(r["enc"], "other"), enc=r["enc"], flog=r["end"]["log"],
                        exact="_stop" not in r and not r["failed"], turns=turns, plays=plays))
    return dict(run=x["run_hash"], win=bool(x.get("win")), strong=bool(x.get("strong")), player=x.get("player_token"), fights=out)


def sums(f):
    s = collections.Counter()
    for en, costs, n in f["turns"]:
        add_turn(s, en, costs, n)
    for cid, c in f["plays"]:
        add_play(s, cid, c)
    return s


def pooled(clusters):
    """clusters: list of Counter sums (one per run) -> {metric: (value, se)}"""
    out = {}
    for m, (a, b) in RATIO.items():
        Y = [c[a] for c in clusters]
        X = [c[b] for c in clusters]
        sx = sum(X)
        if not sx:
            out[m] = (None, None)
            continue
        R, n = sum(Y) / sx, len(X)
        se = math.sqrt(n / (n - 1) * sum((y - R * xx) ** 2 for y, xx in zip(Y, X))) / sx if n > 1 else None
        out[m] = (round(R, 3), None if se is None else round(se, 3))
    return out


def pct(v, q):
    v = sorted(v)
    return round(v[min(len(v) - 1, int(q * len(v)))], 3)


def build(a):
    import codex_replay as C
    jobs = [(p, x) for p, x in C._clean(a.src, None) if 7 in (x.get("version") or [])]
    runs, errs = [], collections.Counter()
    with concurrent.futures.ProcessPoolExecutor(a.workers) as ex:
        for i, r in enumerate(ex.map(one_run, jobs, chunksize=4)):
            if "error" in r:
                errs["run: " + r["error"][:40]] += 1
                continue
            runs.append(r)
            if i % 100 == 0:
                print(f"  {i}/{len(jobs)}", flush=True)
    fights = [(r, f) for r in runs for f in r["fights"]]
    for r, f in fights:
        if "error" in f:
            errs["fight: " + f["error"][:40]] += 1
    fights = [(r, f) for r, f in fights if "error" not in f]
    flog = collections.Counter(f["flog"] for _, f in fights)
    split = dict(all=lambda r, f: True, strong=lambda r, f: r["strong"], run_won=lambda r, f: r["win"], run_lost=lambda r, f: not r["win"],
                 fight_won=lambda r, f: f["flog"] == "victory", fight_lost=lambda r, f: f["flog"] == "loss")
    cells = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    nf = collections.Counter()
    for r, f in fights:
        s = sums(f)
        if not s["turns"] and not s["spent"]:
            continue
        for kind in (f["kind"], "all") if f["kind"] in ("hallway", "elite", "boss") else ("all",):
            for sp, ok in split.items():
                if ok(r, f):
                    k = (f["char"], f["act"], kind, sp)
                    cells[k][r["run"]] += s
                    nf[k] += 1
    rows = []
    for k in sorted(cells, key=lambda k: (k[0] != "IRONCLAD", k[0], k[1], ("all", "hallway", "elite", "boss", "event", "other").index(k[2]), list(split).index(k[3]))):
        cl = list(cells[k].values())
        if len(cl) < MIN_RUNS:
            continue
        p = pooled(cl)
        rows.append(list(k) + [nf[k], sum(c["turns"] for c in cl), len(cl)] + [x for m in METRICS for x in p[m]])
    ref = collections.defaultdict(dict)
    per = collections.defaultdict(list)
    for r in runs:
        acc = collections.defaultdict(collections.Counter)
        for f in r["fights"]:
            if "error" not in f:
                acc[(f["char"], f["act"])] += sums(f)
        for k, s in acc.items():
            if s["turns"] >= MIN_TURNS:
                per[k].append(s)
    for (ch, act), L in sorted(per.items()):
        e = dict(runs=len(L))
        for m, (u, d) in RATIO.items():
            v = [s[u] / s[d] for s in L if s[d]]
            if v:
                e[m] = [pct(v, q) for q in (.1, .25, .5, .75, .9)]
        ref[ch][str(act)] = e
    out = dict(meta=dict(source="spire-codex clean A10 v0.111.0 replays with replay_version 7 (E104); Sim.exact on the human's actions, exact prefix",
                         runs=len(runs), fights=len(fights), exact_fights=sum(f["exact"] for _, f in fights), fight_end=dict(flog), dropped=dict(errs),
                         acts="0-based", min_turns_ref=MIN_TURNS, min_runs_cell=MIN_RUNS, kinds="hallway / elite / boss (data/catalog.json); event fights only in all",
                         metrics=dict(energy_bound="cards left in hand at end of turn with cost > energy left / cards left with cost >= 0",
                                      draw_bound="turns ending with >= 1 energy and no card of cost 0..energy left (cost -1: Unplayable and X-cost, never affordable)",
                                      wasted="energy left on draw-bound turns / all turns", wasted_on="energy left per draw-bound turn",
                                      basic="energy paid for STRIKE_* / DEFEND_* plays / all energy paid (human plays)",
                                      unspent="energy left per turn", played="human card plays per ended turn", hand="cards in hand at end of turn"),
                         ref="per (run, act) value: [p10, p25, p50, p75, p90] over runs with >= min_turns_ref ended turns in the act"),
               cols=["char", "act", "kind", "split", "fights", "turns", "runs"] + [f"{m}{s}" for m in METRICS for s in ("", "_se")],
               cells=rows, ref=ref)
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    with open(a.out, "w", encoding="utf-8", newline="\n") as g:
        g.write("{" + ",\n".join(f"{json.dumps(k)}: " + (json.dumps(v, separators=(",", ":")) if k != "cells" else "[\n" + ",\n".join(json.dumps(c, separators=(",", ":")) for c in v) + "]")
                                 for k, v in out.items()) + "}\n")
    print(f"{len(runs)} runs, {len(fights)} fights ({out['meta']['exact_fights']} exact to the end); dropped {dict(errs)}; fight ends {dict(flog)} -> {a.out}")
    show(argparse.Namespace(table=a.out, char=None, split=None))


SHOW = ("energy_bound", "draw_bound", "wasted", "basic", "unspent", "played", "hand")


def show(a):
    t = json.load(open(a.table, encoding="utf-8"))
    ix = {c: i for i, c in enumerate(t["cols"])}
    print("| char | act | kind | split | fights | turns | runs | " + " | ".join(SHOW) + " |\n|" + "---|" * (7 + len(SHOW)))
    for c in t["cells"]:
        if (a.char and c[0] != a.char) or (a.split and c[3] not in a.split.split(",")):
            continue
        print(f"| {c[0]} | {c[1] + 1} | {c[2]} | {c[3]} | {c[4]} | {c[5]} | {c[6]} | "
              + " | ".join("-" if c[ix[m]] is None else f"{c[ix[m]]:.2f} ({c[ix[m + '_se']] or 0:.2f})" for m in SHOW) + " |")


def ours(a):
    """the same metrics per act from operator run logs (end-turn actions carrying hand_left; plays priced by their base cost, as the operator's)"""
    sys.path.insert(0, ROOT)
    from agent import synergy
    tab = json.load(open(a.table, encoding="utf-8"))["ref"] if os.path.exists(a.table) else {}
    print("| run | act | turns | " + " | ".join(SHOW) + " |\n|" + "---|" * (3 + len(SHOW)))
    tot = collections.defaultdict(list)
    for d in a.runs:
        acc, act, ch, n = collections.defaultdict(collections.Counter), None, None, 0
        for l in open(os.path.join(d, "events.jsonl"), encoding="utf-8"):
            e = json.loads(l)
            if e["kind"] == "fight_start":
                act, ch, n = (e.get("scenario") or {}).get("act"), (e.get("scenario") or {}).get("character"), 0
            if e["kind"] != "action":
                continue
            t = str(e.get("text", ""))
            if t.startswith("play "):
                base = t.split()[1].split("#")[0].rstrip("+")
                add_play(acc[act], base, synergy.item_tags(base)[2])
                n += 1
            elif t.startswith("end turn"):
                if "hand_left" in e:
                    add_turn(acc[act], e.get("energy_left"), e.get("hand_cost") or [None] * len(e["hand_left"]), n)
                n = 0
        for act in sorted(k for k in acc if k is not None):
            s = acc[act]
            tot[act].append(s)
            p = pooled([s])
            ref = (tab.get(ch) or {}).get(str(act), {})
            print(f"| {os.path.basename(d.rstrip('/'))} | {act + 1} | {s['turns']} | "
                  + " | ".join("-" if p[m][0] is None else f"{p[m][0]:.2f}" + (f" [{ref[m][2]:.2f}/{ref[m][4]:.2f}]" if m in ref else "") for m in SHOW) + " |")
    for act, L in sorted(tot.items()):
        p = pooled(L)
        print(f"| all ({len(L)}) | {act + 1} | {sum(s['turns'] for s in L)} | " + " | ".join("-" if p[m][0] is None else f"{p[m][0]:.2f} ({p[m][1] or 0:.2f})" for m in SHOW) + " |")


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    b = sp.add_parser("build")
    b.add_argument("--src", default=os.path.join(ROOT, "target", "codex", "replays"))
    b.add_argument("--out", default=os.path.join(ROOT, "data", "bottleneck_baseline.json"))
    b.add_argument("--workers", type=int, default=12)
    s = sp.add_parser("show")
    s.add_argument("table", nargs="?", default=os.path.join(ROOT, "data", "bottleneck_baseline.json"))
    s.add_argument("--char")
    s.add_argument("--split")
    o = sp.add_parser("ours")
    o.add_argument("runs", nargs="+")
    o.add_argument("--table", default=os.path.join(ROOT, "data", "bottleneck_baseline.json"))
    a = ap.parse_args()
    dict(build=build, show=show, ours=ours)[a.cmd](a)


if __name__ == "__main__":
    main()
