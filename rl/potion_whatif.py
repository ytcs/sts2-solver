#!/usr/bin/env python3
"""When should a potion be drunk? Observes what the model duplicates and compares forced potion rules.

  .venv/bin/python rl/potion_whatif.py --ckpt target/runs/r4/ckpt.pt --scenario target/runs/phrog_dup.json --attempts 3000

Rules (the model plays everything else): `free` (as it likes), `with PS` (only while Perfected Strike is in hand), `not before turn T`, `never`.
Also reported for the free rule: the card played right after the potion, and how often Perfected Strike was in the opening hand.
"""
import argparse, collections, json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import C, SEC
from search import load
from ppo import net_policy

NAMES = sts2.names()
OH, _ = SEC["hand"]
OG, _ = SEC["global"]
CF, T13 = C["CARD_F"], C["MAX_CREATURES"] + 1
POT = slice(C["OFF_POTION"], C["OFF_DISCARD"])
PS = NAMES["card"].index("PERFECTED_STRIKE") + 1


def ruled(base, rule, arg=0):
    def act(obs, mask):
        m = mask.copy()
        if rule != "free":
            turn = obs[:, OG + 1]
            hand_ids = obs[:, OH:OH + C["MAX_HAND"] * CF].reshape(len(obs), C["MAX_HAND"], CF)[:, :, 0]
            has_ps = (hand_ids == PS).any(1)
            if rule == "never":
                bad = np.ones(len(obs), bool)
            elif rule == "with_ps":
                bad = ~has_ps
            else:  # delay
                bad = turn < arg
            m[bad, POT] = 0
            empty = m.sum(1) == 0
            m[empty] = mask[empty]
        return base(obs, m)
    return act


def run(policy, scen, n, seed=99, observe=False):
    env = sts2.VecEnv(n, [scen], seed=seed, max_steps=400, win=1.0, loss=-1.0, hp_bonus=0.5)
    obs, mask = env.reset()
    got = np.zeros(n, bool)
    win = np.zeros(n, bool); valid = np.zeros(n, bool); hp = np.zeros(n)
    pending = np.zeros(n, bool)       # potion drunk, next card not yet played
    dup = [None] * n
    ps_open = obs[:, OH:OH + C["MAX_HAND"] * CF].reshape(n, C["MAX_HAND"], CF)[:, :, 0].__eq__(PS).any(1).copy()
    while not got.all():
        a = policy(obs, mask)
        if observe:
            hand = obs[:, OH:OH + C["MAX_HAND"] * CF].reshape(n, C["MAX_HAND"], CF)
            for i in np.nonzero(~got)[0]:
                x = int(a[i])
                if C["OFF_POTION"] <= x < C["OFF_DISCARD"]:
                    pending[i] = True
                elif pending[i] and 1 <= x < C["OFF_POTION"]:
                    h = (x - 1) // T13
                    dup[i] = NAMES["card"][int(hand[i, h, 0]) - 1] + ("+" if hand[i, h, 1] > 0 else "")
                    pending[i] = False
        obs, mask, r, d, info = env.step(a)
        new = (d > 0) & ~got
        if new.any():
            ei = env.episode_info()
            idx = np.nonzero(new)[0]
            oc = info["outcome"][idx]
            win[idx] = oc == 1; valid[idx] = (oc == 1) | (oc == -1) | (oc == 2); hp[idx] = ei["hp_end"][idx]; got[idx] = True
    return win, valid, hp, dup, ps_open


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--attempts", type=int, default=3000)
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    scen = json.load(open(a.scenario))
    net = load(a.ckpt)
    g = net_policy(net)
    mh = scen["max_hp"]
    res = {}
    w, v, hp, dup, ps_open = run(ruled(g, "free"), scen, a.attempts, observe=True)
    se = lambda p, n: (p * (1 - p) / max(n, 1)) ** 0.5
    print(f"free: win {w[v].mean():.3f} (±{se(w[v].mean(), v.sum()):.3f}); Perfected Strike in the opening hand in {ps_open.mean():.0%} of fights")
    res["free"] = float(w[v].mean())
    res["ps_open"] = float(ps_open.mean())
    by = collections.defaultdict(list)
    for i in range(a.attempts):
        by[dup[i] or "(no card played after)"].append(i)
    print("card duplicated -> share of fights, win rate:")
    res["dup"] = {}
    for k, idx in sorted(by.items(), key=lambda kv: -len(kv[1])):
        idx = np.array(idx)
        res["dup"][k] = dict(share=len(idx) / a.attempts, win=float(w[idx].mean()))
        print(f"   {k:24s} {len(idx) / a.attempts:6.1%}   win {w[idx].mean():.3f}")
    for name, pol in [("only with Perfected Strike in hand", ruled(g, "with_ps")), ("not before turn 2", ruled(g, "delay", 2)), ("not before turn 3", ruled(g, "delay", 3)),
                      ("not before turn 4", ruled(g, "delay", 4)), ("never drink it", ruled(g, "never"))]:
        w2, v2, hp2, _, _ = run(pol, scen, a.attempts)
        res[name] = float(w2[v2].mean())
        print(f"{name:36s} win {w2[v2].mean():.3f} (±{se(w2[v2].mean(), v2.sum()):.3f})  HP left {hp2[w2 & v2].mean() * mh:.1f}")
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
