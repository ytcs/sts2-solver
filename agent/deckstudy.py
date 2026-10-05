"""Deck-building approaches compared on one hard target: final deck quality against compute (simulator evaluations, fights, seconds).

    python -m agent.deckstudy --target AEONGLASS_BOSS --screens 20 --sequences 6 --policies skip,greedy,greedy_pick,greedy_h,mock_h,rollout_tag:4,elite_freq,hindsight_h \
        --attempts 64 --final-attempts 256 --mults 1,2,3,4,6 --out evals/ds_aeonglass_a.json
    python -m agent.deckstudy --report evals/ds_aeonglass_a.json evals/ds_aeonglass_b.json     # merge result files and print the comparison table

The task (as in `agent.deckbench`): the starter deck, `--screens` card-reward screens of 3 random Ironclad cards (rarities 60/37/3%) plus skip, the same sequences for
every approach (paired). The score of a final deck is the solver's win rate against the target averaged over start HP x`--mults` ("smooth"; Aeonglass needs
1,2,3,4,6), at `--final-attempts`. Compute is counted per approach with its OWN evaluation cache, so nothing is free because another approach ran first.

Approaches
  skip             never picks
  greedy           the old rule: best plain win rate now, skip when no gain
  greedy_pick      the smooth objective, always picks the best option
  greedy_h         the smooth objective, skips only if every option is worse than the current deck
  mock_h           greedy_h plus a completion bonus: each option is also priced together with the bucket-complement cards the pool offers (tags only choose them), mixed
                   in by the chance of meeting them in the screens left (the "worse now, better later" hypothesis, cheap version)
  rollout_tag:R    each option is valued by the mean final smooth score of R sampled futures, continued by a cheap bucket-deficiency picker (always picks)
  rollout_rand:R   the same with a random continuation (how much does the continuation matter)
  elite_freq       a genetic search over complete 20-card decks (offline, once per target) gives how often each card is in the best decks; online the offered card
                   with the highest frequency (diminishing with copies) is taken: no simulator call at pick time
  hindsight_h      beam search that knows the whole sequence (reference: not available in play)
and, once per target, `ga_ceiling`: the best complete deck of the genetic search (any cards, no offers): what 20 added cards can reach at all.
"""
import argparse
import collections
import json
import os
import random
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from agent import card_tags, deckbench  # noqa: E402
from agent.deckbench import STARTER, Scorer, card_pool, make_sequence, run_policy  # noqa: E402

ROOT = deckbench.ROOT
RARITY_W = {"Common": 0.0, "Uncommon": 0.1, "Rare": 0.2}


def _rarity_of(pool):
    return {c: r for r, cs in pool.items() for c in cs}


def _line(deck, tags):
    ids = [c for c, k in STARTER for _ in range(k)] + [x.rstrip("+") for x in deck if not x.startswith("@")]
    return card_tags.deck_line(ids, tags)


def tag_pick(deck, offer, tags, rar, rng):
    """Cheap continuation: the offered card that fills the biggest bucket deficiency (a small bonus for rarity, random tie-break). Always picks."""
    gaps = card_tags.deficiencies(_line(deck, tags))
    best = max(offer, key=lambda c: (sum(gaps.get(b, 0) for b in tags.get(c, {}).get("buckets", [])) + RARITY_W.get(rar.get(c), 0.0) + rng.random() * 1e-3))
    return best


# ------------------------------------------------------------------------------------------------------------------------------ policies

def run_rollout(seq, sc, R, cont, rng, tags, rar, pool):
    deck = []
    for si, offer in enumerate(seq):
        rem = len(seq) - si - 1
        futs = [make_sequence(random.Random(rng.randrange(1 << 30)), rem, pool) for _ in range(R)]
        cands = [None] + list(offer)
        finals = []
        for c in cands:
            d0 = deck if c is None else deck + [c]
            fin = []
            for fut in futs:
                d = list(d0)
                fr = random.Random(rng.randrange(1 << 30))
                for o in fut:
                    d.append(tag_pick(d, o, tags, rar, fr) if cont == "tag" else fr.choice(o))
                fin.append(d)
            finals.append(fin)
        flat = [d for f in finals for d in f]
        v = sc.smooth(flat)
        means = [float(np.mean(v[i * R:(i + 1) * R])) for i in range(len(cands))]
        best = 1 + int(np.argmax(means[1:]))
        if means[best] >= means[0]:  # skip only when every option is worse (as greedy_h)
            deck = deck + [cands[best]]
    return deck


def run_mock(seq, sc, tags, rar, pool, k=3):
    """greedy_h whose option value is (1-p) * S(deck + c) + p * S(deck + c + k complement cards), p = chance to meet the complement in the screens left."""
    deck = []
    allc = [c for cs in pool.values() for c in cs]
    for si, offer in enumerate(seq):
        rem = len(seq) - si - 1
        cands = [deck] + [deck + [c] for c in offer]
        comp = []
        for d in cands:
            gaps = card_tags.deficiencies(_line(d, tags))
            need = []
            for b in sorted(gaps, key=lambda b: -gaps[b]):
                extra = [c for c in card_tags.candidates(tags, b) if c in allc and c not in d and c not in need]
                need += extra[:1]
            need = need[:k]
            n_match = len({c for b in gaps for c in card_tags.candidates(tags, b) if c in allc})
            p = min(1.0, rem * 3 * (n_match / max(1, len(allc))) / max(1, len(need)))
            comp.append((d + need, p if need else 0.0))
        v_now = sc.smooth(cands)
        v_comp = sc.smooth([c for c, _ in comp])
        v = [(1 - p) * a + p * b for a, b, (_, p) in zip(v_now, v_comp, comp)]
        best = max(range(1, 4), key=lambda i: v[i])
        if v[best] >= v[0]:
            deck = deck + [offer[best - 1]]
    return deck


def ga_search(sc, pool, tags, size, pop, gens, rng, max_rare=2, max_copies=2):
    """Offline genetic search over complete decks of `size` added cards (any cards of the pool, at most `max_rare` rares, `max_copies` of a card). Heuristic mutation:
    half of the time the new card fills a bucket deficiency of the deck (the human move of the literature), else a random pool card. Returns (history of
    (deck, score) of every generation, the best deck, its score)."""
    allc = [c for cs in pool.values() for c in cs]
    rar = _rarity_of(pool)

    def ok(d):
        return sum(rar.get(c) == "Rare" for c in d) <= max_rare and all(n <= max_copies for n in collections.Counter(d).values())

    def rand_deck():
        while True:
            d = rng.sample(allc, size)
            if ok(d):
                return d

    def mutate(d):
        d = list(d)
        for _ in range(rng.randint(1, 3)):
            i = rng.randrange(len(d))
            if rng.random() < 0.5:
                gaps = card_tags.deficiencies(_line(d, tags))
                if gaps:
                    b = rng.choice(list(gaps))
                    cs = [c for c in card_tags.candidates(tags, b) if c in allc]
                    if cs:
                        d[i] = rng.choice(cs[:12])
                        continue
            d[i] = rng.choice(allc)
        return d if ok(d) else None

    def cross(a, b):
        pool_ = a + b
        rng.shuffle(pool_)
        d = pool_[:size]
        return d if ok(d) else None

    population = [rand_deck() for _ in range(pop)]
    history = []
    for g in range(gens):
        scores = sc.smooth(population)
        ranked = sorted(zip(population, scores), key=lambda t: -t[1])
        history.append(ranked)
        elite = [d for d, _ in ranked[: pop // 4]]
        nxt = list(elite)
        while len(nxt) < pop:
            child = cross(rng.choice(elite), rng.choice(elite)) if rng.random() < 0.4 else mutate(rng.choice(elite))
            if child:
                nxt.append(child)
        population = nxt
    final = sorted(zip(population, sc.smooth(population)), key=lambda t: -t[1])
    history.append(final)
    return history, final[0][0], final[0][1]


def card_frequencies(history, top=6, last=4):
    """How often each card is in the best decks of the last generations (weighted by rank)."""
    f = collections.Counter()
    for ranked in history[-last:]:
        for r, (d, _) in enumerate(ranked[:top]):
            for c in d:
                f[c] += 1.0 - 0.5 * r / max(1, top - 1)
    return f


def run_elite_freq(seq, freq, rng):
    deck = []
    for offer in seq:
        have = collections.Counter(deck)
        deck.append(max(offer, key=lambda c: (freq.get(c, 0.0) / (1 + have[c]), rng.random())))
    return deck


# ------------------------------------------------------------------------------------------------------------------------------ study

def run(a):
    from agent.engine import Engine
    eng = Engine()
    pool = card_pool()
    rar = _rarity_of(pool)
    tags = card_tags.load()
    mults = [float(x) for x in a.mults.split(",")]
    rng = random.Random(a.seed)
    policies = a.policies.split(",")
    out = dict(target=a.target, screens=a.screens, mults=mults, args=vars(a), rows=[], offline={})
    final = Scorer(eng, a.target, a.final_attempts, a.hp, mults)
    freq = None
    if "elite_freq" in policies or a.ga_ceiling:
        t0 = time.time()
        sc = Scorer(eng, a.target, a.ga_attempts, a.hp, mults)
        hist, best, score = ga_search(sc, pool, tags, a.screens, a.ga_pop, a.ga_gens, random.Random(a.seed + 77))
        freq = card_frequencies(hist)
        w, se, hl = final.many([best])[0]
        bh = final.by_hp([best])[0]
        out["offline"] = dict(ga_secs=time.time() - t0, ga_evals=sc.calls, ga_fights=sc.calls * a.ga_attempts, ceiling=dict(deck=best, smooth_search=score, by_hp=bh, smooth=float(np.mean(bh))),
                              freq={k: round(v, 2) for k, v in freq.most_common(25)})
        print(f"GA (offline): {time.time() - t0:.0f}s, {sc.calls} evals; best complete deck smooth {float(np.mean(bh)):.3f} by HP {[round(x, 2) for x in bh]}; top cards {freq.most_common(8)}", flush=True)
    for s in range(a.sequences):
        seq = make_sequence(rng, a.screens, pool)  # the same sequences for every process with the same --seed / --screens
        row = dict(sequence=seq, policies={})
        for p in policies:
            sc = Scorer(eng, a.target, a.rollout_attempts if p.startswith("rollout") else a.attempts, a.hp, mults)
            prng = random.Random(a.seed * 1000 + s)
            t0 = time.time()
            if p.startswith("rollout"):
                kind, R = p.split(":")
                deck = run_rollout(seq, sc, int(R), "tag" if kind == "rollout_tag" else "rand", prng, tags, rar, pool)
            elif p == "mock_h":
                deck = run_mock(seq, sc, tags, rar, pool)
            elif p == "elite_freq":
                deck = run_elite_freq(seq, freq, prng)
            else:
                deck = run_policy(p, seq, sc, 0.0, a.beam, prng, 0)
            secs = time.time() - t0
            bh = final.by_hp([deck])[0]
            row["policies"][p] = dict(deck=deck, picks=len(deck), secs=secs, evals=sc.calls, fights=sc.calls * sc.attempts, by_hp=bh, smooth=float(np.mean(bh)))
        out["rows"].append(row)
        print(f"sequence {s + 1}/{a.sequences}: " + "  ".join(f"{p} {row['policies'][p]['smooth']:.2f} ({row['policies'][p]['secs']:.0f}s)" for p in policies), flush=True)
        json.dump(out, open(os.path.join(ROOT, a.out), "w"))
    report([os.path.join(ROOT, a.out)])


def report(paths):
    docs = [json.load(open(p)) for p in paths]
    mults = docs[0]["mults"]
    n = min(len(d["rows"]) for d in docs)
    pol = {}
    for d in docs:
        for p in next(iter(d["rows"]), {"policies": {}})["policies"]:
            pol[p] = [r["policies"][p] for r in d["rows"][:n]]
    off = {}
    for d in docs:
        off.update(d.get("offline", {}))
    ref = pol.get("greedy_h")
    print(f"\n{n} paired sequences; smooth = win averaged over start HP x{'/'.join(str(m) for m in mults)}")
    print(f"{'approach':16s} {'smooth':>7s} {'se':>5s} {'vs greedy_h':>16s} " + " ".join(f"x{m:g}".rjust(5) for m in mults) + f" {'picks':>5s} {'evals':>7s} {'kfights':>8s} {'secs':>7s}")
    for p, rows in pol.items():
        sm = np.array([r["smooth"] for r in rows])
        bh = np.mean([r["by_hp"] for r in rows], axis=0)
        diff = ""
        if ref and p != "greedy_h":
            dd = sm - np.array([r["smooth"] for r in ref])
            diff = f"{dd.mean():+.3f} +- {dd.std(ddof=1) / n ** 0.5 if n > 1 else 0:.3f}"
        print(f"{p:16s} {sm.mean():7.3f} {sm.std(ddof=1) / n ** 0.5 if n > 1 else 0:5.3f} {diff:>16s} " + " ".join(f"{x:5.2f}" for x in bh)
              + f" {np.mean([r['picks'] for r in rows]):5.1f} {np.mean([r['evals'] for r in rows]):7.0f} {np.mean([r['fights'] for r in rows]) / 1000:8.1f} {np.mean([r['secs'] for r in rows]):7.0f}")
    if off:
        c = off["ceiling"]
        print(f"{'ga_ceiling':16s} {c['smooth']:7.3f} {'':5s} {'(offline)':>16s} " + " ".join(f"{x:5.2f}" for x in c["by_hp"]) + f" {'':5s} {off['ga_evals']:7d} {off['ga_fights'] / 1000:8.1f} {off['ga_secs']:7.0f}")
        print("   (elite_freq pays the GA once per target; the ceiling is a complete deck of any cards, not reachable from the offers)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", default="AEONGLASS_BOSS")
    ap.add_argument("--screens", type=int, default=20)
    ap.add_argument("--sequences", type=int, default=6)
    ap.add_argument("--policies", default="skip,greedy,greedy_pick,greedy_h")
    ap.add_argument("--attempts", type=int, default=64)
    ap.add_argument("--rollout-attempts", type=int, default=32)
    ap.add_argument("--final-attempts", type=int, default=256)
    ap.add_argument("--beam", type=int, default=3)
    ap.add_argument("--hp", type=int, default=80)
    ap.add_argument("--mults", default="1,2,3,4,6")
    ap.add_argument("--ga-pop", type=int, default=16)
    ap.add_argument("--ga-gens", type=int, default=10)
    ap.add_argument("--ga-attempts", type=int, default=32)
    ap.add_argument("--ga-ceiling", action="store_true", help="also run the genetic search and report its best complete deck")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", default="evals/ds.json")
    ap.add_argument("--report", nargs="*")
    a = ap.parse_args()
    if a.report:
        return report(a.report)
    run(a)


if __name__ == "__main__":
    main()
