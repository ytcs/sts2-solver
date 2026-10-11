#!/usr/bin/env python3
"""Review round 3: (1) the remove-down-to-15 decks vs the run's own act-1 / act-2 bosses at full HP (is the shed set load-bearing earlier?);
(2) the pair-side card table D(c) = mean over reference end-game decks of P(pair | D + c) - P(pair | D), and its validation against
the remove-down order. usage: tools/delta_table.py <arm rows> <events dir> --seeds ... --winner <events.jsonl> --out <json>"""
import argparse, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("events")
    ap.add_argument("--seeds", required=True)
    ap.add_argument("--winner", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--attempts", type=int, default=48)
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    from agent import runmodel as R
    eng = Engine()
    rows = {json.loads(l)["seed"]: json.loads(l) for l in open(a.rows)}

    def mean_win(scs, att, seed_=None, groups=None):
        res = eng.solve(scs, attempts=att, seed=seed_, groups=groups)
        return np.array([np.nanmean([np.nan if x is None else x for x in q["wins"]]) for q in res])

    refs, order = [], {}
    for seed in a.seeds.split(","):
        pair = list(rows[seed]["deck_strength"]["final deck"].keys())
        f = glob.glob(os.path.join(a.events, "*", seed, "events.jsonl"))[0]
        starts = [json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l]
        sc0 = next(e["scenario"] for e in starts if e["encounter"] in pair)
        bosses = {e["act"]: e["encounter"] for e in starts if e["encounter"].endswith("_BOSS") and e.get("act") in (0, 1)}
        cur, removed = [dict(c) for c in sc0["deck"]], []
        while len(cur) > 15:
            idx = [i for i, c in enumerate(cur) if not c["id"].startswith("ASCENDERS_BANE")]
            scs = [dict(sc0, encounter=b, deck=cur[:i] + cur[i + 1:], hp=sc0["max_hp"], potions=[]) for i in idx for b in pair]
            w = mean_win(scs, 24, groups=[k % 2 for k in range(len(scs))]).reshape(len(idx), 2).mean(1)
            j = idx[int(np.argmax(w))]
            removed.append(cur[j]["id"])
            cur = cur[:j] + cur[j + 1:]
        order[seed] = removed
        refs.append((seed, sc0, pair, cur))
        line = f"{seed}: removed {removed}"
        for act, b in sorted(bosses.items()):
            ws = mean_win([dict(sc0, encounter=b, deck=d, hp=sc0["max_hp"], potions=[]) for d in (sc0["deck"], cur)], 96, 104729, groups=[0, 0])
            line += f" | A{act + 1} {b}: drafted {ws[0]:.2f}, 15 cards {ws[1]:.2f}"
        print(line, flush=True)
    win_starts = [json.loads(l) for l in open(a.winner, encoding="utf-8") if '"fight_start"' in l]
    wsc = next(e["scenario"] for e in win_starts if e["encounter"] == "QUEEN_BOSS")
    refs.append(("winner", wsc, ["QUEEN_BOSS", "AEONGLASS_BOSS"], [dict(c) for c in wsc["deck"]]))
    cards = [c["id"] for c in R.CAT["cards"]["IRONCLAD"] if c.get("rarity") not in ("Basic", "Starter")]
    table = {}
    base = {}
    for name, sc, pair, deck in refs:
        scs = [dict(sc, encounter=b, deck=deck, hp=sc["max_hp"], potions=[]) for b in pair]
        base[name] = mean_win(scs, 128, 7, groups=[0, 1]).mean()
        scs = [dict(sc, encounter=b, deck=deck + [{"id": c, "upgrade": 0}], hp=sc["max_hp"], potions=[]) for c in cards for b in pair]
        w = mean_win(scs, a.attempts, 7, groups=[k % 2 for k in range(len(scs))]).reshape(len(cards), 2).mean(1)
        for c, x in zip(cards, w):
            table.setdefault(c, {})[name] = float(x - base[name])
        print(f"ref {name}: base {base[name]:.3f}, deck {len(deck)}", flush=True)
    delta = {c: float(np.mean(list(v.values()))) for c, v in table.items()}
    json.dump(dict(delta=delta, per_ref=table, base={k: float(v) for k, v in base.items()}, order=order), open(a.out, "w"), indent=1)
    ranked = sorted(delta.items(), key=lambda x: -x[1])
    print("top:", [(c, round(d, 3)) for c, d in ranked[:15]])
    print("bottom:", [(c, round(d, 3)) for c, d in ranked[-15:]])
    import scipy.stats as ss
    for seed, removed in order.items():
        drafted = [c for c in removed if c in delta]
        if len(drafted) >= 3:
            rho = ss.spearmanr(range(len(drafted)), [delta[c] for c in drafted])
            print(f"{seed}: removal order vs D (drafted cards, n {len(drafted)}): Spearman {rho.statistic:+.2f}")
    wdeck = [c["id"] for c in wsc["deck"] if c["id"] in delta]
    print("winner's drafted cards D:", sorted(((c, round(delta[c], 3)) for c in wdeck), key=lambda x: x[1]))


if __name__ == "__main__":
    main()
