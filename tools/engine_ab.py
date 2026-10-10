#!/usr/bin/env python3
"""Paired full fights, Engine.decide with batched rounds (FastSearch.decide_many) vs one FastSearch.decide call per round, same game seeds:
win rate and end HP per arm. Scenarios = fight_start scenarios from run logs (default: act-2 / act-3 bosses).
usage: tools/engine_ab.py <events.jsonl glob> [--enc _BOSS] [--acts 1,2] [--seeds 4] [--rounds 16] [--shard i/n] [--out f.jsonl]"""
import argparse, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def scenarios(pattern, enc, acts):
    out = []
    for f in sorted(glob.glob(pattern)):
        for l in open(f, encoding="utf-8"):
            e = json.loads(l)
            if e["kind"] == "fight_start" and e["encounter"].endswith(enc) and (e.get("scenario") or {}).get("act") in acts:
                out.append((os.path.basename(os.path.dirname(f)), e["scenario"]))
    return out


def play(eng, sc, seed, rounds, batched):
    import sts2
    fs = eng.fs
    if not batched:
        fs.decide_many = lambda s, sim, seeds, worth=None: [fs.decide(s, sim, x, worth=worth) for x in seeds]
    sim = sts2.Sim(json.dumps(sc), seed)
    eng.seed = seed * 1000
    steps = 0
    while sim.outcome() == 0 and sim.stage() != "over" and steps < 400:
        d = eng.decide(sc, sim, 1.0, rounds=rounds if sim.stage() != "choice" else 2, keep_potions=eng.keep())
        sim.step(d["action"])
        steps += 1
    if not batched:
        del fs.decide_many
    return int(sim.outcome() == 1), max(0, json.loads(sim.snapshot())["player"]["hp"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("logs")
    ap.add_argument("--enc", default="_BOSS")
    ap.add_argument("--acts", default="1,2")
    ap.add_argument("--seeds", type=int, default=4)
    ap.add_argument("--rounds", type=int, default=16)
    ap.add_argument("--shard", default="0/1")
    ap.add_argument("--out", default="target/engine_ab.jsonl")
    a = ap.parse_args()
    from agent.engine import Engine
    i, n = map(int, a.shard.split("/"))
    jobs = [(src, sc, s) for src, sc in scenarios(a.logs, a.enc, {int(x) for x in a.acts.split(",")}) for s in range(a.seeds)][i::n]
    eng = Engine()
    with open(a.out, "a", encoding="utf-8") as f:
        for src, sc, s in jobs:
            for arm, batched in (("batched", True), ("sequential", False)):
                w, hp = play(eng, sc, 7000 + s, a.rounds, batched)
                f.write(json.dumps(dict(src=src, enc=sc["encounter"], seed=s, arm=arm, win=w, hp=hp)) + "\n")
                f.flush()


if __name__ == "__main__":
    main()
