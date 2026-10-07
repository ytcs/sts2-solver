"""The event catalog (`data/events.json`) for the run model (`docs/rebuild.md` S5): every non-ancient event of the act and shared lists, its `IsAllowed`
condition and its options as effects in a small vocabulary, read from the decompiled game (`decomp/MegaCrit.Sts2.Core.Models.Events`, cited per entry).

`apply` plays an option's effects on a `runmodel.RunState` with simple rules for the choices the game leaves to the player (which card is removed,
upgraded, transformed or enchanted). Fights are not played here: they come back as requests the rollout plays through the predictor, then it applies
the request's `extra` effects (and the hallway rewards when `rewards == "hallway"`). Unmodelled effects (minigames, open-ended loops) are no-ops that
set a flag.
"""
import json
import os
import re

PATH = os.path.join(os.path.dirname(__file__), "..", "data", "events.json")
VOCAB = {"hp", "hp_frac", "max_hp", "max_hp_set", "gold", "gold_set", "card_add", "card_add_one_of", "card_add_random", "card_remove", "card_upgrade",
         "card_upgrade_random", "card_downgrade_random", "card_transform", "card_enchant", "card_duplicate_all", "relic", "relic_one_of", "relic_random",
         "relic_remove", "potion", "potion_random", "potion_remove", "fight", "choice", "chance", "unmodelled"}
PARAMS = {"fight": {"rewards", "extra"}, "choice": {"offered"}, "chance": {"then", "else"}}  # extra keys an effect of that kind carries
CARD_RARITIES = ("Common", "Uncommon", "Rare")
POTION_TO_CARD = {"Common": "Common", "Token": "Common", "Uncommon": "Uncommon", "Rare": "Rare", "Event": "Rare"}  # TheFutureOfPotions.cs:147-160
UNREMOVABLE = ("ASCENDERS_BANE",)
_CAT = None


def catalog():
    """{id: entry}, loaded once."""
    global _CAT
    if _CAT is None:
        with open(PATH, encoding="utf-8") as f:
            _CAT = {e["id"]: e for e in json.load(f)["events"]}
    return _CAT


def get(event):
    """An entry by class id (`DenseVegetation`), loc key (`DENSE_VEGETATION`) or English title (`Dense Vegetation`), else None."""
    cat = catalog()
    if event in cat:
        return cat[event]
    low = str(event).strip().lower()
    return next((e for e in cat.values() if e["key"].lower() == low or e["title"].lower() == low), None)


def options(event):
    """The top-level option labels, in the game's order ({Var} placeholders are filled at runtime)."""
    e = get(event)
    return [o["label"] for o in e["options"]] if e else []


def _pattern(label):
    parts = re.split(r"\{\w+\}", label)
    return re.compile(r"\s*" + r".+?".join(re.escape(p) for p in parts) + r"\s*$", re.I)


def match(event, labels):
    """Screen option texts (`Trudge On: Gain 90 Gold. Lose 8 HP.` or just the title) -> the catalog options, in screen order (None where nothing
    matches). Each catalog option is used once, so repeated labels (`Locked`) pair up in order."""
    e = get(event)
    if not e:
        return [None] * len(labels)
    free, out = list(e["options"]), []
    for text in labels:
        head = text.split(":", 1)[0].strip()
        o = next((o for o in free if _pattern(o["label"]).match(head) or _pattern(o["label"]).match(text.strip())), None)
        if o is not None:
            free.remove(o)
        out.append(o)
    return out


# ------------------------------------------------------------------------------------------------------------------------------------- conditions

def _card(cid):
    """The catalog row of a card id (any pool), or {}."""
    from agent import runmodel as RM
    for rows in RM.CAT["cards"].values():
        for r in rows:
            if r["id"] == cid:
                return r
    return {}


def _basic(c):
    return _card(c["id"]).get("rarity") == "Basic"


def allowed(event, st):
    """`IsAllowed` on a RunState (act index 0-based, as the game's CurrentActIndex). Deck-enchantability is approximated as true."""
    e = get(event)
    if not e or not e.get("allowed", True):
        return False
    c, deck = e["condition"], st.deck
    tradable = len(st.relics) - 1  # the starter relic is not tradable
    checks = {
        "min_act": lambda v: st.act >= v, "max_act": lambda v: st.act <= v, "act": lambda v: st.act == v,
        "min_gold": lambda v: st.gold >= v, "min_hp": lambda v: st.hp >= v, "max_hp_frac": lambda v: st.hp <= v * st.max_hp,
        "min_floor": lambda v: getattr(st, "floor", v) >= v, "min_potions": lambda v: len(st.potions) >= v, "min_tradable_relics": lambda v: tradable >= v,
        "min_removable": lambda v: sum(x["id"] not in UNREMOVABLE for x in deck) >= v,
        "min_transformable": lambda v: sum(x["id"] not in UNREMOVABLE for x in deck) >= v,
        "min_basic_strikes": lambda v: sum(x["id"].startswith("STRIKE_") for x in deck) >= v,
        "min_basic_defends": lambda v: sum(x["id"].startswith("DEFEND_") for x in deck) >= v,
        "has_basic": lambda v: any(_basic(x) for x in deck) == v,
        "no_event_pet": lambda v: (not any(x["id"] == "BYRDONIS_EGG" for x in deck)) == v,
    }
    return all(checks[k](v) for k, v in c.items() if k in checks)


# ------------------------------------------------------------------------------------------------------------------------------------- effects

def DEFER(st, cards, k):
    """A `pick` that hands card offers back to the caller (result["offers"]) instead of adding them."""
    return None


def first(st, cards, k):
    """The default `pick`: the first k offered (the offer is a random draw, so this is a random pick)."""
    return cards[:k]


def default_choose(st, opts):
    """A sub-choice: leave when the page has a way out (Give Up, Exit, Leave, Extract), else the first option that is fully modelled."""
    ex = next((i for i, o in enumerate(opts) if o.get("exit")), None)
    if ex is not None:
        return ex
    return next((i for i, o in enumerate(opts) if not unmodelled(o["effects"])), 0)


def unmodelled(effects):
    """True if any effect (at any depth) is unmodelled."""
    for e in effects:
        if "unmodelled" in e:
            return True
        if "choice" in e and any(unmodelled(o["effects"]) for o in e["choice"]):
            return True
        if "chance" in e and (unmodelled(e["then"]) or unmodelled(e["else"])):
            return True
        if "fight" in e and unmodelled(e["extra"]):
            return True
    return False


def _upgradable(c):
    return c.get("upgrade", 0) < _card(c["id"]).get("max_upgrade", 1)


def _pool_cards(pool, rarity=None, type=None, cost0=False, exclude=()):
    from agent import runmodel as RM
    rows = RM.CAT["cards"].get(pool, [])
    return [r["id"] for r in rows if r["rarity"] in CARD_RARITIES and (rarity is None or r["rarity"] == rarity) and (type is None or r.get("type") == type)
            and (not cost0 or (r.get("cost") == 0 and not r.get("x"))) and r["id"] not in exclude]


def _remove_targets(st, spec, rng):
    """Which cards a removal takes: curses first, then Strikes, then Defends, then the first card (a player's usual order)."""
    if isinstance(spec, int):
        spec = {"n": spec}
    deck = [c for c in st.deck if c["id"] not in UNREMOVABLE]
    if "match" in spec:
        deck = [c for c in deck if c["id"].startswith(spec["match"]) and (not spec.get("basic") or _basic(c))]
        return deck[:spec["n"]]
    if "random" in spec:
        pool = [c for c in deck if not _basic(c)] if spec["random"] == "non_basic" else deck
        pool = pool or deck
        return rng.sample(pool, min(spec["n"], len(pool)))
    rank = lambda c: (0 if _card(c["id"]).get("rarity") == "Curse" else 1 if c["id"].startswith("STRIKE_") else 2 if c["id"].startswith("DEFEND_") else 3)
    return sorted(deck, key=rank)[:spec["n"]]


def _transform_targets(st, n):
    """Basic Strikes, then Defends, then other basics, then the first non-curse card."""
    rank = lambda c: (0 if c["id"].startswith("STRIKE_") else 1 if c["id"].startswith("DEFEND_") else 2 if _basic(c) else 3)
    deck = [c for c in st.deck if c["id"] not in UNREMOVABLE and _card(c["id"]).get("rarity") not in ("Curse", "Status")]
    return sorted(deck, key=rank)[:n]


def _offer(st, draws, spec, pool):
    """`of` distinct cards [(id, upgrade)] for a card_add_random (default odds: the hallway rarity odds at the current offset, which an event does not
    move; `CardCreationOptions.cs:116-129`)."""
    out = []
    for _ in range(spec["of"]):
        rarity = spec.get("rarity")
        if rarity is None and spec.get("odds") != "uniform" and pool != "COLORLESS":
            rarity, _ = draws.card_rarity("hallway", st.offset)
        typ = spec.get("type")
        if typ == "random":
            typ = draws.rng.choice(("Attack", "Skill") if rarity == "Common" else ("Attack", "Skill", "Power"))
        taken = [c for c, _ in out]
        cands = _pool_cards(pool, rarity, typ, spec.get("cost0", False), taken) or _pool_cards(pool, None, typ, spec.get("cost0", False), taken)
        if cands:
            out.append((draws.rng.choice(cands), int(bool(spec.get("upgraded")))))
    return out


def apply(st, effects, draws, choose=None, pick=None):
    """Apply effects to `st` in place. `draws`: a `runmodel.Draws` (its rng drives every random outcome). `choose(st, options) -> index` for sub-choices
    (default `default_choose`); `pick(st, cards, k) -> chosen` for card offers (default `first`; `DEFER` returns them in result["offers"]).

    Returns {"fights": [{"fight", "rewards", "extra"}], "offers": [{"cards", "pick"}], "unmodelled": [text], "dead": bool}."""
    choose, pick = choose or default_choose, pick or first
    res = {"fights": [], "offers": [], "unmodelled": [], "dead": False}
    _apply(st, effects, draws, choose, pick, res, {})
    if res["unmodelled"]:
        st.unmodelled = True
    return res


def _apply(st, effects, draws, choose, pick, res, ctx):
    from agent import runmodel as RM
    rng = draws.rng
    for e in effects:
        if res["dead"]:
            return
        k = next(iter(e))  # the effect's kind is its first key; the rest are its PARAMS
        v = e[k]
        if k == "hp":
            st.hp = max(0, min(st.max_hp, st.hp + v))
        elif k == "hp_frac":
            st.hp = min(st.max_hp, st.hp + int(v * st.max_hp))
        elif k == "max_hp":
            if st.max_hp + v < 1:  # a max HP loss that reaches 0 kills (TabletOfTruth.cs LoseMaxHpAndUpgrade)
                st.hp = 0
            else:
                st.max_hp += v
                st.hp = min(st.max_hp, st.hp + v) if v > 0 else min(st.hp, st.max_hp)  # a gain heals as much (CreatureCmd.cs:847-853)
        elif k == "max_hp_set":
            st.max_hp, st.hp = v, min(st.hp, v)
        elif k == "gold":
            st.gold = max(0, st.gold + (rng.randint(*v) if isinstance(v, list) else v))
        elif k == "gold_set":
            st.gold = v
        elif k == "card_add":
            st.deck.append({"id": v, "upgrade": 0})
        elif k == "card_add_one_of":
            st.deck.append({"id": rng.choice(v), "upgrade": 0})
        elif k == "card_add_random":
            pool = {"character": draws.character, "colorless": "COLORLESS"}.get(v["pool"], v["pool"])
            spec = dict(v, rarity=POTION_TO_CARD.get(ctx.get("potion_rarity"), "Common")) if v.get("rarity") == "potion" else v
            cards = _offer(st, draws, spec, pool)
            chosen = pick(st, cards, v["pick"])
            if chosen is None:
                res["offers"].append({"cards": cards, "pick": v["pick"]})
            else:
                st.deck += [{"id": c, "upgrade": u} for c, u in chosen]
        elif k == "card_remove":
            for c in _remove_targets(st, v, rng):
                st.deck.remove(c)
        elif k == "card_upgrade":
            for _ in range(v):
                c = next((c for c in st.deck if _upgradable(c) and not _basic(c)), None) or next((c for c in st.deck if _upgradable(c)), None)
                if c is not None:
                    c["upgrade"] = c.get("upgrade", 0) + 1
        elif k == "card_upgrade_random":
            cands = [c for c in st.deck if _upgradable(c)]
            for c in (cands if v == "all" else rng.sample(cands, min(v, len(cands)))):
                c["upgrade"] = c.get("upgrade", 0) + 1
        elif k == "card_downgrade_random":
            cands = [c for c in st.deck if c.get("upgrade")]
            for c in rng.sample(cands, min(v, len(cands))):
                c["upgrade"] = 0
        elif k == "card_transform":
            spec = v if isinstance(v, dict) else {"n": v}
            targets = _transform_targets(st, spec["n"])
            if spec.get("basic"):
                targets = [c for c in targets if _basic(c)]
            for c in targets:
                # a card transforms into a random Common / Uncommon / Rare card of its own pool, never itself (`CardFactory.cs:168-200`)
                to = spec.get("to") or rng.choice(_pool_cards(draws.character, exclude=(c["id"],)))
                st.deck[st.deck.index(c)] = {"id": to, "upgrade": 0}
        elif k == "card_enchant":
            ok = lambda c: not c.get("enchantment") and _card(c["id"]).get("rarity") not in ("Curse", "Status") and \
                (v.get("type") is None or _card(c["id"]).get("type") == v["type"])
            cands = [c for c in st.deck if ok(c) and not _basic(c)] + [c for c in st.deck if ok(c) and _basic(c)]
            for c in cands[:v["n"]]:
                c["enchantment"] = {"id": v["id"], "amount": v["amount"]}
        elif k == "card_duplicate_all":
            st.deck += [dict(c) for c in st.deck]
        elif k == "relic":
            if v not in st.relic_ids():
                st.relics.append(v)
        elif k == "relic_one_of":
            st.relics.append(rng.choice(v))
        elif k == "relic_random":
            r = draws.relic(st.relic_ids(), v.get("rarity"))
            if r:
                st.relics.append(r)
        elif k == "relic_remove":
            if len(st.relics) > 1:
                st.relics.pop(rng.randrange(1, len(st.relics)))  # any but the starter
        elif k == "potion":
            if len(st.potions) < st.slots:
                st.potions.append(v)
        elif k == "potion_random":
            n, rarity = (v, None) if isinstance(v, int) else (v["n"], v.get("rarity"))
            for _ in range(n):
                if len(st.potions) < st.slots:
                    st.potions.append(draws.potion() if rarity is None else rng.choice([p["id"] for p in RM.pool("potions", draws.character, rarity)]))
        elif k == "potion_remove":
            i = rng.randrange(len(st.potions)) if v == "random" and st.potions else v if isinstance(v, int) and v < len(st.potions) else None
            if i is not None:
                pid = st.potions.pop(i)
                ctx["potion_rarity"] = next((p["rarity"] for rows in RM.CAT["potions"].values() for p in rows if p["id"] == pid), "Common")
        elif k == "fight":
            res["fights"].append({"fight": v, "rewards": e.get("rewards", "hallway"), "extra": e.get("extra", [])})
        elif k == "choice":
            opts = v
            if e.get("offered") and e["offered"] < len(opts):
                keep = sorted(rng.sample(range(len(opts)), e["offered"]))
                opts = [opts[i] for i in keep]
            _apply(st, opts[choose(st, opts)]["effects"], draws, choose, pick, res, ctx)
        elif k == "chance":
            _apply(st, e["then"] if rng.random() < v else e["else"], draws, choose, pick, res, ctx)
        elif k == "unmodelled":
            res["unmodelled"].append(v)
        else:
            raise ValueError(f"unknown event effect {k}")
        if st.hp <= 0:
            res["dead"] = True


def play_option(st, event, option, draws, choose=None, pick=None):
    """Apply one top-level option (index or label) of an event; a death ends the run at this event (`st.end`)."""
    e = get(event)
    opts = e["options"]
    o = opts[option] if isinstance(option, int) else next(x for x in opts if _pattern(x["label"]).match(option.split(":", 1)[0]))
    res = apply(st, o["effects"], draws, choose, pick)
    if res["dead"]:
        st.end = (st.act, "event", e["id"])
    return res
