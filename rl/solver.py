#!/usr/bin/env python3
"""The combat solver: trained network + determinized play-out search, for many fights at once.

  Python:   from solver import Solver
            S = Solver()                                  # models/solver_b128.pt, search 3 options x 8 futures
            res = S.solve(scenarios, attempts=32)         # list of scenario dicts (deck variants ...) -> one result per scenario
  CLI:      .venv/bin/python rl/solver.py --scenarios variants.json --attempts 32 [--no-search] [--out results.json]

Every (scenario, attempt) is one fight; all of them are searched together in one big pool (`fastsearch.FastSearch`: the search runs as a Rust state machine,
Python only evaluates the networks). `solve` returns, per scenario, the win rate with its standard error, the mean fraction of max HP lost over all fights
(a loss or stall is charged the full starting HP), the mean HP left on wins and the outcome counts. Attempts differ in the random streams only
(shuffles, random enemy choices); the solver plays each of them as a player would, seeing only what a player sees.
"""
import argparse, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import load
from fastsearch import FastSearch
from ppo import net_policy

_M = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
DEFAULT_CKPT = os.path.join(_M, "solver_b128.pt")
# value heads averaged into the search's evaluation (policy stays b128's): +2.5 points of win rate at no extra cost (docs/solver.md)
DEFAULT_VALUE_CKPTS = [os.path.join(_M, "solver_c128.pt"), os.path.join(_M, "solver_d128.pt")]
# a checkpoint adopted through `python -m agent.improve adopt` (after it passed the gate) replaces the defaults
_CUR = os.path.join(_M, "current.json")
if os.path.exists(_CUR):
    _c = json.load(open(_CUR))
    DEFAULT_CKPT, DEFAULT_VALUE_CKPTS = _c["policy"], _c["values"]


class Solver:
    def __init__(self, ckpt=DEFAULT_CKPT, M=3, K=8, pmin=0.0, margin=0.0, max_steps=300, value_ckpts="default", roots=None, groups=2, conf=1.01, roll_ckpt=None, amp=None,
                 threads=None):
        """`ckpt`: a checkpoint path, or several (comma-separated string / list) = an ensemble for both policy and value; `value_ckpts`: extra networks
        whose value heads are averaged in while the policy stays the first network's. Defaults: 3 options x 8 futures per decision (best cost / quality).
        `roots`: fights in flight (default 2048 on CUDA, 256 on the CPU); `amp`: bf16 inside CUDA graphs (default on CUDA)."""
        if threads:
            torch.set_num_threads(threads)
        self.net = load(ckpt)
        if value_ckpts == "default":
            value_ckpts = DEFAULT_VALUE_CKPTS if ckpt == DEFAULT_CKPT else None
        self.value_nets = [load(c) for c in value_ckpts] if value_ckpts else []
        self.max_steps = max_steps
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        # a big pool of fights in flight keeps the network batches large (2048 roots x 24 play-outs); bf16 inside CUDA graphs is free (docs/solver.md)
        self.fs = FastSearch(self.net, self.value_nets, M, K, conf=conf, pmin=pmin, margin=margin, max_steps=max_steps, roots=roots or (2048 if cuda else 256), groups=groups,
                             roll_net=load(roll_ckpt) if roll_ckpt else None, amp=cuda if amp is None else amp)
        self.fs.warm()

    def _greedy(self, scenarios, attempts, seed):
        """The network alone (its most probable action every time): rows (outcome, hp_lost, length, hp_end)."""
        flat = [scenarios[i] for _ in range(attempts) for i in range(len(scenarios))]  # attempt-major
        env = sts2.VecEnv(len(flat), flat, seed=seed, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True)
        pol = net_policy(self.net)
        obs, mask = env.reset()
        got = np.zeros(len(flat), bool)
        out = [None] * len(flat)
        while not got.all():
            obs, mask, r, d, info = env.step(pol(obs, mask))
            new = (d > 0) & ~got
            if new.any():
                ei = env.episode_info()
                for i in np.nonzero(new)[0]:
                    out[i] = (int(info["outcome"][i]), float(ei["hp_lost"][i]), int(ei["length"][i]), float(ei["hp_end"][i]))
                got |= new
        return out

    def solve(self, scenarios, attempts=32, search=True, seed=0, verbose=False):
        """One result dict per scenario (same order)."""
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        S = len(scenarios)
        t0 = time.time()
        if search:
            js = np.tile(np.arange(S, dtype=np.uint32), attempts)  # attempt-major: the pool is always spread over the scenarios
            jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
            r = self.fs.run(scenarios, js, jd)
            rows = [(r[i, 1], r[i, 2], r[i, 4], r[i, 3]) for i in range(len(js))]
        else:
            rows = self._greedy(scenarios, attempts, seed)
        if verbose:
            print(f"{S * attempts} fights in {time.time() - t0:.0f}s ({S * attempts / (time.time() - t0):.1f} fights/s)", flush=True)
        res = []
        for i in range(S):
            r = np.array(rows[i::S], dtype=np.float64)
            valid = (r[:, 0] == 1) | (r[:, 0] == -1) | (r[:, 0] == 2)
            win = (r[:, 0] == 1) & valid
            n = int(valid.sum())
            p = win.sum() / max(n, 1)
            res.append(dict(win=float(p), win_se=float((p * (1 - p) / max(n, 1)) ** 0.5), hp_lost=float(r[valid, 1].mean()) if n else None,
                            hp_lost_se=float(r[valid, 1].std(ddof=1) / n ** 0.5) if n > 1 else None,
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
    ap.add_argument("--roots", type=int, default=None); ap.add_argument("--groups", type=int, default=2)
    ap.add_argument("--conf", type=float, default=1.01); ap.add_argument("--roll-ckpt")
    ap.add_argument("--out"); ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    scen = json.load(open(a.scenarios))
    if isinstance(scen, dict):
        scen = [scen]
    S = Solver(a.ckpt, a.M, a.K, value_ckpts=(a.value_extra or None) if a.value_extra or a.ckpt != DEFAULT_CKPT else "default", roots=a.roots, groups=a.groups, conf=a.conf,
               roll_ckpt=a.roll_ckpt)
    res = S.solve(scen, a.attempts, search=not a.no_search, seed=a.seed, verbose=True)
    for sc, r in zip(scen, res):
        print(f"{sc.get('name', '?'):28s} win {r['win']:.3f} ±{r['win_se']:.3f}  HP lost {100 * (r['hp_lost'] or 0):.0f}%  HP left on win {r['hp_left_on_win']:.0f}")
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
