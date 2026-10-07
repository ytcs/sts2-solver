"""The run model (`docs/rebuild.md` S5): what the rest of a run can hand out, sampled at the game's coded odds, so a macro choice is priced by paired
rollouts in one currency, P(win the run).

This module is the sampling layer: rewards, shops, potions, relics, unknown rooms and gold, each with its source in the decompiled game (`decomp/`,
summarised with citations in `docs/research/game_code.md` C). The public counters it starts from (potion chance, rare offset, unknown-room odds, removal
price) are `agent/tracker.py`'s. Exact pool order and per-relic `IsAllowed` checks are approximated: the draws have the game's distribution, not its seed.
"""
import json
import os
import random

import numpy as np

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



# ------------------------------------------------------------------------------------------------------------------------------------- rollouts

ACT_NAMES = {0: ("Overgrowth", "Underdocks"), 1: ("Hive",), 2: ("Glory",)}
# a typical route of a later act (rooms before the boss), the map's room mix (`game_code.md` C1): 6-7 rests, ~11 unknowns, 3 shops, 5-8 elites per map
TEMPLATE = {1: "M M ? M ? E R ? T M E ? $ R", 2: "M M ? M ? E R ? T M ? $ R"}
BASICS = ("STRIKE_", "DEFEND_", "ASCENDERS_BANE")
CURSES = {c["id"] for c in CAT["cards"].get("CURSE", [])}
WEAK_FIGHTS = {0: 3, 1: 2, 2: 2}


class RunState:
    """What a rollout carries: the run's public state. `base` is a fight scenario template (character, ascension, max energy, orb slots ...)."""

    def __init__(self, base, act, act_name, hp, max_hp, gold, deck, relics, potions, slots, counters, seen=None, bosses=(), frontier=None, nodes=None,
                 monsters=0):
        self.base, self.act, self.act_name, self.hp, self.max_hp, self.gold = base, act, act_name, hp, max_hp, gold
        self.deck, self.relics, self.potions, self.slots = [dict(c) for c in deck], list(relics), list(potions), slots
        self.potion_p, self.offset, self.unknown, self.removals = counters
        self.seen = {k: list(v) for k, v in (seen or {}).items()}
        self.bosses, self.frontier, self.nodes, self.monsters = list(bosses), frontier, nodes, monsters
        self.floors, self.start_act, self.end = 0, act, None  # rooms entered in the rollout; where it ended: (act, kind, encounter)

    def copy(self):
        s = RunState.__new__(RunState)
        s.__dict__.update({k: (v.copy() if isinstance(v, (list, dict)) else v) for k, v in self.__dict__.items()})
        s.deck = [dict(c) for c in self.deck]
        s.seen = {k: list(v) for k, v in self.seen.items()}
        if "events_seen" in self.__dict__:
            s.events_seen = set(self.events_seen)
        return s

    def relic_ids(self):
        return tuple(r if isinstance(r, str) else r["id"] for r in self.relics)

    def scenario(self, encounter, hp=None, deck=None, potions=None):
        pots = self.potions if potions is None else potions
        return dict(self.base, name=f"rm_{encounter}", encounter=encounter, act=self.act, hp=int(hp or self.hp), max_hp=self.max_hp, gold=self.gold,
                    deck=deck or self.deck, relics=[r if isinstance(r, dict) else {"id": r} for r in self.relics],
                    potions=[{"id": p, "slot": i} for i, p in enumerate(pots)], max_potion_slots=self.slots)

    def draw_encounter(self, kind, rng):
        """The next fight of a pool: the game deals each pool from a bag without repeats until it empties (`sts2-acts`, `ActModel.GenerateRooms`)."""
        from agent import pools
        P = pools.pool(self.act_name, kind)
        met = self.seen.setdefault(kind, [])
        k = len(met) % len(P)
        left = [e for e in P if e not in (met[-k:] if k else [])] or P
        e = rng.choice(left)
        met.append(e)
        return e


class BasePolicy:
    """The cheap policy a rollout follows after the priced choice (`docs/rebuild.md` S5); deterministic given the draws."""

    def node(self, st, options):
        """options: [(key, room type)] -> key. Low HP: rest, avoid elites; high HP: elites first; else treasure, unknowns and hallways."""
        f = st.hp / st.max_hp
        if f < 0.45:
            rank = {"R": 0, "?": 1, "T": 1, "$": 2, "M": 3, "E": 9}
        elif f > 0.75:
            rank = {"E": 0, "T": 1, "?": 2, "M": 3, "$": 4, "R": 5}
        else:
            rank = {"T": 0, "?": 1, "M": 2, "R": 3, "$": 4, "E": 6}
        return min(options, key=lambda o: rank.get(o[1], 5))[0]

    def event(self, st, entry):
        """An event option by a crude score of its effects (relics, removals, upgrades and max HP up; HP down weighs more at low HP; curses down)."""
        lowhp = st.hp < 0.4 * st.max_hp

        def score(effects):
            v = 0.0
            for e in effects:
                k, x = next(iter(e.items()))
                if k in ("relic", "relic_one_of", "relic_random"):
                    v += 3
                elif k == "card_remove":
                    v += 1.5 * (x if isinstance(x, int) else x.get("n", 1))
                elif k in ("card_upgrade", "card_upgrade_random"):
                    v += 1.0 * (x if isinstance(x, int) else 3)
                elif k == "max_hp":
                    v += 0.1 * x
                elif k in ("hp", "hp_frac"):
                    hp = x * st.max_hp if k == "hp_frac" else x
                    v += hp * (0.08 if lowhp else 0.03)
                elif k == "gold":
                    v += 0.01 * (sum(x) / 2 if isinstance(x, list) else x)
                elif k == "card_add" and str(x).upper() in CURSES:
                    v -= 2
                elif k == "fight":
                    v += -2 if lowhp else 0.5
                elif k == "unmodelled":
                    v -= 0.5
            return v
        opts = [o for o in entry["options"] if not o["key"].endswith("_LOCKED")]
        best = max(opts, key=lambda o: score(o["effects"])) if opts else None
        return entry["options"].index(best) if best else 0

    def potions(self, st, kind):
        """The belt allowed in this fight: all of it at elites and bosses, none in hallways."""
        return list(st.potions) if kind in ("elite", "boss") else []

    def rest(self, st):
        return "rest" if st.hp < 0.5 * st.max_hp else "smith"

    def smith(self, st):
        return next((c for c in st.deck if not c.get("upgrade") and not c["id"].startswith(BASICS)), None) or \
            next((c for c in st.deck if not c.get("upgrade") and c["id"] != "ASCENDERS_BANE"), None)

    def shop(self, st, items):
        """The removal of a basic card when affordable."""
        for kind, _id, price in items:
            basic = next((c for c in st.deck if c["id"].startswith(BASICS[:2])), None)
            if kind == "remove" and basic is not None and st.gold >= price:
                return [(kind, basic, price)]
        return []


def reference_fights(st, rng):
    """What a card pick is scored against: the act's boss at full HP and an elite of the act at 70%."""
    from agent import pools
    boss = st.bosses[0] if st.bosses else rng.choice(pools.pool(st.act_name, "boss"))
    return [(boss, st.max_hp), (rng.choice(pools.pool(st.act_name, "elite")), int(0.7 * st.max_hp))]


def worth(P, max_hp):
    """The linear worth of an ending distribution (the search's objective): -1 for a loss, 1 + 0.5 x end HP / max HP for a win."""
    import heads as H
    c = H.centers().numpy()
    return -P[..., 0] + (P[..., 1:] * (1 + 0.5 * np.minimum(c / max_hp, 1.0))).sum(-1)


def play(st, rng, pol, first=None):
    """One rollout as a generator: yields a list of fight scenarios and receives the predictor's [n, NC] for them; returns 1 for a won run, 0 for a
    death. `first(st, draws)`: the priced choice, applied to the state before the rollout starts (it may return an event's result to play out)."""
    import predictor as PR
    from agent import events as EV, pools
    dr = Draws(rng, st.base["character"], st.act)
    pre = first(st, dr) if first else None  # the priced choice; an event option returns its fights and whether it killed me

    def fight(kind, encounter):
        allowed = pol.potions(st, kind)
        P = yield [st.scenario(encounter, potions=allowed)]
        end = float(PR.sample_end(P, np.random.default_rng(rng.randrange(1 << 30)))[0])
        st.hp = int(min(end, st.max_hp))
        st.potions = [p for p in st.potions if p not in allowed]  # a potion allowed in a fight counts as spent (a lower bound on what is left)
        if end <= 0:
            st.end = (st.act, kind, encounter)
        return end > 0

    def rewards(kind):
        st.gold += dr.gold(kind)
        dropped, st.potion_p = dr.potion_drop(st.potion_p, kind == "elite")
        if dropped and len(st.potions) < st.slots:
            st.potions.append(dr.potion())
        cards, st.offset = dr.card_reward(kind, st.offset)
        refs = reference_fights(st, rng)
        decks = [st.deck] + [st.deck + [{"id": c, "upgrade": u}] for c, u in cards]
        P = yield [st.scenario(e, hp, deck=d, potions=[]) for d in decks for e, hp in refs]
        best = int(np.argmax(worth(P, st.max_hp).reshape(len(decks), len(refs)).mean(1)))
        if best:
            st.deck.append({"id": cards[best - 1][0], "upgrade": cards[best - 1][1]})
        if kind == "elite":
            r = dr.relic(st.relic_ids())
            if r:
                st.relics.append(r)

    def event_result(res):
        """The fights an event option started, with their rewards and follow-ups; False on a death."""
        if res.get("dead"):
            return False
        for f in res.get("fights", []):
            if not (yield from fight("hallway", f["fight"])):
                return False
            if f.get("rewards") == "hallway":
                yield from rewards("hallway")
            if EV.apply(st, f.get("extra", []), dr)["dead"]:
                return False
        return True

    def room(t):
        st.floors += 1
        if t == "?":
            u, st.unknown = dr.unknown(st.unknown)
            t = {"monster": "M", "treasure": "T", "shop": "$"}.get(u, "event")
        if t == "event":
            seen = st.__dict__.setdefault("events_seen", set())
            cands = [e for e in EV.catalog().values() if (st.act_name in e["acts"] or "shared" in e["acts"]) and e["id"] not in seen and EV.allowed(e["id"], st)]
            if cands:
                e = rng.choice(cands)
                seen.add(e["id"])
                if not (yield from event_result(EV.play_option(st, e["id"], pol.event(st, e), dr))):
                    return False
            return True
        if t in ("M", "E"):
            pool_kind = "elite" if t == "E" else ("weak" if st.monsters < WEAK_FIGHTS[st.act] else "regular")
            st.monsters += t == "M"
            kind = "elite" if t == "E" else "hallway"
            if not (yield from fight(kind, st.draw_encounter(pool_kind, rng))):
                return False
            yield from rewards(kind)
        elif t == "R":
            if pol.rest(st) == "rest":
                st.hp = min(st.max_hp, st.hp + int(HEAL_REST * st.max_hp))
            else:
                c = pol.smith(st)
                if c is not None:
                    c["upgrade"] = 1
        elif t == "$":
            for kind, card, price in pol.shop(st, dr.shop(st.offset, st.removals, st.relic_ids())):
                st.gold -= price
                if kind == "remove":
                    st.deck.remove(card)
                    st.removals += 1
        elif t == "T":
            st.gold += dr.gold("treasure")
            r = dr.relic(st.relic_ids())
            if r:
                st.relics.append(r)
        return True

    if isinstance(pre, dict) and not (yield from event_result(pre)):
        return 0
    while True:
        if st.nodes is not None and st.frontier:  # the rest of this act on its real map
            while st.frontier:
                key = pol.node(st, [(k, st.nodes[k]["type"]) for k in st.frontier])
                if not (yield from room(st.nodes[key]["type"])):
                    return 0
                st.frontier = [k for k in st.nodes[key]["children"] if k in st.nodes]
        else:
            for t in TEMPLATE.get(st.act, TEMPLATE[1]).split():
                if not (yield from room(t)):
                    return 0
        bosses = st.bosses or [rng.choice(pools.pool(st.act_name, "boss"))]
        if st.act == 2 and len(bosses) < 2:  # A10: a second boss from the same pool
            bosses = bosses + [rng.choice([b for b in pools.pool(st.act_name, "boss") if b not in bosses])]
        for b in bosses:
            if not (yield from fight("boss", b)):
                return 0
        if st.act == 2:
            st.end = (st.act, "won", None)
            return 1
        yield from rewards("boss")
        st.act += 1
        st.act_name, st.bosses, st.nodes, st.frontier, st.monsters, st.seen = ACT_NAMES[st.act][0], [], None, None, 0, {}
        st.hp += int(HEAL_ANCIENT * (st.max_hp - st.hp))
        st.unknown = dict(T.UNKNOWN_BASE)
        dr.act = st.act


class Rollouts:
    """Many rollouts in lockstep: each step batches every live rollout's fights into one predictor call."""

    def __init__(self, predictor, shuffles=4):
        self.pred, self.shuffles = predictor, shuffles

    def run(self, states, seeds, pol=None, firsts=None):
        """P(win the run) per rollout (1 / 0)."""
        pol = pol or BasePolicy()
        gens = [play(s, random.Random(sd), pol, f) for s, sd, f in zip(states, seeds, firsts or [None] * len(states))]
        out = np.full(len(gens), np.nan)
        pending = {}
        for i, g in enumerate(gens):
            try:
                pending[i] = next(g)
            except StopIteration as e:
                out[i] = e.value
        while pending:
            idx = list(pending)
            flat = [sc for i in idx for sc in pending[i]]
            P = self.pred.fight_start(flat, self.shuffles)
            nxt, o = {}, 0
            for i in idx:
                n = len(pending[i])
                try:
                    nxt[i] = gens[i].send(P[o:o + n])
                except StopIteration as e:
                    out[i] = e.value
                o += n
            pending = nxt
        return out
