#!/usr/bin/env python3
"""Certify generated training fights as winnable and grade their difficulty (M3 curriculum, `docs/rl_redesign.md`).

A random fight can be impossible to win (deck, HP and encounter drawn independently). Every fight goes through:
1. `sts2.provably_unwinnable` (a proof from the fight's numbers): dropped.
2. The network's own sampled play, `--attempts` episodes: any win proves the fight winnable (the simulator is the game's rules).
3. Fights the network never won get the solver's search (`--search-attempts`, 3 x 8): a win certifies them, otherwise they are dropped
   (not proven impossible, but out of reach of today's play: no learning signal).
Difficulty = 1 - the network's win rate (the search's when the network never won). The output keeps the certified fights only, each with
`meta.cert = {pwin, swin, hp_lost}`.

  STS2_DEVICE=cuda python tools/certify_fights.py target/m3/train_new.json target/m3/pool_full.json [--ckpt models/solver_h128.pt] [--attempts 8]
"""
import argparse, json, os, sys, time
import numpy as np

T0 = time.time()

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def policy_wins(net, scen, attempts, chunk=2048, seed=17, turn_cap=None):
    """[len(scen)] wins and mean HP lost (fraction of max, a loss = the start HP) over `attempts` sampled episodes each."""
    import torch, sts2, heads as H
    from model import DEV
    wins = np.zeros(len(scen))
    lost = np.zeros(len(scen))
    for c0 in range(0, len(scen), chunk):
        part = scen[c0:c0 + chunk]
        flat = [s for _ in range(attempts) for s in part]  # env i plays part[i % len(part)]
        env = sts2.VecEnv(len(flat), flat, seed=seed + c0, max_steps=600, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=turn_cap or H.TURN_CAP)
        obs, mask = env.reset()
        live = np.ones(len(flat), bool)
        while live.any():
            with torch.no_grad():
                lg, _ = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), value=False)
                act = torch.distributions.Categorical(logits=lg.float()).sample().cpu().numpy().astype(np.int32)
            obs, mask, r, d, info = env.step(act)
            fin = np.nonzero(d & live)[0]
            if len(fin):
                ei = env.episode_info()
                for i in fin:
                    k = c0 + i % len(part)
                    wins[k] += info["outcome"][i] == 1
                    lost[k] += ei["hp_lost"][i]
                    live[i] = False
        print(f"  policy {c0 + len(part)}/{len(scen)} ({time.time() - T0:.0f}s)", flush=True)
    return wins / attempts, lost / attempts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inp")
    ap.add_argument("out")
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--search-attempts", type=int, default=4, help="0 = no search pass: the never-won fights are written to OUT.unsolved.json (the next round's candidates)")
    ap.add_argument("--turn-cap", type=int, default=0, help="certify within this many player turns (a fight still running is a loss); 0 = the training cap (99)")
    ap.add_argument("--limit", type=int, default=0, help="first N fights only (timing)")
    a = ap.parse_args()
    import heads as H
    if a.turn_cap:
        H.TURN_CAP = a.turn_cap  # the env here and the search (fastsearch reads heads.TURN_CAP) both stop at it
    import sts2
    from model import load
    scen = json.load(open(a.inp))
    if a.limit:
        scen = scen[:a.limit]
    t = time.time()
    proved = np.array([bool(sts2.provably_unwinnable(s)) for s in scen])
    print(f"{len(scen)} fights, {proved.sum()} provably unwinnable ({time.time() - t:.0f}s)", flush=True)
    keep = [s for s, p in zip(scen, proved) if not p]
    net = load(a.ckpt)
    t = time.time()
    pw, pl = policy_wins(net, keep, a.attempts, turn_cap=a.turn_cap)
    print(f"policy: {time.time() - t:.0f}s; won at least once {np.mean(pw > 0):.3f}", flush=True)
    del net
    sw = np.full(len(keep), np.nan)
    rest = np.nonzero(pw == 0)[0]
    json.dump([keep[i] for i in rest], open(a.out.replace(".json", "") + ".unsolved.json", "w"))
    if len(rest) and a.search_attempts > 0:
        from solver import Solver
        S = Solver(a.ckpt, value_ckpts=None)
        t = time.time()
        r = S.solve([keep[i] for i in rest], attempts=a.search_attempts, seed=5)
        sw[rest] = [x["win"] for x in r]
        print(f"search on {len(rest)} never-won fights: {time.time() - t:.0f}s; certified {np.mean(sw[rest] > 0):.3f}", flush=True)
    out = []
    for i, s in enumerate(keep):
        if pw[i] > 0 or (sw[i] > 0):
            s = dict(s)
            # difficulty: 1 - the network's win rate; a fight only the search won is harder than any the network won (between 1 - 1/attempts and 1)
            diff = 1.0 - pw[i] if pw[i] > 0 else 1.0 - 0.5 / a.attempts
            s["meta"] = dict(s.get("meta", {}), cert=dict(pwin=float(pw[i]), swin=None if np.isnan(sw[i]) else float(sw[i]), hp_lost=float(pl[i]), diff=float(diff)))
            out.append(s)
    json.dump(out, open(a.out, "w"))
    d = np.array([s["meta"]["cert"]["diff"] for s in out])
    print(f"certified {len(out)} of {len(scen)} -> {a.out}; difficulty (1 - policy win) quartiles {np.quantile(d, [0.25, 0.5, 0.75]).round(3).tolist()}", flush=True)


if __name__ == "__main__":
    main()
