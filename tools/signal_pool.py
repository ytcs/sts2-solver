#!/usr/bin/env python3
import argparse, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def multiplayer_free(fights):
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
    mp = {c["id"] for pool in cat["cards"].values() for c in pool if c.get("multiplayer_only")}
    return [f for f in fights if not any((c if isinstance(c, str) else c["id"]) in mp for c in f["deck"])]


def score(ckpt, scenarios, shuffles, batch=20000):
    from predictor import Predictor, p_win
    P = Predictor(ckpt)
    return np.concatenate([p_win(P.fight_start(scenarios[i:i + batch], shuffles=shuffles)) for i in range(0, len(scenarios), batch)]) if scenarios else np.zeros(0)


def pick(p, n, anchor, rng):
    """n indices: a uniform anchor share, the rest drawn without replacement by p(1-p)"""
    n_anchor = int(round(n * anchor))
    sel = rng.choice(len(p), n_anchor, replace=False).tolist()
    w = p * (1 - p)
    w[sel] = 0.0
    sel = np.array(sorted(sel) + rng.choice(len(p), n - n_anchor, replace=False, p=w / w.sum()).tolist())
    rng.shuffle(sel)
    return sel


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
    cands = multiplayer_free([f for fn in a.cands for f in json.load(open(fn))])
    t = time.time()
    p = score(a.ckpt, cands, a.shuffles)
    print(f"{len(cands)} candidates scored in {time.time() - t:.0f}s")
    rng = np.random.default_rng(a.seed)
    sel = pick(p, a.n, a.anchor, rng)
    bands = [0, .03, .1, .3, .7, .9, .97, 1.01]

    def show(name, ix):
        h = np.histogram(p[ix], bands)[0] / len(ix)
        print(f"{name:8s} n {len(ix)} mean p {p[ix].mean():.3f} mean p(1-p) {(p[ix] * (1 - p[ix])).mean():.3f} | "
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
