#!/usr/bin/env python3
"""Stress the final-pair oracle (review, round 1): from each final-pair deck, greedy remove-down to 18 and 15 cards, and the best
pair of adds among the top single adds from the run's own offers; every end result scored on holdout seeds vs the drafted deck.
usage: tools/oracle_stress.py <arm rows .jsonl> <dir with */<seed>/events.jsonl> --seeds BASE0001,..."""
import argparse, glob, itertools, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("events")
    ap.add_argument("--seeds", required=True)
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    from oracle import offers
    eng = Engine()
    rows = {json.loads(l)["seed"]: json.loads(l) for l in open(a.rows)}
    for seed in a.seeds.split(","):
        r = rows[seed]
        pair = list(r["deck_strength"]["final deck"].keys())
        f = glob.glob(os.path.join(a.events, "*", seed, "events.jsonl"))[0]
        start = next(json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l and any(b in l for b in pair))
        sc0 = start["scenario"]

        def ev(decks, att, seed_=None):
            scs = [dict(sc0, encounter=b, deck=d, hp=sc0["max_hp"], potions=[]) for d in decks for b in pair]
            res = eng.solve(scs, attempts=att, seed=seed_, groups=[i % 2 for i in range(len(scs))])
            w = np.array([[np.nan if x is None else x for x in q["wins"]] for q in res]).reshape(len(decks), 2, -1)
            return np.nanmean(w, 1)

        deck = [dict(c) for c in sc0["deck"]]
        hold = lambda ds: ev(ds, 128, 104729)  # noqa: E731
        out = {}
        cur = deck
        for target in (18, 15):
            while len(cur) > target:
                idx = [i for i, c in enumerate(cur) if not c["id"].startswith("ASCENDERS_BANE")]
                w = ev([cur[:i] + cur[i + 1:] for i in idx], 24)
                j = idx[int(np.nanargmax(np.nanmean(w, 1)))]
                cur = cur[:j] + cur[j + 1:]
            out[f"remove to {target}"] = cur
        pool = list(dict.fromkeys(offers(f)))
        adds = [deck + [{"id": c, "upgrade": u}] for c, u in pool]
        w = ev(adds, 32)
        top = [pool[i] for i in np.argsort(-np.nanmean(w, 1))[:10]]
        pairs = list(itertools.combinations(top, 2))
        pw = ev([deck + [{"id": c, "upgrade": u} for c, u in p] for p in pairs], 48)
        bp = pairs[int(np.nanargmax(np.nanmean(pw, 1)))]
        out[f"add pair {bp}"] = deck + [{"id": c, "upgrade": u} for c, u in bp]
        out[f"add single {top[0]}"] = deck + [{"id": top[0][0], "upgrade": top[0][1]}]
        keys = list(out)
        h = hold([deck] + [out[k] for k in keys])
        base = h[0]
        line = f"{seed} deck {len(deck)} holdout {np.nanmean(base):.3f}"
        for k, row in zip(keys, h[1:]):
            d = row - base
            line += f" | {k} ({len(out[k])}): {np.nanmean(row):.3f} ({np.nanmean(d):+.3f} +- {np.nanstd(d) / np.sqrt(np.sum(~np.isnan(d))):.3f})"
        print(line, flush=True)


if __name__ == "__main__":
    main()
