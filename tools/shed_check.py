#!/usr/bin/env python3
"""Review rounds 3-4: were the cards the final-pair remove-down sheds load-bearing earlier, and what does never taking them buy at the
pair? For each run: the recorded act-1 / act-2 boss fights and the final pair (deck, relics, HP, potions as recorded; the pair at full
HP without potions) vs the same fights without the shed cards held then. --drafted-only keeps starters (Strike, Defend, Bash) in the
deck and sheds only drafted cards and curses; at the pair it then adds 1-3 starter removals in the remove-down order.
usage: tools/shed_check.py <events dir> --delta target/review/delta.json --seeds ... [--drafted-only] [--pair]"""
import argparse, collections, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
STARTER = ("STRIKE_", "DEFEND_", "BASH")


def strip(deck, shed):
    left, out = collections.Counter(shed), []
    for c in deck:
        if left[c["id"]] > 0:
            left[c["id"]] -= 1
        else:
            out.append(c)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--delta", required=True)
    ap.add_argument("--seeds", required=True)
    ap.add_argument("--drafted-only", action="store_true")
    ap.add_argument("--pair", action="store_true")
    ap.add_argument("--rows", help="arm rows .jsonl: the pair = deck_strength['final deck'] keys (as the other review tools)")
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    eng = Engine()
    order = json.load(open(a.delta))["order"]

    def win(scs, att=128):
        res = eng.solve(scs, attempts=att, seed=104729, groups=[0] * len(scs))
        return [float(np.nanmean([np.nan if x is None else x for x in q["wins"]])) for q in res]

    for seed in a.seeds.split(","):
        full = order[seed]
        shed = [c for c in full if not c.startswith(STARTER)] if a.drafted_only else full
        starters = [c for c in full if c.startswith(STARTER)]
        f = glob.glob(os.path.join(a.events, "*", seed, "events.jsonl"))[0]
        starts = [json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l]
        line = f"{seed} shed {len(shed)} {'drafted / curses' if a.drafted_only else 'cards'}"
        for e in starts:
            if not (e["encounter"].endswith("_BOSS") and e.get("act") in (0, 1)):
                continue
            sc = e["scenario"]
            d = strip(sc["deck"], shed)
            w = win([sc, dict(sc, deck=d)])
            line += f" | A{e['act'] + 1} {e['encounter']} deck {len(sc['deck'])}: {w[0]:.2f} -> {w[1]:.2f} (-{len(sc['deck']) - len(d)})"
        if a.pair:
            rows = {json.loads(l)["seed"]: json.loads(l) for l in open(a.rows)} if a.rows else {}
            pair = list(rows[seed]["deck_strength"]["final deck"].keys()) if seed in rows else []
            bosses = [e for e in starts if e["encounter"] in pair]
            if pair and bosses:
                sc = bosses[0]["scenario"]
                base = dict(sc, hp=sc["max_hp"], potions=[])
                variants = [sc["deck"], strip(sc["deck"], shed)] + [strip(sc["deck"], shed + starters[:k]) for k in (1, 2, 3)] + [strip(sc["deck"], full[:k]) for k in (1, 2, 3)]
                ws = np.array(win([dict(base, encounter=b, deck=d) for d in variants for b in pair])).reshape(len(variants), len(pair)).mean(1)
                line += f" | pair {pair} deck {len(variants[0])}: drafted {ws[0]:.3f}, without shed {ws[1]:.3f} ({len(variants[1])} cards)" + "".join(
                    f", +{k} starter removal{'s' if k > 1 else ''} {ws[1 + k]:.3f}" for k in (1, 2, 3)) + " | remove-down prefix " + ", ".join(
                    f"{k}: {ws[4 + k]:.3f}" for k in (1, 2, 3))
        print(line, flush=True)


if __name__ == "__main__":
    main()
