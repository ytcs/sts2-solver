#!/usr/bin/env python3
"""Final-pair oracle: for a baseline run that died at the final pair, the drafted deck vs the pair at full HP and at
arrival (held potions), the best removals, and a hill-climb over the cards the run was offered (card rewards, shop cards): how much the
same route's offers could have bought. P = mean over the two bosses, search wins, common seeds.
usage: tools/oracle.py <arm rows .jsonl> <dir with */<seed>/events.jsonl> [--seeds BASE0001,..] [--attempts 32] [--steps 8]"""
import argparse, glob, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def offers(events_path):
    from agent import macro, screen as scr
    out = []
    for raw in open(events_path, encoding="utf-8"):
        e = json.loads(raw)
        if e["kind"] != "macro" or e.get("screen") not in ("CARD_REWARD", "SHOP"):
            continue
        if e["screen"] == "CARD_REWARD":
            opts, _skip = macro.parse_card_options(e["state"])
            out += [(cid, up) for _n, _name, cid, up in opts if cid]
        else:
            for _, label in scr.options(e["state"]):
                m = re.match(r"^(\d+)g card (.+?)(?:\(|:)", label)
                if m:
                    cid, up = macro.card_from_name(m.group(2).strip())
                    if cid:
                        out.append((cid, up))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("events")
    ap.add_argument("--seeds", default="")
    ap.add_argument("--attempts", type=int, default=32)
    ap.add_argument("--steps", type=int, default=8)
    a = ap.parse_args()
    import numpy as np
    from agent.engine import Engine
    eng = Engine()
    rows = [json.loads(l) for l in open(a.rows)]
    want = set(a.seeds.split(",")) if a.seeds else None
    for r in rows:
        pair = list(((r.get("deck_strength") or {}).get("final deck") or {}).keys())
        if r["floor"] < 48 or len(pair) != 2 or (want and r["seed"] not in want):
            continue
        f = glob.glob(os.path.join(a.events, "*", r["seed"], "events.jsonl"))[0]
        start = None
        for raw in open(f, encoding="utf-8"):
            e = json.loads(raw)
            if e["kind"] == "fight_start" and e["encounter"] in pair:
                start = start or e
        sc0 = start["scenario"]

        def scen(deck, enc, hp=None, potions=None):
            return dict(sc0, encounter=enc, deck=deck, hp=hp or sc0["max_hp"], potions=[] if potions is None else potions)

        def evaluate(decks, att=a.attempts, seed=None, **kw):
            scs = [scen(d, b, **kw) for d in decks for b in pair]
            res = eng.solve(scs, attempts=att, seed=seed, groups=[i % 2 for i in range(len(scs))])
            w = np.array([[np.nan if x is None else x for x in rr["wins"]] for rr in res]).reshape(len(decks), 2, -1)
            return w.mean(1)  # [deck, attempt]: mean over the two bosses

        deck = [dict(c) for c in sc0["deck"]]
        base = evaluate([deck], att=96)[0]
        arrival = evaluate([deck], att=96, hp=sc0["hp"], potions=sc0.get("potions", []))[0]
        hp_only = evaluate([deck], att=96, hp=sc0["hp"])[0]
        pots_only = evaluate([deck], att=96, potions=sc0.get("potions", []))[0]
        print(f"{r['seed']} pair {pair} deck {len(deck)} | full HP {np.nanmean(base):.3f} | arrival {sc0['hp']}/{sc0['max_hp']} + {len(sc0.get('potions', []))} potions "
              f"{np.nanmean(arrival):.3f} (HP only {np.nanmean(hp_only):.3f}, potions at full HP {np.nanmean(pots_only):.3f})", flush=True)

        def confirmed(cur, cands, top=3):
            """screen every candidate, re-measure the top few on fresh seeds: (index, gain, se) of the best confirmed, or None"""
            w = evaluate([cur] + cands)
            order = np.argsort(-np.nanmean(w[1:] - w[0], 1))[:top]
            v = evaluate([cur] + [cands[i] for i in order], att=128, seed=7919)
            g = v[1:] - v[0]
            se = np.nanstd(g, 1) / np.sqrt(np.sum(~np.isnan(g), 1))
            k = int(np.argmax(np.nanmean(g, 1)))
            return int(order[k]), float(np.nanmean(g[k])), float(se[k]), float(np.nanmean(v[1 + k]))
        cur = deck
        for k in range(3):
            idx = [i for i in range(len(cur)) if not cur[i]["id"].startswith("ASCENDERS_BANE")]
            j, g, se, p = confirmed(cur, [cur[:i] + cur[i + 1:] for i in idx])
            print(f"  remove {k + 1}: {cur[idx[j]]['id']} -> {p:.3f} ({g:+.3f} +- {se:.3f}, confirmed on fresh seeds)", flush=True)
            if g <= 2 * se:
                break
            cur = cur[:idx[j]] + cur[idx[j] + 1:]
        pool = offers(f)
        cur, left = [dict(c) for c in deck], list(pool)
        for step in range(a.steps):
            moves = [("+", (cid, up), cur + [{"id": cid, "upgrade": up}]) for cid, up in dict.fromkeys(left)]
            moves += [("-", c["id"], cur[:i] + cur[i + 1:]) for i, c in enumerate(cur)]
            i, g, se, p = confirmed(cur, [m[2] for m in moves])
            if g <= 2 * se:
                break
            kind, what, cur = moves[i]
            if kind == "+":
                left.remove(what)
            print(f"  step {step + 1}: {kind}{what} -> {p:.3f} ({g:+.3f} +- {se:.3f}, confirmed)", flush=True)
        final = evaluate([cur], att=96, seed=104729)[0]
        base2 = evaluate([deck], att=96, seed=104729)[0]
        print(f"  choice oracle (holdout seeds): {np.nanmean(base2):.3f} -> {np.nanmean(final):.3f} (deck {len(deck)} -> {len(cur)}; offered pool {len(pool)})", flush=True)


if __name__ == "__main__":
    main()
