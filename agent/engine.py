import json
import math
import os
import sys
import time

import numpy as np
import torch

from agent import potions, proposal

_RL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl")
sys.path.insert(0, _RL)
import heads  # noqa: E402
from fastsearch import FastSearch  # noqa: E402
from solver import Solver  # noqa: E402

MAX_ROUNDS = 400


def _opportunity_loss(acc):
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


# reserve value of a held potion in objective units (E61: run model ~10 HP = 0.063; near-miss flat 0.03-0.06)
POT_COST = 0.06


class Engine:
    def __init__(self, M=5, K=32, ckpt=None, cover=True, futures=0, exact_turn=True, hp_cap=True, pot_cost=POT_COST):
        self.solver = Solver() if ckpt is None else Solver(ckpt)
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        self.fs = FastSearch(self.solver.net, M, K, roots=1, groups=1, amp=cuda, cover=cover, futures=futures, exact_turn=exact_turn,
                             hp_cap=hp_cap, pot_cost=pot_cost)
        self.pot_cost = pot_cost
        self.worth_ok = bool(self.fs.dist and self.solver.fs.dist)
        assert (proposal.HEAD_BIN, proposal.HEAD_NC) == (heads.BIN, heads.NC), "agent/proposal.py and rl/heads.py disagree on the outcome classes"
        self.fs.warm()
        self.seed = 0
        self.table_seed = 0

    def keep(self, aside=(), boss=False):
        """potions the search must not throw: all of them without a reserve cost, else the ones set aside for a later boss"""
        return True if self.pot_cost <= 0 else set() if boss else set(aside)

    def decide(self, scenario, sim, budget=1.0, seed=None, tol_hp=1.0, keep_potions=False, worth=None, rounds=None):
        t0 = time.perf_counter()
        tol = tol_hp * 0.5 / max(scenario.get("max_hp", 80), 1)
        if worth is not None:
            u = np.asarray(worth["u"], np.float64)
            tol *= float(u.max() - u[0]) / 2.5
        n_rounds, acc, first, rounds = rounds, {}, None, 0
        # potion reserve cost: none after the final boss; win-only worth spans 1 (loss 0, win 1) vs 2 for the linear objective
        self.fs.pot_cost = 0.0 if worth is not None and worth.get("final") else self.pot_cost / 2 if worth is not None else self.pot_cost
        held = potions.held_indices(sim, keep_potions)
        def _held(t):
            return t.startswith("potion") and (keep_potions is True or potions.text_index(t) in held)
        skip = {a for a, t in sim.legal() if t.startswith("discard potion") or _held(t)}
        # held potions leave the searched copy, so no line of the search can throw them
        search = sim.without_potions(held) if held else sim
        n_rows = [0, 0]
        while True:
            self.seed += 1
            r = self.fs.decide(scenario, search, (self.seed if seed is None else seed + rounds), worth=worth)
            st = getattr(self.fs, "stats", None) or {}
            n_rows[0] += int(st.get("policy_rows", 0))
            n_rows[1] += int(st.get("value_rows", 0))
            rounds += 1
            if first is None:
                first = r
            if not r["searched"]:
                break
            for a, q, ok in zip(r["opts"], r["q"], r["legal"]):
                if ok and a not in skip and not np.isnan(q):
                    acc.setdefault(a, []).append(float(q))
            if n_rounds is not None:
                if rounds >= n_rounds:
                    break
            elif time.perf_counter() - t0 >= budget or rounds >= MAX_ROUNDS or (rounds >= 4 and _opportunity_loss(acc) < tol):
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
                    seconds=round(time.perf_counter() - t0, 2), rows=tuple(n_rows), options=opts)

    def solve(self, scenarios, attempts=64, seed=None, groups=None, worth=None):
        return self.solver.solve(scenarios, attempts=attempts, seed=self.table_seed if seed is None else seed, groups=groups,
                                 worth=None if worth is None else [worth] * len(scenarios))

    def play_on(self, scenario, starts, seeds, worth=None, record=False):
        fs = self.solver.fs
        old = fs.record
        fs.record = record
        try:
            rows = fs.run([scenario] * len(starts), np.arange(len(starts), dtype=np.uint32), np.asarray(seeds, np.uint64), starts=list(starts),
                          worth=None if worth is None else [worth] * len(starts))
            out = [(int(r[1]), float(r[3]), float(r[6])) for r in rows]
            if record:
                out = [o + (fs.job_actions(j),) for j, o in enumerate(out)]
        finally:
            fs.record = old
        return out


def play_fight(eng, scenario, seed, budget, tol_hp=0.25, max_steps=400, keep_potions=False, worth=None):
    import sts2
    sim = sts2.Sim(json.dumps(scenario), seed)
    hp0 = json.loads(sim.snapshot())["player"]["hp"]
    steps = 0
    while sim.outcome() == 0 and steps < max_steps and sim.stage() != "over":
        d = eng.decide(scenario, sim, 0.3 if sim.stage() == "choice" else budget, tol_hp=tol_hp, keep_potions=keep_potions, worth=worth)
        sim.step(d["action"])
        steps += 1
    hp1 = json.loads(sim.snapshot())["player"]["hp"]
    return sim.outcome(), hp0 - max(hp1, 0), steps
