#!/usr/bin/env python3
"""Do the tables need search depth 2? Card-reward-like screens (a deck from the eval set, 3 cards seen in other decks of the same character, plus skip)
priced against that deck's own encounter at depth 1 and depth 2 (same seeds). Reports, per screen, the variant differences at each depth, their
correlation, and how often both depths pick the same best option (or options within 2 paired se of each other).

  STS2_DEVICE=cuda python tools/depth_tables.py [--screens 20] [--attempts 128] [--out evals/depth_tables.json]
"""
import argparse, json, os, random, sys

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, ROOT)


def deck_of(s):
    d = dict(s)
    enc = d.pop("encounter")
    d["deck"] = [dict(id=c.rstrip("+"), upgrade=int(c.endswith("+"))) if isinstance(c, str) else c for c in d["deck"]]
    d["relics"] = [dict(id=r) if isinstance(r, str) else r for r in d["relics"]]
    d["potions"] = [dict(id=p) if isinstance(p, str) else p for p in d["potions"]]
    return d, enc


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--screens", type=int, default=20)
    ap.add_argument("--attempts", type=int, default=128)
    ap.add_argument("--seed", type=int, default=11)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "depth_tables.json"))
    a = ap.parse_args()
    from agent import macro
    from agent.engine import Engine
    rng = random.Random(a.seed)
    allsc = json.load(open(os.path.join(ROOT, "data", "train", "eval.json")))
    pool = {}
    for s in allsc:
        for c in s["deck"]:
            c = c if isinstance(c, str) else c["id"]
            if not c.startswith(("STRIKE", "DEFEND")):
                pool.setdefault(s["character"], set()).add(c.rstrip("+"))
    cands = [s for s in allsc if s["encounter"].endswith(("_ELITE", "_BOSS"))]
    rng.shuffle(cands)
    eng = Engine()
    fs = eng.solver.fs
    rows = []
    for s in cands[:a.screens]:
        deck, enc = deck_of(s)
        cards = rng.sample(sorted(pool[s["character"]]), 3)
        variants = [dict(name="skip")] + [dict(name=c, add=[c]) for c in cards]
        spec = dict(encounters=[enc], attempts=a.attempts, hp="current", hold=(), variants=variants)
        res = {}
        for leaf in (1, 2):
            fs.leaf_turns, fs.roll_cap = leaf, 60 * leaf
            _, summ = macro.evaluate(eng, deck, spec)
            res[leaf] = [(summ[vi]["win"] - summ[0]["win"], summ[vi]["dse"] or 0.0) for vi in range(1, len(variants))]
        fs.leaf_turns, fs.roll_cap = 2, 120
        best = {L: max(range(4), key=lambda i: 0.0 if i == 0 else res[L][i - 1][0]) for L in (1, 2)}
        d1, d2 = [x for x, _ in res[1]], [x for x, _ in res[2]]
        rows.append(dict(name=s["name"], enc=enc, cards=cards, d1=d1, d2=d2, se1=[e for _, e in res[1]], se2=[e for _, e in res[2]], best1=best[1], best2=best[2]))
        print(f"{s['name']:14s} {enc[:22]:22s} d1 {np.round(d1, 3)} d2 {np.round(d2, 3)}  best {best[1]} / {best[2]}", flush=True)
    d1 = np.concatenate([r["d1"] for r in rows])
    d2 = np.concatenate([r["d2"] for r in rows])
    agree = np.mean([r["best1"] == r["best2"] for r in rows])
    # the depth-1 pick is acceptable if depth 2 rates it within 2 paired se of depth 2's best
    def val2(r, i):
        return 0.0 if i == 0 else r["d2"][i - 1]
    near = np.mean([val2(r, r["best2"]) - val2(r, r["best1"]) <= 2 * max(r["se2"]) for r in rows])
    loss = np.mean([val2(r, r["best2"]) - val2(r, r["best1"]) for r in rows])
    summary = dict(screens=len(rows), attempts=a.attempts, corr=float(np.corrcoef(d1, d2)[0, 1]), mean_d1=float(d1.mean()), mean_d2=float(d2.mean()),
                   same_best=float(agree), within_2se=float(near), mean_regret_win=float(loss))
    print(json.dumps(summary, indent=1))
    json.dump(dict(summary=summary, rows=rows), open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
