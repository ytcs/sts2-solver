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


HP_BANDS = (0.5, 0.65, 0.8)
POT_BANDS = (0.8, 0.95)


def pot_key(encounter, p):
    k = "boss" if encounter.endswith("_BOSS") else "elite" if "ELITE" in encounter else "hall"
    return f"{k}:{int(np.searchsorted(POT_BANDS, p, side='right'))}"


def hp_band(scenario):
    return int(np.searchsorted(HP_BANDS, scenario.get("hp", 0) / max(scenario.get("max_hp", 1), 1), side="right"))


def _move(win, pos):
    """class mass moved to fractional class positions, split linearly between neighbours"""
    pos = np.clip(pos, 0, len(win) - 1)
    lo = np.floor(pos).astype(int)
    hi = np.minimum(lo + 1, len(win) - 1)
    fr = pos - lo
    out = np.zeros_like(win)
    np.add.at(out, lo, win * (1 - fr))
    np.add.at(out, hi, win * fr)
    return out


def calibrate(P, scenarios, params):
    """recorded-fight calibration (tools/calib_fit.py): win mass rescaled to a logistic map of logit P(win); the HP change on a win mapped
    through the (group, starting-HP band) quantile map from the predictor's pooled change distribution to the realised one; P keeps the
    outcome-head class layout"""
    P = np.array(P, np.float64)
    w = np.asarray(params["w"], np.float64)
    for i, sc in enumerate(scenarios):
        x, g = calib_features(sc, 1.0 - P[i, 0])
        pw = 1.0 / (1.0 + np.exp(-float(x @ w)))
        win = P[i, 1:]
        s = win.sum()
        if s > 0:
            qm = params["hp_qmap"].get(f"{g}:{hp_band(sc)}") or params["hp_qmap"].get(g)
            if qm is not None:
                hp0 = sc.get("hp", 0)
                end = hp0 + np.interp(H.CENTERS - hp0, qm[0], qm[1])
                win = _move(win, (end - H.CENTERS[0]) / H.BIN)
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
