"""The solver as a service for the harness.

  eng = Engine()                                  # loads the networks once (STS2_DEVICE=cuda for the GPU; about a minute incl. graph capture)
  d = eng.decide(scenario, sim, budget=1.0)       # micro: best next action of a fight in progress, within a time budget (seconds)
  r = eng.solve(scenarios, attempts=64)           # macro / prediction: win rate and HP lost of fights played from their start

`decide` conditions on what a player can observe (hand, piles as multisets, HP / block / energy, powers, relic counters, potions, the enemies' intents and
known patterns) and resamples hidden information (draw order, enemy random branches, RNG) for every future. One round tries the policy's 5 likeliest
actions on 32 futures each (~10 ms); rounds repeat with fresh futures until the budget is spent or the best action is clearly ahead, so an obvious turn
costs a fraction of a second and a high-stakes turn can be given more time. `budget=0` is a single round.
"""
import json
import math
import os
import sys
import time

import numpy as np
import torch

from agent import potions

_RL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl")
sys.path.insert(0, _RL)
from fastsearch import FastSearch  # noqa: E402
from solver import Solver  # noqa: E402

MAX_ROUNDS = 400


def _read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def _opportunity_loss(acc):
    """Expected regret of stopping now (Bayesian expected opportunity loss, as in ranking-and-selection): the largest, over the other options, of
    E[max(0, mu_other - mu_best)] with the means' standard errors from the round means (at least 4 rounds each). It is ~0 both when one option is clearly ahead
    and when the options tie (all lines cost the same), and large only when thinking can still change the outcome. Units: return (1 HP = 0.5 / max HP)."""
    ranked = sorted(((np.mean(v), np.var(v, ddof=1) / len(v)) for v in acc.values() if len(v) >= 4), reverse=True)
    if len(ranked) < 2:
        return 0.0 if len(acc) <= 1 else math.inf
    m1, v1 = ranked[0]
    worst = 0.0
    for m2, v2 in ranked[1:]:
        se = math.sqrt(v1 + v2 + 1e-12)
        d = (m1 - m2) / se
        pdf = math.exp(-0.5 * d * d) / math.sqrt(2 * math.pi)
        cdf_neg = 0.5 * math.erfc(d / math.sqrt(2))
        worst = max(worst, se * (pdf - d * cdf_neg))
    return worst


class Engine:
    def __init__(self, M=5, K=32, ckpt=None, value_ckpts="default"):
        """`ckpt` / `value_ckpts`: other networks than the adopted ones (a gate or an A/B)."""
        self.solver = Solver() if ckpt is None else Solver(ckpt, value_ckpts=value_ckpts)
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        self.fs = FastSearch(self.solver.net, self.solver.value_nets, M, K, conf=1.01, roots=1, groups=1, amp=cuda)
        # live play uses the fight's HP-worth curve only with networks trained for it (`models/current.json` "util": true, written at adoption)
        cur = os.path.join(_RL, "..", "models", "current.json")
        self.util_trained = ckpt is None and os.path.exists(cur) and bool(_read_json(cur).get("util"))
        self.fs.warm()
        self.seed = 0
        # seed of the tables (`solve` without a seed): the harness sets it per screen, so a re-run on the same screen repeats the same draws (and every variant
        # of one call shares them: common random numbers) while the next screen, or `--seed N`, draws fresh ones
        self.table_seed = 0

    def decide(self, scenario, sim, budget=1.0, seed=None, tol_hp=1.0, keep_potions=False, util=None):
        """Best next action for the fight in `sim`. Returns dict(action, json, text, searched, rounds, seconds, options=[dict(action, text, p, q)]);
        Search stops at `budget` seconds or when the expected regret of the leading action is below `tol_hp` HP; `json` is the oracle-script form of the action (sent to the bridge's `do`); a selection is answered pick by pick (see `agent.harness`)."""
        t0 = time.perf_counter()
        tol = tol_hp * 0.5 / max(scenario.get("max_hp", 80), 1)  # the return counts half the HP fraction left
        # `util`: the fight's HP-worth curve (101 floats, `rl/utility.py`) = what each ending is worth for the rest of the act; None = the linear return
        self.fs.set_util(util)
        acc, first, rounds = {}, None, 0
        # simulator slots of the potions held back (`agent.potions`): the scenario's order (a thrown potion leaves its slot empty; the slots do not shift)
        held = potions.held_indices(scenario, keep_potions)
        def _held(t):
            return t.startswith("potion") and (keep_potions is True or potions.text_index(t) in held)
        skip = {a for a, t in sim.legal() if t.startswith("discard potion") or _held(t)}  # the bridge cannot discard a potion, and a tie must never throw one away
        # held potions leave the searched copy entirely: filtering only the first action still let deeper lines of the tree throw them (and value those lines)
        search = sim.without_potions(held) if held else sim
        while True:
            self.seed += 1
            r = self.fs.decide(scenario, search, (self.seed if seed is None else seed + rounds))
            rounds += 1
            if first is None:
                first = r
            if not r["searched"]:
                break  # a forced move: nothing to refine
            for a, q, ok in zip(r["opts"], r["q"], r["legal"]):
                if ok and a not in skip and not np.isnan(q):
                    acc.setdefault(a, []).append(float(q))
            if time.perf_counter() - t0 >= budget or rounds >= MAX_ROUNDS or (rounds >= 4 and _opportunity_loss(acc) < tol):
                break
        text = dict(sim.legal())
        opts = []
        for a, p, ok in zip(first["opts"], first["p"], first["legal"]):
            if ok:
                q = float(np.mean(acc[a])) if a in acc else None
                opts.append(dict(action=a, text=text.get(a, f"#{a}"), p=round(float(p), 3), q=None if q is None else round(q, 3)))
        best = max((o for o in opts if o["q"] is not None), key=lambda o: o["q"], default=None)
        a = best["action"] if best else first["action"]
        self.fs.set_util(None)
        return dict(action=a, json=sim.action_json(a), text=text.get(a, f"#{a}"), searched=first["searched"], rounds=rounds,
                    seconds=round(time.perf_counter() - t0, 2), options=opts)

    def solve(self, scenarios, attempts=64, seed=None, util=None, groups=None):
        """Fights played from their start by the batch solver: one dict per scenario (win, win_se, hp_lost, hp_left_on_win, attempts, aborted).
        `util`: play them under this HP-worth curve (the results stay raw HP outcomes); None = the linear return."""
        self.solver.fs.set_util(util)
        try:
            return self.solver.solve(scenarios, attempts=attempts, seed=self.table_seed if seed is None else seed, groups=groups)
        finally:
            self.solver.fs.set_util(None)

    def play_on(self, scenario, starts, seeds):
        """Fights continued from the simulators `starts` (one per job, e.g. determinized copies of a live fight) by the batch solver with job seeds `seeds`:
        one (outcome, end HP fraction) per job (outcome 1 = win)."""
        rows = self.solver.fs.run([scenario] * len(starts), np.arange(len(starts), dtype=np.uint32), np.asarray(seeds, np.uint64), starts=list(starts))
        return [(int(r[1]), float(r[3])) for r in rows]


def play_fight(eng, scenario, seed, budget, tol_hp=0.25, max_steps=400, keep_potions=False, util=None):
    """One fight in the simulator from its start, every decision by `Engine.decide` at the given time cap. Returns (outcome, HP lost, steps)."""
    import sts2
    sim = sts2.Sim(json.dumps(scenario), seed)
    hp0 = json.loads(sim.snapshot())["player"]["hp"]
    steps = 0
    while sim.outcome() == 0 and steps < max_steps and sim.stage() != "over":
        d = eng.decide(scenario, sim, 0.3 if sim.stage() == "choice" else budget, tol_hp=tol_hp, keep_potions=keep_potions, util=util)
        sim.step(d["action"])
        steps += 1
    hp1 = json.loads(sim.snapshot())["player"]["hp"]
    return sim.outcome(), hp0 - max(hp1, 0), steps
