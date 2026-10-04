#!/usr/bin/env python3
"""The combat solver: trained network + determinized play-out search, for many fights at once.

  Python:   from solver import Solver
            S = Solver()                                  # models/solver_b128.pt, search 3 options x 8 futures
            res = S.solve(scenarios, attempts=32)         # list of scenario dicts (deck variants ...) -> one result per scenario
  CLI:      .venv/bin/python rl/solver.py --scenarios variants.json --attempts 32 [--no-search] [--out results.json]

Every (scenario, attempt) is one fight; all of them are searched together in large batches (the network and the simulator are batch machines:
throughput grows with the batch). `solve` returns, per scenario, the win rate with its standard error, the mean fraction of max HP lost over all fights
(a loss or stall is charged the full starting HP), the mean HP left on wins and the outcome counts. Attempts differ in the random streams only
(shuffles, random enemy choices); the solver plays each of them as a player would, seeing only what a player sees.
"""
import argparse, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import Searcher, load
from ppo import net_policy

DEFAULT_CKPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models", "solver_b128.pt")


class Solver:
    def __init__(self, ckpt=DEFAULT_CKPT, M=3, K=8, pmin=0.0, force=False, margin=0.0, threads=8, batch=600, max_steps=300, value_ckpts=None):
        """`ckpt`: a checkpoint path, or several (comma-separated string / list) = an ensemble for both policy and value; `value_ckpts`: extra networks
        whose value heads are averaged in while the policy stays the first network's. Defaults: 3 options x 8 futures per decision (best cost / quality)."""
        torch.set_num_threads(threads)
        self.net = load(ckpt)
        self.value_nets = [load(c) for c in value_ckpts] if value_ckpts else None
        self.M, self.K, self.pmin, self.force, self.margin = M, K, pmin, force, margin
        self.batch, self.max_steps = batch, max_steps

    def _fights(self, scen_per_fight, search, seed):
        """Plays one fight per entry of `scen_per_fight`; returns rows (outcome, hp_lost, length, hp_end)."""
        rows = []
        for i0 in range(0, len(scen_per_fight), self.batch):
            chunk = scen_per_fight[i0:i0 + self.batch]
            if search:
                s = Searcher(self.net, len(chunk), self.M, self.K, self.margin, seed=seed + i0, max_steps=self.max_steps, pmin=self.pmin, force=self.force, value_nets=self.value_nets)
                rec = s.play(chunk, seed=seed + i0, verbose=False, with_records=True, round_robin=True)
                rows += [(r[1], r[2], r[3], r[4]) for r in rec]
            else:
                env = sts2.VecEnv(len(chunk), chunk, seed=seed + i0, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True)
                pol = net_policy(self.net)
                obs, mask = env.reset()
                got = np.zeros(len(chunk), bool)
                out = [None] * len(chunk)
                while not got.all():
                    obs, mask, r, d, info = env.step(pol(obs, mask))
                    new = (d > 0) & ~got
                    if new.any():
                        ei = env.episode_info()
                        for i in np.nonzero(new)[0]:
                            out[i] = (int(info["outcome"][i]), float(ei["hp_lost"][i]), int(ei["length"][i]), float(ei["hp_end"][i]))
                        got |= new
                rows += out
        return rows

    def solve(self, scenarios, attempts=32, search=True, seed=0, verbose=False):
        """One result dict per scenario (same order)."""
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        S = len(scenarios)
        flat = [scenarios[i] for _ in range(attempts) for i in range(S)]  # attempt-major: roots spread over scenarios
        t0 = time.time()
        rows = self._fights(flat, search, seed)
        if verbose:
            print(f"{len(flat)} fights in {time.time() - t0:.0f}s ({len(flat) / (time.time() - t0):.1f} fights/s)", flush=True)
        res = []
        for i in range(S):
            r = np.array(rows[i::S], dtype=np.float64)
            valid = (r[:, 0] == 1) | (r[:, 0] == -1) | (r[:, 0] == 2)
            win = (r[:, 0] == 1) & valid
            n = int(valid.sum())
            p = win.sum() / max(n, 1)
            res.append(dict(win=float(p), win_se=float((p * (1 - p) / max(n, 1)) ** 0.5), hp_lost=float(r[valid, 1].mean()) if n else None,
                            hp_left_on_win=float(r[win, 3].mean() * scenarios[i]["max_hp"]) if win.any() else 0.0, attempts=n, aborted=int((~valid).sum())))
        return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenarios", required=True, help="JSON file: one scenario or a list of scenarios (oracle format)")
    ap.add_argument("--attempts", type=int, default=32)
    ap.add_argument("--ckpt", default=DEFAULT_CKPT)
    ap.add_argument("--M", type=int, default=3); ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--value-extra", nargs="*", default=[], help="extra checkpoints whose value heads are averaged in")
    ap.add_argument("--no-search", action="store_true", help="the network alone (greedy)")
    ap.add_argument("--threads", type=int, default=8); ap.add_argument("--batch", type=int, default=600)
    ap.add_argument("--out"); ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    scen = json.load(open(a.scenarios))
    if isinstance(scen, dict):
        scen = [scen]
    S = Solver(a.ckpt, a.M, a.K, threads=a.threads, batch=a.batch, value_ckpts=a.value_extra or None)
    res = S.solve(scen, a.attempts, search=not a.no_search, seed=a.seed, verbose=True)
    for sc, r in zip(scen, res):
        print(f"{sc.get('name', '?'):28s} win {r['win']:.3f} ±{r['win_se']:.3f}  HP lost {100 * (r['hp_lost'] or 0):.0f}%  HP left on win {r['hp_left_on_win']:.0f}")
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
