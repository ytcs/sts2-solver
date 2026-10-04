#!/usr/bin/env python3
"""Where does the solver's time go?  Plays the first N fights of a scenario set with the default solver config in one process and prints
the timers and work counters of the Searcher (this is the baseline for every speed experiment).

  .venv/bin/python rl/profile_search.py --eval data/train/eval.json --roots 300 [--threads 8] [--M 3 --K 8]
"""
import argparse, json, os, sys, time
import numpy as np
import torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from search import Searcher, load
from solver import DEFAULT_CKPT, DEFAULT_VALUE_CKPTS

ap = argparse.ArgumentParser()
ap.add_argument("--eval", default="data/train/eval.json")
ap.add_argument("--roots", type=int, default=300)
ap.add_argument("--threads", type=int, default=8)
ap.add_argument("--M", type=int, default=3); ap.add_argument("--K", type=int, default=8)
ap.add_argument("--conf", type=float, default=1.01)
ap.add_argument("--seed", type=int, default=5)
a = ap.parse_args()
torch.set_num_threads(a.threads)
net = load(DEFAULT_CKPT)
vn = [load(c) for c in DEFAULT_VALUE_CKPTS]
scen = json.load(open(a.eval))[:a.roots]
s = Searcher(net, len(scen), a.M, a.K, 0.0, seed=a.seed, max_steps=300, conf=a.conf, value_nets=vn)
t0 = time.time()
rec = s.play(scen, seed=a.seed, verbose=False, with_records=True, round_robin=False)
dt = time.time() - t0
out = np.array([r[1] for r in rec])
print(f"{len(scen)} fights in {dt:.1f}s = {len(scen) / dt:.2f} fights/s; win {np.mean(out == 1):.3f} hp_lost_all {np.mean([r[2] if r[1] == 1 else 1.0 for r in rec]):.3f}")
tot = sum(s.timers.values())
for k, v in sorted(s.timers.items(), key=lambda kv: -kv[1]):
    print(f"  {k:24s} {v:7.1f}s {100 * v / dt:5.1f}%")
print(f"  {'(python/other)':24s} {dt - tot:7.1f}s {100 * (dt - tot) / dt:5.1f}%")
for k, v in s.counts.items():
    print(f"  {k:24s} {v:>12,d}  per fight {v / len(scen):10.1f}")
