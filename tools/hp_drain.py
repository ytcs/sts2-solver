#!/usr/bin/env python3
"""Per-fight HP leak of recorded baseline runs: actual end HP of each won fight vs the predictor's E[end HP | win] at its start.
Predictors given as checkpoint names (models/); the same fights for each, so differences are paired."""
import argparse, os, sys
import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path[:0] = [os.path.join(ROOT, "rl"), ROOT, os.path.join(ROOT, "tools")]
import postmortem as PM  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tags", nargs="+")
    ap.add_argument("--root", action="append", default=[], help="roots holding evals/baseline/<tag>.jsonl + target/baseline/<tag>/")
    ap.add_argument("--preds", default="solver_r6.pt,solver_r8c.pt")
    a = ap.parse_args()
    from predictor import Predictor
    fights = []
    for tag in a.tags:
        root = next((r for r in a.root + [ROOT] if os.path.exists(os.path.join(r, "evals", "baseline", f"{tag}.jsonl"))), ROOT)
        for run in PM.load_runs(tag, root):
            fights += [dict(f, tag=tag) for f in run.fights if f["hp1"] is not None and f["hp1"] > 0]
    print(f"{len(fights)} won fights from {', '.join(a.tags)}")
    ex = {}
    for name in a.preds.split(","):
        pw, eh = PM.expected_hp(Predictor(os.path.join(ROOT, "models", name), batch=1024), fights)
        ex[name] = np.array([min(f["sc"]["max_hp"], e) - f["hp1"] for f, e in zip(fights, eh)])
        e = ex[name]
        print(f"{name}: excess HP lost per fight {e.mean():+.2f} +- {e.std(ddof=1) / np.sqrt(len(e)):.2f}; P(win) mean {np.mean(pw):.3f}")
        for kind in ("_WEAK", "_NORMAL", "_ELITE", "_BOSS"):
            m = np.array([f["enc"].endswith(kind) for f in fights])
            if m.any():
                print(f"   {kind[1:].lower():7s} n {m.sum():4d}: {e[m].mean():+.2f} +- {e[m].std(ddof=1) / np.sqrt(m.sum()):.2f}")
    names = list(ex)
    if len(names) == 2:
        d = ex[names[1]] - ex[names[0]]
        print(f"{names[1]} - {names[0]}: {d.mean():+.2f} +- {d.std(ddof=1) / np.sqrt(len(d)):.2f} HP per fight")


if __name__ == "__main__":
    main()
