import functools
import json
import os
import random

import numpy as np

from agent import tracker as T

CAT = json.load(open(os.path.join(os.path.dirname(__file__), "..", "data", "catalog.json")))
RARITIES = ("Common", "Uncommon", "Rare")
CARD_ODDS = {"hallway": (0.0149, 0.37), "elite": (0.05, 0.40), "shop": (0.045, 0.37), "boss": (1.0, 0.0)}
UPGRADE_PER_ACT = 0.125
GOLD = {"hallway": (10, 20), "elite": (35, 45), "boss": (100, 100), "treasure": (42, 52)}
POVERTY = 0.75
RELIC_ODDS = (0.50, 0.33, 0.17)
RELIC_PRICE = {"Common": 175, "Uncommon": 225, "Rare": 275, "Shop": 200}
CARD_PRICE = {"Common": 50, "Uncommon": 75, "Rare": 150}
POTION_PRICE = {"Common": 50, "Uncommon": 75, "Rare": 100}
POTION_RARITY = ((0.10, "Rare"), (0.35, "Uncommon"), (1.0, "Common"))
HEAL_REST, HEAL_ANCIENT = 0.30, 0.80


@functools.cache
def pool(kind, character, rarity=None, types=None):
    src = CAT[kind]
    rows = list(src.get(character, []))
    if kind in ("relics", "potions"):
        rows += src.get("SHARED", [])
    return tuple(r for r in rows if (rarity is None or r["rarity"] == rarity) and (types is None or r.get("type") in types) and not r.get("multiplayer_only"))


class Draws:
    def __init__(self, rng, character, act=0):
        self.rng, self.character, self.act = rng, character, act

    def card_rarity(self, source, offset):
        rare, unc = CARD_ODDS[source]
        u = self.rng.random()
        r = "Rare" if u < rare + (offset if source != "boss" else 0) else "Uncommon" if u < rare + offset + unc else "Common"
        if source in ("hallway", "elite", "boss"):
            offset = T.OFFSET_START if r == "Rare" else min(T.OFFSET_CAP, offset + T.OFFSET_STEP)
        return r, offset

    def card_reward(self, source, offset, n=3, exclude=()):
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
            cands = [c["id"] for c in CAT["cards"].get("COLORLESS", []) if c["rarity"] == rr and not c.get("multiplayer_only")]
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
        u, acc, rolled = self.rng.random(), 0.0, "event"
        for t, p in odds.items():
            acc += p
            if u < acc:
                rolled = t
                break
        return rolled, {t: (T.UNKNOWN_BASE[t] if t == rolled else p + T.UNKNOWN_BASE[t]) for t, p in odds.items()}


ACT_NAMES = {0: ("Overgrowth", "Underdocks"), 1: ("Hive",), 2: ("Glory",)}
TEMPLATE = {1: "M M ? M ? E R ? T M E ? $ R", 2: "M M ? M ? E R ? T M ? $ R"}
BASICS = ("STRIKE_", "DEFEND_", "ASCENDERS_BANE")
CURSES = {c["id"] for c in CAT["cards"].get("CURSE", [])}
WEAK_FIGHTS = {0: 3, 1: 2, 2: 2}


class RunState:
    def __init__(self, base, act, act_name, hp, max_hp, gold, deck, relics, potions, slots, counters, seen=None, bosses=(), frontier=None, nodes=None,
                 monsters=0):
        self.base, self.act, self.act_name, self.hp, self.max_hp, self.gold = base, act, act_name, hp, max_hp, gold
        self.deck, self.relics, self.potions, self.slots = [dict(c) for c in deck], list(relics), list(potions), slots
        self.potion_p, self.offset, self.unknown, self.removals = counters
        self.seen = {k: list(v) for k, v in (seen or {}).items()}
        self.bosses, self.frontier, self.nodes, self.monsters = list(bosses), frontier, nodes, monsters
        self.floors, self.start_act, self.end = 0, act, None
        self.ready = self.ready_worth = None

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

    def scenario(self, encounter, hp=None, deck=None, potions=None, relics=None, act=None):
        pots = self.potions if potions is None else potions
        return dict(self.base, name=f"rm_{encounter}", encounter=encounter, act=self.act if act is None else act, hp=int(hp or self.hp), max_hp=self.max_hp, gold=self.gold,
                    deck=deck or self.deck, relics=[r if isinstance(r, dict) else {"id": r} for r in (self.relics if relics is None else relics)],
                    potions=[{"id": p, "slot": i} for i, p in enumerate(pots)], max_potion_slots=self.slots)

    def draw_encounter(self, kind, rng):
        from agent import pools
        P = pools.pool(self.act_name, kind)
        met = self.seen.setdefault(kind, [])
        k = len(met) % len(P)
        left = [e for e in P if e not in (met[-k:] if k else [])] or P
        e = rng.choice(left)
        met.append(e)
        return e


class BasePolicy:
    gates = False

    def node(self, st, options):
        f = st.hp / st.max_hp
        if f < 0.45:
            rank = {"R": 0, "?": 1, "T": 1, "$": 2, "M": 3, "E": 9}
        elif f > 0.75:
            rank = {"E": 0, "T": 1, "?": 2, "M": 3, "$": 4, "R": 5}
        else:
            rank = {"T": 0, "?": 1, "M": 2, "R": 3, "$": 4, "E": 6}
        return min(options, key=lambda o: rank.get(o[1], 5))[0]

    def event(self, st, entry):
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
        return list(st.potions) if kind in ("elite", "boss") else []

    def rest(self, st):
        return "rest" if st.hp < 0.5 * st.max_hp else "smith"

    def smith(self, st):
        return next((c for c in st.deck if not c.get("upgrade") and not c["id"].startswith(BASICS)), None) or \
            next((c for c in st.deck if not c.get("upgrade") and c["id"] != "ASCENDERS_BANE"), None)

    def shop(self, st, items, rng):
        buys, gold, deck = [], st.gold, list(st.deck)
        refs = reference_fights(st, rng)
        basic = next((c for c in deck if c["id"].startswith(BASICS[:2])), None)
        rm = next((it for it in items if it[0] == "remove"), None)
        if basic is not None and rm is not None and gold >= rm[2]:
            buys.append(("remove", basic, rm[2]))
            gold -= rm[2]
            deck.remove(basic)
        cands = [it for it in items if it[0] in ("card", "relic") and it[2] <= gold]
        if not cands:
            return buys
        variants = [(deck, st.relics)] + [(deck + [{"id": i, "upgrade": 0}], st.relics) if k == "card" else (deck, st.relics + [i]) for k, i, _ in cands]
        P = yield [st.scenario(e, hp, deck=d, potions=[], relics=r) for d, r in variants for e, hp in refs]
        best = int(np.argmax(worth(P, st.max_hp).reshape(len(variants), len(refs)).mean(1)))
        return buys + ([cands[best - 1]] if best else [])


def reference_fights(st, rng):
    from agent import pools
    boss = st.bosses[0] if st.bosses else rng.choice(pools.pool(st.act_name, "boss"))
    return [(boss, st.max_hp), (rng.choice(pools.pool(st.act_name, "elite")), int(0.7 * st.max_hp))]


def worth(P, max_hp):
    import heads as H
    c = H.CENTERS
    return -P[..., 0] + (P[..., 1:] * (1 + 0.5 * np.minimum(c / max_hp, 1.0))).sum(-1)


READY_W = {"boss": 0.5, "elite": 0.5}


def readiness(st, pol):
    import predictor as PR
    from agent import pools
    bosses, elites = pools.pool(st.act_name, "boss"), pools.pool(st.act_name, "elite")
    allowed = pol.potions(st, "boss")
    P = yield [st.scenario(b, potions=allowed) for b in bosses] + [st.scenario(e, potions=pol.potions(st, "elite")) for e in elites]
    Pb, Pe = P[:len(bosses)], P[len(bosses):]
    pb, wb = PR.p_win(Pb), worth(Pb, st.max_hp)
    if st.act == 2 and len(bosses) > 1:
        pairs = [(i, j) for i in range(len(bosses)) for j in range(len(bosses)) if i != j]
        h = PR.end_hp(Pb)
        left = [p for p in st.potions if p not in allowed]
        P2 = yield [st.scenario(bosses[j], hp=max(1, min(st.max_hp, round(float(h[i])))), potions=left) for i, j in pairs]
        p1 = np.array([pb[i] for i, _ in pairs])
        pb, wb = p1 * PR.p_win(P2), p1 * worth(P2, st.max_hp) - (1 - p1)
    w = READY_W
    return (float(w["boss"] * pb.mean() + w["elite"] * PR.p_win(Pe).mean()),
            float(w["boss"] * wb.mean() + w["elite"] * worth(Pe, st.max_hp).mean()))


LAST_ACT = max(ACT_NAMES)
RULES = ("late", "clip", "prod", "disc", "mean")
FLOOR, DISCOUNT = 0.05, 0.5


def gates(st, pol):
    """(act, P(win) vs its elite pool, P(clear its boss gate)) for st.act and every later act"""
    import predictor as PR
    from agent import pools
    out, hp = [], st.hp
    for j in range(st.act, LAST_ACT + 1):
        name = st.act_name if j == st.act else ACT_NAMES[j][0]
        if j > st.act:
            hp = min(st.max_hp, hp + int(HEAL_ANCIENT * (st.max_hp - hp)))
        known = st.bosses if j == st.act else []
        bosses, elites = list(known or pools.pool(name, "boss")), pools.pool(name, "elite")
        belt = pol.potions(st, "boss") if j == st.act else []
        double = j == LAST_ACT and len(bosses) > 1
        pairs = ([(0, 1)] if len(known) > 1 else [(a, b) for a in range(len(bosses)) for b in range(len(bosses)) if a != b]) if double else []
        firsts = sorted({a for a, _ in pairs}) if double else range(len(bosses))
        P = yield [st.scenario(e, hp=hp, potions=[], act=j) for e in elites] + [st.scenario(bosses[i], hp=hp, potions=belt, act=j) for i in firsts]
        pe, Pb = float(PR.p_win(P[:len(elites)]).mean()), P[len(elites):]
        pb1, h1 = dict(zip(firsts, PR.p_win(Pb))), dict(zip(firsts, PR.end_hp(Pb)))
        if double:
            P2 = yield [st.scenario(bosses[b], hp=max(1, min(st.max_hp, round(float(h1[a])))), potions=[], act=j) for a, b in pairs]
            pb = float(np.mean([pb1[a] * w for (a, _), w in zip(pairs, PR.p_win(P2))]))
        else:
            pb = float(np.mean(list(pb1.values())))
            hp = max(1, round(float(np.mean(list(h1.values())))))
        out.append((j, pe, pb))
    return out


def combine(g, rule="late", floor=FLOOR):
    p = np.array([x[1:] for x in g]).ravel()
    if rule == "mean":
        return float(p.mean())
    lo = np.full(len(p), floor if rule in ("late", "clip") else 1e-9)
    if rule == "late":
        lo[:2] = 1e-9
    w = np.repeat(DISCOUNT ** np.arange(len(g)) if rule == "disc" else np.ones(len(g)), 2)
    return float(np.exp((w * np.log(np.maximum(p, lo))).sum()))


def play(st, rng, pol, first=None):
    import predictor as PR
    from agent import events as EV, pools
    dr = Draws(rng, st.base["character"], st.act)
    act0 = st.act
    pre = first(st, dr) if first else None

    def fight(kind, encounter):
        allowed = pol.potions(st, kind)
        P = yield [st.scenario(encounter, potions=allowed)]
        end = float(PR.sample_end(P, np.random.default_rng(rng.randrange(1 << 30)))[0])
        st.hp = int(min(end, st.max_hp))
        st.potions = [p for p in st.potions if p not in allowed]
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
            for kind, item, price in (yield from pol.shop(st, dr.shop(st.offset, st.removals, st.relic_ids()), rng)):
                st.gold -= price
                if kind == "remove":
                    st.deck.remove(item)
                    st.removals += 1
                elif kind == "card":
                    st.deck.append({"id": item, "upgrade": 0})
                elif kind == "relic":
                    st.relics.append(item)
        elif t == "T":
            st.gold += dr.gold("treasure")
            r = dr.relic(st.relic_ids())
            if r:
                st.relics.append(r)
        return True

    if isinstance(pre, dict) and not (yield from event_result(pre)):
        return 0
    while True:
        if st.nodes is not None and st.frontier is not None:
            while st.frontier:
                key = pol.node(st, [(k, st.nodes[k]["type"]) for k in st.frontier])
                if not (yield from room(st.nodes[key]["type"])):
                    return 0
                st.frontier = [k for k in st.nodes[key]["children"] if k in st.nodes]
        else:
            for t in TEMPLATE.get(st.act, TEMPLATE[1]).split():
                if not (yield from room(t)):
                    return 0
        if pol.gates and st.act == act0:
            st.gates = yield from gates(st, pol)
        bosses = st.bosses or [rng.choice(pools.pool(st.act_name, "boss"))]
        if st.act == 2 and len(bosses) < 2:
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
        if st.act == act0 + 1:
            st.ready, st.ready_worth = yield from readiness(st, pol)


class Rollouts:
    def __init__(self, predictor, shuffles=4, seed=1000):
        self.pred, self.shuffles, self.seed = predictor, shuffles, seed

    def run(self, states, seeds, pol=None, firsts=None):
        pol = pol or BasePolicy()
        return self.drive([play(s, random.Random(sd), pol, f) for s, sd, f in zip(states, seeds, firsts or [None] * len(states))])

    def drive(self, gens):
        out = [None] * len(gens)
        pending = {}
        for i, g in enumerate(gens):
            try:
                pending[i] = next(g)
            except StopIteration as e:
                out[i] = e.value
        while pending:
            idx = list(pending)
            flat = [sc for i in idx for sc in pending[i]]
            P = self.pred.fight_start(flat, self.shuffles, self.seed)
            nxt, o = {}, 0
            for i in idx:
                n = len(pending[i])
                try:
                    nxt[i] = gens[i].send(P[o:o + n])
                except StopIteration as e:
                    out[i] = e.value
                o += n
            pending = nxt
        return out if any(not isinstance(x, (int, float)) for x in out) else np.array(out, float)
