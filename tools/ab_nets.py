#!/usr/bin/env python3
"""Fight-level A/B of network configurations at a given search width (live play is 5 options x 32 futures): the batch solver plays the same fights
with the same seeds per configuration; paired differences vs the first, standard errors over scenarios.

  STS2_DEVICE=cuda python tools/ab_nets.py --conf "today=models/solver_b128.pt|models/solver_c128.pt,models/solver_d128.pt" \
      --conf heads=target/m1/heads/ckpt.pt [--kind boss] [--limit 400] [--attempts 8] [--M 5 --K 32] [--out evals/ab_nets.json]

Fights: data/train/eval.json, half of them at a random 25-100 % of max HP (as `tools/ab_leaf.py`). `name=policy|value1,value2`: extra value nets.
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--conf", action="append", required=True)
    ap.add_argument("--limit", type=int, default=400)
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--seed", type=int, default=909)
    ap.add_argument("--M", type=int, default=5)
    ap.add_argument("--K", type=int, default=32)
    ap.add_argument("--kind", default="boss")
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "ab_nets.json"))
    a = ap.parse_args()
    from solver import Solver
    import torch
    rng = np.random.default_rng(a.seed)
    fights = json.load(open(os.path.join(ROOT, "data", "train", "eval.json")))
    if a.kind != "all":
        fights = [s for s in fights if str(s.get("encounter", "")).endswith("_" + a.kind.upper())]
    fights = fights[:a.limit]
    for k, s in enumerate(fights):
        if k % 2:
            s["hp"] = max(1, int(round(s["max_hp"] * rng.uniform(0.25, 1.0))))
    res, names = {}, []
    for c in a.conf:
        name, rest = c.split("=", 1)
        pol, _, vals = rest.partition("|")
        S = Solver(pol, M=a.M, K=a.K, value_ckpts=[v for v in vals.split(",") if v] or None)
        t = time.time()
        r = S.solve(fights, attempts=a.attempts, seed=7)
        dt = time.time() - t
        win = np.array([x["win"] for x in r])
        lost = np.array([np.mean([(s["hp"] - e) / s["max_hp"] for e in x["ends"]]) if x["ends"] else 1.0 for x, s in zip(r, fights)])
        res[name] = dict(win=win, lost=lost, secs=dt)
        names.append(name)
        print(f"{name}: win {win.mean():.4f}  HP lost {lost.mean():.4f} of max  ({dt:.0f}s)", flush=True)
        del S
        torch.cuda.empty_cache()
    out = {}
    for n in names[1:]:
        dw, dl = res[n]["win"] - res[names[0]]["win"], res[n]["lost"] - res[names[0]]["lost"]
        k = len(dw) ** 0.5
        out[n] = dict(win=float(dw.mean()), win_se=float(dw.std(ddof=1) / k), lost=float(dl.mean()), lost_se=float(dl.std(ddof=1) / k))
        print(f"{n} - {names[0]}: win {dw.mean():+.4f} +- {dw.std(ddof=1) / k:.4f}   HP lost {dl.mean():+.4f} +- {dl.std(ddof=1) / k:.4f}", flush=True)
    json.dump(dict(diff=out, means={n: dict(win=float(v["win"].mean()), lost=float(v["lost"].mean()), secs=v["secs"]) for n, v in res.items()},
                   M=a.M, K=a.K, kind=a.kind, n=len(fights), attempts=a.attempts), open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
