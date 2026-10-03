#!/usr/bin/env python3
"""Expert iteration: distill the play-out search into the network.

  collect:  .venv/bin/python rl/exit.py collect --ckpt C --train target/train/train.json --roots 200 --out target/exit/d1.npz
  train:    .venv/bin/python rl/exit.py train --ckpt C --data target/exit/d1.npz [more.npz ...] --out target/exit/c1.pt

`collect` plays whole fights with the search (`rl/search.py`) and keeps, for every searched decision, the observation, the legal mask,
the searched actions with their estimated returns Q and the realized return z of the fight. `train` fits the policy to
softmax(Q / tau) over the searched actions (cross-entropy) and the value head to a blend of z and the search value, so the network
alone plays closer to what the search would play. Continue with PPO (`rl/ppo.py --resume C --warm`) or run the search again.
"""
import argparse, json, os, sys, time
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import Searcher, load


def final_reward(rec, hp_bonus=0.5):
    # rec = (scenario, outcome, hp_lost, length, hp_end)
    return (1.0 + hp_bonus * rec[4]) if rec[1] == 1 else -1.0


def collect(a):
    torch.set_num_threads(a.threads)
    net = load(a.ckpt)
    out = {k: [] for k in ("obs", "mask", "acts", "legal", "q", "p", "z")}
    t0 = time.time()
    for chunk in range(a.chunks):
        rec = []
        s = Searcher(net, a.roots, a.M, a.K, a.margin, seed=a.seed + chunk, max_steps=a.max_steps, conf=a.conf, record=rec)
        fights = s.play(a.train, seed=a.seed * 1000 + chunk, verbose=False, with_records=True)
        z = np.array([final_reward(r) for r in fights])
        for d in rec:
            for k in ("obs", "mask", "acts", "legal", "q", "p"):
                out[k].append(d[k])
            out["z"].append(z[d["root"]])
        print(f"chunk {chunk}: {len(rec)} searched decisions, win {np.mean([r[1] == 1 for r in fights]):.3f}, {time.time() - t0:.0f}s", flush=True)
    os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
    np.savez_compressed(a.out, **{k: np.array(v) for k, v in out.items()})


def train(a):
    torch.set_num_threads(a.threads)
    ck = torch.load(a.ckpt)
    net = load(a.ckpt)
    net.train()
    data = {k: [] for k in ("obs", "mask", "acts", "legal", "q", "p", "z")}
    for f in a.data:
        d = np.load(f)
        for k in data:
            data[k].append(d[k])
    D = {k: np.concatenate(v) for k, v in data.items()}
    n = len(D["z"])
    print("samples", n, flush=True)
    obs = torch.nan_to_num(torch.from_numpy(D["obs"].astype(np.float32)), posinf=60000.0, neginf=-60000.0)
    mask = torch.from_numpy(D["mask"].astype(np.int64))
    acts = torch.from_numpy(D["acts"].astype(np.int64))
    legal = torch.from_numpy(D["legal"])
    q = torch.from_numpy(D["q"]).float()
    z = torch.from_numpy(D["z"]).float()
    # target distribution over the searched actions
    qm = q.masked_fill(~legal, -1e9)
    tgt = torch.softmax(qm / a.tau, 1)
    vtgt = a.blend * z + (1 - a.blend) * qm.max(1).values
    opt = torch.optim.Adam(net.parameters(), lr=a.lr)
    perm_n = int(n * (1 - a.holdout))
    for ep in range(a.epochs):
        perm = torch.randperm(perm_n)
        tot = [0.0, 0.0, 0]
        for s0 in range(0, perm_n, a.mb):
            ix = perm[s0:s0 + a.mb]
            lg, v = net(obs[ix], mask[ix])
            logp = F.log_softmax(lg, 1)
            lp = logp.gather(1, acts[ix])  # [mb, M] log-probs of the searched actions
            pl = -(tgt[ix] * lp.masked_fill(~legal[ix], 0.0)).sum(1).mean()
            vl = F.smooth_l1_loss(v, vtgt[ix])
            loss = pl + a.vf * vl
            opt.zero_grad(); loss.backward(); torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0); opt.step()
            tot[0] += pl.item() * len(ix); tot[1] += vl.item() * len(ix); tot[2] += len(ix)
        with torch.no_grad():
            hx = torch.arange(perm_n, n)
            if len(hx):
                lg, v = net(obs[hx], mask[hx])
                lp = F.log_softmax(lg, 1).gather(1, acts[hx])
                hpl = -(tgt[hx] * lp.masked_fill(~legal[hx], 0.0)).sum(1).mean().item()
                top = (acts[hx].gather(1, qm[hx].argmax(1, keepdim=True)).squeeze(1) == lg.argmax(1)).float().mean().item()
            else:
                hpl = top = float("nan")
        print(f"epoch {ep}: policy CE {tot[0] / tot[2]:.4f} value {tot[1] / tot[2]:.4f} | holdout CE {hpl:.4f} agree-with-search-best {top:.3f}", flush=True)
    args = dict(ck.get("args", {}))
    torch.save({"net": net.state_dict(), "opt": opt.state_dict(), "it": 0, "steps": 0, "args": args}, a.out)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("collect")
    c.add_argument("--ckpt", required=True); c.add_argument("--train", required=True); c.add_argument("--out", required=True)
    c.add_argument("--roots", type=int, default=200); c.add_argument("--chunks", type=int, default=1)
    c.add_argument("--M", type=int, default=5); c.add_argument("--K", type=int, default=6); c.add_argument("--margin", type=float, default=0.0)
    c.add_argument("--conf", type=float, default=0.97); c.add_argument("--seed", type=int, default=0)
    c.add_argument("--threads", type=int, default=4); c.add_argument("--max-steps", type=int, default=300)
    t = sub.add_parser("train")
    t.add_argument("--ckpt", required=True); t.add_argument("--data", nargs="+", required=True); t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=4); t.add_argument("--mb", type=int, default=512); t.add_argument("--lr", type=float, default=1e-4)
    t.add_argument("--tau", type=float, default=0.1); t.add_argument("--blend", type=float, default=0.5); t.add_argument("--vf", type=float, default=0.5)
    t.add_argument("--holdout", type=float, default=0.05); t.add_argument("--threads", type=int, default=4)
    a = ap.parse_args()
    collect(a) if a.cmd == "collect" else train(a)


if __name__ == "__main__":
    main()
