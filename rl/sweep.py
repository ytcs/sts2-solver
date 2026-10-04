#!/usr/bin/env python3
"""Speed / quality of search configurations on one scenario set (paired: same fights and seeds), one process, networks loaded once.

  STS2_DEVICE=cuda .venv/bin/python rl/sweep.py --eval data/train/eval.json --attempts 2 "base" "conf=0.95" "M=2,K=8" "amp=1" "roll=models/solver_a64.pt"
Each config: comma-separated key=value over FastSearch arguments (M, K, conf, pmin, margin, roll_cap, amp, value_amp, graph_E, roots, groups, value=0|1|2|3 (value nets used),
roll=<ckpt> (play-out network), policy=<ckpt[,ckpt..]> (the network that ranks the real fight's options)).
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
ap.add_argument("--attempts", type=int, default=2)
ap.add_argument("--seed", type=int, default=1)
ap.add_argument("configs", nargs="+")
a = ap.parse_args()
scen = json.load(open(a.eval))
S = len(scen)
js = np.tile(np.arange(S, dtype=np.uint32), a.attempts)
jd = np.uint64(a.seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
nets = {}
pool = {}
SEARCH_DEFAULTS = dict(M=3, K=8, conf=1.01, pmin=0.0, margin=0.0, roll_cap=60, lead=True, greedy_roll=False, merge_dec=True, carry=False, strat=False, k1=0, z=1.0)


def get(p):
    if p not in nets:
        nets[p] = load(p)
    return nets[p]


for cfg in a.configs:
    kw = dict(roots=2048, groups=2)
    value_n = 2
    policy = DEFAULT_CKPT
    for item in filter(None, cfg.split(";" if ";" in cfg else ",")):
        if item == "base":
            continue
        k, v = item.split("=")
        if k == "value":
            value_n = int(v)
        elif k == "roll":
            kw["roll_net"] = get(v)
        elif k == "policy":
            policy = v
        elif k in ("amp", "value_amp", "lead", "compile", "merge_dec", "carry", "strat"):
            kw[k] = bool(int(v))
        elif k in ("M", "K", "roll_cap", "graph_E", "roots", "groups", "k1"):
            kw[k] = int(v)
        else:
            kw[k] = float(v)
    # networks, bf16 and the shapes fix the compiled graphs: configurations that differ only in the search parameters share one FastSearch (no recompiling)
    gkey = (policy, value_n, kw.get("roll_net") and id(kw["roll_net"]), kw.get("amp"), kw.get("value_amp"), kw.get("M", 3), kw.get("graph_E", 8), kw.get("compile", True))
    if gkey not in pool:
        pool[gkey] = FastSearch(get(policy), [get(c) for c in DEFAULT_VALUE_CKPTS[:value_n]], **kw)
        pool[gkey].warm()
    fs = pool[gkey]
    for k, v in {**SEARCH_DEFAULTS, **kw}.items():
        setattr(fs, k, v)
    fs.roots, fs.groups = kw.get("roots", 2048), kw.get("groups", 2)
    t = time.time()
    r = fs.run(scen, js, jd)
    dt = time.time() - t
    oc = r[:, 1]
    win = (oc == 1)
    valid = np.isin(oc, (1, -1, 2))
    hp = r[:, 2]
    n = len(r)
    p = win.mean()
    print(f"{cfg:40s} {n / dt:7.1f} fights/s  win {p:.4f} (+-{np.sqrt(p * (1 - p) / n):.4f})  hp_lost {hp[valid].mean():.4f}  pol rows/fight {fs.stats['policy_rows'] / n:.0f} val {fs.stats['value_rows'] / n:.0f}  ({dt:.0f}s)", flush=True)
