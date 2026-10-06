#!/usr/bin/env python3
"""Paired, fight-clustered reading of a `tools/bench_search.py` output: every configuration against a baseline on the same states.

  python tools/bench_paired.py evals/bench_search.jsonl [--base live] [--top 5]

Per configuration: mean regret difference vs the baseline (negative = better) with a bootstrap over FIGHTS (decisions of one fight are correlated),
win / tie / loss counts per state, and the states with the largest regret for the baseline and the configuration, with the referee's sample size and
standard error on the best and the picked action (a gap resting on few repetitions is noise).
"""
import argparse, json, collections
import numpy as np


def regret(rec, n):
    best_k = max((k for k, r in rec["ref"].items() if r["n"]), key=lambda k: rec["ref"][k]["v"])
    pick = rec["ref"].get(rec["picks"].get(n))
    if pick is None or not pick["n"]:
        return None, best_k
    return rec["ref"][best_k]["v"] - pick["v"], best_k


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("path")
    ap.add_argument("--base", default="live")
    ap.add_argument("--top", type=int, default=5)
    ap.add_argument("--kinds", default="all,boss,elite,hallway")
    a = ap.parse_args()
    recs = [json.loads(l) for l in open(a.path)]
    names = list(dict.fromkeys(n for r in recs for n in r["picks"]))
    rng = np.random.default_rng(0)
    for kind in a.kinds.split(","):
        rs = [r for r in recs if kind == "all" or r["kind"] == kind]
        print(f"\n== {kind} ({len(rs)} states, {len({r['file'] for r in rs})} fights): difference vs {a.base} (negative = less regret), fight-clustered bootstrap")
        for n in names:
            if n == a.base:
                continue
            rows = []
            for r in rs:
                x, _ = regret(r, n)
                b, _ = regret(r, a.base)
                if x is not None and b is not None:
                    rows.append((r["file"], x - b))
            if not rows:
                continue
            by = collections.defaultdict(list)
            for f, d in rows:
                by[f].append(d)
            fights = list(by)
            boots = []
            for _ in range(1000):
                pick = rng.choice(len(fights), len(fights))
                vals = [d for i in pick for d in by[fights[i]]]
                boots.append(np.mean(vals))
            d = np.array([d for _, d in rows])
            w, t, l = int((d < -1e-9).sum()), int((np.abs(d) <= 1e-9).sum()), int((d > 1e-9).sum())
            lo, hi = np.percentile(boots, [2.5, 97.5])
            print(f"  {n:13s} mean {d.mean():+.4f}  95% [{lo:+.4f}, {hi:+.4f}]  better/tie/worse {w}/{t}/{l}")
    for n in [a.base] + [m for m in names if m != a.base][:0]:
        pass
    for n in (a.base, "leaf2", "leafend"):
        if n not in names:
            continue
        scored = sorted(((regret(r, n)[0] or 0, r) for r in recs), key=lambda x: -x[0])[:a.top]
        print(f"\nlargest regrets of {n}:")
        for x, r in scored:
            _, best = regret(r, n)
            pk = r["picks"][n]
            rb, rp = r["ref"][best], r["ref"].get(pk, {})
            print(f"  {x:.4f} {r['kind']:7s} {r['file']} step {r['step']}: best `{best}` v {rb['v']:.3f} se {rb['se']} n {rb['n']} | picked `{pk}` v {rp.get('v', float('nan')):.3f} se {rp.get('se')} n {rp.get('n')}")


if __name__ == "__main__":
    main()
