#!/usr/bin/env python3
"""Payoff test of the decision layer's HP worth (`docs/rl_redesign.md` M1b): does a search that maximises the run's worth of each ending beat today's linear
return where it counts, P(win the act boss afterwards)?

Real-run fights (corpus, potions removed): fight A (hallway / elite) is followed by a boss B of the same act with the same deck, either at once ("no rest")
or after a rest (+30 % max HP). The worth of ending A with h HP is V(h) = P(win B | start at h) (after the rest: V(min(h + heal, max))), death 0.
- The search's table: V from the outcome head of the network at B's first state (one forward per HP value).
- The judge (independent of the network's head): the batch solver's win rate on B at a grid of start HP, interpolated.
Arms: A searched with the linear return vs with the table; same job seeds (common random numbers); score = mean judged V of A's endings, paired per fight.

  STS2_DEVICE=cuda python tools/payoff_worth.py [--n 240] [--attempts 8] [--judge-attempts 24] [--M 3 --K 8] [--out evals/payoff_worth.json]
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, ROOT)
GRID = (0.1, 0.25, 0.4, 0.55, 0.7, 0.85, 1.0)
HEAL = 0.3


def act_pool(enc):
    from agent.pools import POOLS
    for name, d in POOLS.items():
        if any(enc in d[k] for k in ("weak", "regular", "elite")):
            return name, d
    return None, None


def net_pwin(net, scens):
    """P(win) from the outcome head at each scenario's first state."""
    import torch, sts2, heads as H
    from model import DEV
    out = []
    for a in range(0, len(scens), 2048):
        part = scens[a:a + 2048]
        env = sts2.VecEnv(len(part), part, seed=11, max_steps=600, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=H.TURN_CAP)
        obs, mask = env.reset()
        with torch.no_grad():
            _, _, ol = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), outcome=True)
            out.append(1.0 - torch.softmax(ol.float(), 1)[:, 0].cpu().numpy())
    return np.concatenate(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--n", type=int, default=240)
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--judge-attempts", type=int, default=24)
    ap.add_argument("--M", type=int, default=3)
    ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--seed", type=int, default=5)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "payoff_worth.json"))
    a = ap.parse_args()
    from solver import Solver
    import heads as H
    rng = np.random.default_rng(a.seed)
    fights = []
    for f in ("fights_holdout.json", "fights_train.json"):
        fights += json.load(open(os.path.join(ROOT, "data", "corpus", f)))
    pairs = []
    for s in fights:
        name, pool = act_pool(s["encounter"])
        if pool is None:
            continue
        A = dict(s, potions=[], name=s["name"] + "_A")
        B = dict(A, encounter=str(rng.choice(pool["boss"])), name=s["name"] + "_B")
        pairs.append((A, B))
    rng.shuffle(pairs)
    pairs = pairs[:a.n]
    print(f"{len(pairs)} pairs", flush=True)
    S = Solver(a.ckpt, M=a.M, K=a.K, value_ckpts=None)
    t = time.time()
    # the search's tables: V_net(h) for h = 0..max at B's first state
    curves = []
    flat = []
    for A, B in pairs:
        mx = int(B["max_hp"])
        flat += [dict(B, hp=h) for h in range(1, mx + 1)]
    pw = net_pwin(S.net, flat)
    k = 0
    for A, B in pairs:
        mx = int(B["max_hp"])
        curves.append(np.maximum.accumulate(np.concatenate([[0.0], pw[k:k + mx]])))  # monotone in HP
        k += mx
    print(f"net curves {time.time() - t:.0f}s", flush=True)
    # the judge: the solver's win rate on B over a grid of start HP
    t = time.time()
    js = [dict(B, hp=max(1, int(round(g * B["max_hp"])))) for _, B in pairs for g in GRID]
    jr = S.solve(js, attempts=a.judge_attempts, seed=a.seed + 100)
    judge = []
    for i, (_, B) in enumerate(pairs):
        hs = [0] + [max(1, int(round(g * B["max_hp"]))) for g in GRID]
        ws = [0.0] + [jr[i * len(GRID) + j]["win"] for j in range(len(GRID))]
        judge.append((np.array(hs, float), np.maximum.accumulate(np.array(ws))))
    print(f"judge {len(js)} x {a.judge_attempts} boss fights {time.time() - t:.0f}s", flush=True)

    def table(curve, mx, rest):
        hp = H.centers().numpy()
        h = np.minimum(hp + (round(HEAL * mx) if rest else 0), mx)
        u = np.interp(h, np.arange(len(curve)), curve)
        return dict(u=np.concatenate([[0.0], u]).astype(np.float32))

    def judged(i, end, mx, rest):
        hs, ws = judge[i]
        h = np.minimum(np.asarray(end, float) + (round(HEAL * mx) if rest else 0), mx)
        return np.where(np.asarray(end) > 0, np.interp(h, hs, ws), 0.0)

    As = [A for A, _ in pairs]
    arms = {"linear": None,
            "table_norest": [table(c, int(B["max_hp"]), False) for c, (_, B) in zip(curves, pairs)],
            "table_rest": [table(c, int(B["max_hp"]), True) for c, (_, B) in zip(curves, pairs)]}
    ends = {}
    for name, w in arms.items():
        t = time.time()
        r = S.solve(As, attempts=a.attempts, seed=a.seed, worth=w)
        ends[name] = [np.array([e if e is not None else np.nan for e in x["ends_abs"]], float) for x in r]
        print(f"arm {name}: {time.time() - t:.0f}s", flush=True)
    out = {}
    for rest in (False, True):
        tag = "rest" if rest else "norest"
        base = np.array([np.nanmean(judged(i, ends["linear"][i], int(B["max_hp"]), rest)) for i, (_, B) in enumerate(pairs)])
        cand = np.array([np.nanmean(judged(i, ends[f"table_{tag}"][i], int(B["max_hp"]), rest)) for i, (_, B) in enumerate(pairs)])
        winA0 = np.array([np.nanmean(e > 0) for e in ends["linear"]])
        winA1 = np.array([np.nanmean(e > 0) for e in ends[f"table_{tag}"]])
        hp0 = np.array([np.nanmean(e) for e in ends["linear"]])
        hp1 = np.array([np.nanmean(e) for e in ends[f"table_{tag}"]])
        d = cand - base
        se = d.std(ddof=1) / len(d) ** 0.5
        out[tag] = dict(p_boss_linear=float(base.mean()), p_boss_table=float(cand.mean()), diff=float(d.mean()), se=float(se),
                        winA_diff=float((winA1 - winA0).mean()), winA_se=float((winA1 - winA0).std(ddof=1) / len(d) ** 0.5),
                        endhp_diff=float((hp1 - hp0).mean()), endhp_se=float((hp1 - hp0).std(ddof=1) / len(d) ** 0.5))
        print(f"[{tag}] P(win boss after A): linear {base.mean():.4f}  table {cand.mean():.4f}  diff {d.mean():+.4f} +- {se:.4f}   "
              f"A win {out[tag]['winA_diff']:+.4f} +- {out[tag]['winA_se']:.4f}  A end HP {out[tag]['endhp_diff']:+.2f} +- {out[tag]['endhp_se']:.2f}", flush=True)
    json.dump(dict(results=out, n=len(pairs), attempts=a.attempts, judge_attempts=a.judge_attempts, M=a.M, K=a.K, grid=GRID, heal=HEAL), open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
