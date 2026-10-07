"""The fight predictor (`docs/rebuild.md` section 2): the distribution of how a fight ends, from the network's outcome head.

  P = Predictor("models/solver_h128.pt").fight_start(scenarios, shuffles=8)   # [S, NC]: class 0 a loss, class b a win with end HP in bin b

A fight-start prediction averages the head over sampled opening shuffles and starting rolls (`VecEnv` seeds): exact, since all of it is revealed
before the first decision. The allowed potions are the belt the scenario carries. Classes and bins: `heads.py`.
"""
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
    def __init__(self, ckpt, batch=4096):
        self.net = load(ckpt) if isinstance(ckpt, str) else ckpt
        assert getattr(self.net, "heads", False), "the predictor needs an outcome-head network"
        self.batch = batch

    @torch.no_grad()
    def fight_start(self, scenarios, shuffles=8, seed=1000):
        P = np.zeros((len(scenarios), H.NC))
        if not scenarios:
            return P
        sj = [json.dumps(s) for s in scenarios]  # once for every shuffle
        for s in range(shuffles):
            env = sts2.VecEnv(len(sj), sj, seed=seed + s, max_steps=600, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=H.TURN_CAP)
            o, _ = env.reset()
            for b in range(0, len(sj), self.batch):
                # the outcome head only (`heads_out`: the same logits as the full pass, without the policy heads or the action mask)
                ol, _ = self.net.heads_out(torch.from_numpy(o[b:b + self.batch].copy()).to(DEV))
                P[b:b + self.batch] += torch.softmax(ol.float(), 1).cpu().numpy() / shuffles
        return P


def p_win(P):
    return 1.0 - P[..., 0]


def end_hp(P):
    """Expected end HP given a win ([S]); 0 where the win probability is 0."""
    c = H.centers().numpy()
    w = P[..., 1:].sum(-1)
    return np.where(w > 0, (P[..., 1:] * c).sum(-1) / np.maximum(w, 1e-12), 0.0)


def sample_end(P, rng):
    """One sampled ending per row: end HP (0 = a loss), using the bin centres."""
    c = np.concatenate([[0.0], H.centers().numpy()])
    cum = np.cumsum(P, -1)
    u = rng.random(len(P))[:, None]
    return c[np.minimum((u > cum).sum(-1), H.NC - 1)]
