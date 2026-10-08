#!/usr/bin/env python3
import argparse, json, os, random, sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fuzz_gen_mix as fg  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
CHAR_W = {"IRONCLAD": 30, "SILENT": 17.5, "DEFECT": 17.5, "NECROBINDER": 17.5, "REGENT": 17.5}
BELT_RELICS = {"POTION_BELT": 2}


UNMODELLED = {"FUR_COAT"}


def load_tables():
    cls = json.load(open(os.path.join(ROOT, "data", "relic_classes.json")))["relics"]
    anc = json.load(open(os.path.join(ROOT, "data", "ancients.json")))
    return cls, anc


class Curriculum:
    def __init__(self, catalog):
        self.g = fg.Gen(catalog)
        self.cls, anc = load_tables()
        ok = lambda rid: rid in self.g.have["relic"] and rid in self.cls and rid not in UNMODELLED and (self.cls[rid]["class"] in ("combat", "potion_linked") or "combat" in self.cls[rid].get("also", []))  # noqa: E731
        self.ok = ok
        self.by_act = {int(k): v for k, v in anc["by_act"].items()}
        self.ancients = anc["ancients"]
        self.weights = anc["ancient_weights"]
        self.upgrades = self.ancients["OROBAS"]["starter_upgrades"]
        cat = self.g.cat["relics"]
        self.pool = {ch: [x["id"] for x in cat.get(ch, []) + cat["SHARED"] if x["rarity"] in ("Common", "Uncommon", "Rare", "Shop", "Event")] for ch in CHAR_W}
        self.event = [x["id"] for x in cat["EVENT"] if x["rarity"] != "Starter"]
        self.encs = {}
        for e in self.g.encs:
            act = {0: 0, 1: 0, 2: 1, 3: 2}.get(e["act"])
            if act is not None:
                self.encs.setdefault(act, []).append(e)

    def relics(self, r, ch, act, easy):
        starter = fg.STARTERS[ch][1]
        has_starter = r.random() < 0.85
        if easy:
            return [starter] if has_starter and starter in self.g.have["relic"] else []
        drawn = []
        darv = False
        for a in range(1, act + 2):
            if a == 1:
                drawn.append(r.choice(self.by_act[1]))
                continue
            key = "2" if a == 2 else ("3_given_act2_darv" if darv else "3_given_act2_not_darv")
            names, w = zip(*self.weights[key].items())
            who = r.choices(names, w)[0]
            darv |= who == "DARV"
            opts = [x for x in self.ancients[who]["relics"] if x in self.by_act[a]]
            if opts:
                drawn.append(r.choice(opts))
        if "TOUCH_OF_OROBAS" in drawn:
            starter = self.upgrades.get(starter, starter)
        n = {0: r.randint(0, 3), 1: r.randint(2, 6), 2: r.randint(4, 9)}[act]
        pool = self.pool[ch] + self.event
        tries = 0
        while n > 0 and tries < 60:
            tries += 1
            x = r.choice(pool)
            if x not in drawn:
                drawn.append(x)
                n -= 1
        out = ([starter] if has_starter and starter in self.g.have["relic"] else []) + [x for x in drawn if self.ok(x) and x != starter]
        res = []
        for rid in out:
            props = self.g.relic_props.get(rid)
            if props and r.random() < 0.6:
                res.append({"id": rid, "props": {name: (r.random() < 0.5) if kind == "flag" else r.choice([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]) for name, kind in props}})
            else:
                res.append(rid)
        if res and r.random() < 0.3:
            r.shuffle(res)
        return res

    def potions(self, r, ch, relic_ids, pmax, easy):
        if easy:
            return [], 2
        slots = 2 + sum(BELT_RELICS.get(x, 0) for x in relic_ids)
        if r.random() < 0.1:
            slots = r.randint(slots, pmax)
        slots = min(slots, pmax)
        n = r.choices(range(slots + 1), [1] + [1.5] * (slots - 1) + [3])[0]
        pool = [p for p in self.g.potions[ch] + self.g.potions["SHARED"] if p["usage"] in ("CombatOnly", "AnyTime")]
        return [{"id": r.choice(pool)["id"], "slot": i} for i in range(n)], slots

    def scenario(self, i, seed, stage, pmax, character=None, act=None, rooms=None, n_add=None):
        r = random.Random(f"m3/{seed}/{i}")
        easy = stage == "easy"
        ch = character or r.choices(list(CHAR_W), list(CHAR_W.values()))[0]
        act = (0 if easy else r.choice([0, 1, 2])) if act is None else act
        encs = self.encs[act]
        if easy:
            encs = [e for e in encs if e["room"] not in ("Elite", "Boss")]
        if rooms:
            encs = [e for e in encs if e["room"] in rooms]
        w = [1 if e["weak"] else 4 if e["room"] in ("Elite", "Boss") else 3 for e in encs]
        enc = r.choices(encs, w)[0]
        _, _, hp0, energy, orbs = fg.STARTERS[ch]
        max_hp = hp0 + act * r.randint(5, 25) + r.randint(0, 15)
        focus = r.choices(["mix", "colorless", "junk", "gen", "turn"], [55, 15, 8, 12, 10])[0]
        deck = self.g.make_deck(r, ch, act, focus, upg_p=[0.15, 0.35, 0.5][act], enchant_p=0.03, n_add=n_add)
        if easy:
            deck = deck[:len(fg.STARTERS[ch][0]) + r.randint(0, 4)]
        elif r.random() < 0.3:
            other = r.choice([c for c in CHAR_W if c != ch])
            for _ in range(r.randint(1, 3)):
                c = self.g.pick_card(r, [x for x in self.g.cards[other] if x["rarity"] in ("Common", "Uncommon", "Rare")])
                deck.append(c["id"])
        relics = self.relics(r, ch, act, easy)
        rids = [x if isinstance(x, str) else x["id"] for x in relics]
        potions, slots = self.potions(r, ch, rids, pmax, easy)
        hp = int(round(max_hp * (r.uniform(0.75, 1.0) if easy else r.uniform(0.2, 1.0)))) or 1
        floor = {0: r.randint(1, 16), 1: r.randint(18, 33), 2: r.randint(35, 50)}[act]
        return {"name": f"m3_{seed}_{i}", "ascension": 10, "encounter": enc["id"], "character": ch, "hp": hp, "max_hp": max_hp, "max_energy": energy,
                "base_orb_slots": orbs, "max_potion_slots": slots, "gold": r.choice([0, 50, 99, 150, 300]), "seed": f"m3{seed}-{i}", "total_floor": floor,
                "act": act, "deck": deck, "relics": relics, "potions": potions, "meta": {"stage": stage, "focus": focus}}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--seed", default="31")
    ap.add_argument("--out", required=True)
    ap.add_argument("--stage", default="full", choices=["full", "easy"])
    ap.add_argument("--potions-max", type=int, default=8)
    ap.add_argument("--catalog", default=next((q for q in (os.path.join(ROOT, "target/train/catalog.json"), os.path.join(ROOT, "data/catalog.json")) if os.path.exists(q))))
    a = ap.parse_args()
    import sts2
    C = Curriculum(a.catalog)
    out, i, bad = [], 0, Counter()

    def valid(chunk):
        try:
            sts2.VecEnv(1, chunk, seed=0)
            return chunk
        except Exception as e:  # noqa: BLE001
            if len(chunk) == 1:
                bad[str(e)[:60]] += 1
                return []
            h = len(chunk) // 2
            return valid(chunk[:h]) + valid(chunk[h:])
    while len(out) < a.n:
        chunk = [C.scenario(i + k, a.seed, a.stage, a.potions_max) for k in range(min(2000, a.n - len(out)))]
        i += len(chunk)
        out += valid(chunk)
    json.dump(out, open(a.out, "w"))
    print(f"{len(out)} fights -> {a.out} (rejected {sum(bad.values())}: {dict(bad.most_common(4))}); characters {dict(Counter(s['character'] for s in out))}; "
          f"acts {dict(Counter(s['act'] for s in out))}; potions {dict(sorted(Counter(len(s['potions']) for s in out).items()))}; "
          f"relics mean {sum(len(s['relics']) for s in out) / len(out):.1f}")


if __name__ == "__main__":
    main()
