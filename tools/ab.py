#!/usr/bin/env python3
"""Paired comparison of two baseline arms (evals/baseline/*.jsonl) on their common seeds: death floor, act-1 / act-2 boss passed,
deck strength vs each act's bosses (mean over that act's bosses), deck size; mean difference B - A with its paired se.
Usage: tools/ab.py A.jsonl B.jsonl [more B part files...]"""
import json
import sys

import numpy as np


def rows(paths):
    out = {}
    for p in paths:
        for l in open(p, encoding="utf-8"):
            r = json.loads(l)
            if r.get("result") in ("win", "loss"):
                out.setdefault(r["seed"], r)
    return out


def metrics(r):
    ds = r.get("deck_strength") or {}
    m = dict(floor=float(r["floor"] or 0), win=float(r["result"] == "win"), act1=float(int(r.get("act") or 1) >= 2 or r["result"] == "win"),
             act2=float(int(r.get("act") or 1) >= 3 or r["result"] == "win"), deck=float(r.get("deck_size") or 0))
    for k in ("A1 boss deck", "A2 boss deck", "A3 boss deck", "final deck"):
        if isinstance(ds.get(k), dict) and ds[k]:
            m[k] = float(np.mean(list(ds[k].values())))
    return m


def main(argv):
    a, b = rows(argv[:1]), rows(argv[1:])
    common = sorted(set(a) & set(b))
    print(f"{len(common)} common seeds (A {len(a)}, B {len(b)}): {argv[0]} vs {', '.join(argv[1:])}")
    ma, mb = {s: metrics(a[s]) for s in common}, {s: metrics(b[s]) for s in common}
    for k in ("win", "floor", "act1", "act2", "deck", "A1 boss deck", "A2 boss deck", "A3 boss deck", "final deck"):
        both = [s for s in common if k in ma[s] and k in mb[s]]
        if not both:
            continue
        x, y = np.array([ma[s][k] for s in both]), np.array([mb[s][k] for s in both])
        d = y - x
        se = d.std(ddof=1) / np.sqrt(len(d)) if len(d) > 1 else float("nan")
        print(f"  {k:14s} A {x.mean():7.3f}  B {y.mean():7.3f}  B-A {d.mean():+7.3f} +- {se:.3f}  (n {len(d)})")


if __name__ == "__main__":
    main(sys.argv[1:])
