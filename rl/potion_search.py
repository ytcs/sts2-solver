#!/usr/bin/env python3
"""Does the play-out search find the right potion timing by itself? (see rl/potion_whatif.py for the rules)"""
import argparse, collections, json, os, sys
import numpy as np
import torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import Searcher, load
from potion_whatif import ruled, PS, NAMES, OH, OG, CF, T13
from model import C
from trace import decode

ap = argparse.ArgumentParser()
ap.add_argument("--ckpt", required=True); ap.add_argument("--scenario", required=True)
ap.add_argument("--roots", type=int, default=300); ap.add_argument("--M", type=int, default=5); ap.add_argument("--K", type=int, default=8)
ap.add_argument("--threads", type=int, default=8)
a = ap.parse_args()
torch.set_num_threads(a.threads)
scen = json.load(open(a.scenario))
path = "target/runs/potion_search_tmp.json"
json.dump([scen], open(path, "w"))
net = load(a.ckpt)
for name, rule in [("free", None), ("potion only with Perfected Strike in hand", "with_ps")]:
    tr = [[] for _ in range(a.roots)]
    s = Searcher(net, a.roots, a.M, a.K, 0.0, seed=5, max_steps=300)
    if rule:
        ident = lambda obs, mask: mask
        wrap = ruled(lambda o, m: m, rule)  # returns the (possibly restricted) mask
        s.mask_fn = lambda obs, mask: wrap(obs, mask)
    rec = np.array(s.play(path, seed=21, verbose=False, with_records=True, trace=tr))
    w = rec[:, 1] == 1
    dups = collections.Counter(); first_turn = collections.Counter(); hold = 0
    for t in tr:
        pending = False; used = None
        for st in t:
            if st["a"] is None:
                break
            x = st["a"]
            if C["OFF_POTION"] <= x < C["OFF_DISCARD"]:
                pending = True
                d = decode(st["obs"]); used = d["turn"]
                first_turn[used] += 1
            elif pending and 1 <= x < C["OFF_POTION"]:
                d = decode(st["obs"]); dups[d["hand"][(x - 1) // T13]["name"]] += 1; pending = False
    print(f"search M={a.M} K={a.K}, {name}: win {w.mean():.3f} (±{(w.mean() * (1 - w.mean()) / len(w)) ** 0.5:.3f}), HP left on a win {rec[w, 4].mean() * scen['max_hp']:.1f}")
    print("   potion drunk on turn:", dict(sorted(first_turn.items())), "| never drunk:", a.roots - sum(first_turn.values()))
    print("   card duplicated:", dict(dups.most_common()))
