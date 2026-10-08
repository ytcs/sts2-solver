#!/usr/bin/env python3
"""Where `rl/exit.py train` spends a chunk: replaying the fights to rows (`Data.rows` -> `sts2.replay_rows`) vs the forward / backward passes.

  STS2_DEVICE=cuda python tools/perf_exit_train.py target/exit/r4s_000.npz [--chunks 3 --chunk 2048 --mb 2048] [--no-train]

Times `--chunks` chunks of `--chunk` fights (the first ones of the parts, in file order) and prints a sha256 of the replayed rows: two builds of the
extension that replay the same rows print the same digest.
"""
import argparse, hashlib, os, sys, time

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("data", nargs="+")
    ap.add_argument("--init", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--chunks", type=int, default=3)
    ap.add_argument("--chunk", type=int, default=2048)
    ap.add_argument("--mb", type=int, default=2048)
    ap.add_argument("--no-train", action="store_true")
    a = ap.parse_args()
    import exit as X
    from model import DEV, load
    t = time.time()
    data = X.Data(a.data, 0.02)
    print(f"{len(data)} fights loaded in {time.time() - t:.1f}s", flush=True)
    net = None if a.no_train else load(a.init).train()
    opt = None if net is None else torch.optim.AdamW(net.parameters(), lr=1e-5)
    h = hashlib.sha256()
    t_rep = t_fb = 0.0
    rows = 0
    for c in range(a.chunks):
        fights = data.index[c * a.chunk:(c + 1) * a.chunk]
        if not fights:
            break
        t = time.time()
        r = data.rows(fights)
        t_rep += time.time() - t
        rows += len(r[0])
        h.update(r[0].tobytes())
        h.update(r[1].tobytes())
        if net is None:
            continue
        torch.cuda.synchronize() if DEV.type == "cuda" else None
        t = time.time()
        for b in range(0, len(r[0]), a.mb):
            o = torch.from_numpy(r[0][b:b + a.mb]).to(DEV)
            m = torch.from_numpy(r[1][b:b + a.mb].astype(np.int64)).to(DEV)
            lg, _, ol = net(o, m, outcome=True)[:3]
            loss = torch.log_softmax(lg.float(), 1).clamp(min=-30).mean() + ol.float().logsumexp(1).mean()
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
        torch.cuda.synchronize() if DEV.type == "cuda" else None
        t_fb += time.time() - t
    print(f"{rows} rows: replay {t_rep:.2f}s ({rows / max(t_rep, 1e-9):,.0f} rows/s), forward+backward {t_fb:.2f}s; rows digest {h.hexdigest()[:16]}")


if __name__ == "__main__":
    main()
