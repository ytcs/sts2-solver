#!/usr/bin/env python3
"""Which cards the final-pair remove-down drops (review, round 2): greedy remove-down to 15 from each final-pair deck, printing each
removed card, whether it is a starter card, and the floor and screen where a drafted copy entered the deck.
usage: tools/oracle_removed.py <arm rows .jsonl> <dir with */<seed>/events.jsonl> --seeds BASE0001,..."""
import argparse, glob, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
STARTER = ("STRIKE_", "DEFEND_", "BASH", "ASCENDERS_BANE")


def acquisitions(path):
    """[(floor, screen, card id)] for cards taken at card rewards and shops"""
    from agent import macro, screen as scr
    out = []
    for raw in open(path, encoding="utf-8"):
        e = json.loads(raw)
        if e["kind"] != "macro" or e.get("screen") not in ("CARD_REWARD", "SHOP"):
            continue
        hdr = e["state"].split("\n")[1].split(" IRONCLAD")[0]
        lab = scr.option_line(e["state"], str(e.get("choice"))) if hasattr(scr, "option_line") else ""
        lab = re.sub(r"^\d+ ", "", lab or "")
        if e["screen"] == "SHOP":
            m = re.match(r"^\d+g card (.+?)(?:\(|:)", lab)
            name = m.group(1).strip() if m else None
        else:
            m = re.match(r"^(.+?)\(", lab)
            name = m.group(1).strip() if m else None
        cid = macro.card_from_name(name)[0] if name else None
        if cid:
            out.append((hdr, e["screen"], cid))
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
        acq = acquisitions(f)

        def ev(decks, att):
            scs = [dict(sc0, encounter=b, deck=d, hp=sc0["max_hp"], potions=[]) for d in decks for b in pair]
            res = eng.solve(scs, attempts=att, groups=[i % 2 for i in range(len(scs))])
            w = np.array([[np.nan if x is None else x for x in q["wins"]] for q in res]).reshape(len(decks), 2, -1)
            return np.nanmean(np.nanmean(w, 1), 1)

        cur = [dict(c) for c in sc0["deck"]]
        print(f"{seed} pair {pair} deck {len(cur)}; acquired {len(acq)} cards at rewards / shops", flush=True)
        while len(cur) > 15:
            idx = [i for i, c in enumerate(cur) if not c["id"].startswith("ASCENDERS_BANE")]
            p = ev([cur[:i] + cur[i + 1:] for i in idx], 24)
            j = idx[int(np.argmax(p))]
            c = cur[j]
            src = "starter" if c["id"].startswith(STARTER) else next((f"{fl} {sc}" for fl, sc, cid in acq if cid == c["id"]), "other (event / ancient / transform)")
            print(f"  -{c['id']}{'+' if c.get('upgrade') else ''} ({src}) -> {np.max(p):.3f}", flush=True)
            cur = cur[:j] + cur[j + 1:]


if __name__ == "__main__":
    main()
