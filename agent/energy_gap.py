"""Is the network worse (relative to search) on decks with many energy? Held-out scenarios at 3 energy and at 4-7 energy: the network alone (greedy) against the
network + search. The gap search closes is where the network is wrong; if it is wider at high energy, the training mix (3% of scenarios with 4+ energy) is the cause.

    python -m agent.energy_gap --n 400 --attempts 4 [--ckpt PATH] [--out evals/energy_gap.json]
"""
import argparse
import json
import os
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl"))
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def make_set(path, n, seed, energy_prob):
    if not os.path.exists(path):
        subprocess.check_call([sys.executable, os.path.join(ROOT, "tools", "gen_train.py"), "--n", str(n), "--seed", str(seed), "--energy-prob", str(energy_prob), "--out", path],
                              stdout=subprocess.DEVNULL)
    return json.load(open(path))


def report(label, scen, greedy, search):
    out = {}
    for name, rows in (("greedy", greedy), ("search", search)):
        out[name] = dict(win=float(np.mean([r["win"] for r in rows])), hp_lost=float(np.mean([r["hp_lost"] for r in rows])))
    print(f"{label:22s} n={len(scen):4d}  greedy win {out['greedy']['win']:.3f} hp {out['greedy']['hp_lost']:.3f} | search win {out['search']['win']:.3f} hp {out['search']['hp_lost']:.3f}"
          f" | gap win {out['search']['win'] - out['greedy']['win']:+.3f}  hp {out['greedy']['hp_lost'] - out['search']['hp_lost']:+.3f}", flush=True)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--attempts", type=int, default=4)
    ap.add_argument("--ckpt", default="")
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    from solver import Solver
    S = Solver(ckpt=a.ckpt) if a.ckpt else Solver()
    res = {}
    for label, seed, p in (("3 energy (base mix)", 31, 0.0), ("4-7 energy", 32, 1.0)):
        scen = make_set(os.path.join(ROOT, "evals", f"_energy_gap_{seed}.json"), a.n, seed, p)
        g = S.solve(scen, a.attempts, search=False, seed=1)
        s = S.solve(scen, a.attempts, search=True, seed=1)
        res[label] = report(label, scen, g, s)
        if p == 1.0:
            for e in (4, 5, 6, 7):
                idx = [i for i, x in enumerate(scen) if x["max_energy"] == e]
                if len(idx) >= 20:
                    res[f"energy {e}"] = report(f"  energy {e}", [scen[i] for i in idx], [g[i] for i in idx], [s[i] for i in idx])
    if a.out:
        json.dump(res, open(os.path.join(ROOT, a.out), "w"), indent=1)


if __name__ == "__main__":
    main()
