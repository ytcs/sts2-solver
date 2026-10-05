"""The solver as a service for the harness.

  eng = Engine()                                  # loads the networks once (STS2_DEVICE=cuda for the GPU; about a minute incl. graph capture)
  d = eng.decide(scenario, sim, budget=1.0)       # micro: best next action of a fight in progress, within a time budget (seconds)
  r = eng.solve(scenarios, attempts=64)           # macro / prediction: win rate and HP lost of fights played from their start

`decide` conditions on what a player can observe (hand, piles as multisets, HP / block / energy, powers, relic counters, potions, the enemies' intents and
known patterns) and resamples hidden information (draw order, enemy random branches, RNG) for every future. One round tries the policy's 5 likeliest
actions on 32 futures each (~10 ms); rounds repeat with fresh futures until the budget is spent or the best action is clearly ahead, so an obvious turn
costs a fraction of a second and a high-stakes turn can be given more time. `budget=0` is a single round.
"""
import math
import os
import sys
import time

import numpy as np
import torch

_RL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl")
sys.path.insert(0, _RL)
from fastsearch import FastSearch  # noqa: E402
from solver import Solver  # noqa: E402

MAX_ROUNDS = 400


def _separated(acc, z=3.0):
    """True when the best action's mean return exceeds the runner-up's by `z` standard errors (round means as samples, at least 3 rounds)."""
    ranked = sorted(((np.mean(v), np.var(v, ddof=1) / len(v), a) for a, v in acc.items() if len(v) >= 3), reverse=True)
    if len(ranked) < 2:
        return len(ranked) == 1 and len(acc) == 1
    (m1, v1, _), (m2, v2, _) = ranked[0], ranked[1]
    return (m1 - m2) > z * math.sqrt(v1 + v2 + 1e-12)


class Engine:
    def __init__(self, M=5, K=32):
        self.solver = Solver()
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        self.fs = FastSearch(self.solver.net, self.solver.value_nets, M, K, conf=1.01, roots=1, groups=1, amp=cuda)
        self.fs.warm()
        self.seed = 0

    def decide(self, scenario, sim, budget=1.0, seed=None):
        """Best next action for the fight in `sim`. Returns dict(action, json, text, searched, rounds, seconds, options=[dict(action, text, p, q)]);
        `json` is the oracle-script form of the action (sent to the bridge's `do`); a selection is answered pick by pick (see `agent.harness`)."""
        t0 = time.perf_counter()
        acc, first, rounds = {}, None, 0
        while True:
            self.seed += 1
            r = self.fs.decide(scenario, sim, (self.seed if seed is None else seed + rounds))
            rounds += 1
            if first is None:
                first = r
            if not r["searched"]:
                break  # a forced move: nothing to refine
            for a, q, ok in zip(r["opts"], r["q"], r["legal"]):
                if ok and not np.isnan(q):
                    acc.setdefault(a, []).append(float(q))
            if time.perf_counter() - t0 >= budget or rounds >= MAX_ROUNDS or (rounds >= 3 and _separated(acc)):
                break
        text = dict(sim.legal())
        opts = []
        for a, p, ok in zip(first["opts"], first["p"], first["legal"]):
            if ok:
                q = float(np.mean(acc[a])) if a in acc else None
                opts.append(dict(action=a, text=text.get(a, f"#{a}"), p=round(float(p), 3), q=None if q is None else round(q, 3)))
        best = max((o for o in opts if o["q"] is not None), key=lambda o: o["q"], default=None)
        a = best["action"] if best else first["action"]
        return dict(action=a, json=sim.action_json(a), text=text.get(a, f"#{a}"), searched=first["searched"], rounds=rounds,
                    seconds=round(time.perf_counter() - t0, 2), options=opts)

    def solve(self, scenarios, attempts=64, seed=0):
        """Fights played from their start by the batch solver: one dict per scenario (win, win_se, hp_lost, hp_left_on_win, attempts, aborted)."""
        return self.solver.solve(scenarios, attempts=attempts, seed=seed)
