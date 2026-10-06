#!/usr/bin/env python3
"""Fight-level A/B of the search's play-out depth (`leaf_turns`): the batch solver plays whole fights with each setting, same scenarios and seeds.

  STS2_DEVICE=cuda python tools/ab_leaf.py [--leafs 1,2] [--limit 1200] [--attempts 4] [--out evals/ab_leaf.json]

Fights: data/train/eval.json (held out from training), half of them at a random 25-100 % of max HP (as `tools/gate_util.py`). Per setting: win rate,
HP lost (a loss counts the whole start HP), wall time; paired differences vs the first setting with standard errors over scenarios, overall and per act.
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
from solver import Solver  # noqa: E402

CAPS = {1: 60, 2: 120, 3: 180}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--leafs", default="1,2")
    ap.add_argument("--limit", type=int, default=1200)
    ap.add_argument("--attempts", type=int, default=4)
    ap.add_argument("--seed", type=int, default=909)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "ab_leaf.json"))
    a = ap.parse_args()
    rng = np.random.default_rng(a.seed)
    fights = json.load(open(os.path.join(ROOT, "data", "train", "eval.json")))[:a.limit]
    for k, s in enumerate(fights):
        if k % 2:
            s["hp"] = max(1, int(round(s["max_hp"] * rng.uniform(0.25, 1.0))))
    solver = Solver()
    res = {}
    for leaf in [int(x) for x in a.leafs.split(",")]:
        solver.fs.leaf_turns, solver.fs.roll_cap = leaf, CAPS.get(leaf, 400)
        t = time.time()
        r = solver.solve(fights, attempts=a.attempts, seed=7)
        dt = time.time() - t
        win = np.array([x["win"] for x in r])
        lost = np.array([np.mean([(s["hp"] - e) / s["max_hp"] for e in x["ends"]]) if x["ends"] else 1.0 for x, s in zip(r, fights)])
        res[leaf] = dict(win=win, lost=lost, secs=dt)
        print(f"leaf {leaf}: win {win.mean():.4f}  HP lost {lost.mean():.4f} of max  ({dt:.0f}s)", flush=True)
    base = int(a.leafs.split(",")[0])
    acts = np.array([str(s.get("act")) for s in fights])
    out = {}
    for leaf, v in res.items():
        if leaf == base:
            continue
        row = {}
        for name, sel in [("all", np.ones(len(fights), bool))] + [(f"act {x}", acts == x) for x in sorted(set(acts))]:
            dw = v["win"][sel] - res[base]["win"][sel]
            dl = v["lost"][sel] - res[base]["lost"][sel]
            row[name] = dict(n=int(sel.sum()), win=float(dw.mean()), win_se=float(dw.std(ddof=1) / sel.sum() ** 0.5),
                             lost=float(dl.mean()), lost_se=float(dl.std(ddof=1) / sel.sum() ** 0.5))
            print(f"leaf {leaf} - leaf {base} [{name}, n {sel.sum()}]: win {dw.mean():+.4f} +- {row[name]['win_se']:.4f}   HP lost {dl.mean():+.4f} +- {row[name]['lost_se']:.4f}", flush=True)
        out[str(leaf)] = dict(diff=row, secs=v["secs"], win=float(v["win"].mean()), lost=float(v["lost"].mean()))
    out[str(base)] = dict(secs=res[base]["secs"], win=float(res[base]["win"].mean()), lost=float(res[base]["lost"].mean()))
    json.dump(out, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
