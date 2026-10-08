#!/usr/bin/env python3
import argparse, glob, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402


def sample_states(files, n, rng):
    parts = [np.load(f, allow_pickle=True) for f in files]
    sizes = np.array([len(z["d_fight"]) for z in parts])
    picks = rng.choice(sizes.sum(), n, replace=False)
    starts = np.concatenate([[0], np.cumsum(sizes)])
    out = []
    for g in np.sort(picks):
        pi = int(np.searchsorted(starts, g, side="right") - 1)
        z, d = parts[pi], int(g - starts[pi])
        scen = json.loads(str(z["scenarios"]))
        f = int(z["d_fight"][d])
        acts = z["acts"][z["f_off"][f]:z["f_off"][f + 1]]
        out.append(dict(scenario=scen[z["f_scen"][f]], seed=int(z["f_seed"][f]), prefix=[int(x) for x in acts[:int(z["d_step"][d])]]))
    return out


def search(fs, states, seed):
    sims = []
    for s in states:
        sim = sts2.Sim(json.dumps(s["scenario"]), s["seed"])
        for x in s["prefix"]:
            sim.step(x)
        sims.append(sim)
    S = len(states)
    fs.run([s["scenario"] for s in states], np.arange(S, dtype=np.uint32), np.uint64(seed) * np.uint64(1_000_003) + np.arange(S, dtype=np.uint64), starts=sims)
    W = fs.M
    opts, q = np.full((S, W), -1, np.int64), np.full((S, W), np.nan)
    for idx, eng in fs._runs:
        for jl, j in enumerate(idx):
            acts, searched, o, _p, qq, legal = eng.moves(jl)
            if len(acts) and searched[0]:
                ok = legal[0, :W].astype(bool) & np.isfinite(qq[0, :W])
                opts[j] = np.where(ok, o[0, :W], -1)
                q[j] = np.where(ok, qq[0, :W], np.nan)
    fs._runs = []
    return opts, q


def minmax(q):
    lo, hi = np.nanmin(q, 1, keepdims=True), np.nanmax(q, 1, keepdims=True)
    return (q - lo) / np.maximum(hi - lo, 1e-6)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("parts", nargs="+")
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--states", type=int, default=2000)
    ap.add_argument("--M", type=int, default=5)
    ap.add_argument("--K", type=int, default=32)
    ap.add_argument("--repeats", type=int, default=2)
    ap.add_argument("--ref-K", type=int, default=0, help="reference search: same M, this many futures per option")
    ap.add_argument("--roots", type=int, default=1024)
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    from fastsearch import FastSearch
    from model import load
    files = sorted(f for p in a.parts for f in glob.glob(p))
    rng = np.random.default_rng(a.seed)
    states = sample_states(files, a.states, rng)
    net = load(a.ckpt)

    def make(K):
        fs = FastSearch(net, M=a.M, K=K, record=True, roots=a.roots, amp=True)
        fs.max_steps = 1
        fs.warm()
        return fs
    fs = make(a.K)
    runs = []
    for r in range(a.repeats):
        t = time.time()
        runs.append(search(fs, states, 1000 + r))
        print(f"repeat {r}: {a.M}x{a.K} on {len(states)} states in {time.time() - t:.0f}s", flush=True)
    ref = None
    if a.ref_K:
        fs = make(a.ref_K)
        t = time.time()
        ref = search(fs, states, 7777)
        print(f"reference {a.M}x{a.ref_K} in {time.time() - t:.0f}s", flush=True)
    (o0, q0), (o1, q1) = runs[0], runs[1]
    ok = (np.isfinite(q0).sum(1) >= 2) & (o0 == o1).all(1) & (np.isfinite(q0) == np.isfinite(q1)).all(1)
    if ref is not None:
        ok &= (ref[0] == o0).all(1) & (np.isfinite(ref[1]) == np.isfinite(q0)).all(1)
    q0, q1 = q0[ok], q1[ok]
    print(f"\n{ok.sum()} of {len(states)} states searched with the same options in every run")
    b0, b1 = np.nanargmax(q0, 1), np.nanargmax(q1, 1)
    print(f"best option repeats across seeds: {np.mean(b0 == b1):.3f}")
    d = q0 - q1
    noise = np.nanstd(d) / np.sqrt(2)
    srt = -np.sort(-np.where(np.isfinite(q0), (q0 + q1) / 2, -np.inf), 1)
    gap = srt[:, 0] - srt[:, 1]
    print(f"se of one option's estimate: {noise:.4f} (linear return units; win = +1 + 0.5 x HP fraction, loss = -1)")
    print(f"gap best - second (mean of two runs): median {np.median(gap):.4f}; share of states with gap > 2 x se x sqrt(2): {np.mean(gap > 2 * noise * np.sqrt(2) / np.sqrt(2)):.3f}")
    n0, n1 = minmax(q0), minmax(q1)
    m = np.isfinite(n0) & np.isfinite(n1)
    print(f"min-max normalised estimates (anchored target), correlation across seeds: {np.corrcoef(n0[m], n1[m])[0, 1]:.3f}")
    if ref is not None:
        qr = ref[1][ok]
        br = np.nanargmax(qr, 1)
        print(f"best option = reference ({a.M}x{a.ref_K}) best: run 0 {np.mean(b0 == br):.3f}, run 1 {np.mean(b1 == br):.3f}")
        reg = np.nanmax(qr, 1) - qr[np.arange(len(qr)), b0]
        print(f"regret of the run's pick under the reference: mean {reg.mean():.4f}, share > 0.05: {np.mean(reg > 0.05):.3f}")


if __name__ == "__main__":
    main()
