#!/usr/bin/env python3
"""Review round 3: were the cards the final-pair remove-down sheds load-bearing earlier? For each run, the recorded act-1 and act-2 boss
fights (deck, relics, HP, potions as they were) vs the same fights without the shed cards held at that point (starters capped at the
number shed). usage: tools/shed_check.py <events dir> --delta target/review/delta.json --seeds ..."""
import argparse, collections, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--delta", required=True)
    ap.add_argument("--seeds", required=True)
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    eng = Engine()
    order = json.load(open(a.delta))["order"]
    for seed in a.seeds.split(","):
        shed = collections.Counter(order[seed])
        f = glob.glob(os.path.join(a.events, "*", seed, "events.jsonl"))[0]
        starts = [json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l]
        line = f"{seed} shed {dict(shed)}"
        for e in starts:
            if not (e["encounter"].endswith("_BOSS") and e.get("act") in (0, 1)):
                continue
            sc = e["scenario"]
            left, deck = collections.Counter(shed), []
            for c in sc["deck"]:
                if left[c["id"]] > 0:
                    left[c["id"]] -= 1
                else:
                    deck.append(c)
            res = eng.solve([sc, dict(sc, deck=deck)], attempts=128, seed=104729, groups=[0, 0])
            w = [np.nanmean([np.nan if x is None else x for x in q["wins"]]) for q in res]
            line += f" | A{e['act'] + 1} {e['encounter']} at {sc['hp']}/{sc['max_hp']} deck {len(sc['deck'])}: as played {w[0]:.2f}, shed {len(sc['deck']) - len(deck)} -> {w[1]:.2f}"
        print(line, flush=True)


if __name__ == "__main__":
    main()
