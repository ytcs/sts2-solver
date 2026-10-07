#!/usr/bin/env python3
"""Search distillation (expert iteration, `docs/rl_redesign.md` M3 rounds): a large-budget search plays fights with recording on; every searched decision
becomes a training row whose policy target is the search's preference over the options it tried (softmax of their estimated returns at temperature
`--tau`), and whose outcome target is how that fight really ended. `ppo.py --distill FILE` mixes these rows into every update, so the network learns
lines its own sampled play would almost never find.

  STS2_DEVICE=cuda python rl/distill.py --ckpt models/solver_h128.pt --fights target/m3/frontier.json --out target/m3/distill_r1.npz [--M 5 --K 32] [--attempts 4]

Rows: obs [N, OBS] f32, mask [N, ACTIONS] u8, opts [N, M] i16 (dense action indices, -1 = not tried), tgt [N, M] f32 (sums to 1), cls [N] i16 (outcome class
of the fight, -1 aborted), won [N] u8.
"""
import argparse, json, os, sys, time
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads as H


def collect(net, fights, M, K, attempts, seed, tau, roots=1024):
    from fastsearch import FastSearch
    fs = FastSearch(net, M=M, K=K, record=True, roots=roots, amp=True)
    fs.warm()
    S = len(fights)
    js = np.tile(np.arange(S, dtype=np.uint32), attempts)
    jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
    res = fs.run(fights, js, jd)
    rows = dict(obs=[], mask=[], opts=[], tgt=[], cls=[], won=[])
    for idx, eng in fs._runs:
        for j_local, j in enumerate(idx):
            oc, hp_end = res[j, 1], res[j, 6]
            if oc not in (1, -1, 2):
                continue
            cls = int(H.end_class(oc == 1, hp_end))
            acts, searched, opts, p, q, legal = eng.moves(j_local)
            if not searched.any():
                continue
            obs, mask = sts2.replay(fights[js[j]], int(jd[j]), acts)
            for t in np.nonzero(searched)[0]:
                ok = legal[t, :M].astype(bool) & np.isfinite(q[t, :M])
                if ok.sum() < 2:
                    continue
                qq = np.where(ok, q[t, :M], -np.inf)
                e = np.exp((qq - qq[ok].max()) / tau)
                rows["obs"].append(obs[t].astype(np.float32))  # fp16 overflows (some observation fields exceed 65504)
                rows["mask"].append(mask[t])
                rows["opts"].append(np.where(ok, opts[t, :M], -1).astype(np.int16))
                rows["tgt"].append((e / e.sum()).astype(np.float32))
                rows["cls"].append(cls)
                rows["won"].append(oc == 1)
    return {k: np.array(v) for k, v in rows.items()}, res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--fights", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--M", type=int, default=5)
    ap.add_argument("--K", type=int, default=32)
    ap.add_argument("--attempts", type=int, default=4)
    ap.add_argument("--tau", type=float, default=0.02, help="temperature over the options' estimated returns (win +1 / loss -1, +0.5 x HP fraction)")
    ap.add_argument("--seed", type=int, default=77)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    from model import load
    fights = json.load(open(a.fights))
    if a.limit:
        fights = fights[:a.limit]
    t = time.time()
    rows, res = collect(load(a.ckpt), fights, a.M, a.K, a.attempts, a.seed, a.tau)
    np.savez_compressed(a.out, **rows)
    win = float(np.mean(res[:, 1] == 1))
    print(f"{len(fights)} fights x {a.attempts}: search win {win:.3f}; {len(rows['cls'])} decision rows ({rows['won'].mean() if len(rows['won']) else 0:.2f} from won fights) "
          f"-> {a.out} ({time.time() - t:.0f}s)", flush=True)


if __name__ == "__main__":
    main()
