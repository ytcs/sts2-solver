#!/usr/bin/env python3
"""Throughput of expert-iteration collection (`rl/exit.py collect`) on a fixed fight set, with a digest of everything it records.

  STS2_DEVICE=cuda python tools/perf_collect.py [--fights target/exit/pool_r4s.json --n 512 --roots 256 --M 5 --K 32] [--profile-gpu]

Plays the first `--n` fights (x `--attempts`) with the collection's search (top-M root, record, bf16 graphs; job seeds as `collect` makes them),
then extracts the moves of every fight the way `collect` does. Prints fights/s of the search, the extraction time, FastSearch's timers, the
engine's cycle counters, the GPU time per graph with `--profile-gpu`, and a sha256 over the results and every recorded move: two builds that
play the same fights the same way print the same digest.
"""
import argparse, hashlib, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fights", default=os.path.join(ROOT, "data", "bench", "mix.json"))
    ap.add_argument("--n", type=int, default=512)
    ap.add_argument("--skip", type=int, default=0, help="fights skipped at the start of the file")
    ap.add_argument("--attempts", type=int, default=1)
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--roots", type=int, default=256)
    ap.add_argument("--M", type=int, default=5)
    ap.add_argument("--K", type=int, default=32)
    ap.add_argument("--threads", type=int, default=None)
    ap.add_argument("--seed", type=int, default=101)
    ap.add_argument("--profile-gpu", action="store_true")
    ap.add_argument("--repeat", type=int, default=1, help="runs of the same jobs (the first one also checks the digest of the others)")
    a = ap.parse_args()
    from fastsearch import FastSearch
    from model import load
    import heads as H
    data = json.load(open(a.fights))
    scen = [d["scenario"] if "scenario" in d and "deck" not in d else d for d in (data["scenarios"] if isinstance(data, dict) else data)]
    scen = scen[a.skip:a.skip + a.n]
    fs = FastSearch(load(a.ckpt), M=a.M, K=a.K, record=True, roots=a.roots, amp=True, threads=a.threads, profile_gpu=a.profile_gpu)
    t = time.time()
    fs.warm()
    print(f"{len(scen)} scenarios x {a.attempts}, roots {a.roots}, {a.M}x{a.K}, threads {fs.threads}; warm {time.time() - t:.1f}s", flush=True)
    jobs = [(i, att) for att in range(a.attempts) for i in range(len(scen))]
    js = np.array([i for i, _ in jobs], dtype=np.uint32)
    jd = np.array([np.uint64(a.seed) * np.uint64(1_000_003) + np.uint64(att * len(scen) + i) for i, att in jobs], dtype=np.uint64)
    digests = []
    for rep in range(a.repeat):
        fs.timers.clear()
        t0 = time.time()
        res = fs.run(scen, js, jd)
        dt = time.time() - t0
        # the extraction `collect` does per fight
        t1 = time.time()
        h = hashlib.sha256(res.tobytes())
        n_dec = 0
        for idx, eng in fs._runs:
            for jl, j in enumerate(idx):
                oc, hp_end = res[j, 1], res[j, 6]
                if oc not in (1, -1, 2):
                    continue
                acts, searched, opts, _p, q, legal = eng.moves(jl)
                for x in (acts, searched, opts, _p, q, legal):
                    h.update(np.ascontiguousarray(x).tobytes())
                H.end_class(oc == 1, hp_end)
                for tt in np.nonzero(searched)[0]:
                    ok = legal[tt, :a.M].astype(bool) & np.isfinite(q[tt, :a.M])
                    if ok.sum() < 2:
                        continue
                    n_dec += 1
                    np.where(ok, opts[tt, :a.M], -1).astype(np.int16), np.where(ok, q[tt, :a.M], np.nan).astype(np.float32)
        t_ext = time.time() - t1
        fs._runs = []
        digests.append(h.hexdigest()[:16])
        st = fs.stats
        print(f"run {rep}: {len(js)} fights in {dt:.1f}s = {len(js) / dt:.2f} fights/s; extraction {t_ext:.2f}s; win {np.mean(res[:, 1] == 1):.4f}; "
              f"{n_dec} decisions; digest {digests[-1]}", flush=True)
        for k, v in sorted(fs.timers.items(), key=lambda kv: -kv[1]):
            if k != "warm":
                print(f"    {k:12s} {v:7.2f}s {100 * v / dt:5.1f}%")
        cy = {k: st[k] for k in ("cy_step", "cy_obs", "cy_fork", "cy_legal", "cy_main", "cy_endturn")}
        tot = sum(v for k, v in cy.items() if k != "cy_endturn")
        print("    engine cycles: " + ", ".join(f"{k[3:]} {100 * v / max(tot, 1):.0f}%" for k, v in cy.items()) +
              f"; sim steps {st['sim_steps']:,}, policy rows {st['policy_rows']:,}, value rows {st['value_rows']:,}, forks {st['forks']:,}, "
              f"cycles {st['cycles']}, rows/cycle {st['rows_per_cycle']:.0f}")
        if a.profile_gpu:
            tot = 0
            for k, (ms, n, rows, padded) in sorted(fs.gpu_ms().items()):
                tot += ms
                print(f"    GPU {k:12s} {ms / 1000:6.2f}s  {n:6d} replays  {rows / max(n, 1):6.0f} rows/replay  {1000 * ms / max(rows, 1):.2f} us/row  "
                      f"padding {100 * (padded / max(rows, 1) - 1):.0f}%")
            print(f"    GPU replays {tot / 1000:.2f}s of {dt:.2f}s")
            fs._ev.clear()
    if len(set(digests)) > 1:
        print("DIGESTS DIFFER between runs:", digests)


if __name__ == "__main__":
    main()
