#!/usr/bin/env python3
"""Soundness check of `sts2.provably_unwinnable`: decks made of cards the prover can vouch for, against every encounter; every fight it
flags must never be won by any policy (random, the scripted heuristic, a trained network, sampled many times). A single win is a bug in
the prover.

  .venv/bin/python tools/check_bounds.py --n 4000 --episodes 40000 [--ckpt target/runs/r3/ckpt_400.pt]
"""
import argparse, json, os, random, sys, collections
import numpy as np
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl"))
import sts2

PURE = {
    "IRONCLAD": ["BASH", "BLOOD_WALL", "BLUDGEON", "BREAK", "BREAKTHROUGH", "BURNING_PACT", "CINDER", "CONFLAGRATION", "DISMANTLE", "EVIL_EYE", "HEADBUTT",
                 "HEMOKINESIS", "IMPERVIOUS", "IRON_WAVE", "MOLTEN_FIST", "POMMEL_STRIKE", "SECOND_WIND", "SHRUG_IT_OFF", "SPITE", "SWORD_BOOMERANG", "TAUNT",
                 "THUNDERCLAP", "TREMBLE", "TRUE_GRIT", "TWIN_STRIKE", "UPPERCUT"],
    "SILENT": ["ACROBATICS", "ASSASSINATE", "BACKFLIP", "BACKSTAB", "BUBBLE_BUBBLE", "CALCULATED_GAMBLE", "DAGGER_SPRAY", "DAGGER_THROW", "DASH", "DEFLECT",
               "ECHOING_SLASH", "ESCAPE_PLAN", "EXPERTISE", "FLICK_FLACK", "HAND_TRICK", "LEG_SWEEP", "PREPARED", "REFLEX", "RICOCHET", "SLICE", "SUCKER_PUNCH",
               "SUPPRESS", "THE_HUNT", "UNTOUCHABLE"],
}
STARTER = {
    "IRONCLAD": (["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH"], "BURNING_BLOOD", 80),
    "SILENT": (["STRIKE_SILENT"] * 5 + ["DEFEND_SILENT"] * 5 + ["NEUTRALIZE", "SURVIVOR"], "RING_OF_THE_SNAKE", 70),
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=4000)
    ap.add_argument("--episodes", type=int, default=40000)
    ap.add_argument("--ckpt")
    ap.add_argument("--seed", type=int, default=1)
    a = ap.parse_args()
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    cat = json.load(open(next(q for q in (os.path.join(root, "target/train/catalog.json"), os.path.join(root, "data/catalog.json")) if os.path.exists(q))))
    encs = [e["id"] for e in cat["encounters"]]
    r = random.Random(a.seed)
    scen, flagged = [], []
    tried = 0
    while len(scen) < a.n and tried < a.n * 40:
        tried += 1
        ch = r.choice(["IRONCLAD", "SILENT"])
        base, relic, hp0 = STARTER[ch]
        deck = list(base) + [r.choice(PURE[ch]) for _ in range(r.choice([0, 0, 1, 2, 4, 8, 14]))]
        deck = [{"id": c, "upgrade": 1} if r.random() < 0.3 and c not in ("ASCENDERS_BANE",) else c for c in deck]
        if r.random() < 0.8:
            deck.append("ASCENDERS_BANE")
        s = {"name": f"b{tried}", "ascension": 10, "encounter": r.choice(encs), "character": ch, "hp": int(hp0 * r.uniform(0.4, 1.0)), "max_hp": hp0,
             "max_energy": 3, "max_potion_slots": 2, "gold": 0, "seed": f"b{a.seed}-{tried}", "total_floor": 10, "act": 0, "deck": deck, "relics": [relic], "potions": []}
        try:
            why = sts2.provably_unwinnable(s)
        except Exception:
            continue
        if why:
            scen.append(s)
            flagged.append(why)
    print(f"{len(scen)} flagged scenarios out of {tried} generated; characters {dict(collections.Counter(s['character'] for s in scen))}")
    print("e.g.", flagged[0] if flagged else None)
    print("encounters:", dict(collections.Counter(s['encounter'] for s in scen).most_common(5)))
    from baselines import random_policy, heuristic_policy
    pols = {"random": random_policy(1), "heuristic": heuristic_policy()}
    if a.ckpt:
        import torch
        from search import load
        from ppo import net_policy
        torch.set_num_threads(8)
        net = load(a.ckpt)
        pols["net(sampled)"] = net_policy(net, greedy=False)
        pols["net(greedy)"] = net_policy(net)
    bad = 0
    for name, pol in pols.items():
        env = sts2.VecEnv(1000, scen, seed=a.seed + 100, max_steps=300)
        obs, mask = env.reset()
        eps = wins = 0
        hp_left = []
        while eps < a.episodes:
            obs, mask, rew, done, info = env.step(pol(obs, mask))
            if done.any():
                ei = env.episode_info()
                for i in np.nonzero(done)[0]:
                    o = int(info["outcome"][i])
                    if o in (1, -1, 2):
                        eps += 1
                        if o == 1:
                            wins += 1
                            bad += 1
                            print("WIN in a fight declared unwinnable:", scen[int(ei["scenario"][i])]["name"], flagged[int(ei["scenario"][i])])
        print(f"{name}: {eps} episodes, {wins} wins")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
