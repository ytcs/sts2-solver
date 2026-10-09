#!/usr/bin/env python3
"""One combat-loop round's fight list: a signal pool, plan-shaped decks, enabler pairs, big late-act decks at bosses, and forced-source slices
(player Intangible sources, expert/plan core cards, long Silent poison fights)."""
import argparse, json, os, random, sys, time
from collections import Counter

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import bench  # noqa: E402
import gen_curriculum as gc  # noqa: E402
import signal_pool as sp  # noqa: E402
from agent import plans  # noqa: E402

UPGRADE_P = (0.15, 0.35, 0.5)
OFFERED = ("Common", "Uncommon", "Rare")
SHAPED_HP = (0.45, 1.0)
CORE = {"NOXIOUS_FUMES": "SILENT", "PYRE": "IRONCLAD", "CRUELTY": "IRONCLAD", "SERPENT_FORM": "SILENT", "BURST": "SILENT", "BULLET_TIME": "SILENT",
        "CALCIFY": "NECROBINDER", "FURNACE": "REGENT", "DEFRAGMENT": "DEFECT"}
POISON = ["DEADLY_POISON", "POISONED_STAB", "BOUNCING_FLASK", "SNAKEBITE", "HAZE", "OUTBREAK", "BUBBLE_BUBBLE", "MIRAGE", "ACCELERANT"]


class Builder:
    def __init__(self, seed, catalog, potions_max):
        self.C = gc.Curriculum(catalog)
        self.seed, self.pmax = seed, potions_max
        self.r = random.Random(f"round/{seed}")
        self.n = 0
        self.max_upgrade = self.C.g.max_upgrade
        self.offered = {ch: [c for c in self.C.g.cards[ch] if c["rarity"] in OFFERED] for ch in gc.CHAR_W}
        self.plans = [p for p in plans.load() if sp.multiplayer_free([{"deck": p["core"] + p.get("support", [])}])]
        self.targets = {p["id"]: list(bench.plan_targets(p).items()) for p in self.plans}

    def generated(self, **kw):
        self.n += 1
        return self.C.scenario(self.n, f"round{self.seed}", "full", self.pmax, **kw)

    def loadout(self, **kw):
        sc = self.generated(**kw)
        return dict(sc, hp=max(1, round(sc["max_hp"] * self.r.uniform(*SHAPED_HP))))

    def fill(self, deck, ch, k):
        return deck + [self.C.g.pick_card(self.r, self.offered[ch])["id"] for _ in range(k)]

    def upgrade(self, deck, act):
        p = self.r.choice([0.0, UPGRADE_P[act]])
        return [{"id": c, "upgrade": 1} if self.max_upgrade.get(c, 0) > 0 and self.r.random() < p else c for c in deck]

    def plan_deck(self, p, kind, act):
        r = self.r
        enablers = p.get("enablers") or list(dict.fromkeys(p["core"]))
        if kind == "minus":
            version = f"{r.choice(['', 'core'])}-{r.choice(enablers)}"
            return version, self.upgrade(plans.deck(p, version), act)
        if kind == "fill":
            k = r.randint(2, 8)
            return f"core+{k}", self.upgrade(self.fill(plans.deck(p, "core"), p["character"], k), act)
        return kind, self.upgrade(plans.deck(p, kind), act)

    def plan_fight(self, p, kind):
        (act, _hp), encs = self.r.choice(self.targets[p["id"]])
        version, deck = self.plan_deck(p, kind, act)
        sc = self.loadout(character=p["character"], act=act)
        have = {bench._cid(x) for x in sc["relics"]}
        relics = sc["relics"] + [x for x in p.get("relics", []) if x not in have]
        return dict(sc, encounter=self.r.choice(encs), deck=deck, relics=relics, meta=dict(source="plan", plan=p["id"], version=version))

    def plan_variant(self, i):
        p = self.plans[i % len(self.plans)]
        sc = self.plan_fight(p, self.r.choices(["full", "core", "minus", "fill"], [3, 2, 3, 3])[0])
        return [dict(sc, name=f"round{self.seed}:plan:{i}")]

    def pair(self, i):
        r = self.r
        if i % 2 == 0:
            p = self.plans[(i // 2) % len(self.plans)]
            base = self.plan_fight(p, r.choice(["full", "fill"]))
            have = set(bench._deck_ids(base))
            card = r.choice([c for c in (p.get("enablers") or p["core"]) if c in have])
        else:
            for _ in range(20):
                base = self.loadout(act=r.choice([0, 1, 2]), rooms=("Elite", "Boss"))
                card = bench._enabler(base)
                if card:
                    break
            else:
                return []
            base["meta"] = dict(source="generated", focus=base["meta"]["focus"])
        name = f"round{self.seed}:pair:{i}"
        meta = dict(base["meta"], source="pair", pair=name, origin=base["meta"]["source"])
        return [dict(base, name=f"{name}:base", meta=dict(meta, enabler=None)),
                dict(bench._without(base, card), name=f"{name}:-{card}", meta=dict(meta, enabler=card))]

    def late(self, i, lo, hi):
        r = self.r
        size = r.randint(lo, hi)
        ch = r.choices(list(gc.CHAR_W), list(gc.CHAR_W.values()))[0]
        sc = self.loadout(character=ch, act=r.choice([1, 2]), rooms=("Boss",), n_add=size - len(gc.fg.STARTERS[ch][0]) - 1)
        deck = list(sc["deck"])
        while len(deck) > size:
            deck.pop(r.randrange(len(deck)))
        deck = self.fill(deck, ch, size - len(deck))
        return [dict(sc, name=f"round{self.seed}:late:{i}", deck=deck, meta=dict(source="late", size=size, focus=sc["meta"]["focus"]))]

    def forced(self, kind, i):
        r = self.r
        if kind == "intangible":
            src = r.choices(["WRAITH_FORM", "APPARITION", "GHOST_IN_A_JAR"], [3, 1, 1])[0]
            kw = dict(force_potions=[src]) if src == "GHOST_IN_A_JAR" else dict(force_cards=[src] * r.choice([1, 1, 2] if src == "WRAITH_FORM" else [1, 2, 3]))
            sc, meta = self.generated(character="SILENT" if src == "WRAITH_FORM" and r.random() < 0.5 else None, **kw), dict(card=src)
        elif kind == "core":
            card = list(CORE)[i % len(CORE)]
            sc, meta = self.generated(character=CORE[card], force_cards=[card] * r.choice([1, 1, 2])), dict(card=card)
        else:
            cards = (["NOXIOUS_FUMES"] if r.random() < 0.8 else []) + [r.choice(POISON) for _ in range(r.randint(3, 6))]
            sc, meta = self.loadout(character="SILENT", act=r.choice([1, 2]), rooms=("Elite", "Boss"), force_cards=cards), dict(cards=len(cards))
        return [dict(sc, name=f"round{self.seed}:{kind}:{i}", meta=dict(meta, source=kind, focus=sc["meta"]["focus"]))]


def constructs(sc, why):
    import sts2
    try:
        sts2.Sim(json.dumps(sc), 0)
        return True
    except Exception as ex:  # noqa: BLE001
        why[str(ex)[:60]] += 1
        return False


def build(make, n, why):
    """a pair stays whole or is dropped whole"""
    out, i, tries = [], 0, 0
    while len(out) < n and tries < 20 * n + 100:
        group = make(i)
        i += 1
        tries += len(group) or 1
        if group and len(sp.multiplayer_free(group)) == len(group) and all(constructs(sc, why) for sc in group):
            out += group
    return out[:n]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "round", "pool.json"))
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", json.load(open(os.path.join(ROOT, "models", "current.json")))["predictor"]),
                    help="predictor that scores the candidates (default: the live predictor, models/current.json)")
    ap.add_argument("--signal", type=int, default=90000, help="fights drawn from scored candidates (15%% uniform anchor, the rest by p(1-p))")
    ap.add_argument("--cand-mult", type=float, default=4.0, help="generated candidates per signal fight")
    ap.add_argument("--cands", nargs="*", default=[], help="extra candidate fight files (e.g. the main checkout's data/corpus/fights_train.json)")
    ap.add_argument("--plans", type=int, default=30000, help="plan-shaped decks (data/plans.json) vs the plan's act elites, bosses and threats")
    ap.add_argument("--pairs", type=int, default=8000, help="enabler pairs (2 fights each)")
    ap.add_argument("--late", type=int, default=14000, help="big decks at act 2-3 bosses")
    ap.add_argument("--late-size", type=int, nargs=2, default=[29, 35])
    ap.add_argument("--intangible", type=int, default=9000, help="decks carrying a player Intangible source (Wraith Form, Apparition, Ghost in a Jar)")
    ap.add_argument("--core", type=int, default=18000, help="normal decks of the card's character plus 1-2 copies of an expert/plan core card (CORE)")
    ap.add_argument("--poison", type=int, default=8000, help="Silent decks with 3-7 poison cards at act 2-3 elites and bosses, shaped HP")
    ap.add_argument("--anchor", type=float, default=0.15)
    ap.add_argument("--shuffles", type=int, default=4)
    ap.add_argument("--potions-max", type=int, default=8)
    ap.add_argument("--seed", type=int, default=5)
    ap.add_argument("--catalog", default=os.path.join(ROOT, "data", "catalog.json"))
    a = ap.parse_args()
    t0 = time.time()
    B = Builder(a.seed, a.catalog, a.potions_max)
    why = Counter()
    n_cand = int(a.signal * a.cand_mult)
    cands = build(lambda i: [dict(B.generated(), name=f"round{a.seed}:cand:{i}")], n_cand, why)
    for fn in a.cands:
        cands += [sc for sc in sp.multiplayer_free(json.load(open(fn))) if constructs(sc, why)]
    slices = dict(plans=build(B.plan_variant, a.plans, why), pairs=build(B.pair, 2 * a.pairs, why),
                  late=build(lambda i: B.late(i, *a.late_size), a.late, why),
                  **{k: build(lambda i, k=k: B.forced(k, i), getattr(a, k), why) for k in ("intangible", "core", "poison")})
    print(f"{len(cands)} candidates, {sum(map(len, slices.values()))} shaped fights built in {time.time() - t0:.0f}s; scoring with {os.path.basename(a.ckpt)}", flush=True)
    rest = [sc for s in slices.values() for sc in s]
    p = sp.score(a.ckpt, cands + rest, a.shuffles)
    pc, pr = p[:len(cands)], p[len(cands):]
    rng = np.random.default_rng(a.seed)
    sel = sp.pick(pc, min(a.signal, len(cands)), a.anchor, rng)
    pool = [dict(cands[i], meta=dict(cands[i].get("meta", {}), source="signal", p0=round(float(pc[i]), 4))) for i in sel]
    pool += [dict(sc, meta=dict(sc["meta"], p0=round(float(x), 4))) for sc, x in zip(rest, pr)]
    rng.shuffle(pool)
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    json.dump(pool, open(a.out, "w"))
    by = Counter(sc["meta"]["source"] for sc in pool)
    mean_p = {s: np.mean([sc["meta"]["p0"] for sc in pool if sc["meta"]["source"] == s]) for s in by}
    print(f"round pool: {len(pool)} fights | " + ", ".join(f"{s} {by[s]} (p0 {mean_p[s]:.2f})" for s in by)
          + f" | characters {dict(Counter(sc['character'] for sc in pool))} | acts {dict(sorted(Counter(sc['act'] for sc in pool).items()))}"
          + f" | deck size {np.mean([len(sc['deck']) for sc in pool]):.1f} | rejected {sum(why.values())} {dict(why.most_common(3))} | {time.time() - t0:.0f}s -> {a.out}")


if __name__ == "__main__":
    main()
