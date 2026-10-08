#!/usr/bin/env python3
import argparse, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import load, net_policy
from fastsearch import FastSearch
import heads

_M = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
_c = json.load(open(os.path.join(_M, "current.json")))
DEFAULT_CKPT = os.path.join(_M, _c["policy"])
PREDICTOR_CKPT = os.path.join(_M, _c["predictor"])


class Solver:
    def __init__(self, ckpt=DEFAULT_CKPT, M=3, K=8, max_steps=300, roots=None, groups=2, conf=1.01, roll_ckpt=None, amp=None, threads=None, cover=False):
        if threads:
            torch.set_num_threads(threads)
        self.net = load(ckpt, set_version=False)
        self.max_steps = max_steps
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        self.fs = FastSearch(self.net, M, K, conf=conf, max_steps=max_steps, roots=roots or (2048 if cuda else 256), groups=groups,
                             roll_net=load(roll_ckpt, set_version=False) if roll_ckpt else None, amp=cuda if amp is None else amp, cover=cover)
        self.fs.warm()

    def _greedy(self, scenarios, attempts, seed):
        flat = [scenarios[i] for _ in range(attempts) for i in range(len(scenarios))]
        env = sts2.VecEnv(len(flat), flat, seed=seed, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=heads.TURN_CAP,
                          obs_version=getattr(self.net, "obs_version", 1))
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

    def solve(self, scenarios, attempts=32, search=True, seed=0, verbose=False, groups=None, worth=None):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        S = len(scenarios)
        t0 = time.time()
        if search:
            js = np.tile(np.arange(S, dtype=np.uint32), attempts)
            if groups is None:
                jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
            else:
                g = np.asarray(groups, dtype=np.uint64)[js]
                att = (np.arange(len(js)) // S).astype(np.uint64)
                jd = np.uint64(seed) * np.uint64(1_000_003) + g * np.uint64(attempts) + att
            r = self.fs.run(scenarios, js, jd, worth=worth)
            rows = [(r[i, 1], r[i, 2], r[i, 4], r[i, 3], r[i, 6]) for i in range(len(js))]
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
                            hp_left_on_win=float(r[win, 3].mean() * scenarios[i]["max_hp"]) if win.any() else 0.0, attempts=n, aborted=int((~valid).sum()),
                            ends=np.where(win, r[:, 3] * scenarios[i]["max_hp"], 0.0)[valid].tolist(),
                            ends_abs=[float(x) if ok else None for x, ok in zip(r[:, 4], valid)] if r.shape[1] > 4 else None,
                            wins=[float(w) if ok else None for w, ok in zip(win, valid)]))
        return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenarios", required=True, help="JSON file: one scenario or a list of scenarios (oracle format)")
    ap.add_argument("--attempts", type=int, default=32)
    ap.add_argument("--ckpt", default=DEFAULT_CKPT)
    ap.add_argument("--M", type=int, default=3); ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--no-search", action="store_true", help="the network alone (greedy)")
    ap.add_argument("--roots", type=int, default=None); ap.add_argument("--groups", type=int, default=2)
    ap.add_argument("--conf", type=float, default=1.01); ap.add_argument("--roll-ckpt")
    ap.add_argument("--out"); ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    scen = json.load(open(a.scenarios))
    if isinstance(scen, dict):
        scen = [scen]
    S = Solver(a.ckpt, a.M, a.K, roots=a.roots, groups=a.groups, conf=a.conf, roll_ckpt=a.roll_ckpt)
    res = S.solve(scen, a.attempts, search=not a.no_search, seed=a.seed, verbose=True)
    for sc, r in zip(scen, res):
        print(f"{sc.get('name', '?'):28s} win {r['win']:.3f} ±{r['win_se']:.3f}  HP lost {100 * (r['hp_lost'] or 0):.0f}%  HP left on win {r['hp_left_on_win']:.0f}")
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
