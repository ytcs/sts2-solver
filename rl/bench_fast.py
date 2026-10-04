#!/usr/bin/env python3
"""Throughput / quality of the Rust search engine: the first N fights of a scenario set (attempts per scenario), default solver configuration.

  .venv/bin/python rl/bench_fast.py --eval data/train/eval.json --n 300 [--roots 256 --groups 2 --threads 8]
"""
import argparse, json, os, sys, time
import numpy as np
import torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from model import load
from solver import DEFAULT_CKPT, DEFAULT_VALUE_CKPTS
from fastsearch import FastSearch

ap = argparse.ArgumentParser()
ap.add_argument("--eval", default="data/train/eval.json")
ap.add_argument("--n", type=int, default=300)
ap.add_argument("--attempts", type=int, default=1)
ap.add_argument("--roots", type=int, default=256)
ap.add_argument("--groups", type=int, default=2)
ap.add_argument("--torch-threads", type=int, default=8)
ap.add_argument("--threads", type=int, default=None)
ap.add_argument("--M", type=int, default=3); ap.add_argument("--K", type=int, default=8)
ap.add_argument("--conf", type=float, default=1.01)
ap.add_argument("--roll-cap", type=int, default=60)
ap.add_argument("--seed", type=int, default=5)
ap.add_argument("--no-value-ens", action="store_true")
ap.add_argument("--no-graphs", action="store_true")
ap.add_argument("--amp", action="store_true")
ap.add_argument("--no-compile", action="store_true")
ap.add_argument("--no-lead", action="store_true")
ap.add_argument("--record", action="store_true")
ap.add_argument("--lead-greedy", action="store_true")
ap.add_argument("--profile-gpu", action="store_true")
ap.add_argument("--E", type=int, default=8)
a = ap.parse_args()
torch.set_num_threads(a.torch_threads)
net = load(DEFAULT_CKPT)
vn = [] if a.no_value_ens else [load(c) for c in DEFAULT_VALUE_CKPTS]
scen = json.load(open(a.eval))[:a.n]
fs = FastSearch(net, vn, a.M, a.K, conf=a.conf, roll_cap=a.roll_cap, roots=a.roots, groups=a.groups, threads=a.threads, use_graphs=not a.no_graphs, graph_E=a.E, amp=a.amp, profile_gpu=a.profile_gpu, compile=not a.no_compile, lead=not a.no_lead, lead_greedy=a.lead_greedy, record=a.record)
fs.warm()
js = np.tile(np.arange(len(scen), dtype=np.uint32), a.attempts)
jd = (np.arange(len(js), dtype=np.uint64) + np.uint64(a.seed * 1000003))
t0 = time.time()
r = fs.run(scen, js, jd)
dt = time.time() - t0
done = r[:, 5] > 0
oc = r[:, 1]
win = (oc == 1)
hp = np.where(win, r[:, 2], 1.0)
print(f"{len(r)} fights in {dt:.1f}s = {len(r) / dt:.2f} fights/s; finished {done.mean():.3f}; win {win.mean():.3f} hp_lost_all {hp.mean():.3f}; outcomes {dict(zip(*np.unique(oc, return_counts=True)))}")
for k, v in sorted(fs.timers.items(), key=lambda kv: -kv[1]):
    print(f"  {k:14s} {v:7.1f}s {100 * v / dt:5.1f}%")
for k, v in fs.stats.items():
    print(f"  {k:16s} {v:>14,.1f}  per fight {v / len(r):10.1f}")
if a.profile_gpu:
    tot = 0
    for k, (ms, n, rows) in sorted(fs.gpu_ms().items()):
        tot += ms
        print(f"  GPU {k:14s} {ms / 1000:6.1f}s  {n:7d} replays  {rows / max(n, 1):7.0f} rows/replay  {ms / max(n, 1):5.2f} ms/replay  {1000 * ms / max(rows, 1):.2f} us/row")
    print(f"  GPU total {tot / 1000:.1f}s of {dt:.1f}s")
