#!/usr/bin/env python3
"""Payoff test of potion prices in the search (`docs/rl_redesign.md` M1b, decision layer 3.2): fight A (hallway / elite of a real run, with its belt) is
followed by a boss B of the same act with the same deck, after a rest (+30 % max HP), carrying the potions A did not use. Score = P(win B), judged by the
batch solver over a grid of start HP for every subset of the belt (independent of the network's heads).

Arms (same job seeds):
  free     A searched with the linear return, potions free (price 0): the search throws whatever helps A
  none     A without its potions (the live search plans without them); every potion reaches B
  table    A searched with U(h) = P(win B | h, whole belt kept) from the outcome head (decision layer), potions free
  priced   as `table`, minus price_k = P(win B | belt) - P(win B | belt without k) (outcome head, mean over 30-100 % HP) for each potion A uses
           (or, at a leaf, the potion-use head's probability that it will)

  STS2_DEVICE=cuda python tools/payoff_potions.py --ckpt target/m1b/pot_frozen/ckpt.pt [--n 240] [--attempts 8] [--judge-attempts 16]
"""
import argparse, itertools, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tools"))
from payoff_worth import act_pool, net_pwin  # noqa: E402

GRID = (0.1, 0.25, 0.4, 0.55, 0.7, 0.85, 1.0)
HEAL = 0.3


def pots(s):
    return [p["id"] if isinstance(p, dict) else p for p in s.get("potions", [])]


def with_pots(s, ids):
    return dict(s, potions=[{"id": p, "slot": i} for i, p in enumerate(ids)])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True, help="an outcome-head + potion-use-head network")
    ap.add_argument("--n", type=int, default=240)
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--judge-attempts", type=int, default=16)
    ap.add_argument("--M", type=int, default=3)
    ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--seed", type=int, default=6)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "payoff_potions.json"))
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
        P = pots(s)
        if pool is None or not P or len(P) > 3:
            continue
        A = with_pots(dict(s, name=s["name"] + "_A"), P)
        B = dict(A, encounter=str(rng.choice(pool["boss"])), name=s["name"] + "_B")
        pairs.append((A, B, P))
    rng.shuffle(pairs)
    pairs = pairs[:a.n]
    print(f"{len(pairs)} pairs, potions per belt {np.mean([len(p) for _, _, p in pairs]):.2f}", flush=True)
    S = Solver(a.ckpt, M=a.M, K=a.K, value_ckpts=None)
    # tables from the heads: V(h) with the whole belt, and each potion's price
    t = time.time()
    flat, idx = [], []
    for i, (A, B, P) in enumerate(pairs):
        mx = int(B["max_hp"])
        for drop in [None] + list(range(len(P))):
            belt = [p for j, p in enumerate(P) if j != drop]
            for h in range(1, mx + 1):
                flat.append(with_pots(dict(B, hp=min(mx, h + round(HEAL * mx))), belt))
            idx.append((i, drop, mx))
    pw = net_pwin(S.net, flat)
    V, k = {}, 0
    for i, drop, mx in idx:
        V[(i, drop)] = np.maximum.accumulate(np.concatenate([[0.0], pw[k:k + mx]]))
        k += mx
    worth_table, worth_priced = [], []
    for i, (A, B, P) in enumerate(pairs):
        mx = int(B["max_hp"])
        u = np.concatenate([[0.0], np.interp(np.minimum(H.centers().numpy(), mx), np.arange(mx + 1), V[(i, None)])]).astype(np.float32)
        lo = int(0.3 * mx)
        price = [max(0.0, float((V[(i, None)][lo:] - V[(i, j)][lo:]).mean())) for j in range(len(P))]
        worth_table.append(dict(u=u))
        worth_priced.append(dict(u=u, price=price))
    print(f"head tables {time.time() - t:.0f}s; mean price {np.mean([np.mean(w['price']) for w in worth_priced]):.4f}", flush=True)
    # judge: the solver's win on B for every subset of the belt over a grid of HP (after the rest)
    t = time.time()
    js, jkey = [], []
    for i, (A, B, P) in enumerate(pairs):
        mx = int(B["max_hp"])
        for r in range(len(P) + 1):
            for sub in itertools.combinations(range(len(P)), r):
                for g in GRID:
                    js.append(with_pots(dict(B, hp=max(1, int(round(g * mx)))), [P[j] for j in sub]))
                    jkey.append((i, sub))
    jr = S.solve(js, attempts=a.judge_attempts, seed=a.seed + 100)
    judge = {}
    for n0 in range(0, len(js), len(GRID)):
        i, sub = jkey[n0]
        mx = int(pairs[i][1]["max_hp"])
        hs = [0] + [max(1, int(round(g * mx))) for g in GRID]
        judge[(i, sub)] = (np.array(hs, float), np.maximum.accumulate(np.array([0.0] + [jr[n0 + j]["win"] for j in range(len(GRID))])))
    print(f"judge {len(js)} x {a.judge_attempts} boss fights {time.time() - t:.0f}s", flush=True)
    # arms: A's endings and which belt slots were used
    As = [A for A, _, _ in pairs]
    arms = {}
    for name in ("free", "none", "table", "priced"):
        t = time.time()
        scen = [with_pots(A, []) for A in As] if name == "none" else As
        worth = {"table": worth_table, "priced": worth_priced}.get(name)
        res = S.fs.run(scen, np.tile(np.arange(len(scen), dtype=np.uint32), a.attempts),
                       np.uint64(a.seed) * np.uint64(1_000_003) + np.arange(len(scen) * a.attempts, dtype=np.uint64), worth=worth)
        arms[name] = res
        print(f"arm {name}: {time.time() - t:.0f}s", flush=True)
    n = len(pairs)
    score, used_n, winA, endA = {}, {}, {}, {}
    for name, res in arms.items():
        sc = np.zeros((n, a.attempts)); un = np.zeros((n, a.attempts)); wa = np.zeros((n, a.attempts)); ea = np.zeros((n, a.attempts))
        for j in range(len(res)):
            i, att = j % n, j // n
            A, B, P = pairs[i]
            mx = int(B["max_hp"])
            won, end, kept = res[j, 1] == 1, res[j, 6], int(res[j, 7])
            sub = tuple(range(len(P))) if name == "none" else tuple(k for k in range(len(P)) if (kept >> k) & 1)
            hs, ws = judge[(i, sub)]
            sc[i, att] = float(np.interp(min(mx, end + round(HEAL * mx)), hs, ws)) if won else 0.0
            un[i, att] = len(P) - len(sub)
            wa[i, att] = won
            ea[i, att] = end
        score[name], used_n[name], winA[name], endA[name] = sc.mean(1), un.mean(1), wa.mean(1), ea.mean(1)
    out = dict(n=n, attempts=a.attempts, judge_attempts=a.judge_attempts, M=a.M, K=a.K, heal=HEAL, arms={}, diffs={})
    for name in arms:
        out["arms"][name] = dict(p_boss=float(score[name].mean()), potions_used=float(used_n[name].mean()), winA=float(winA[name].mean()), endA=float(endA[name].mean()))
        print(f"{name:7s} P(win B) {score[name].mean():.4f}  potions used in A {used_n[name].mean():.2f}  A win {winA[name].mean():.4f}  A end HP {endA[name].mean():.2f}", flush=True)
    for x, y in (("priced", "free"), ("priced", "none"), ("priced", "table"), ("none", "free"), ("table", "free")):
        d = score[x] - score[y]
        se = float(d.std(ddof=1) / len(d) ** 0.5)
        out["diffs"][f"{x}-{y}"] = dict(diff=float(d.mean()), se=se)
        print(f"{x} - {y}: P(win B) {d.mean():+.4f} +- {se:.4f}", flush=True)
    json.dump(out, open(a.out, "w"), indent=1)


def sts2_pot():
    import sts2
    return sts2.names()["pot"]


if __name__ == "__main__":
    main()
