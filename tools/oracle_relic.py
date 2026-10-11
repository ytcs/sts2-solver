#!/usr/bin/env python3
"""Gold into relics vs removals at the final pair (review, round 2): for each final-pair deck, the shop relics the run was offered and
did not buy (simulator-supported ids) vs the removals the same gold buys at A10 prices (100, +50 each), all scored vs the pair on
holdout seeds. usage: tools/oracle_relic.py <arm rows .jsonl> <dir with */<seed>/events.jsonl> --seeds BASE0001,..."""
import argparse, glob, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def shop_relics(path, have):
    from agent import price as PR, screen as scr
    out = {}
    for raw in open(path, encoding="utf-8"):
        e = json.loads(raw)
        if e["kind"] != "macro" or e.get("screen") != "SHOP":
            continue
        for _, lab in scr.options(e["state"]):
            m = re.match(r"^(\d+)g relic (.+?)(?:\(|:)", lab)
            if m:
                name = m.group(2).strip()
                rid = ("FAKE_" if name.endswith("???") else "") + PR._ident(name)
                if rid in PR._ids("relics") and rid not in have:
                    out.setdefault(rid, int(m.group(1)))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("events")
    ap.add_argument("--seeds", required=True)
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    eng = Engine()
    rows = {json.loads(l)["seed"]: json.loads(l) for l in open(a.rows)}
    for seed in a.seeds.split(","):
        pair = list(rows[seed]["deck_strength"]["final deck"].keys())
        f = glob.glob(os.path.join(a.events, "*", seed, "events.jsonl"))[0]
        sc0 = next(json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l and any(b in l for b in pair))["scenario"]
        have = {r["id"] if isinstance(r, dict) else r for r in sc0.get("relics", [])}
        offered = shop_relics(f, have)

        def ev(variants, att, seed_=None):
            scs = [dict(sc0, encounter=b, deck=d, relics=rl, hp=sc0["max_hp"], potions=[]) for d, rl in variants for b in pair]
            res = eng.solve(scs, attempts=att, seed=seed_, groups=[i % 2 for i in range(len(scs))])
            w = np.array([[np.nan if x is None else x for x in q["wins"]] for q in res]).reshape(len(variants), 2, -1)
            return np.nanmean(w, 1)

        deck, relics = [dict(c) for c in sc0["deck"]], list(sc0.get("relics", []))
        # removals the cheapest offered relic's gold would buy, greedy
        cheapest = min(offered.values()) if offered else 200
        k, cost, price = 0, 0, 100
        while cost + price <= cheapest:
            cost += price; k += 1; price += 50
        cur = deck
        for _ in range(k):
            idx = [i for i, c in enumerate(cur) if not c["id"].startswith("ASCENDERS_BANE")]
            w = ev([(cur[:i] + cur[i + 1:], relics) for i in idx], 32)
            j = idx[int(np.nanargmax(np.nanmean(w, 1)))]
            cur = cur[:j] + cur[j + 1:]
        variants = [(deck, relics), (cur, relics)] + [(deck, relics + [{"id": rid}]) for rid in offered]
        names = ["drafted", f"{k} removals ({cost}g)"] + [f"+{rid} ({g}g)" for rid, g in offered.items()]
        h = ev(variants, 128, 104729)
        base = h[0]
        line = f"{seed} deck {len(deck)}: drafted {np.nanmean(base):.3f}"
        for n_, row in zip(names[1:], h[1:]):
            d = row - base
            line += f" | {n_} {np.nanmean(d):+.3f}+-{np.nanstd(d) / np.sqrt(np.sum(~np.isnan(d))):.3f}"
        print(line, flush=True)


if __name__ == "__main__":
    main()
