#!/usr/bin/env python3
import argparse, glob, json, os, sys, time

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import sts2  # noqa: E402
from target_noise import sample_states, search  # noqa: E402

SE32 = 0.078


def build(a):
    from fastsearch import FastSearch
    from model import load
    files = sorted(f for p in a.parts for f in glob.glob(p))
    states = sample_states(files, a.states, np.random.default_rng(a.seed))
    net = load(a.ckpt)
    out = {}
    for K, seed in ((a.K, 7777), (32, 1000)):
        fs = FastSearch(net, M=5, K=K, record=True, roots=a.roots, amp=True)
        fs.max_steps = 1
        fs.warm()
        t = time.time()
        out[K] = search(fs, states, seed)
        print(f"5x{K} on {len(states)} states in {time.time() - t:.0f}s", flush=True)
        del fs
        torch.cuda.empty_cache()
    (o, q), (o32, q32) = out[a.K], out[32]
    ok = (np.isfinite(q).sum(1) >= 2) & (o == o32).all(1)
    se = SE32 * np.sqrt(32 / a.K)
    srt = -np.sort(-np.where(np.isfinite(q), q, -np.inf), 1)
    sig = ok & (srt[:, 0] - srt[:, 1] > 2 * np.sqrt(2) * se)
    best = o[np.arange(len(o)), np.nanargmax(np.where(np.isfinite(q), q, -np.inf), 1)]
    pick32 = o32[np.arange(len(o)), np.nanargmax(np.where(np.isfinite(q32), q32, -np.inf), 1)]
    print(f"{ok.sum()} usable states, {sig.sum()} with a significant reference gap (> {2 * np.sqrt(2) * se:.3f})")
    print(f"live 5x32 search agrees with the reference: significant {np.mean(pick32[sig] == best[sig]):.3f}, all {np.mean(pick32[ok] == best[ok]):.3f}")
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    np.savez_compressed(a.out, states=np.array(json.dumps(states)), opts=o, q=q, ok=ok, sig=sig, best=best, pick32=pick32, K=a.K)
    print(f"-> {a.out}")


@torch.no_grad()
def evaluate(a):
    from model import load, DEV
    z = np.load(a.ref, allow_pickle=True)
    states = json.loads(str(z["states"]))
    ok, sig, best, opts, q = z["ok"], z["sig"], z["best"], z["opts"], z["q"]
    obs, mask = [], []
    for s in states:
        o, m = sts2.replay(s["scenario"], s["seed"], s["prefix"])
        obs.append(o[-1]); mask.append(m[-1])
    obs, mask = np.stack(obs), np.stack(mask)
    print(f"reference: {ok.sum()} usable states, {sig.sum()} significant; live 5x32 agreement: significant {np.mean(z['pick32'][sig] == best[sig]):.3f}, "
          f"all {np.mean(z['pick32'][ok] == best[ok]):.3f}")
    for ck in a.ckpts:
        net = load(ck).eval()
        acts = []
        for b in range(0, len(obs), 2048):
            lg = net(torch.from_numpy(obs[b:b + 2048]).to(DEV), torch.from_numpy(mask[b:b + 2048].astype(np.int64)).to(DEV), value=False)[0]
            acts.append(lg.argmax(1).cpu().numpy())
        g = np.concatenate(acts)
        on = (opts == g[:, None])
        inopt = on.any(1)
        reg = np.nanmax(np.where(np.isfinite(q), q, -np.inf), 1) - np.nansum(np.where(on, np.nan_to_num(q), 0.0), 1)
        m = ok & inopt
        print(f"{os.path.basename(ck):26s} agree: significant {np.mean(g[sig] == best[sig]):.3f}  all {np.mean(g[ok] == best[ok]):.3f}  | "
              f"regret (greedy among the options) {reg[m].mean():.4f}  off the options {1 - inopt[ok].mean():.3f}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build"); b.add_argument("parts", nargs="+"); b.add_argument("--ckpt", required=True); b.add_argument("--out", required=True)
    b.add_argument("--states", type=int, default=4000); b.add_argument("--K", type=int, default=256); b.add_argument("--roots", type=int, default=1024)
    b.add_argument("--seed", type=int, default=0)
    e = sub.add_parser("eval"); e.add_argument("ref"); e.add_argument("ckpts", nargs="+")
    a = ap.parse_args()
    build(a) if a.cmd == "build" else evaluate(a)


if __name__ == "__main__":
    main()
