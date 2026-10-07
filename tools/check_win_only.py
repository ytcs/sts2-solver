#!/usr/bin/env python3
"""Small real check of the S4 plumbing (rebuild S4) on the adopted networks: the win-only worth through the live Engine's three paths.

1. `Engine.solve`: a few contested boss fights from the corpus bench, searched under the linear return vs the win-only table (same seeds: paired).
2. `Engine.decide` (the live 5x32 search) on one fight start under the win-only table: q in the table's units.
3. `proposal.price` (the three potion arms with the save arm recorded and replayed for keep) on one fight start with potions.

  STS2_DEVICE=cuda python tools/check_win_only.py [--attempts 16] [--out evals/check_win_only.json]
"""
import argparse, json, os, sys, time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
FIGHTS = (7, 16, 188, 183)  # data/bench/corpus.json: Waterfall Giant, Vantom, Test Subject, Knowledge Demon (bench wins 0.38-0.88)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--attempts", type=int, default=16)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "check_win_only.json"))
    a = ap.parse_args()
    import numpy as np
    import sts2
    from agent import proposal
    from agent.engine import Engine
    bench = json.load(open(os.path.join(ROOT, "data", "bench", "corpus.json")))
    scen = [dict(bench[i]["scenario"], potions=[]) for i in FIGHTS]
    t0 = time.time()
    eng = Engine()
    print(f"engine {time.time() - t0:.0f}s, worth_ok={eng.worth_ok}", flush=True)
    out = dict(fights=[s["encounter"] for s in scen], attempts=a.attempts)
    rows = []
    for name, w in (("linear", None), ("win only", "win")):
        t = time.time()
        res = [eng.solve([s], attempts=a.attempts, seed=11, worth=proposal.win_only_worth(s["max_hp"]) if w else None)[0] for s in scen]
        rows.append((name, res, time.time() - t))
    out["solve"] = {}
    for name, res, secs in rows:
        out["solve"][name] = [dict(win=r["win"], hp_left_on_win=r["hp_left_on_win"], wins=r["wins"]) for r in res]
        print(f"{name:9s} ({secs:.0f}s): " + "  ".join(f"{s['encounter'][:14]} win {r['win']:.2f} HP {r['hp_left_on_win']:.0f}" for s, r in zip(scen, res)), flush=True)
    lin, wo = [np.array(r["wins"], float) for r in rows[0][1]], [np.array(r["wins"], float) for r in rows[1][1]]
    d = np.concatenate([b - a_ for a_, b in zip(lin, wo)])
    out["win_diff"] = dict(mean=float(d.mean()), se=float(d.std(ddof=1) / len(d) ** 0.5))
    print(f"win only - linear: win {d.mean():+.3f} +- {d.std(ddof=1) / len(d) ** 0.5:.3f} (paired attempts, {len(d)})", flush=True)

    s = scen[1]
    sim = sts2.Sim(json.dumps(s), 3)
    w = proposal.win_only_worth(s["max_hp"])
    t = time.time()
    dl = eng.decide(s, sim, budget=0.5, keep_potions=True, worth=w)
    dlin = eng.decide(s, sim, budget=0.5, keep_potions=True)
    print(f"decide win-only ({time.time() - t:.1f}s both): {dl['text']} q {[o['q'] for o in dl['options']]}; linear: {dlin['text']} q {[o['q'] for o in dlin['options']]}", flush=True)
    out["decide"] = dict(win_only=[o["q"] for o in dl["options"]], linear=[o["q"] for o in dlin["options"]])

    sp = bench[FIGHTS[1]]["scenario"]  # Vantom with its two potions
    sim = sts2.Sim(json.dumps(sp), 3)
    while sim.stage() != "play":
        sim.step(sim.legal()[0][0])
    t = time.time()
    pr = proposal.price(eng, sp, sim, worth=proposal.win_only_worth(sp["max_hp"]), attempts=8, seed=1)
    print(f"proposal ({time.time() - t:.1f}s):", flush=True)
    print("\n".join(proposal.table(pr, 1, "win only (check)")), flush=True)
    out["proposal"] = pr
    with open(a.out, "w") as f:
        json.dump(out, f, indent=1)


if __name__ == "__main__":
    main()
