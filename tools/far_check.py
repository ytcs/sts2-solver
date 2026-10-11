#!/usr/bin/env python3
"""Does stage A's far factor see what a card is worth at the later bosses? Per run: the act-1 boss deck (as recorded) + each reference
card scored like `price.search_check` far rungs (act-2 / act-3 boss pools at full HP x the first informative multiple, progress when
floored) vs E85's measured win change at that run's recorded act-2 / act-3 boss fights (`tools/card_timing.py` rows).
usage: tools/far_check.py <events dir> --timing target/review/card_timing.jsonl --out target/review/far_check.jsonl [--deck-act 0|1|2]
--deck-act: whose deck is scored (the first boss fight of that act); rungs at or after it."""
import argparse, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--timing", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--attempts", type=int, default=48)
    ap.add_argument("--deck-act", type=int, default=0)
    a = ap.parse_args()
    import numpy as np
    from agent import macro, pools
    from agent.engine import Engine
    from agent.price import INFORMATIVE, P_FLOOR
    eng = Engine()
    timing = [json.loads(l) for l in open(a.timing)]
    cards = list(timing[0]["cards"])
    with open(a.out, "w") as out:
        for f in sorted(glob.glob(os.path.join(a.events, "*", "*", "events.jsonl"))):
            seed = os.path.basename(os.path.dirname(f))
            boss1 = next((e for e in (json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l and '_BOSS"' in l) if e.get("act") == a.deck_act), None)
            if boss1 is None:
                continue
            sc = boss1["scenario"]
            row = dict(seed=seed, deck=len(sc["deck"]), rungs={})
            for act in [x for x in (1, 2) if x >= a.deck_act]:
                measured = [r for r in timing if r["seed"] == seed and r["act"] == act and r["enc"].endswith("_BOSS")]
                encs = [e for n in pools.act_names(act) for e in pools.pool(n, "boss")]
                at = lambda deck, m: [dict(sc, deck=deck, encounter=e, act=act, hp=int(sc["max_hp"] * m), max_hp=int(sc["max_hp"] * m)) for e in encs]  # noqa: E731
                ps = [np.mean([r["win"] for r in eng.solve(at(sc["deck"], m), attempts=24, seed=104729)]) for m in macro.SMOOTH_MULTS]
                mult = next((m for m, p in zip(macro.SMOOTH_MULTS, ps) if p >= INFORMATIVE), macro.SMOOTH_MULTS[-1])
                decks = [sc["deck"]] + [sc["deck"] + [{"id": c, "upgrade": 0}] for c in cards]
                scen = [x for d in decks for x in at(d, mult)]
                res = eng.solve(scen, attempts=a.attempts, seed=104729, groups=[i % len(encs) for i in range(len(scen))])
                win = np.array([r["win"] for r in res]).reshape(len(decks), len(encs)).mean(1)
                prog = np.array([np.nanmean([np.nan if x is None else x for x in (r.get("progress") or r["wins"])]) for r in res]).reshape(len(decks), len(encs)).mean(1)
                meas = win if win[0] >= INFORMATIVE else prog
                row["rungs"][act] = dict(mult=mult, base_win=float(win[0]), floored=bool(win[0] < INFORMATIVE),
                                         surrogate={c: float(np.log(max(meas[i + 1], P_FLOOR) / max(meas[0], P_FLOOR))) for i, c in enumerate(cards)},
                                         dprog={c: float(prog[i + 1] - prog[0]) for i, c in enumerate(cards)},
                                         measured={c: float(np.mean([r["cards"][c]["win"] - r["base"]["win"] for r in measured])) for c in cards} if measured else None)
            out.write(json.dumps(row) + "\n")
            out.flush()
            print(seed, flush=True)


if __name__ == "__main__":
    main()
