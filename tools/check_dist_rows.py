#!/usr/bin/env python3
"""Equivalence of the two value paths of the search (M1b): the outcome head's class probabilities combined in Rust with the linear worth (dist rows) vs
the scalar value computed on the GPU (`rl/heads.py` `value`). Same fights, same seeds: per-fight results should agree up to float ties.

  STS2_DEVICE=cuda python tools/check_dist_rows.py [--ckpt models/solver_h128.pt] [--n 400] [--attempts 2] [--M 3 --K 8]
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--data", default=os.path.join(ROOT, "data", "train", "eval.json"))
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--attempts", type=int, default=2)
    ap.add_argument("--M", type=int, default=3)
    ap.add_argument("--K", type=int, default=8)
    a = ap.parse_args()
    from solver import Solver
    import torch
    fights = json.load(open(a.data))[:a.n]
    res = {}
    for dist in (False, True):
        S = Solver(a.ckpt, M=a.M, K=a.K, value_ckpts=None, dist=dist)
        t = time.time()
        r = S.solve(fights, attempts=a.attempts, seed=3)
        res[dist] = (r, time.time() - t, dict(S.fs.stats))
        del S
        torch.cuda.empty_cache()
    (r0, t0, s0), (r1, t1, s1) = res[False], res[True]
    w0 = np.array([w for x in r0 for w in x["wins"]], float)
    w1 = np.array([w for x in r1 for w in x["wins"]], float)
    e0 = np.array([e for x in r0 for e in x["ends_abs"]], float)
    e1 = np.array([e for x in r1 for e in x["ends_abs"]], float)
    same = (w0 == w1) & (e0 == e1)
    print(f"scalar: win {w0.mean():.4f} end HP {e0.mean():.2f} ({t0:.0f}s)   dist: win {w1.mean():.4f} end HP {e1.mean():.2f} ({t1:.0f}s)")
    print(f"identical fights: {same.mean():.3f} of {len(same)}; end_cap scalar {s0.get('end_cap')} dist {s1.get('end_cap')}; end_stuck {s0.get('end_stuck')} / {s1.get('end_stuck')}")
    d = w1 - w0
    print(f"win diff {d.mean():+.4f} +- {d.std(ddof=1) / len(d) ** 0.5:.4f}")


if __name__ == "__main__":
    main()
