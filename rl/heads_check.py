#!/usr/bin/env python3
"""How well a network's value predicts the fight it is in (`docs/rl_redesign.md` M1 calibration report), on its own greedy play of a scenario set.

  STS2_DEVICE=cuda python rl/heads_check.py target/m1/heads/ckpt.pt [target/m1/control/ckpt.pt ...] [--data target/train/eval.json] [--n 600]

Per network: MSE of the value against the realized return (`+1 + 0.5 x HP fraction left` / `-1`) over every visited state (comparable for scalar and
outcome-head networks). For an outcome-head network also: Brier of P(win), a reliability table by predicted decile, and on won fights the coverage of
the predicted end-HP quantiles (share of states whose real end HP is below the predicted q10 / q50 / q90 of the win distribution).
"""
import argparse, json, os, sys

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads as H
from model import DEV, load


@torch.no_grad()
def collect(net, scen, seed=5, max_steps=600):
    env = sts2.VecEnv(len(scen), scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=H.TURN_CAP)
    obs, mask = env.reset()
    n = len(scen)
    live = np.ones(n, bool)
    rows = [[] for _ in range(n)]  # per env: (value, p_win, cdf over win classes) per state
    final = [None] * n
    while live.any():
        o, m = torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV)
        if net.heads:
            lg, v, ol = net(o, m, outcome=True)
            p = torch.softmax(ol.float(), 1).cpu().numpy()
        else:
            lg, v = net(o, m)
            p = None
        v = v.float().cpu().numpy()
        for i in np.nonzero(live)[0]:
            rows[i].append((v[i], None if p is None else p[i]))
        obs, mask, r, d, info = env.step(lg.argmax(1).cpu().numpy().astype(np.int32))
        if d.any():
            ei = env.episode_info()
            for i in np.nonzero(d & live)[0]:
                final[i] = (int(info["outcome"][i]), float(ei["hp_end"][i]), int(ei["hp_end_abs"][i]))
                live[i] = False
    return rows, final


def report(net, rows, final):
    vs, gs, pw, won, cls_true, cdfs = [], [], [], [], [], []
    for r, f in zip(rows, final):
        oc, frac, hp = f
        if oc not in (1, -1, 2):
            continue
        g = 1.0 + 0.5 * frac if oc == 1 else -1.0
        for v, p in r:
            vs.append(v); gs.append(g)
            if p is not None:
                pw.append(1.0 - p[0]); won.append(oc == 1)
                if oc == 1:
                    cls_true.append(int(H.end_class(True, hp)))
                    cdfs.append(np.cumsum(p[1:]) / max(p[1:].sum(), 1e-9))
    vs, gs = np.array(vs), np.array(gs)
    out = dict(states=len(vs), fights=sum(1 for f in final if f[0] in (1, -1, 2)), value_mse=float(((vs - gs) ** 2).mean()), win=float(np.mean([f[0] == 1 for f in final])))
    if pw:
        pw, won = np.array(pw), np.array(won, float)
        out["brier_pwin"] = float(((pw - won) ** 2).mean())
        out["brier_base"] = float(((won.mean() - won) ** 2).mean())
        dec = np.minimum((pw * 10).astype(int), 9)
        out["reliability"] = [(k / 10, int((dec == k).sum()), round(float(pw[dec == k].mean()), 3), round(float(won[dec == k].mean()), 3)) for k in range(10) if (dec == k).any()]
        if cdfs:
            cdfs, ct = np.array(cdfs), np.array(cls_true)
            q = {}
            for lvl in (0.1, 0.5, 0.9):
                qcls = (cdfs < lvl).sum(1) + 1  # predicted quantile class
                q[f"below_q{int(lvl * 100)}"] = round(float((ct < qcls).mean()), 3)
            out["end_hp_coverage"] = q
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ckpts", nargs="+")
    ap.add_argument("--data", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "train", "eval.json"))
    ap.add_argument("--n", type=int, default=600)
    ap.add_argument("--out")
    a = ap.parse_args()
    scen = json.load(open(a.data))[:a.n]
    res = {}
    for c in a.ckpts:
        net = load(c)
        rows, final = collect(net, scen)
        res[c] = report(net, rows, final)
        print(c, json.dumps(res[c]), flush=True)
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
