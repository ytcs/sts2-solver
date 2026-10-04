#!/usr/bin/env python3
"""How many fights of a scenario set are winnable at all, and how often does a policy win them?

  .venv/bin/python rl/winnable.py --ckpt target/runs/r4/ckpt.pt --eval target/train/eval.json --attempts 16

Every scenario is attempted `attempts` times with the policy *sampled* (different RNG streams each time). A scenario counts as winnable
(a lower bound: nobody proved the others lost) when at least one attempt won. Reported: the share of winnable scenarios, the mean per-attempt
win rate over all scenarios and over the winnable ones, and the mean HP lost over the winnable ones.
"""
import argparse, json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import load
from ppo import net_policy


def run(policy, scen, attempts, seed=4242, max_steps=400, hp_bonus=0.5):
    n = len(scen) * attempts
    env = sts2.VecEnv(n, scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=hp_bonus, round_robin=True)
    obs, mask = env.reset()
    got = np.zeros(n, bool)
    win = np.zeros(n, bool)
    hp_lost = np.zeros(n, np.float32)
    valid = np.zeros(n, bool)
    while not got.all():
        obs, mask, r, d, info = env.step(policy(obs, mask))
        new = (d > 0) & ~got
        if new.any():
            ei = env.episode_info()
            idx = np.nonzero(new)[0]
            oc = info["outcome"][idx]
            win[idx] = oc == 1
            valid[idx] = (oc == 1) | (oc == -1) | (oc == 2)
            hp_lost[idx] = ei["hp_lost"][idx]
            got[idx] = True
    S = len(scen)
    return win.reshape(attempts, S), valid.reshape(attempts, S), hp_lost.reshape(attempts, S)


def report(name, win, valid, hp_lost):
    p = (win & valid).sum(0) / np.maximum(valid.sum(0), 1)  # per-scenario win rate
    winnable = win.any(0)
    out = {
        "policy": name,
        "scenarios": int(win.shape[1]),
        "winnable (>=1 win)": round(float(winnable.mean()), 3),
        "win rate, all": round(float(p.mean()), 3),
        "win rate, winnable only": round(float(p[winnable].mean()), 3),
        "HP lost, all": round(float(hp_lost[valid].mean()), 3),
        "HP lost, winnable only": round(float(hp_lost[:, winnable][valid[:, winnable]].mean()), 3),
        "always won": round(float((win.all(0)).mean()), 3),
        "never won": round(float((~winnable).mean()), 3),
    }
    print(json.dumps(out), flush=True)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--eval", required=True)
    ap.add_argument("--attempts", type=int, default=16)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--threads", type=int, default=12)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    scen = json.load(open(a.eval))
    if a.limit:
        scen = scen[:a.limit]
    net = load(a.ckpt)
    w, v, h = run(net_policy(net, greedy=False), scen, a.attempts)
    report(os.path.basename(a.ckpt) + " (sampled)", w, v, h)


if __name__ == "__main__":
    main()
