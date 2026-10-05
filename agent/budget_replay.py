"""Does more search fix the misses? Replays recorded fights from their start in the simulator with the live decision procedure (`Engine.decide`) at several
per-decision time caps and several seeds, and compares the HP lost.

    python -m agent.budget_replay --run 20261004-224215 --encounters AXEBOTS_NORMAL,TERROR_EEL_ELITE --budgets 0.3,1.5,6 --seeds 3 --out evals/budget_replay.json

A fight is replayed at the HP and with the deck / relics / potions of its recorded start. Potions held by `hold` in the live run are not special-cased here.
"""
import argparse
import json
import os
import sys
import time

import numpy as np
import sts2

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from agent.engine import Engine  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def starts(run, encounters=None, first_only=False):
    out = []
    for l in open(os.path.join(ROOT, "runs", run, "events.jsonl"), encoding="utf-8"):
        e = json.loads(l)
        if e["kind"] == "fight_start" and (not encounters or e["encounter"] in encounters):
            out.append(e)
    return out


def play_fight(eng, sc, seed, budget, tol_hp=0.0, max_steps=400):
    """One fight in the simulator, every decision by `Engine.decide` with the given time cap. Returns (outcome, HP lost, steps)."""
    sim = sts2.Sim(json.dumps(sc), seed)
    hp0 = json.loads(sim.snapshot())["player"]["hp"]
    steps = 0
    while sim.outcome() == 0 and steps < max_steps:
        stage = sim.stage()
        if stage == "over":
            break
        d = eng.decide(sc, sim, 0.3 if stage == "choice" else budget, tol_hp=tol_hp)
        sim.step(d["action"])
        steps += 1
    hp1 = json.loads(sim.snapshot())["player"]["hp"]
    return sim.outcome(), hp0 - max(hp1, 0), steps


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--encounters", default="")
    ap.add_argument("--budgets", default="0.3,1.5,6")
    ap.add_argument("--seeds", type=int, default=3)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    eng = Engine()
    encs = [x for x in a.encounters.split(",") if x]
    budgets = [float(x) for x in a.budgets.split(",")]
    results = []
    for e in starts(a.run, encs):
        sc = e["scenario"]
        row = dict(run=a.run, encounter=e["encounter"], hp=sc["hp"], predicted=e.get("predicted"), runs={})
        for b in budgets:
            res = []
            t0 = time.time()
            for s in range(1, a.seeds + 1):
                o, lost, n = play_fight(eng, sc, 1000 + s, b)
                res.append(dict(outcome=o, hp_lost=lost, steps=n))
            row["runs"][str(b)] = dict(results=res, mean_hp_lost=float(np.mean([r["hp_lost"] for r in res])), wins=sum(r["outcome"] == 1 for r in res), secs=round(time.time() - t0))
            print(e["encounter"], "budget", b, "mean HP lost %.1f" % row["runs"][str(b)]["mean_hp_lost"], "wins", row["runs"][str(b)]["wins"], "/", a.seeds, flush=True)
        results.append(row)
        if a.out:
            os.makedirs(os.path.dirname(os.path.join(ROOT, a.out)), exist_ok=True)
            json.dump(results, open(os.path.join(ROOT, a.out), "w"))
    return results


if __name__ == "__main__":
    main()
