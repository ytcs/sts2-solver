#!/usr/bin/env python3
"""What-if analysis of one fight with the solver: which single card helps or hurts the deck.

  .venv/bin/python rl/whatif.py --scenario data/examples/phrog_dup.json --attempts 512 [--out whatif.json]

* removal: for every distinct card (name, upgrade) of the deck, the fight is played with one copy removed and compared with the full deck;
  the winners of this list are cards the deck is better without, the losers the cards that matter most in this fight.
* addition (--add CARD[,CARD..]): the same for one extra copy of each card.
All variants are solved together (`Solver.solve`), so the comparison is paired in the sense that every variant gets the same number of attempts.
The output of --out feeds `rl/trace.py --analysis`.
"""
import argparse, collections, copy, json, os, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from solver import Solver


def deck_key(c):
    return (c, 0) if isinstance(c, str) else (c["id"], c.get("upgrade", 0))


def label(key):
    return key[0] + ("+" if key[1] else "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--attempts", type=int, default=512)
    ap.add_argument("--add", default="", help="comma separated card ids to try as one extra copy")
    ap.add_argument("--out", default="")
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    scen = json.load(open(a.scenario))
    counts = collections.OrderedDict()
    for c in scen["deck"]:
        counts[deck_key(c)] = counts.get(deck_key(c), 0) + 1
    variants, names = [scen], ["full deck"]
    for key in counts:
        s2 = copy.deepcopy(scen)
        for i, c in enumerate(s2["deck"]):
            if deck_key(c) == key:
                del s2["deck"][i]
                break
        variants.append(s2)
        names.append("remove " + label(key))
    for cid in filter(None, a.add.split(",")):
        s2 = copy.deepcopy(scen)
        s2["deck"].append(cid)
        variants.append(s2)
        names.append("add " + cid)
    res = Solver().solve(variants, attempts=a.attempts, seed=a.seed)
    base = res[0]
    out = dict(base=dict(win=base["win"], hp_left=base["hp_left_on_win"], n=base["attempts"], se=base["win_se"]), removal=[], addition=[])
    print(f"{'full deck':28s} win {base['win']:.3f} (+-{base['win_se']:.3f})  HP lost {100 * base['hp_lost']:.0f}%  HP left on win {base['hp_left_on_win']:.0f}")
    for nm, r in zip(names[1:], res[1:]):
        kind, card = nm.split(" ", 1)
        row = dict(card=card, win=r["win"], se=r["win_se"], hp_lost=r["hp_lost"], hp_left=r["hp_left_on_win"])
        if kind == "remove":
            row["copies"] = counts[next(k for k in counts if label(k) == card)]
            out["removal"].append(row)
        else:
            out["addition"].append(row)
        print(f"{nm:28s} win {r['win']:.3f} (+-{r['win_se']:.3f})  d {100 * (r['win'] - base['win']):+5.1f} pts  HP lost {100 * r['hp_lost']:.0f}%  HP left on win {r['hp_left_on_win']:.0f}")
    if a.out:
        json.dump(out, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
