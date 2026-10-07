#!/usr/bin/env python3
"""The frontier of a fight set (expert iteration, M3 rounds): fights a large-budget search wins on a seed where the live-width search loses on the SAME
seed (common random numbers: the real fight depends only on its job seed), i.e. fights decided by the quality of decisions, not by luck.

  STS2_DEVICE=cuda python tools/frontier.py --ckpt models/solver_h128.pt --fights target/m3/hard.json --out target/m3/frontier.json \
      [--live 5x32] [--big 8x64x3] [--attempts 4]

`--big MxKxL`: options x futures x play-out depth (player turns). Writes the frontier fights with meta.frontier = {live, big} win rates and prints the
share of fights each width wins and the frontier size.
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def wins(net, fights, M, K, L, attempts, seed):
    from fastsearch import FastSearch
    fs = FastSearch(net, M=M, K=K, leaf_turns=L, roots=1024, amp=True)
    fs.warm()
    S = len(fights)
    js = np.tile(np.arange(S, dtype=np.uint32), attempts)
    jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
    t = time.time()
    r = fs.run(fights, js, jd)
    print(f"  {M}x{K}x{L}: {len(js)} fights {time.time() - t:.0f}s", flush=True)
    return (r[:, 1] == 1).reshape(attempts, S).T  # [S, attempts]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--fights", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--live", default="5x32x2")
    ap.add_argument("--big", default="8x64x3")
    ap.add_argument("--attempts", type=int, default=4)
    ap.add_argument("--seed", type=int, default=91)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    from model import load
    net = load(a.ckpt)
    fights = json.load(open(a.fights))
    if a.limit:
        fights = fights[:a.limit]
    lv = [int(x) for x in a.live.split("x")]
    bg = [int(x) for x in a.big.split("x")]
    w_live = wins(net, fights, *lv, a.attempts, a.seed)
    w_big = wins(net, fights, *bg, a.attempts, a.seed)
    gain = (w_big & ~w_live).any(1)
    out = []
    for i, s in enumerate(fights):
        if gain[i]:
            s = dict(s)
            s["meta"] = dict(s.get("meta", {}), frontier=dict(live=float(w_live[i].mean()), big=float(w_big[i].mean())))
            out.append(s)
    json.dump(out, open(a.out, "w"))
    print(f"{len(fights)} fights: live wins {w_live.mean():.3f} (any {w_live.any(1).mean():.3f}), big {w_big.mean():.3f} (any {w_big.any(1).mean():.3f}); "
          f"seeds big wins / live loses {(w_big & ~w_live).mean():.3f}, the reverse {(w_live & ~w_big).mean():.3f}; frontier {len(out)} -> {a.out}", flush=True)


if __name__ == "__main__":
    main()
