#!/usr/bin/env python3
"""Search speed / accuracy on a scenario set: same fights (paired), several search settings.

  .venv/bin/python rl/bench_search.py --ckpt C --eval target/train/eval.json --roots 200 --configs "5,8,0,0;4,4,0.03,1;..."   # M,K,pmin,force
"""
import argparse, json, os, sys, time
import numpy as np
import torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from search import Searcher, load
from ppo import evaluate, net_policy

ap = argparse.ArgumentParser()
ap.add_argument("--ckpt", required=True); ap.add_argument("--eval", required=True)
ap.add_argument("--roots", type=int, default=200)
ap.add_argument("--configs", default="5,8,0,0")
ap.add_argument("--conf", type=float, default=1.01)
ap.add_argument("--threads", type=int, default=8)
ap.add_argument("--greedy", action="store_true")
a = ap.parse_args()
torch.set_num_threads(a.threads)
net = load(a.ckpt)
if a.greedy:
    r = evaluate(net_policy(net), a.eval, a.roots, 1, 777, 300, 0.5)
    print(f"greedy policy: win {r['win']:.3f} hp_lost_all {r['hp_lost_all']:.3f}", flush=True)
for cfg in a.configs.split(";"):
    M, K, pmin, force, margin, gr, full = (cfg.split(",") + ["0", "0", "0"])[:7]
    s = Searcher(net, a.roots, int(M), int(K), float(margin), seed=3, max_steps=300, conf=a.conf, pmin=float(pmin), force=bool(int(force)), greedy_roll=bool(int(gr)), full=bool(int(full)))
    t = time.time()
    res = s.play(a.eval, verbose=False)
    dt = time.time() - t
    print(f"M={M} K={K} pmin={pmin} force={force} margin={margin} greedy_roll={gr} full={full} conf={a.conf}: win {res['win']:.3f} hp_lost_all {res['hp_lost_all']:.3f} stall {res['stall']:.3f}  {dt:.0f}s ({dt / a.roots:.2f}s/fight)", flush=True)
