"""Deck-building test bed: how good is a pick policy when the future offers are unknown?

A start deck (the Ironclad starter), a target fight (any encounter, full HP), and N mock card-reward screens of 3 random Ironclad cards (rarities 60/37/3%) plus
"skip". A policy sees one screen at a time and picks; the final deck is scored by the solver's win rate (and HP lost) against the target. All policies see the
SAME reward sequences, so differences are paired. `hindsight` knows the whole sequence in advance (beam search over pick sequences): the headroom a better method
could still gain over any online policy.

    python -m agent.deckbench --target KNIGHTS_ELITE --screens 6 --sequences 8 --policies skip,random,greedy,hindsight --out evals/deckbench.json

Policies:
  skip       never picks (the starter deck)
  random     picks a random option (skip included)
  greedy     the current method: picks the option with the best win rate against the target now (skip when nothing is better by `--tau` standard errors)
  hindsight  best pick sequence for the known sequence (beam search; width `--beam`)
"""
import argparse
import json
import os
import random
import re
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

STARTER = [("STRIKE_IRONCLAD", 5), ("DEFEND_IRONCLAD", 4), ("BASH", 1), ("ASCENDERS_BANE", 1)]
RARITY_ODDS = (("Common", 0.60), ("Uncommon", 0.37), ("Rare", 0.03))


def card_pool():
    """Ironclad reward cards by rarity, from the simulator's own card table."""
    root = os.path.join(ROOT, "crates", "sts2sim", "src", "content")
    pool_src = open(os.path.join(root, "gen_pools.rs"), encoding="utf-8").read().split("pub static IRONCLAD")[1].split("];")[0]
    ids = set(re.findall(r"ids::card::([A-Z0-9_]+)", pool_src))
    rar = {}
    for m in re.finditer(r"CardDef::new\(ids::card::([A-Z0-9_]+), -?\d+, CardType::\w+, CardRarity::(\w+)", open(os.path.join(root, "gen_cards.rs"), encoding="utf-8").read()):
        rar[m.group(1)] = m.group(2)
    out = {"Common": [], "Uncommon": [], "Rare": []}
    for cid in sorted(ids):
        if rar.get(cid) in out:
            out[rar[cid]].append(cid)
    return out


def make_sequence(rng, screens, pool):
    seq = []
    for _ in range(screens):
        offer = []
        while len(offer) < 3:
            r = rng.random()
            tier = "Common" if r < RARITY_ODDS[0][1] else ("Uncommon" if r < RARITY_ODDS[0][1] + RARITY_ODDS[1][1] else "Rare")
            c = rng.choice(pool[tier])
            if c not in offer:
                offer.append(c)
        seq.append(offer)
    return seq


def scenario(deck, target, hp=80):  # hp is overridden by Scorer.hp
    d = [dict(id=c, upgrade=0) for c, k in STARTER for _ in range(k)] + [dict(id=c, upgrade=0) for c in deck]
    return dict(name="bench", ascension=10, encounter=target, character="IRONCLAD", hp=hp, max_hp=hp, max_energy=3, gold=0, max_potion_slots=2, base_orb_slots=0,
                seed="bench", total_floor=1, act=0, deck=d, relics=[dict(id="BURNING_BLOOD")], potions=[])


class Scorer:
    """Solver win rate and HP lost of decks against the target; results are cached by deck."""

    def __init__(self, eng, target, attempts, hp=80, mults=(1.0, 1.5, 2.0, 3.0)):
        self.eng, self.target, self.attempts, self.cache, self.calls, self.hp, self.mults = eng, target, attempts, {}, 0, hp, tuple(mults)

    def many(self, decks, hp=None):
        hp = hp or self.hp
        key = lambda d: (hp, tuple(sorted(d)))  # noqa: E731
        todo = list(dict.fromkeys(key(d) for d in decks if key(d) not in self.cache))
        if todo:
            res = self.eng.solve([scenario(list(d), self.target, hp) for _, d in todo], attempts=self.attempts)
            for k, r in zip(todo, res):
                self.cache[k] = (r["win"], r["win_se"], r["hp_lost"] or 0.0)
            self.calls += len(todo)
        return [self.cache[key(d)] for d in decks]

    def by_hp(self, decks):
        """Win rate of every deck at each start-HP multiple: list (per deck) of lists (per multiple)."""
        per = [self.many(decks, int(self.hp * m)) for m in self.mults]
        return [[p[i][0] for p in per] for i in range(len(decks))]

    def smooth(self, decks, mults=None):
        """A graded objective that does not go flat when every deck loses: the win rate averaged over handicapped start HPs (x1 .. x3). A deck that is far
        from beating the target still wins with enough HP; the HP it needs is what picks reduce."""
        per = [self.many(decks, int(self.hp * m)) for m in (mults or self.mults)]
        return [float(np.mean([p[i][0] for p in per])) for i in range(len(decks))]


def run_policy(name, seq, sc, tau, beam, rng):
    deck = []
    if name == "skip":
        return deck
    if name == "random":
        for offer in seq:
            k = rng.randrange(4)
            if k < 3:
                deck.append(offer[k])
        return deck
    if name == "greedy":
        for offer in seq:
            cands = [deck] + [deck + [c] for c in offer]
            res = sc.many(cands)
            base_w, base_se = res[0][0], res[0][1]
            best = max(range(1, 4), key=lambda i: (res[i][0], -res[i][2]))
            gain = res[best][0] - base_w
            se = (res[best][1] ** 2 + base_se ** 2) ** 0.5
            if gain > tau * se or (gain >= 0 and res[best][2] < res[0][2] - 0.02 and tau == 0):
                deck = deck + [offer[best - 1]]
        return deck
    if name in ("greedy_h", "greedy_pick"):
        # greedy_h: pick the option with the best smooth objective (never skips unless every option is worse than the current deck)
        for offer in seq:
            cands = [deck] + [deck + [c] for c in offer]
            v = sc.smooth(cands)
            best = max(range(1, 4), key=lambda i: v[i])
            if v[best] >= v[0] - (0.0 if name == "greedy_h" else 1e9):
                deck = deck + [offer[best - 1]]
        return deck
    if name == "hindsight_h":
        beams = [([], 0.0)]
        for offer in seq:
            cands = []
            for d, _ in beams:
                cands.append(d)
                cands += [d + [c] for c in offer]
            cands = [list(x) for x in {tuple(sorted(c)): c for c in cands}.values()]
            v = sc.smooth(cands)
            scored = sorted(zip(cands, v), key=lambda t: -t[1])
            beams = [(d, x) for d, x in scored[:beam]]
        return beams[0][0]
    if name == "hindsight":
        beams = [([], 0.0)]
        for offer in seq:
            cands = []
            for d, _ in beams:
                cands.append(d)
                cands += [d + [c] for c in offer]
            cands = [list(x) for x in {tuple(sorted(c)): c for c in cands}.values()]
            res = sc.many(cands)
            scored = sorted(zip(cands, res), key=lambda t: (-t[1][0], t[1][2]))
            beams = [(d, r[0]) for d, r in scored[:beam]]
        return beams[0][0]
    raise ValueError(name)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", default="KNIGHTS_ELITE")
    ap.add_argument("--screens", type=int, default=6)
    ap.add_argument("--sequences", type=int, default=6)
    ap.add_argument("--policies", default="skip,random,greedy,hindsight")
    ap.add_argument("--attempts", type=int, default=128, help="solver attempts per deck during the search")
    ap.add_argument("--final-attempts", type=int, default=512, help="attempts for the final score of each policy's deck")
    ap.add_argument("--tau", type=float, default=0.0, help="greedy: pick only if the gain exceeds tau standard errors")
    ap.add_argument("--beam", type=int, default=6)
    ap.add_argument("--hp", type=int, default=80, help="start HP of the target fight (lower = harder)")
    ap.add_argument("--mults", default="1,1.5,2,3", help="start-HP multiples of the smooth objective (hard fights need larger ones: Aeonglass 1,2,3,4,6)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    from agent.engine import Engine
    eng = Engine()
    pool = card_pool()
    rng = random.Random(a.seed)
    policies = a.policies.split(",")
    rows = []
    t0 = time.time()
    for s in range(a.sequences):
        seq = make_sequence(rng, a.screens, pool)
        mults = [float(x) for x in a.mults.split(",")]
        sc = Scorer(eng, a.target, a.attempts, a.hp, mults)
        final = Scorer(eng, a.target, a.final_attempts, a.hp, mults)
        row = dict(sequence=seq, policies={})
        for p in policies:
            deck = run_policy(p, seq, sc, a.tau, a.beam, random.Random(a.seed * 1000 + s))
            w, se, hl = final.many([deck])[0]
            bh = final.by_hp([deck])[0]
            row["policies"][p] = dict(deck=deck, win=w, win_se=se, hp_lost=hl, picks=len(deck), evals=sc.calls, by_hp=bh, smooth=float(np.mean(bh)))
        rows.append(row)
        print(f"sequence {s + 1}/{a.sequences} ({time.time() - t0:.0f}s): " + "  ".join(f"{p} {row['policies'][p]['win']:.2f}/{row['policies'][p]['smooth']:.2f}" for p in policies) + "  (win at start HP / smooth)", flush=True)
        if a.out:
            json.dump(dict(target=a.target, screens=a.screens, rows=rows), open(os.path.join(ROOT, a.out), "w"))
    print("\nmean final win rate vs", a.target, "after", a.screens, "screens:")
    for p in policies:
        ws = [r["policies"][p]["win"] for r in rows]
        hl = [r["policies"][p]["hp_lost"] for r in rows]
        sm = [r["policies"][p]["smooth"] for r in rows]
        bh = np.mean([r["policies"][p]["by_hp"] for r in rows], axis=0)
        print(f"  {p:10s} win {np.mean(ws):.3f} (se {np.std(ws) / max(1, len(ws)) ** 0.5:.3f})  smooth {np.mean(sm):.3f} (se {np.std(sm) / max(1, len(sm)) ** 0.5:.3f})  hp lost {np.mean(hl):.2f}  picks {np.mean([r['policies'][p]['picks'] for r in rows]):.1f}"
              f"  win by start HP x{'/'.join(str(m) for m in mults)}: {' '.join(f'{x:.2f}' for x in bh)}")


if __name__ == "__main__":
    main()
