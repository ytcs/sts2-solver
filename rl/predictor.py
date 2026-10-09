import os
import sys

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2  # noqa: E402
import heads as H  # noqa: E402
from model import DEV, load  # noqa: E402


class Predictor:
    def __init__(self, ckpt, batch=4096):
        self.net = load(ckpt) if isinstance(ckpt, str) else ckpt
        assert getattr(self.net, "heads", False), "the predictor needs an outcome-head network"
        self.batch = batch

    @torch.no_grad()
    def fight_start(self, scenarios, shuffles=8, seed=1000):
        P = np.zeros((len(scenarios), H.NC))
        if not scenarios:
            return P
        starts = sts2.FightStarts(scenarios)
        for s in range(shuffles):
            o, _ = starts.observe(seed + s)
            for b in range(0, len(scenarios), self.batch):
                ol = self.net.heads_out(torch.from_numpy(o[b:b + self.batch].copy()).to(DEV))
                P[b:b + self.batch] += torch.softmax(ol.float(), 1).cpu().numpy() / shuffles
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
