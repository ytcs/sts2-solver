#!/usr/bin/env python3
"""Paired fights through Engine.decide (the live decision path) with the potion margin gate off vs on: win, end HP, potions used,
and throws on turns with no incoming attack (wasted-timing proxy). Objective + V HP x potions kept reported at several V."""
import argparse, json, os, sys
import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path[:0] = [os.path.join(ROOT, "rl"), ROOT]


def incoming(snap):
    tot = 0
    for e in snap.get("enemies", []):
        if e.get("alive", True):
            for it in e.get("intents", []) or []:
                tot += int(it.get("total_damage") or 0)
    return tot


def play(eng, sc, seed, rounds, max_steps=400):
    import sts2
    sim = sts2.Sim(json.dumps(sc), seed)
    used = idle = gated = 0
    steps = 0
    while sim.outcome() == 0 and sim.stage() != "over" and steps < max_steps:
        snap = json.loads(sim.snapshot())
        d = eng.decide(sc, sim, 1.0, rounds=rounds if sim.stage() != "choice" else 2)
        if d.get("potion_gated"):
            gated += 1
        if d["text"].startswith("potion"):
            used += 1
            idle += incoming(snap) == 0
        sim.step(d["action"])
        steps += 1
    end = json.loads(sim.snapshot())["player"]
    return sim.outcome(), max(0, end["hp"]), used, idle, gated


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bench", default=os.path.join(ROOT, "data", "bench", "nearmiss_potions.json"))
    ap.add_argument("--n", type=int, default=150)
    ap.add_argument("--rounds", type=int, default=16)
    ap.add_argument("--seed", type=int, default=3)
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "potion_gate.jsonl"))
    a = ap.parse_args()
    from agent.engine import Engine
    rows = json.load(open(a.bench))
    idx = np.random.default_rng(a.seed).choice(len(rows), size=min(a.n, len(rows)), replace=False)
    arms = {"no gate": Engine(pot_margin_se=0.0), "gate": None}
    arms["gate"] = arms["no gate"]
    res = {k: [] for k in arms}
    f = open(a.out, "w")
    for j, bi in enumerate(idx):
        r = rows[bi]
        sc = r["scenario"]
        for k in arms:
            eng = arms[k]
            eng.pot_margin_se = 0.0 if k == "no gate" else 2.0
            eng.seed = 10_000 * (j + 1)
            oc, hp, used, idle, gated = play(eng, sc, int(r.get("seed", 1)), a.rounds)
            row = dict(i=int(bi), arm=k, win=int(oc == 1), hp=hp, max_hp=sc.get("max_hp", 80), pots=len(sc.get("potions", [])), used=used, idle=idle, gated=gated)
            res[k].append(row)
            f.write(json.dumps(row) + "\n")
            f.flush()
        if (j + 1) % 10 == 0:
            print(f"{j + 1} fights", flush=True)
    se = lambda x: x.std(ddof=1) / np.sqrt(len(x))  # noqa: E731
    A, B = res["no gate"], res["gate"]
    for k in res:
        R = res[k]
        print(f"{k:8s} win {np.mean([x['win'] for x in R]):.3f}  end HP {np.mean([x['hp'] for x in R]):.1f}  potions used {np.mean([x['used'] for x in R]):.2f}  "
              f"idle-turn throws {np.mean([x['idle'] for x in R]):.2f}  gated {np.mean([x['gated'] for x in R]):.2f}")
    for V in (0, 10, 20):
        def obj(x):
            return (1 + 0.5 * x["hp"] / x["max_hp"] if x["win"] else -1) + 0.5 * V / x["max_hp"] * (x["pots"] - x["used"])
        d = np.array([obj(y) - obj(x) for x, y in zip(A, B)])
        print(f"V {V:2d} HP: gate - no gate objective {d.mean():+.4f} +- {se(d):.4f}")
    dw = np.array([y["win"] - x["win"] for x, y in zip(A, B)])
    print(f"win gate - no gate {dw.mean():+.3f} +- {se(dw):.3f}")


if __name__ == "__main__":
    main()
