#!/usr/bin/env python3
"""Review section 11 (user): is a card worth more early? Each recorded fight of each run is solved with its recorded deck and with one
reference card added (paired seeds); binding fights (elite / boss) also at -10 HP for the HP-equivalent slope. Rows -> --out .jsonl.
usage: tools/card_timing.py <events dir> --out target/review/card_timing.jsonl [--seeds ...] [--attempts 64] [--cards ID[+],...] [--min-act N] [--boss-only]"""
import argparse, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
CARDS = "ANGER,SETUP_STRIKE,DISMANTLE,INFERNO,SHRUG_IT_OFF,DEMON_FORM"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--out", required=True)
    ap.add_argument("--seeds")
    ap.add_argument("--attempts", type=int, default=64)
    ap.add_argument("--cards", default=CARDS)
    ap.add_argument("--min-act", type=int, default=0)
    ap.add_argument("--boss-only", action="store_true")
    a = ap.parse_args()
    from agent.engine import Engine
    eng = Engine()
    cards = a.cards.split(",")
    done = set()
    if os.path.exists(a.out):
        done = {(r["seed"], r["i"]) for r in map(json.loads, open(a.out))}
    files = sorted(glob.glob(os.path.join(a.events, "*", "*", "events.jsonl")))
    want = set(a.seeds.split(",")) if a.seeds else None
    with open(a.out, "a") as out:
        for f in files:
            seed = os.path.basename(os.path.dirname(f))
            if want and seed not in want:
                continue
            starts = [json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l]
            for i, e in enumerate(starts):
                if (seed, i) in done or (e.get("act") or 0) < a.min_act or (a.boss_only and not e["encounter"].endswith("_BOSS")):
                    continue
                sc = e["scenario"]
                binding = "ELITE" in e["encounter"] or e["encounter"].endswith("_BOSS")
                scs = [sc] + [dict(sc, deck=sc["deck"] + [{"id": c.rstrip("+"), "upgrade": int(c.endswith("+"))}]) for c in cards]
                if binding:
                    scs.append(dict(sc, hp=max(1, sc["hp"] - 10)))
                res = eng.solve(scs, attempts=a.attempts, seed=104729, groups=[0] * len(scs))
                q = [dict(win=r["win"], lost=r["hp_lost"]) for r in res]
                out.write(json.dumps(dict(seed=seed, i=i, n=len(starts), enc=e["encounter"], act=e.get("act"), floor=e["floor"], hp=sc["hp"],
                                          max_hp=sc["max_hp"], deck=len(sc["deck"]), base=q[0], cards=dict(zip(cards, q[1:1 + len(cards)])),
                                          minus10=q[-1] if binding else None)) + "\n")
                out.flush()
            print(seed, len(starts), flush=True)


if __name__ == "__main__":
    main()
