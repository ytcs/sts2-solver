#!/usr/bin/env python3
"""Fidelity check of the live-fight replay: play real fights through the bridge with simple rules and, after every action, compare the simulator's
prediction (before aligning it) with what the game shows.

  python -m agent.validate --fights 5 [--char ironclad --asc 10] [--out target/replay_check.json]

Prints, per category, how often the simulator's own prediction disagreed with the game. Expected: hand / draw / discard contents after draws (random);
anything else (HP, block, energy, powers, enemy HP, intents) is a divergence to investigate.
"""
import argparse, collections, json, os, sys, time

from agent.bridge import call
from agent.autopilot import parse, choose
from agent.fight import Replayer


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fights", type=int, default=3)
    ap.add_argument("--char", default="ironclad")
    ap.add_argument("--asc", default="10")
    ap.add_argument("--steps", type=int, default=3000)
    ap.add_argument("--out", default="target/replay_check.json")
    ap.add_argument("--no-buy", action="store_true", default=True)
    ap.add_argument("--encounters", default="", help="comma-separated encounter ids to jump to (dev console `fight`), one per fight; HP is topped up first")
    ap.add_argument("--loadout", default="", help="RUN[:ENCOUNTER]: give the test run the deck (non-starter cards), relics and potions recorded at that fight start (dev console)")
    a = ap.parse_args()
    loadout = []
    if a.loadout:
        run, _, enc = a.loadout.partition(":")
        sc = None
        for line in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "runs", run, "events.jsonl"), encoding="utf-8"):
            e = json.loads(line)
            if e["kind"] == "fight_start" and (not enc or e["encounter"] == enc):
                sc = e["scenario"]
        starter = collections.Counter({"STRIKE_IRONCLAD": 5, "DEFEND_IRONCLAD": 4, "BASH": 1, "ASCENDERS_BANE": 1})
        need = collections.Counter(c["id"] for c in sc["deck"]) - starter
        loadout += [f"x relic add {r['id']}" for r in sc["relics"] if r["id"] != "BURNING_BLOOD"]
        loadout += [f"x card {cid} Deck" for cid, n in need.items() for _ in range(n)]
        loadout += [f"x potion {p['id']}" for p in sc.get("potions", [])]

    applied = False
    pending = [e for e in a.encounters.split(",") if e]
    totals = collections.Counter()
    examples = collections.defaultdict(list)
    errors = []
    fights = 0
    steps_in_fights = 0
    rp = None
    t0 = time.time()
    state = call("s")
    for _ in range(a.steps):
        kind, opts, lines = parse(state)
        if kind == "MENU":
            applied = False  # a new run starts: it needs the loadout again
        if "(busy)" in lines[0] or not opts:
            time.sleep(1)
            state = call("s")
            continue
        if kind == "COMBAT":
            raw = call("fight")
            try:
                f = json.loads(raw)
            except Exception:  # noqa: BLE001
                f = None
            if f:
                if rp is None or len(f["log"]) < rp.applied:
                    if rp is not None:
                        pass
                    rp = Replayer(f, seed=fights)
                    fights += 1
                    steps_in_fights = 0
                    enc = f["scenario"]["encounter"]
                    print(f"fight {fights}: {enc} hp {f['scenario']['hp']}/{f['scenario']['max_hp']} deck {len(f['scenario']['deck'])}", flush=True)
                rp.advance(f)
                steps_in_fights += 1
        else:
            if rp is not None:
                for k, v in rp.stats.items():
                    totals[k] += v
                for k, v in rp.examples.items():
                    examples[k].extend(v[:3])
                errors.extend(rp.errors)
                rp = None
                if fights >= a.fights:
                    break
            if pending and kind in ("REWARDS", "CARD_REWARD", "MAP", "EVENT", "SHOP", "RESTSITE", "TREASURE"):
                if loadout and not applied:
                    for c in loadout:
                        call(c)
                    print(f"  loadout applied ({len(loadout)} commands)", flush=True)
                    applied = True
                call("x heal 999")
                r = call("x fight " + pending.pop(0))
                state = call("s")
                continue
        cmd = choose(kind, opts, lines, a)
        state = call("a " + cmd)
        if state.startswith("ERR"):
            state = state.split("\n", 1)[1]
    print(f"\n{fights} fights, {time.time() - t0:.0f}s")
    print("categories (count):")
    for k, v in sorted(totals.items(), key=lambda kv: -kv[1]):
        print(f"  {v:6d}  {k}")
    print("\nexamples:")
    for k, v in examples.items():
        if k.startswith(("diff", "residual", "sync", "intent", "action", "missing")):
            print(f"  [{k}]")
            for t in v[:3]:
                print("     ", t)
    if errors:
        print("\nerrors:", errors[:5])
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    json.dump({"totals": totals, "examples": examples, "errors": errors}, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
