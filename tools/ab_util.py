#!/usr/bin/env python3
"""A/B of the search objective on fights where the ending HP matters: the linear return vs the continuation utility (end-HP distribution head).

  STS2_DEVICE=cuda python tools/ab_util.py --cases data/ab_lowhp.json --n 64 [--budget 1.0] --out target/dist/ab.json

Each case: `fight` (start scenario), `next` (the next dangerous fight) and `heal` (HP regained before it). V(h) = P(win `next` | start it with min(max, h + heal)),
measured once on an HP grid with the batch solver. The utility of the fight's endings is 1 + 0.5 V(h)/V(max) for a win and -1 for a loss (agent.routes.
continuation_util uses the same mapping with the route DP's V). Both arms play the same seeds with `play_fight`; the score that decides is E[V(end HP)]
(a loss counts 0), reported with the win rate and the mean HP lost.
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
from agent.engine import Engine, play_fight  # noqa: E402


def v_curve(eng, nxt, heal, maxhp, attempts):
    grid = sorted({max(1, int(round(x))) for x in np.linspace(1, maxhp, 12)})
    scen = [dict(nxt, hp=min(nxt["max_hp"], g + heal), name=f"v{g}") for g in grid]
    res = eng.solve(scen, attempts=attempts)
    w = np.array([r["win"] for r in res])
    hp = np.arange(maxhp + 1)
    V = np.interp(hp, grid, w)
    V[0] = 0.0
    return V


def util_of(V, maxhp, hp_bonus=0.5):
    U = np.clip(V / max(V[maxhp], 1e-6), 0, 1)
    return [-1.0] + [1.0 + hp_bonus * float(U[min(maxhp, max(1, int(round((b + 0.5) / 20 * maxhp))))]) for b in range(20)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cases", required=True)
    ap.add_argument("--n", type=int, default=64)
    ap.add_argument("--budget", type=float, default=1.0)
    ap.add_argument("--tol", type=float, default=0.5)
    ap.add_argument("--v-attempts", type=int, default=64)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    eng = Engine()
    assert eng.dist_head is not None, "models/dist_b128.pt missing: train it first (rl/dist.py)"
    out = []
    for c in json.load(open(a.cases)):
        f = c["fight"]
        maxhp = f["max_hp"]
        t0 = time.time()
        V = v_curve(eng, c["next"], c["heal"], maxhp, a.v_attempts)
        util = util_of(V, maxhp)
        row = dict(name=c["name"], V=[round(float(V[int(round(x * maxhp))]), 3) for x in (0.1, 0.25, 0.5, 0.75, 1.0)])
        for arm, u in (("linear", None), ("util", util)):
            ends, wins, lost = [], 0, []
            for k in range(a.n):
                o, hl, _ = play_fight(eng, f, 31000 + k, a.budget, tol_hp=a.tol, util=u)
                hp_end = f["hp"] - hl if o == 1 else 0
                wins += o == 1
                lost.append(hl if o == 1 else f["hp"])
                ends.append(float(V[max(0, min(maxhp, hp_end))]) if o == 1 else 0.0)
            e = np.array(ends)
            row[arm] = dict(EV=round(float(e.mean()), 4), EV_se=round(float(e.std(ddof=1) / len(e) ** 0.5), 4), win=round(wins / a.n, 3), hp_lost=round(float(np.mean(lost)), 1))
        row["secs"] = round(time.time() - t0)
        out.append(row)
        print(json.dumps(row), flush=True)
    if a.out:
        json.dump(out, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
