import json
import os
import re

PATH = os.path.join(os.path.dirname(__file__), "..", "data", "events.json")
ANCIENT_PATH = os.path.join(os.path.dirname(__file__), "..", "data", "ancient_relics.json")
CHARACTERS = ("IRONCLAD", "SILENT", "DEFECT", "NECROBINDER", "REGENT")
CARD_RARITIES = ("Common", "Uncommon", "Rare")
POTION_TO_CARD = {"Common": "Common", "Token": "Common", "Uncommon": "Uncommon", "Rare": "Rare", "Event": "Rare"}
UNREMOVABLE = ("ASCENDERS_BANE",)
_CAT = None
_ANC = None


def catalog():
    global _CAT
    if _CAT is None:
        with open(PATH, encoding="utf-8") as f:
            _CAT = {e["id"]: e for e in json.load(f)["events"]}
    return _CAT


def get(event):
    cat = catalog()
    if event in cat:
        return cat[event]
    low = str(event).strip().lower()
    return next((e for e in cat.values() if e["key"].lower() == low or e["title"].lower() == low), None)


def options(event):
    e = get(event)
    return [o["label"] for o in e["options"]] if e else []


def _pattern(label):
    parts = re.split(r"\{\w+\}", label)
    return re.compile(r"\s*" + r".+?".join(re.escape(p) for p in parts) + r"\s*$", re.I)


def _pages(options):
    yield options
    for o in options:
        for eff in o["effects"]:
            if "choice" in eff:
                yield from _pages(eff["choice"])


def match(event, labels, options=None):
    e = get(event)
    if not e:
        return [None] * len(labels)
    free, out = list(e["options"] if options is None else options), []
    for text in labels:
        head = text.split(":", 1)[0].strip()
        hits = [o for o in free if _pattern(o["label"]).match(head) or _pattern(o["label"]).match(text.strip())]
        o = max(hits, key=lambda o: len(re.sub(r"\{\w+\}", "", o["label"]))) if hits else None
        if o is not None:
            free = [x for x in free if x is not o and {x["key"], o["key"]} != {o["key"].removesuffix("_LOCKED"), o["key"].removesuffix("_LOCKED") + "_LOCKED"}]
        out.append(o)
    return out


def _card(cid):
    from agent import runmodel as RM
    for rows in RM.CAT["cards"].values():
        for r in rows:
            if r["id"] == cid:
                return r
    return {}


def _basic(c):
    return _card(c["id"]).get("rarity") == "Basic"


def allowed(event, st):
    e = get(event)
    if not e or not e.get("allowed", True):
        return False
    c, deck = e["condition"], st.deck
    tradable = len(st.relics) - 1
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


def first(st, cards, k):
    return cards[:k]


def default_choose(st, opts):
    ex = next((i for i, o in enumerate(opts) if o.get("exit")), None)
    if ex is not None:
        return ex
    return next((i for i, o in enumerate(opts) if not unmodelled(o["effects"])), 0)


def unmodelled(effects):
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
    return [r["id"] for r in rows if (r["rarity"] in CARD_RARITIES if rarity is None else r["rarity"] == rarity) and (type is None or r.get("type") == type)
            and (not cost0 or (r.get("cost") == 0 and not r.get("x"))) and r["id"] not in exclude and not r.get("multiplayer_only")]


def _remove_targets(st, spec, rng):
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


def _matches(c, spec):
    m = spec.get("match")
    if m is not None and not c["id"].startswith(tuple(m) if isinstance(m, list) else m):
        return False
    return not spec.get("basic") or _basic(c)


def _count(st, n):
    return len(st.deck) if n == "all" else n


def _transform_targets(st, n):
    rank = lambda c: (0 if c["id"].startswith("STRIKE_") else 1 if c["id"].startswith("DEFEND_") else 2 if _basic(c) else 3)
    deck = [c for c in st.deck if c["id"] not in UNREMOVABLE and _card(c["id"]).get("rarity") not in ("Curse", "Status")]
    return sorted(deck, key=rank)[:n]


def _offer(st, draws, spec, pool):
    out = []
    for i in range(spec["of"]):
        p = pool[i % len(pool)] if isinstance(pool, list) else pool
        rarity = spec.get("rarity")
        if rarity is None and spec.get("odds") != "uniform" and p != "COLORLESS":
            rarity, _ = draws.card_rarity("hallway", st.offset)
        typ = spec.get("type")
        if typ == "random":
            typ = draws.rng.choice(("Attack", "Skill") if rarity == "Common" else ("Attack", "Skill", "Power"))
        taken = [c for c, _ in out] + list(spec.get("exclude", ()))
        cands = _pool_cards(p, rarity, typ, spec.get("cost0", False), taken) or _pool_cards(p, None, typ, spec.get("cost0", False), taken)
        if cands:
            out.append((draws.rng.choice(cands), int(bool(spec.get("upgraded")))))
    return out


def apply(st, effects, draws, choose=None, pick=None):
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
        k = next(iter(e))
        v = e[k]
        if k == "hp":
            st.hp = max(0, min(st.max_hp, st.hp + v))
        elif k == "hp_frac":
            st.hp = min(st.max_hp, st.hp + int(v * st.max_hp))
        elif k == "max_hp":
            if st.max_hp + v < 1:
                st.hp = 0
            else:
                st.max_hp += v
                st.hp = min(st.max_hp, st.hp + v) if v > 0 else min(st.hp, st.max_hp)
        elif k == "max_hp_set":
            st.max_hp, st.hp = v, min(st.hp, v)
        elif k == "gold":
            st.gold = max(0, st.gold + (rng.randint(*v) if isinstance(v, list) else v))
        elif k == "gold_set":
            st.gold = v
        elif k == "card_add":
            st.deck.append({"id": v.replace("{character}", draws.character), "upgrade": 0})
        elif k == "card_add_one_of":
            if "n" in e:
                st.deck += [{"id": c, "upgrade": 0} for c in rng.sample(v, min(e["n"], len(v)))]
            else:
                st.deck.append({"id": rng.choice(v), "upgrade": 0})
        elif k == "card_add_random":
            others = [c for c in CHARACTERS if c != draws.character]
            if v["pool"] == "others":
                pool = rng.sample(others, min(v["of"], len(others)))
            elif v["pool"] == "other":
                pool = ctx.setdefault("other_pool", rng.choice(others))
            else:
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
            spec = v if isinstance(v, dict) else {"n": v}
            for _ in range(spec["n"]):
                if "match" in spec or spec.get("basic"):
                    c = next((c for c in reversed(st.deck) if _upgradable(c) and _matches(c, spec)), None)
                else:
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
            if "map" in spec:
                for c in [c for c in st.deck if c["id"] in spec["map"]][:spec["n"]]:
                    st.deck[st.deck.index(c)] = dict(c, id=spec["map"][c["id"]])
                continue
            n = _count(st, spec["n"])
            if "match" in spec:
                targets = [c for c in _transform_targets(st, len(st.deck)) if _matches(c, spec)][:n]
            else:
                targets = _transform_targets(st, n)
                if spec.get("basic"):
                    targets = [c for c in targets if _basic(c)]
            for c in targets:
                to = spec.get("to") or rng.choice(_pool_cards(draws.character, exclude=(c["id"],)))
                st.deck[st.deck.index(c)] = {"id": to, "upgrade": int(bool(spec.get("upgraded")))}
        elif k == "card_enchant":
            ok = lambda c: not c.get("enchantment") and _card(c["id"]).get("rarity") not in ("Curse", "Status") and \
                (v.get("type") is None or _card(c["id"]).get("type") == v["type"]) and _matches(c, v)
            cands = [c for c in st.deck if ok(c) and not _basic(c)] + [c for c in st.deck if ok(c) and _basic(c)]
            n = _count(st, v["n"])
            if v.get("random"):
                cands = rng.sample(cands, min(n, len(cands)))
            for c in cands[:n]:
                c["enchantment"] = {"id": v["id"], "amount": v["amount"]}
        elif k == "card_duplicate_all":
            st.deck += [dict(c) for c in st.deck]
        elif k == "relic":
            if v not in st.relic_ids():
                st.relics.append(v)
        elif k == "relic_one_of":
            if "n" not in e and not e.get("pickup"):
                st.relics.append(rng.choice(v))
            else:
                cands = [r for r in v if r not in st.relic_ids()]
                for r in rng.sample(cands, min(e.get("n", 1), len(cands))):
                    st.relics.append(r)
                    if e.get("pickup") and r in ancient_relics():
                        _apply(st, ancient_relics()[r]["effects"], draws, choose, pick, res, ctx)
        elif k == "relic_random":
            r = draws.relic(st.relic_ids(), v.get("rarity"))
            if r:
                st.relics.append(r)
        elif k == "relic_replace":
            for i, r in enumerate(st.relics):
                if (r if isinstance(r, str) else r["id"]) in v:
                    st.relics[i] = v[r if isinstance(r, str) else r["id"]]
                    break
        elif k == "relic_remove":
            if len(st.relics) > 1:
                st.relics.pop(rng.randrange(1, len(st.relics)))
        elif k == "potion":
            if len(st.potions) < st.slots:
                st.potions.append(v)
        elif k == "potion_random":
            n, rarity = (v, None) if isinstance(v, int) else (v["n"], v.get("rarity"))
            for _ in range(n):
                if len(st.potions) < st.slots:
                    st.potions.append(draws.potion() if rarity is None else rng.choice([p["id"] for p in RM.pool("potions", draws.character, rarity)]))
        elif k == "potion_slots":
            st.slots += v
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


def match_page(event, labels):
    """the catalogued page (first page or a follow-up `choice`) whose options match the most labels"""
    e = get(event)
    if not e:
        return [None] * len(labels)
    heads = [t.split(":", 1)[0].strip() for t in labels]
    return max((match(event, labels, page) for page in _pages(e["options"])),
               key=lambda m: (sum(o is not None for o in m), sum(o is not None and o["label"] == h for o, h in zip(m, heads))))


def play_option(st, event, option, draws, choose=None, pick=None):
    e = get(event)
    o = option if isinstance(option, dict) else e["options"][option] if isinstance(option, int) else match(event, [option])[0]
    if o is None:
        raise ValueError(f"{e['id']}: no option matches {option!r}")
    res = apply(st, o["effects"], draws, choose, pick)
    if res["dead"]:
        st.end = (st.act, "event", e["id"])
    return res


def ancient_relics():
    global _ANC
    if _ANC is None:
        with open(ANCIENT_PATH, encoding="utf-8") as f:
            _ANC = json.load(f)["relics"]
    return _ANC


def _norm(text):
    return re.sub(r"[^a-z0-9]", "", str(text).lower())


def ancient_option(label):
    head = _norm(str(label).split(":", 1)[0])
    if not head:
        return None, None
    anc = ancient_relics()
    rid = next((r for r, e in anc.items() if head in (_norm(e["title"]), _norm(r))), None)
    if rid is None:
        hits = [r for r, e in anc.items() if _norm(e["title"]) in head]
        rid = max(hits, key=lambda r: len(anc[r]["title"])) if hits else None
    return (rid, anc[rid]["effects"]) if rid else (None, None)


def apply_ancient(st, relic_id, draws, choose=None, pick=None):
    e = ancient_relics()[relic_id]
    st.relics.append(relic_id)
    res = apply(st, e["effects"], draws, choose, pick)
    if res["dead"]:
        st.end = (st.act, "ancient", relic_id)
    return res
