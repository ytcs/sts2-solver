#!/usr/bin/env python3
"""Soft-target distillation of the search into the network (all searched decisions, not only disagreements).

  .venv/bin/python rl/train_soft.py --ckpt BASE.pt --data all_*.npz [--mined confirmed_*.npz] --out NEW.pt [--tau 0.1] [--device cuda]

Data (from `rl/mine.py --save-all`): per searched decision the observation, legal mask, the searched actions with their estimated returns Q (end-of-turn
play-outs + value head, 8 paired futures) and the final reward z of the fight it came from.
Loss = cross-entropy of the policy to softmax(Q / tau) over the searched actions
       + value regression to  w * z + (1 - w) * max Q          (z: what really happened, noisy; max Q: the search's estimate, biased by the value head)
       + `anchor` x KL(old policy || new policy) on the same states restricted to ordinary play (keeps what the network already does right).
Optionally the confirmed disagreements (`--mined`) are added with a one-hot target and extra weight.
"""
import argparse, os, sys, time
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from search import load
from model import DEV


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True); ap.add_argument("--data", nargs="+", required=True); ap.add_argument("--mined", nargs="*", default=[]); ap.add_argument("--out", required=True)
    ap.add_argument("--tau", type=float, default=0.1); ap.add_argument("--vw", type=float, default=0.5, help="weight of z in the value target")
    ap.add_argument("--pw", type=float, default=1.0, help="weight of the policy loss (0 = value-only fine-tune)")
    ap.add_argument("--anchor", type=float, default=0.3); ap.add_argument("--mined-weight", type=float, default=3.0)
    ap.add_argument("--epochs", type=int, default=3); ap.add_argument("--lr", type=float, default=1e-4); ap.add_argument("--mb", type=int, default=1024)
    ap.add_argument("--holdout", type=float, default=0.03); ap.add_argument("--seed", type=int, default=0); ap.add_argument("--min-gap", type=float, default=0.0,
                    help="only train the policy on decisions whose best and worst searched option differ by at least this much (the rest carry no signal)")
    ap.add_argument("--threads", type=int, default=8)
    a = ap.parse_args()
    torch.set_num_threads(a.threads); torch.manual_seed(a.seed)
    net, ref = load(a.ckpt), load(a.ckpt)
    cols = ("obs", "mask", "acts", "legal", "q", "z")
    D = {k: np.concatenate([np.load(f)[k] for f in a.data]) for k in cols}
    n = len(D["z"])
    obs = torch.nan_to_num(torch.from_numpy(D["obs"].astype(np.float32)), posinf=60000.0, neginf=-60000.0)
    mask = torch.from_numpy(D["mask"].astype(np.uint8)); acts = torch.from_numpy(D["acts"].astype(np.int64)); legal = torch.from_numpy(D["legal"])
    q = torch.from_numpy(D["q"]).float(); z = torch.from_numpy(D["z"]).float()
    qm = q.masked_fill(~legal, -1e9)
    gap = qm.max(1).values - torch.where(legal, q, torch.full_like(q, 1e9)).min(1).values
    use_pi = gap >= a.min_gap
    tgt = torch.softmax(qm / a.tau, 1)
    vtgt = a.vw * z + (1 - a.vw) * qm.max(1).values
    print(f"{n} searched decisions; {int(use_pi.sum())} with an option spread >= {a.min_gap}", flush=True)
    # confirmed disagreements: one-hot targets, extra weight
    mo = mm = ms = None
    if a.mined:
        M = {k: np.concatenate([np.load(f)[k] for f in a.mined]) for k in ("obs", "mask", "search")}
        mo = torch.nan_to_num(torch.from_numpy(M["obs"].astype(np.float32)), posinf=60000.0, neginf=-60000.0); mm = torch.from_numpy(M["mask"].astype(np.uint8)); ms = torch.from_numpy(M["search"].astype(np.int64))
        print(f"{len(ms)} confirmed disagreements added (weight {a.mined_weight})", flush=True)
    net.to(DEV).train(); ref.to(DEV).eval()
    opt = torch.optim.Adam(net.parameters(), lr=a.lr)
    nh = max(1, int(n * a.holdout)); perm0 = torch.randperm(n); te, tr = perm0[:nh], perm0[nh:]
    def agree(ix):
        with torch.no_grad():
            lg, v = net(obs[ix].to(DEV), mask[ix].long().to(DEV))
        best = acts[ix].gather(1, qm[ix].argmax(1, keepdim=True)).squeeze(1)
        return (lg.argmax(1).cpu() == best).float().mean().item(), F.mse_loss(v.cpu(), vtgt[ix]).item()
    print("holdout before: policy agrees with the search's best option %.3f, value mse %.4f" % agree(te), flush=True)
    steps = 0; t0 = time.time()
    for ep in range(a.epochs):
        perm = tr[torch.randperm(len(tr))]
        for s0 in range(0, len(perm), a.mb):
            ix = perm[s0:s0 + a.mb]
            o, m = obs[ix].to(DEV), mask[ix].long().to(DEV)
            lg, v = net(o, m)
            logp = F.log_softmax(lg, 1)
            lp = logp.gather(1, acts[ix].to(DEV))
            w = use_pi[ix].float().to(DEV)
            pl = -((tgt[ix].to(DEV) * lp.masked_fill(~legal[ix].to(DEV), 0.0)).sum(1) * w).sum() / w.sum().clamp(min=1)
            vl = F.smooth_l1_loss(v, vtgt[ix].to(DEV))
            with torch.no_grad():
                rlg, _ = ref(o, m)
            kl = (F.softmax(rlg, 1) * (F.log_softmax(rlg, 1).clamp(min=-30) - logp.clamp(min=-30))).sum(1).mean()
            loss = a.pw * pl + 0.5 * vl + a.anchor * kl
            if mo is not None:
                j = torch.randint(0, len(ms), (max(1, a.mb // 8),))
                mlg, _ = net(mo[j].to(DEV), mm[j].long().to(DEV))
                loss = loss + a.mined_weight * F.cross_entropy(mlg, ms[j].to(DEV))
            opt.zero_grad(); loss.backward(); torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0); opt.step(); steps += 1
        print(f"epoch {ep}: policy {pl.item():.3f} value {vl.item():.4f} kl-to-old {kl.item():.4f} | holdout agrees %.3f, value mse %.4f  ({time.time() - t0:.0f}s)" % agree(te), flush=True)
    ck = torch.load(a.ckpt)
    torch.save({"net": net.state_dict(), "args": ck.get("args", {}), "it": ck.get("it", 0), "steps": ck.get("steps", 0)}, a.out)
    print("saved", a.out)


if __name__ == "__main__":
    main()
