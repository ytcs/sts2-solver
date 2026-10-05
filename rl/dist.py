#!/usr/bin/env python3
"""End-HP distribution head: P(lost) and P(win with HP fraction in each of 20 bins) for a fight state, on top of a FROZEN solver network's trunk
(`Net.trunk`, the context its value head reads). The search can then score a state by any utility of the ending HP, E[U] = sum_b P(b) U(b), instead of
the mean of the linear return (+1 + 0.5 x HP fraction / -1), which cannot see that a fight's losses come in clusters one enemy turn apart.

  train:  python rl/dist.py train --data target/dist/a.npz[,b.npz] --base models/solver_b128.pt --out models/dist_b128.pt [--epochs 30]
  check:  python rl/dist.py check --data target/dist/held.npz --head models/dist_b128.pt

Data: `tools/gen_dist.py` (search-played fights, labels 0 = loss, 1..20 = win in HP-fraction bin b-1).
"""
import argparse, os, sys, time
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from model import DEV, load, mlp  # noqa: E402

NB = 20
NC = NB + 1  # class 0 = loss


def bin_centers():
    """HP fraction at the centre of each win bin (class 1..20)."""
    return (np.arange(NB) + 0.5) / NB


def linear_util(hp_bonus=0.5, win=1.0, loss=-1.0):
    """The solver's own return as a utility over the classes: E[U] then equals the mean linear return (up to the bin width)."""
    return np.concatenate([[loss], win + hp_bonus * bin_centers()]).astype(np.float32)


class DistHead(nn.Module):
    def __init__(self, d2):
        super().__init__()
        self.head = mlp(d2, d2, NC)

    def forward(self, gctx):
        return self.head(gctx)


def load_head(path, base=None):
    """(base network, head) on DEV in eval mode. `base`: an already loaded network to share (it must be the checkpoint the head was trained on)."""
    ck = torch.load(path, map_location="cpu")
    net = base if base is not None else load(ck["base"])
    h = DistHead(ck["d2"])
    h.load_state_dict(ck["head"])
    return net, h.to(DEV).eval()


@torch.no_grad()
def features(net, obs, bs=4096):
    out = []
    for a in range(0, len(obs), bs):
        o = torch.from_numpy(obs[a:a + bs].astype(np.float32)).to(DEV)
        out.append(net.trunk(o).float().cpu())
    return torch.cat(out)


def read(paths):
    obs, lab, job = [], [], []
    for p in paths.split(","):
        z = np.load(p)
        obs.append(z["obs"]); lab.append(z["label"]); job.append(z["job"] + (0 if not job else int(job[-1].max()) + 1))
    return np.concatenate(obs), np.concatenate(lab).astype(np.int64), np.concatenate(job)


def report(logits, lab, tag):
    p = torch.softmax(logits, 1).numpy()
    nll = float(F.cross_entropy(logits, torch.from_numpy(lab)).item())
    u = linear_util()
    mean_pred, mean_real = float((p @ u).mean()), float(u[lab].mean())
    pl, rl = float(p[:, 0].mean()), float((lab == 0).mean())
    # calibration of P(loss) in deciles
    order = np.argsort(p[:, 0])
    dec = np.array_split(order, 10)
    cal = " ".join(f"{p[i, 0].mean():.2f}/{(lab[i] == 0).mean():.2f}" for i in dec)
    print(f"{tag}: n {len(lab)}  NLL {nll:.3f}  P(loss) pred {pl:.3f} real {rl:.3f}  E[linear return] pred {mean_pred:.3f} real {mean_real:.3f}\n  P(loss) by decile pred/real: {cal}", flush=True)


def train(a):
    obs, lab, job = read(a.data)
    net = load(a.base)
    for p in net.parameters():
        p.requires_grad_(False)
    t0 = time.time()
    X = features(net, obs)
    print(f"{len(X)} states, features {tuple(X.shape)} in {time.time() - t0:.0f}s", flush=True)
    # held out by fight (whole fights, no leakage between states of one fight)
    rng = np.random.default_rng(0)
    jobs = np.unique(job)
    hold = set(rng.choice(jobs, max(1, len(jobs) // 10), replace=False).tolist())
    te = np.array([j in hold for j in job])
    Xtr, ytr, Xte, yte = X[~te].to(DEV), torch.from_numpy(lab[~te]).to(DEV), X[te], lab[te]
    head = DistHead(X.shape[1]).to(DEV)
    opt = torch.optim.AdamW(head.parameters(), lr=a.lr, weight_decay=1e-4)
    n = len(Xtr)
    for ep in range(a.epochs):
        perm = torch.randperm(n, device=DEV)
        tot = 0.0
        head.train()
        for s in range(0, n, a.bs):
            idx = perm[s:s + a.bs]
            loss = F.cross_entropy(head(Xtr[idx]), ytr[idx])
            opt.zero_grad(); loss.backward(); opt.step()
            tot += float(loss) * len(idx)
        if ep % 5 == 4 or ep == a.epochs - 1:
            head.eval()
            with torch.no_grad():
                lt = head(Xte.to(DEV)).cpu()
            print(f"epoch {ep + 1}: train NLL {tot / n:.3f}", flush=True)
            report(lt, yte, "  held-out")
    torch.save(dict(base=a.base, d2=X.shape[1], head=head.state_dict(), nb=NB), a.out)
    print("saved", a.out, flush=True)


def check(a):
    obs, lab, _ = read(a.data)
    net, head = load_head(a.head)
    with torch.no_grad():
        lt = head(features(net, obs).to(DEV)).cpu()
    report(lt, lab, "check")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["train", "check"])
    ap.add_argument("--data", required=True)
    ap.add_argument("--base", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models", "solver_b128.pt"))
    ap.add_argument("--head")
    ap.add_argument("--out", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models", "dist_b128.pt"))
    ap.add_argument("--epochs", type=int, default=30)
    ap.add_argument("--bs", type=int, default=2048)
    ap.add_argument("--lr", type=float, default=1e-3)
    a = ap.parse_args()
    (train if a.cmd == "train" else check)(a)


if __name__ == "__main__":
    main()
