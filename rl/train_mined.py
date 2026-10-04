#!/usr/bin/env python3
"""Fine-tune the network on confirmed search-vs-policy disagreements (rl/mine.py), without forgetting.

  .venv/bin/python rl/train_mined.py --ckpt models/solver_base.pt --mined target/exit/mine1.npz [more.npz] --states target/train/train.json --out models/solver_v2.pt

Loss = cross-entropy of the policy towards the action the search preferred (confirmed with many futures) on the mined states
       + `anchor` x KL(original policy || new policy) and value MSE to the original value on ordinary states drawn from the network's own play
         (so everything the network already does right stays as it was).
The mined states are the ones where the network is demonstrably worse than one step of search; the anchors keep the rest of the behaviour.
"""
import argparse, json, os, sys
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import load


def anchors(net, path, n, seed):
    scen = json.load(open(path))
    env = sts2.VecEnv(2000, scen, seed=seed, max_steps=300)
    obs, mask = env.reset()
    rng = np.random.default_rng(seed)
    O, M = [], []
    while sum(len(o) for o in O) < n:
        with torch.no_grad():
            lg, _ = net(torch.from_numpy(obs.copy()), torch.from_numpy(mask.astype(np.int64)))
        sel = rng.random(len(obs)) < 0.25
        O.append(np.clip(obs[sel], -60000, 60000).copy()); M.append(mask[sel].copy())
        a = torch.multinomial(torch.softmax(lg, 1), 1).squeeze(1).numpy().astype(np.int32)
        obs, mask, *_ = env.step(a)
    return np.concatenate(O)[:n], np.concatenate(M)[:n]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True); ap.add_argument("--mined", nargs="+", required=True); ap.add_argument("--states", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--anchor", type=float, default=1.0); ap.add_argument("--anchors", type=int, default=20000)
    ap.add_argument("--epochs", type=int, default=6); ap.add_argument("--lr", type=float, default=3e-5); ap.add_argument("--mb", type=int, default=256)
    ap.add_argument("--smooth", type=float, default=0.1); ap.add_argument("--threads", type=int, default=8); ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    torch.manual_seed(a.seed)
    ck = torch.load(a.ckpt)
    net, ref = load(a.ckpt), load(a.ckpt)
    D = {k: np.concatenate([np.load(f)[k] for f in a.mined]) for k in ("obs", "mask", "policy", "search")}
    n = len(D["search"])
    print("mined states:", n, flush=True)
    mo = torch.nan_to_num(torch.from_numpy(D["obs"].astype(np.float32)), posinf=60000.0, neginf=-60000.0)
    mm = torch.from_numpy(D["mask"].astype(np.int64)); ms = torch.from_numpy(D["search"].astype(np.int64)); mp = torch.from_numpy(D["policy"].astype(np.int64))
    ao, am = anchors(ref, a.states, a.anchors, a.seed)
    ao, am = torch.from_numpy(ao), torch.from_numpy(am.astype(np.int64))
    with torch.no_grad():
        ref_lg, ref_v = [], []
        for i in range(0, len(ao), 2048):
            lg, v = ref(ao[i:i + 2048], am[i:i + 2048]); ref_lg.append(lg); ref_v.append(v)
        ref_lg, ref_v = torch.cat(ref_lg), torch.cat(ref_v)
        ref_p = torch.softmax(ref_lg, 1)
    hold = max(1, n // 10)
    perm0 = torch.randperm(n)
    tr, te = perm0[hold:], perm0[:hold]
    opt = torch.optim.Adam(net.parameters(), lr=a.lr)
    def hit(ix):
        with torch.no_grad():
            lg, _ = net(mo[ix], mm[ix])
        return (lg.argmax(1) == ms[ix]).float().mean().item(), (lg.argmax(1) == mp[ix]).float().mean().item()
    print(f"before: holdout picks the search's action {hit(te)[0]:.3f}, the old policy action {hit(te)[1]:.3f}", flush=True)
    net.train()
    for ep in range(a.epochs):
        perm = tr[torch.randperm(len(tr))]
        for s0 in range(0, len(perm), a.mb):
            ix = perm[s0:s0 + a.mb]
            lg, _ = net(mo[ix], mm[ix])
            logp = F.log_softmax(lg, 1)
            tgt = torch.full_like(logp, 0.0)
            legal = mm[ix] > 0
            tgt = (legal.float() * a.smooth / legal.sum(1, keepdim=True).clamp(min=1))
            tgt.scatter_(1, ms[ix][:, None], 1.0 - a.smooth + tgt.gather(1, ms[ix][:, None]))
            ce = -(tgt * logp.clamp(min=-30)).sum(1).mean()
            j = torch.randint(0, len(ao), (a.mb * 2,))
            alg, av = net(ao[j], am[j])
            kl = (ref_p[j] * (torch.log(ref_p[j].clamp(min=1e-8)) - F.log_softmax(alg, 1).clamp(min=-30))).sum(1).mean()
            vl = F.mse_loss(av, ref_v[j])
            loss = ce + a.anchor * kl + 0.5 * vl
            opt.zero_grad(); loss.backward(); torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0); opt.step()
        print(f"epoch {ep}: ce {ce.item():.3f} kl-to-old {kl.item():.4f} | holdout picks search's action {hit(te)[0]:.3f}, old policy's {hit(te)[1]:.3f}", flush=True)
    torch.save({"net": net.state_dict(), "args": ck.get("args", {}), "it": ck.get("it", 0), "steps": ck.get("steps", 0)}, a.out)
    print("saved", a.out)


if __name__ == "__main__":
    main()
