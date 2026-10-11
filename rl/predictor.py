import json
import os
import sys

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2  # noqa: E402
import heads as H  # noqa: E402
from model import DEV, load  # noqa: E402


class Predictor:
    def __init__(self, ckpt, batch=4096, calib=None):
        self.net = load(ckpt) if isinstance(ckpt, str) else ckpt
        assert getattr(self.net, "heads", False), "the predictor needs an outcome-head network"
        self.batch = batch
        self.calib = json.load(open(calib)) if isinstance(calib, str) else calib

    @torch.no_grad()
    def fight_start(self, scenarios, shuffles=8, seed=1000):
        if not scenarios:
            return np.zeros((0, H.NC))
        starts = sts2.FightStarts(scenarios)
        # all shuffles in one forward pass: one host sync per batch instead of one per shuffle
        o = np.concatenate([starts.observe(seed + s)[0].copy() for s in range(shuffles)])
        n = len(scenarios)
        out = np.empty((len(o), H.NC))
        for b in range(0, len(o), self.batch):
            ol = self.net.heads_out(torch.from_numpy(o[b:b + self.batch]).to(DEV))
            out[b:b + self.batch] = torch.softmax(ol.float(), 1).cpu().numpy()
        P = out.reshape(shuffles, n, H.NC).mean(0)
        return calibrate(P, scenarios, self.calib) if self.calib else P


def calib_features(scenario, p):
    enc, act = str(scenario.get("encounter", "")), int(scenario.get("act") or 0)
    boss, elite = enc.endswith("_BOSS"), "ELITE" in enc
    z = np.log(np.clip(p, 1e-4, 1 - 1e-4) / np.clip(1 - p, 1e-4, 1 - 1e-4))
    return np.array([z, 1.0, elite, boss, act == 1, act == 2, z * elite, z * boss], np.float64), f"{act}:{'boss' if boss else 'elite' if elite else 'hall'}"


def calibrate(P, scenarios, params):
    """recorded-fight calibration (tools/calib_fit.py): win mass rescaled to a logistic map of logit P(win), win classes shifted by the
    group's end-HP bias; P keeps the outcome-head class layout"""
    P = np.array(P, np.float64)
    w = np.asarray(params["w"], np.float64)
    for i, sc in enumerate(scenarios):
        x, g = calib_features(sc, 1.0 - P[i, 0])
        pw = 1.0 / (1.0 + np.exp(-float(x @ w)))
        win = P[i, 1:]
        s = win.sum()
        if s > 0:
            k = int(round(params["hp_shift"].get(g, 0.0) / H.BIN))
            win = np.roll(win, k)
            if k > 0:
                win[-1] += win[:k].sum(); win[:k] = 0
            elif k < 0:
                win[0] += win[k:].sum(); win[k:] = 0
            P[i, 1:] = win / s * pw
        P[i, 0] = 1.0 - P[i, 1:].sum()
    return P


def p_win(P):
    return 1.0 - P[..., 0]


def end_hp(P):
    c = H.CENTERS
    w = P[..., 1:].sum(-1)
    return np.where(w > 0, (P[..., 1:] * c).sum(-1) / np.maximum(w, 1e-12), 0.0)


def sample_end(P, rng):
    c = H.CENTERS0
    cum = np.cumsum(P, -1)
    u = rng.random(len(P))[:, None]
    return c[np.minimum((u > cum).sum(-1), H.NC - 1)]
