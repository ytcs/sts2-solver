import json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from model import load
from fastsearch import FastSearch

_M = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
_c = json.load(open(os.path.join(_M, "current.json")))
DEFAULT_CKPT = os.path.join(_M, _c["policy"])
PREDICTOR_CKPT = os.path.join(_M, os.environ.get("STS2_PREDICTOR") or _c["predictor"])


class Solver:
    def __init__(self, ckpt=DEFAULT_CKPT, M=3, K=8, roots=None, groups=2, threads=None, cover=False):
        if threads:
            torch.set_num_threads(threads)
        self.net = load(ckpt)
        cuda = torch.cuda.is_available() and os.environ.get("STS2_DEVICE", "cpu").startswith("cuda")
        self.fs = FastSearch(self.net, M, K, max_steps=300, roots=roots or (2048 if cuda else 256), groups=groups, amp=cuda, cover=cover)
        self.fs.warm()

    def solve(self, scenarios, attempts=32, seed=0, groups=None, worth=None):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        S = len(scenarios)
        per = None
        if not np.isscalar(attempts):
            # per-scenario attempts (several evaluations in one batch): each job keeps the seed it gets in a call of its own
            A = np.asarray(attempts, dtype=np.int64)
            js = np.concatenate([np.full(a, i, dtype=np.uint32) for i, a in enumerate(A)])
            att = np.concatenate([np.arange(a, dtype=np.uint64) for a in A])
            g = np.asarray(groups if groups is not None else np.arange(S), dtype=np.uint64)[js]
            jd = np.uint64(seed) * np.uint64(1_000_003) + g * A[js].astype(np.uint64) + att
            r = self.fs.run(scenarios, js, jd, worth=worth)
            allrows = [(r[i, 1], r[i, 2], r[i, 4], r[i, 3], r[i, 6], r[i, 8]) for i in range(len(js))]
            per = [[allrows[j] for j in np.flatnonzero(js == i)] for i in range(S)]
        else:
            js = np.tile(np.arange(S, dtype=np.uint32), attempts)
            if groups is None:
                jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
            else:
                g = np.asarray(groups, dtype=np.uint64)[js]
                att = (np.arange(len(js)) // S).astype(np.uint64)
                jd = np.uint64(seed) * np.uint64(1_000_003) + g * np.uint64(attempts) + att
            r = self.fs.run(scenarios, js, jd, worth=worth)
            rows = [(r[i, 1], r[i, 2], r[i, 4], r[i, 3], r[i, 6], r[i, 8]) for i in range(len(js))]
        res = []
        for i in range(S):
            r = np.array(rows[i::S] if per is None else per[i], dtype=np.float64)
            valid = (r[:, 0] == 1) | (r[:, 0] == -1) | (r[:, 0] == 2)
            win = (r[:, 0] == 1) & valid
            n = int(valid.sum())
            p = win.sum() / max(n, 1)
            res.append(dict(win=float(p), win_se=float((p * (1 - p) / max(n, 1)) ** 0.5), hp_lost=float(r[valid, 1].mean()) if n else None,
                            hp_lost_se=float(r[valid, 1].std(ddof=1) / n ** 0.5) if n > 1 else None,
                            hp_left_on_win=float(r[win, 3].mean() * scenarios[i]["max_hp"]) if win.any() else 0.0, attempts=n, aborted=int((~valid).sum()),
                            ends=np.where(win, r[:, 3] * scenarios[i]["max_hp"], 0.0)[valid].tolist(),
                            ends_abs=[float(x) if ok else None for x, ok in zip(r[:, 4], valid)] if r.shape[1] > 4 else None,
                            wins=[float(w) if ok else None for w, ok in zip(win, valid)],
                            progress=[float(x) if ok else None for x, ok in zip(r[:, 5], valid)] if r.shape[1] > 5 else None))
        return res

