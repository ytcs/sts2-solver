#!/usr/bin/env python3
"""A collection pool weighted by training signal (the user's curriculum idea): fights the player already saturates, or loses whatever it plays,
teach little; fights near the middle teach the most.

  STS2_DEVICE=cuda tools/signal_pool.py --ckpt target/exit/r3.pt --cands target/exit/cand_r4.json --n 35000 --out target/exit/pool_r4s.json \
      [--uniform-out target/exit/pool_r4u.json] [--anchor 0.15] [--seed 0]

Every candidate (multiplayer-only cards left out) is scored with the predictor (`rl/predictor.py`, the outcome head at the fight start; calibrated per
band on round 3: predicted 0.51 vs real 0.54 in [0.3, 0.7), 0.995 vs 0.995 above 0.97). The pool draws `--anchor` of its fights uniformly (the outcome
head still sees every kind of fight: the run model prices easy and hopeless ones too) and the rest without replacement with weight p(1 - p), the
variance of the fight's outcome (round 3: the 0.1-0.9 band held 33 % of the fights and 73 % of the near-miss losses; the >= 0.97 band 44 % and 2 %).
Selection is on the fight's configuration, before any seed is played: the outcome labels of the collected fights stay unbiased.
`--uniform-out`: a uniform pool of the same size from the same candidates (the control arm).
"""
import argparse, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def multiplayer_free(fights):
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
    mp = {c["id"] for pool in cat["cards"].values() for c in pool if c.get("multiplayer_only")}
    return [f for f in fights if not any((c if isinstance(c, str) else c["id"]) in mp for c in f["deck"])]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--cands", nargs="+", required=True, help="candidate fight files (e.g. tools/gen_curriculum.py output, data/corpus/fights_train.json)")
    ap.add_argument("--n", type=int, required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--uniform-out")
    ap.add_argument("--anchor", type=float, default=0.15)
    ap.add_argument("--shuffles", type=int, default=4)
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    from predictor import Predictor, p_win
    cands = multiplayer_free([f for fn in a.cands for f in json.load(open(fn))])
    t = time.time()
    p = p_win(Predictor(a.ckpt).fight_start(cands, shuffles=a.shuffles))
    print(f"{len(cands)} candidates scored in {time.time() - t:.0f}s")
    rng = np.random.default_rng(a.seed)
    n_anchor = int(round(a.n * a.anchor))
    pick = set(rng.choice(len(cands), n_anchor, replace=False).tolist())
    w = p * (1 - p)
    w[list(pick)] = 0.0
    rest = rng.choice(len(cands), a.n - n_anchor, replace=False, p=w / w.sum())
    sel = np.array(sorted(pick) + rest.tolist())
    rng.shuffle(sel)
    bands = [0, .03, .1, .3, .7, .9, .97, 1.01]

    def show(name, ix):
        h = np.histogram(p[ix], bands)[0] / len(ix)
        print(f"{name:8s} n {len(ix)} mean p {p[ix].mean():.3f} mean p(1-p) {w[ix].mean() if name == 'x' else (p[ix] * (1 - p[ix])).mean():.3f} | "
              + " ".join(f"[{lo:.2f},{hi:.2f}) {x:.3f}" for lo, hi, x in zip(bands, bands[1:], h)))
    show("cands", np.arange(len(cands)))
    show("signal", sel)
    json.dump([cands[i] | {"meta": cands[i].get("meta", {}) | {"p0": round(float(p[i]), 4)}} for i in sel], open(a.out, "w"))
    print(f"-> {a.out}")
    if a.uniform_out:
        uni = rng.choice(len(cands), a.n, replace=False)
        show("uniform", uni)
        json.dump([cands[i] | {"meta": cands[i].get("meta", {}) | {"p0": round(float(p[i]), 4)}} for i in uni], open(a.uniform_out, "w"))
        print(f"-> {a.uniform_out}")


if __name__ == "__main__":
    main()
