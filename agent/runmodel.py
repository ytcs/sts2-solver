"""The run model (`docs/rebuild.md` S5): what the rest of a run can hand out, sampled at the game's coded odds, so a macro choice is priced by paired
rollouts in one currency, P(win the run).

This module is the sampling layer: rewards, shops, potions, relics, unknown rooms and gold, each with its source in the decompiled game (`decomp/`,
summarised with citations in `docs/research/game_code.md` C). The public counters it starts from (potion chance, rare offset, unknown-room odds, removal
price) are `agent/tracker.py`'s. Exact pool order and per-relic `IsAllowed` checks are approximated: the draws have the game's distribution, not its seed.
"""
import json
import os
import random

from agent import tracker as T

CAT = json.load(open(os.path.join(os.path.dirname(__file__), "..", "data", "catalog.json")))
RARITIES = ("Common", "Uncommon", "Rare")
# card rarity per source at A7+ (Scarcity): rare base, uncommon base; the offset is added to the rare chance (`Odds/CardRarityOdds.cs:13-41, 96-111`)
CARD_ODDS = {"hallway": (0.0149, 0.37), "elite": (0.05, 0.40), "shop": (0.045, 0.37), "boss": (1.0, 0.0)}
UPGRADE_PER_ACT = 0.125  # non-rare reward cards upgrade with chance act index x 0.125 at A7+ (`Factories/CardFactory.cs:23, 283-305`)
GOLD = {"hallway": (10, 20), "elite": (35, 45), "boss": (100, 100), "treasure": (42, 52)}  # x0.75 at A3 Poverty (`Models/EncounterModel.cs:64-99`)
POVERTY = 0.75
RELIC_ODDS = (0.50, 0.33, 0.17)  # common / uncommon / rare for elites, chests and shops (`Factories/RelicFactory.cs:80-94`)
RELIC_PRICE = {"Common": 175, "Uncommon": 225, "Rare": 275, "Shop": 200}  # x U(0.85, 1.15) (`Models/RelicModel.cs:304-315`)
CARD_PRICE = {"Common": 50, "Uncommon": 75, "Rare": 150}  # x U(0.95, 1.05), colorless x1.15, one class card at half (`MerchantCardEntry.cs:38-51`)
POTION_PRICE = {"Common": 50, "Uncommon": 75, "Rare": 100}
POTION_RARITY = ((0.10, "Rare"), (0.35, "Uncommon"), (1.0, "Common"))  # cumulative (`Factories/PotionFactory.cs:76-95`)
HEAL_REST, HEAL_ANCIENT = 0.30, 0.80  # rest: 30% of max HP; an ancient: 80% of missing HP at A2+ (`AncientEventModel.cs:170-190`)


def pool(kind, character, rarity=None, types=None):
    """Card / relic / potion entries of a character's pool plus the shared one where the game adds it (relics and potions)."""
    src = CAT[kind]
    rows = list(src.get(character, []))
    if kind in ("relics", "potions"):
        rows += src.get("SHARED", [])
    return [r for r in rows if (rarity is None or r["rarity"] == rarity) and (types is None or r.get("type") in types)]


class Draws:
    """Samplers at the coded odds. `rng`: a `random.Random` (paired rollouts share one seed per option)."""

    def __init__(self, rng, character, act=0):
        self.rng, self.character, self.act = rng, character, act

    def card_rarity(self, source, offset):
        """(rarity, offset after this card). Only fight rewards move the offset; shops read it (`CardFactory.cs:50, 244-260`)."""
        rare, unc = CARD_ODDS[source]
        u = self.rng.random()
        r = "Rare" if u < rare + (offset if source != "boss" else 0) else "Uncommon" if u < rare + offset + unc else "Common"
        if source in ("hallway", "elite", "boss"):
            offset = T.OFFSET_START if r == "Rare" else min(T.OFFSET_CAP, offset + T.OFFSET_STEP)
        return r, offset

    def card_reward(self, source, offset, n=3, exclude=()):
        """n distinct cards [(id, upgrade)] and the new offset. An empty rarity falls to the next (`CardFactory.cs:263-281`)."""
        out, seen = [], set(exclude)
        for _ in range(n):
            r, offset = self.card_rarity(source, offset)
            for rr in RARITIES[RARITIES.index(r):] + RARITIES[:RARITIES.index(r)]:
                cands = [c["id"] for c in pool("cards", self.character, rr) if c["id"] not in seen]
                if cands:
                    cid = self.rng.choice(cands)
                    seen.add(cid)
                    up = int(rr != "Rare" and self.rng.random() < self.act * UPGRADE_PER_ACT)
                    out.append((cid, up))
                    break
        return out, offset

    def gold(self, source):
        lo, hi = GOLD[source]
        return int(self.rng.randint(lo, hi) * POVERTY)

    def potion_drop(self, chance, elite):
        """(dropped, chance after) (`Odds/PotionRewardOdds.cs:54-72`)."""
        dropped = self.rng.random() < chance + (T.POTION_ELITE if elite else 0)
        return dropped, chance + (-T.POTION_STEP if dropped else T.POTION_STEP)

    def potion(self):
        u = self.rng.random()
        rarity = next(r for c, r in POTION_RARITY if u <= c)
        return self.rng.choice([p["id"] for p in pool("potions", self.character, rarity)])

    def relic_rarity(self):
        u = self.rng.random()
        return "Common" if u < RELIC_ODDS[0] else "Uncommon" if u < RELIC_ODDS[0] + RELIC_ODDS[1] else "Rare"

    def relic(self, owned=(), rarity=None):
        rarity = rarity or self.relic_rarity()
        cands = [r["id"] for r in pool("relics", self.character, rarity) if r["id"] not in owned]
        return self.rng.choice(cands) if cands else None

    def shop(self, offset, removals, owned=()):
        """The stock [(kind, id, price)] (`Entities.Merchant/MerchantInventory.cs`): Attack, Attack, Skill, Skill, Power at shop odds (one at half
        price), a colorless Uncommon and Rare, three relics (two by rarity, one Shop tier), three potions, the removal."""
        items, seen = [], set()
        sale = self.rng.randrange(5)
        for i, ty in enumerate(("Attack", "Attack", "Skill", "Skill", "Power")):
            r, _ = self.card_rarity("shop", offset)
            for rr in RARITIES[RARITIES.index(r):] + RARITIES[:RARITIES.index(r)]:
                cands = [c["id"] for c in pool("cards", self.character, rr, (ty,)) if c["id"] not in seen]
                if cands:
                    cid = self.rng.choice(cands)
                    seen.add(cid)
                    price = CARD_PRICE[rr] * self.rng.uniform(0.95, 1.05) * (0.5 if i == sale else 1)
                    items.append(("card", cid, int(price)))
                    break
        for rr in ("Uncommon", "Rare"):
            cands = [c["id"] for c in CAT["cards"].get("COLORLESS", []) if c["rarity"] == rr]
            if cands:
                items.append(("card", self.rng.choice(cands), int(CARD_PRICE[rr] * 1.15 * self.rng.uniform(0.95, 1.05))))
        for rr in (self.relic_rarity(), self.relic_rarity(), "Shop"):
            rid = self.relic(owned + tuple(i[1] for i in items if i[0] == "relic"), rr)
            if rid:
                items.append(("relic", rid, int(RELIC_PRICE[rr] * self.rng.uniform(0.85, 1.15))))
        for _ in range(3):
            pid = self.potion()
            rar = next(p["rarity"] for p in pool("potions", self.character) if p["id"] == pid)
            items.append(("potion", pid, int(POTION_PRICE[rar] * self.rng.uniform(0.95, 1.05))))
        items.append(("remove", None, T.REMOVAL_BASE + T.REMOVAL_STEP * removals))
        return items

    def unknown(self, odds):
        """(room type, odds after) (`Odds/UnknownMapPointOdds.cs:140-175`): monster / treasure / shop, else an event."""
        u, acc, rolled = self.rng.random(), 0.0, "event"
        for t, p in odds.items():
            acc += p
            if u < acc:
                rolled = t
                break
        return rolled, {t: (T.UNKNOWN_BASE[t] if t == rolled else p + T.UNKNOWN_BASE[t]) for t, p in odds.items()}

