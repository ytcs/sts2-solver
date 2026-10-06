#!/usr/bin/env python3
"""Common random numbers check for `Solver.solve(groups=...)`: copies of one scenario evaluated with the same group id must come out identical (or
nearly: GPU batch composition can still move a network output by a rounding step), and a small deck change must have a much smaller standard error of
the difference than with independent seeds.

  STS2_DEVICE=cuda python tools/crn_check.py [--n 8] [--attempts 128]
"""
import argparse, json, os, sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl"))
from solver import Solver  # noqa: E402

EVAL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "train", "eval.json")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=8, help="encounters (boss / elite scenarios of the eval set)")
    ap.add_argument("--attempts", type=int, default=128)
    ap.add_argument("--copies", type=int, default=4)
    a = ap.parse_args()
    scen = [s for s in json.load(open(EVAL)) if any(t in json.dumps(s.get("encounter", s.get("name", ""))) for t in ("BOSS", "ELITE"))][:a.n]
    S = Solver(M=5, K=32)
    rows = []
    for i, sc in enumerate(scen):
        hurt = dict(sc, hp=max(1, int(sc["hp"] * 0.85)))  # a small variant: 15 % less HP at the start
        batch = [sc] * a.copies + [hurt]
        same = S.solve(batch, attempts=a.attempts, seed=7, groups=[0] * len(batch))
        indep = S.solve(batch, attempts=a.attempts, seed=7)
        e = lambda r: np.array(r["ends"])
        dup = [float(np.abs(e(same[c]) - e(same[0])).mean()) for c in range(1, a.copies)]
        d_same = e(same[-1]) - e(same[0])
        d_ind = e(indep[-1]) - e(indep[0])
        rows.append(dict(i=i, win=[round(r["win"], 3) for r in same[:a.copies]], win_indep=[round(r["win"], 3) for r in indep[:a.copies]],
                         dup_end_abs=dup, se_diff_crn=float(d_same.std(ddof=1) / len(d_same) ** 0.5), se_diff_indep=float(d_ind.std(ddof=1) / len(d_ind) ** 0.5)))
        r = rows[-1]
        print(f"{i} {sc.get('name', '?')[:30]:30s} copies(crn) {r['win']}  copies(indep) {r['win_indep']}  |end diff| of copies {np.round(dup, 2)}  "
              f"se of the HP-variant difference: crn {r['se_diff_crn']:.2f}  indep {r['se_diff_indep']:.2f}", flush=True)
    ratio = np.mean([r["se_diff_indep"] for r in rows]) / max(np.mean([r["se_diff_crn"] for r in rows]), 1e-9)
    print(f"mean |end HP diff| between copies: {np.mean([np.mean(r['dup_end_abs']) for r in rows]):.3f} HP; se ratio indep / crn {ratio:.2f} (variance x{ratio ** 2:.1f})")


if __name__ == "__main__":
    main()
