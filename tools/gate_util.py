#!/usr/bin/env python3
"""Gate for HP-worth-conditioned networks (`rl/ppo.py --util-prob`): does the search under the fight's curve U do better on E[U(end HP)] than under the
linear return, and do the new networks lose nothing in linear mode (fight predictions and every table use it)?

  STS2_DEVICE=cuda python tools/gate_util.py --new target/runs/ub/ckpt.pt,target/runs/uc/ckpt.pt,target/runs/ud/ckpt.pt [--attempts 4] [--out evals/gate_util.json]

Fights: data/train/eval.json (held out from training); half of them start at a random 25-100 % of max HP (where the curve binds). Each fight gets one of
`--curves` HP-worth curves drawn with a held-out seed. Arms: old / new networks x linear / curve objective. Scores: E[U] (a loss counts 0, a win U(end HP)),
win rate, mean HP lost, paired differences with standard errors.
"""
import argparse, json, os, sys
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
import utility  # noqa: E402
from agent.engine import Engine  # noqa: E402


def run_arm(eng, groups, attempts, use_curve):
    """{scenario index: array of end-HP fractions per attempt (0 = lost)}."""
    out = {}
    for u, items in groups:
        scen = [s for _, s in items]
        res = eng.solve(scen, attempts=attempts, seed=7, util=u if use_curve else None)
        for (i, s), r in zip(items, res):
            out[i] = np.array(r["ends"]) / s["max_hp"]
    return out


def score(ends, curves):
    eu, win, lost = [], [], []
    for i, e in ends.items():
        u = curves[i]
        eu.append(np.mean([float(np.interp(f, utility.FRAC, u)) if f > 0 else 0.0 for f in e]))
        win.append(np.mean(e > 0))
        lost.append(np.mean(np.where(e > 0, 1.0 - e, 1.0)))
    return np.array(eu), np.array(win), np.array(lost)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--new", required=True, help="policy ckpt, value ckpts (comma separated: first = policy + value, the rest value only)")
    ap.add_argument("--fights", default=os.path.join(ROOT, "data", "train", "eval.json"))
    ap.add_argument("--attempts", type=int, default=4)
    ap.add_argument("--curves", type=int, default=24)
    ap.add_argument("--seed", type=int, default=909)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    rng = np.random.default_rng(a.seed)
    fights = json.load(open(a.fights))
    if a.limit:
        fights = fights[:a.limit]
    for k, s in enumerate(fights):
        if k % 2:
            s["hp"] = max(1, int(round(s["max_hp"] * rng.uniform(0.25, 1.0))))
    pool = [utility.sample(rng, p_linear=0.0) for _ in range(a.curves)]
    which = rng.integers(0, a.curves, len(fights))
    curves = {i: pool[which[i]] for i in range(len(fights))}
    groups = [(pool[c], [(i, fights[i]) for i in range(len(fights)) if which[i] == c]) for c in range(a.curves)]
    groups = [g for g in groups if g[1]]
    new = a.new.split(",")
    res = {}
    for name, eng_args in (("old", {}), ("new", dict(ckpt=new[0], value_ckpts=new[1:] or None))):
        eng = Engine(**eng_args)
        for mode in ("linear", "curve"):
            res[f"{name}_{mode}"] = score(run_arm(eng, groups, a.attempts, mode == "curve"), curves)
            eu, w, l = res[f"{name}_{mode}"]
            print(f"{name:3s} {mode:6s}  E[U] {eu.mean():.4f}  win {w.mean():.4f}  HP lost {l.mean():.4f}", flush=True)
        del eng
    def diff(x, y, k):
        d = res[x][k] - res[y][k]
        return float(d.mean()), float(d.std(ddof=1) / len(d) ** 0.5)
    rows = {}
    for x, y in (("new_curve", "new_linear"), ("new_linear", "old_linear"), ("new_curve", "old_linear"), ("old_curve", "old_linear")):
        rows[f"{x} - {y}"] = {k: diff(x, y, j) for j, k in enumerate(("E[U]", "win", "HP lost"))}
        print(f"{x} - {y}: " + "  ".join(f"{k} {m:+.4f} ± {se:.4f}" for k, (m, se) in rows[f'{x} - {y}'].items()), flush=True)
    ok_lin = rows["new_linear - old_linear"]["win"][0] >= -2 * rows["new_linear - old_linear"]["win"][1] - 0.005
    ok_u = rows["new_curve - new_linear"]["E[U]"][0] > 2 * rows["new_curve - new_linear"]["E[U]"][1]
    print(f"GATE: linear mode no worse: {ok_lin}; curve beats linear on E[U]: {ok_u} -> {'PASS' if ok_lin and ok_u else 'FAIL'}", flush=True)
    if a.out:
        json.dump(dict(rows=rows, means={k: [float(v.mean()) for v in vals] for k, vals in res.items()}, pass_=bool(ok_lin and ok_u)), open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
