#!/usr/bin/env python3
"""Calibration of the potion-use head (`docs/rl_redesign.md` M1b): on the network's own sampled play of a scenario set, every state x occupied belt slot gives
the head's P(this potion is used before the fight ends) and what happened (Monte Carlo: the slot's potion was used up at a later step of the same fight).
Reports Brier against the base rate and a reliability table by predicted decile.

  STS2_DEVICE=cuda python rl/pot_check.py target/m1b/pot_frozen/ckpt.pt [--data target/train/eval.json] [--n 1500] [--out evals/pot_check.json]
"""
import argparse, json, os, sys

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads as H
from model import DEV, load

KP = sts2.layout()["consts"]["MAX_POTIONS"]
PO = {n: o for n, o, s in sts2.layout()["sections"]}["potions"]


@torch.no_grad()
def collect(net, scen, seed=5, max_steps=600):
    env = sts2.VecEnv(len(scen), scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=H.TURN_CAP)
    obs, mask = env.reset()
    n = len(scen)
    live = np.ones(n, bool)
    preds = [[] for _ in range(n)]  # per env, per step: (occupied [KP], p [KP])
    used = [[] for _ in range(n)]  # per env, per step: used bits after the step
    final = [None] * n
    while live.any():
        o, m = torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV)
        lg, _, _, pl = net(o, m, outcome=True, potuse=True)
        p = torch.sigmoid(pl.float()).cpu().numpy()
        occ = obs[:, PO:PO + 2 * KP:2] > 0
        for i in np.nonzero(live)[0]:
            preds[i].append((occ[i].copy(), p[i]))
        act = torch.distributions.Categorical(logits=lg.float()).sample()
        obs, mask, r, d, info = env.step(act.cpu().numpy().astype(np.int32))
        pu = info["pot_used"].copy()
        for i in np.nonzero(live)[0]:
            used[i].append(pu[i])
        for i in np.nonzero(d & live)[0]:
            final[i] = int(info["outcome"][i])
            live[i] = False
    ps, ys = [], []
    for i in range(n):
        if final[i] not in (1, -1, 2):
            continue
        u = np.array([[(b >> k) & 1 for k in range(KP)] for b in used[i]], bool)  # [steps, KP]
        later = np.flip(np.logical_or.accumulate(np.flip(u, 0), 0), 0)  # used at this step or a later one
        for t, (oc, p) in enumerate(preds[i]):
            for k in np.nonzero(oc)[0]:
                ps.append(p[k]); ys.append(later[t, k])
    return np.array(ps), np.array(ys, float)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ckpts", nargs="+")
    ap.add_argument("--data", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "train", "eval.json"))
    ap.add_argument("--n", type=int, default=1500)
    ap.add_argument("--out")
    a = ap.parse_args()
    scen = [s for s in json.load(open(a.data))[:a.n] if s.get("potions")]
    res = {}
    for c in a.ckpts:
        net = load(c)
        ps, ys = collect(net, scen)
        dec = np.minimum((ps * 10).astype(int), 9)
        res[c] = dict(states=int(len(ps)), fights=len(scen), base_rate=float(ys.mean()), brier=float(((ps - ys) ** 2).mean()),
                      brier_base=float(((ys.mean() - ys) ** 2).mean()),
                      reliability=[(k / 10, int((dec == k).sum()), round(float(ps[dec == k].mean()), 3), round(float(ys[dec == k].mean()), 3)) for k in range(10) if (dec == k).any()])
        print(c, json.dumps(res[c]), flush=True)
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
