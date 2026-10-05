"""Does the simulator + solver predict the real game on a fixed deck? Same loadout, same encounter, many fights: HP lost in the real game (dev console loadout, fresh run
per fight, played by the harness solver) against HP lost in the simulator (`Engine.decide`, same time cap, no potions). A harness test, never a scored run.

    python -m agent.calibrate --run 20261004-215036 --encounters SPINY_TOAD_NORMAL,AXEBOTS_NORMAL --real 12 --sim 40 --budget 1.5 --out evals/calibrate_thorns.json

The loadout is the deck / relics of the first recorded `fight_start` of the run (the last one with the encounter list, or `--index`), at full HP (80). Cards and relics with
a pickup effect are skipped (as in the fidelity sweep) and reported, and the simulator gets exactly the loadout that was applied.
"""
import argparse
import collections
import json
import os
import re
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from agent import fidelity_sweep as fs  # noqa: E402
from agent.bridge import call  # noqa: E402
from agent.harness import Harness  # noqa: E402

ROOT = fs.ROOT
STARTER = collections.Counter({"STRIKE_IRONCLAD": 5, "DEFEND_IRONCLAD": 4, "BASH": 1, "ASCENDERS_BANE": 1})
SKIP = {"HEFTY_TABLET", "ARCANE_SCROLL", "YUMMY_COOKIE", "CLAWS", "WAR_PAINT", "CURSED_PEARL", "MAD_SCIENCE"}


def loadout(run, index=-1):
    sc = [json.loads(l) for l in open(os.path.join(ROOT, "runs", run, "events.jsonl"), encoding="utf-8")]
    sc = [e["scenario"] for e in sc if e["kind"] == "fight_start"]
    return sc[index]


def applied(sc):
    """Commands for the console and the scenario the simulator gets (the loadout minus what the console cannot add)."""
    relics = [r["id"] for r in sc["relics"] if r["id"] != "BURNING_BLOOD" and r["id"] not in SKIP and r["id"] in fs.relic_ids()]
    need = collections.Counter(c["id"] for c in sc["deck"] if c["id"] not in SKIP) - STARTER
    cmds = [f"x relic add {r}" for r in relics] + [f"x card {c} Deck" for c, k in need.items() for _ in range(k)]
    deck = [dict(id=c, upgrade=0) for c, k in STARTER.items() for _ in range(k)] + [dict(id=c, upgrade=0) for c, k in need.items() for _ in range(k)]
    simsc = dict(sc, hp=80, max_hp=80, deck=deck, relics=[dict(id="BURNING_BLOOD")] + [dict(id=r) for r in relics], potions=[])
    return cmds, simsc, relics


def real_fight(h, cmds, enc):
    if not fs.fresh_run(h, "ironclad"):
        return None
    for c in cmds:
        call(c)
    call("x heal 999")
    r = call("x fight " + enc)
    if r.startswith(("fail", "ERR")):
        return None
    for _ in range(40):
        h.handle("combat")
        if fs.state_kind(call("s")) not in ("COMBAT", "SELECT"):
            break
    m = re.search(r"HP (\d+)/(\d+)", call("s"))
    won = fs.state_kind(call("s")) != "GAME_OVER"
    return dict(hp_lost=80 - (int(m.group(1)) if m else 0), won=won)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--index", type=int, default=-1)
    ap.add_argument("--encounters", required=True)
    ap.add_argument("--real", type=int, default=12)
    ap.add_argument("--sim", type=int, default=40)
    ap.add_argument("--budget", type=float, default=1.5)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    sc = loadout(a.run, a.index)
    cmds, simsc, relics = applied(sc)
    print("relics applied:", relics, "| deck", len(simsc["deck"]), flush=True)
    from agent.budget_replay import play_fight
    from agent.engine import Engine
    eng = Engine()
    out = dict(run=a.run, budget=a.budget, relics=relics, deck=[c["id"] for c in simsc["deck"]], results={})
    for enc in a.encounters.split(","):
        s = dict(simsc, encounter=enc)
        t0 = time.time()
        sim = []
        for k in range(a.sim):
            o, lost, _ = play_fight(eng, s, 5000 + k, a.budget, keep_potions=True)
            sim.append(dict(hp_lost=lost, won=o == 1))
        print(enc, "sim  mean HP lost %.1f  wins %d/%d  (%ds)" % (np.mean([x["hp_lost"] for x in sim]), sum(x["won"] for x in sim), a.sim, time.time() - t0), flush=True)
        h = Harness()
        h._sync_problem = lambda f: None
        h.handle(f"budget {a.budget}")
        real = []
        for k in range(a.real):
            r = real_fight(h, cmds, enc)
            if r:
                real.append(r)
                print(enc, "real", k, r, flush=True)
        out["results"][enc] = dict(sim=sim, real=real)
        if real:
            print(enc, "REAL mean HP lost %.1f (se %.1f) wins %d/%d   vs SIM %.1f (se %.1f)" % (
                np.mean([x["hp_lost"] for x in real]), np.std([x["hp_lost"] for x in real]) / len(real) ** 0.5, sum(x["won"] for x in real), len(real),
                np.mean([x["hp_lost"] for x in sim]), np.std([x["hp_lost"] for x in sim]) / len(sim) ** 0.5), flush=True)
        if a.out:
            json.dump(out, open(os.path.join(ROOT, a.out), "w"))


if __name__ == "__main__":
    main()
