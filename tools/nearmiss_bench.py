#!/usr/bin/env python3
"""The near-miss benchmark (user's key metric): how many fights the previous player narrowly lost does a new network now win, and how many narrow
wins does it now lose.

  tools/nearmiss_bench.py build target/exit/r4s_*.npz --near target/exit/nm_r4s.json --out data/bench/nearmiss.json [--n 1500] [--close 0.10]
  STS2_DEVICE=cuda tools/nearmiss_bench.py eval BASE CKPT [CKPT ...] [--attempts 2]

`build` takes near-miss losses (`tools/nearmiss.py`: within one turn of a win or <= 20% enemy HP left) and close wins (won with at most `--close` of max
HP left) of one collection, each as its fight start with the ORIGINAL job seed: the fight's own randomness (draws, enemy rolls) is fixed.
`eval` plays every fight from that start (`sts2.Sim` start state: the real fight keeps its RNG) with the live search (5x32) and `--attempts` search seeds
(the search's own sampling and determinizations vary, the fight does not). The player that collected the set lost every near-miss on its own search
seed, so re-running it on fresh search seeds wins some by luck: BASE (that player) measures the luck rate, and each CKPT is paired with BASE on the
same fights and search seeds. Reported per network: win rate on the near-miss losses (flips L -> W) and loss rate on the close wins (flips W -> L),
both with the paired difference to BASE and its se (by fight).
"""
import argparse, glob, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402


def build(a):
    rng = np.random.default_rng(a.seed)
    near = json.load(open(a.near))["near"]
    loss = [dict(kind="near_loss", part=r["part"], fight=r["fight"]) for r in near]
    files = sorted(f for p in a.parts for f in glob.glob(p))
    close = []
    for fn in files:
        z = np.load(fn, allow_pickle=True)
        scen = json.loads(str(z["scenarios"]))
        for i in np.nonzero(z["f_cls"] > 0)[0]:
            si = int(z["f_scen"][i])
            s = sts2.Sim(json.dumps(scen[si]), int(z["f_seed"][i]))
            for x in z["acts"][z["f_off"][i]:z["f_off"][i + 1]]:
                s.step(int(x))
            p = json.loads(s.snapshot())["player"]
            if s.outcome() == 1 and p["hp"] <= a.close * p["max_hp"]:
                close.append(dict(kind="close_win", part=fn, fight=int(i)))
    pick = lambda xs: [xs[i] for i in sorted(rng.choice(len(xs), min(a.n, len(xs)), replace=False))]  # noqa: E731
    rows = []
    cache = {}
    for r in pick(loss) + pick(close):
        if r["part"] not in cache:
            z = np.load(r["part"], allow_pickle=True)
            cache = {r["part"]: (z, json.loads(str(z["scenarios"])))}
        z, scen = cache[r["part"]]
        rows.append(dict(kind=r["kind"], scenario=scen[int(z["f_scen"][r["fight"]])], seed=int(z["f_seed"][r["fight"]]), source=os.path.basename(r["part"])))
    json.dump(rows, open(a.out, "w"))
    print(f"{sum(r['kind'] == 'near_loss' for r in rows)} near-miss losses (of {len(loss)}), {sum(r['kind'] == 'close_win' for r in rows)} close wins "
          f"(of {len(close)}) -> {a.out}")


def play(ck, rows, attempts, roots):
    """[F, attempts] wins of the live search (5x32) from each fight's start, search seeds 0..attempts-1."""
    from fastsearch import FastSearch
    from model import load
    fs = FastSearch(load(ck), M=5, K=32, roots=roots, amp=True)
    fs.warm()
    F = len(rows)
    out = np.zeros((F, attempts))
    for att in range(attempts):
        sims = [sts2.Sim(json.dumps(r["scenario"]), r["seed"]) for r in rows]
        res = fs.run([r["scenario"] for r in rows], np.arange(F, dtype=np.uint32), np.uint64(att + 1) * np.uint64(7_919_993) + np.arange(F, dtype=np.uint64), starts=sims)
        out[:, att] = res[:, 1] == 1
    return out


def evaluate(a):
    rows = json.load(open(a.bench))
    kind = np.array([r["kind"] for r in rows])
    base = None
    for k, ck in enumerate(a.ckpts):
        t = time.time()
        w = play(ck, rows, a.attempts, a.roots)
        line = []
        for kd, name, flip in (("near_loss", "near-miss losses won", lambda x: x), ("close_win", "close wins lost", lambda x: 1 - x)):
            sel = kind == kd
            f = flip(w[sel]).mean(1)
            if base is None:
                line.append(f"{name} {f.mean():.3f}")
            else:
                d = f - flip(base[sel]).mean(1)
                line.append(f"{name} {f.mean():.3f} ({d.mean():+.3f} +- {d.std(ddof=1) / len(d) ** 0.5:.3f})")
        if base is None:
            base = w
        print(f"{os.path.basename(ck):24s} ({time.time() - t:.0f}s) " + " | ".join(line), flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build"); b.add_argument("parts", nargs="+"); b.add_argument("--near", required=True); b.add_argument("--out", required=True)
    b.add_argument("--n", type=int, default=1500); b.add_argument("--close", type=float, default=0.10); b.add_argument("--seed", type=int, default=0)
    e = sub.add_parser("eval"); e.add_argument("ckpts", nargs="+", help="the first is the player that collected the set (the luck baseline)")
    e.add_argument("--bench", default=os.path.join(ROOT, "data", "bench", "nearmiss.json")); e.add_argument("--attempts", type=int, default=2)
    e.add_argument("--roots", type=int, default=1024)
    a = ap.parse_args()
    build(a) if a.cmd == "build" else evaluate(a)


if __name__ == "__main__":
    main()
