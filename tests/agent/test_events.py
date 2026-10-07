"""The event catalog (`data/events.json`) and `agent/events.py`: every entry parses, every effect is in the vocabulary and names real ids, and a few
known events applied to a toy RunState do what the decompiled code does (no network, no GPU)."""
import json
import random

from agent import events as EV
from agent import runmodel as RM

DECK = [{"id": "STRIKE_IRONCLAD", "upgrade": 0}] * 5 + [{"id": "DEFEND_IRONCLAD", "upgrade": 0}] * 4 + [{"id": "BASH", "upgrade": 0},
                                                                                                      {"id": "INFLAME", "upgrade": 0}]


def toy(hp=40, max_hp=80, gold=99):
    return RM.RunState({"character": "IRONCLAD", "ascension": 10}, 0, "Overgrowth", hp, max_hp, gold, DECK, ["BURNING_BLOOD"], [], 3,
                       (0.4, 0.0, {}, 0))


def walk(effects):
    for e in effects:
        yield e
        for o in e.get("choice", []):
            yield from walk(o["effects"])
        for key in ("then", "else", "extra"):
            yield from walk(e.get(key, []))


def test_events_catalog():
    with open(EV.PATH, encoding="utf-8") as f:
        raw = json.load(f)
    cat = EV.catalog()
    assert len(cat) == len(raw["events"]) == 57
    ids = {kind: {r["id"] for rows in RM.CAT[kind].values() for r in rows} for kind in ("cards", "relics", "potions")}
    try:
        import sts2
        encounters = set(sts2.names()["encounter"])
    except ImportError:
        encounters = None
    for e in cat.values():
        assert e["options"] and e["src"].startswith("decomp/") and e["title"] and e["acts"], e["id"]
        assert set(e["acts"]) <= {"Overgrowth", "Underdocks", "Hive", "Glory", "shared"}, e["id"]
        assert EV.options(e["id"]) == [o["label"] for o in e["options"]]
        for o in e["options"]:
            assert o["label"] and isinstance(o["effects"], list), (e["id"], o)
            for x in walk(o["effects"]):
                kind = next(iter(x))
                assert kind in EV.VOCAB and set(x) - {kind} <= EV.PARAMS.get(kind, set()), (e["id"], x)
                if kind in ("card_add", "relic", "potion"):
                    assert x[kind] in ids[{"card_add": "cards", "relic": "relics", "potion": "potions"}[kind]], (e["id"], x)
                if kind in ("card_add_one_of", "relic_one_of"):
                    assert set(x[kind]) <= ids["cards" if kind == "card_add_one_of" else "relics"], (e["id"], x)
                if kind == "fight" and encounters is not None:
                    assert x["fight"] in encounters, (e["id"], x)
    assert EV.get("Dense Vegetation")["id"] == "DenseVegetation" == EV.get("DENSE_VEGETATION")["id"]
    assert not EV.catalog()["WarHistorianRepy"]["allowed"]


def test_events_match_screen():
    m = EV.match("Dense Vegetation", ["Trudge On: Gain 90 Gold. Lose 8 HP.", "Rest: Heal 21 HP. Fight some enemies."])
    assert [o["key"] for o in m] == ["TRUDGE_ON", "REST"]
    m = EV.match("RanwidTheElder", ["Give Fire Potion: lose it, gain a relic", "Give 100 Gold: ...", "Locked"])
    assert [o["key"] for o in m] == ["POTION", "GOLD", "RELIC_LOCKED"]
    m = EV.match("RanwidTheElder", ["Locked", "Give 100 Gold: lose it, gain a relic", "Give Anchor: two relics"])
    assert [o["key"] for o in m] == ["POTION_LOCKED", "GOLD", "RELIC"]
    assert EV.match("Wellspring", ["Dance"]) == [None]


def test_events_apply():
    dr = RM.Draws(random.Random(1), "IRONCLAD")
    st = toy()  # Wellspring 'Bathe': remove a card (a Strike first), add Guilty
    r = EV.play_option(st, "Wellspring", "Bathe", dr)
    assert len(st.deck) == len(DECK) and sum(c["id"] == "STRIKE_IRONCLAD" for c in st.deck) == 4 and st.deck[-1]["id"] == "GUILTY" and not r["fights"]

    st = toy()  # Sapphire Seed 'Consume': heal 9, upgrade one card (the first non-basic)
    EV.play_option(st, "SapphireSeed", "Consume", dr)
    assert st.hp == 49 and [c["id"] for c in st.deck if c.get("upgrade")] == ["INFLAME"]

    st = toy()  # Dense Vegetation 'Rest': heal 30% of max HP, then the Wrigglers with normal monster rewards
    r = EV.play_option(st, "DenseVegetation", 1, dr)
    assert st.hp == 64 and r["fights"] == [{"fight": "DENSE_VEGETATION_EVENT_ENCOUNTER", "rewards": "hallway", "extra": []}]

    st = toy(hp=5)  # Trudge On can kill: the run ends at the event
    r = EV.play_option(st, "DenseVegetation", "Trudge On", dr)
    assert r["dead"] and st.hp == 0 and st.end == (0, "event", "DenseVegetation")

    st = toy()  # max HP gain heals as much; Tablet of Truth stops at the first Give Up
    EV.play_option(st, "ByrdonisNest", "Eat the Egg", dr)
    assert (st.hp, st.max_hp) == (47, 87)
    st = toy()
    EV.play_option(st, "TabletOfTruth", "Decipher", dr)
    assert st.max_hp == 77 and sum(c.get("upgrade", 0) for c in st.deck) == 1

    st = toy()  # unmodelled effects are no-ops that set the flag
    r = EV.play_option(st, "CrystalSphere", "Payment Plan", dr)
    assert r["unmodelled"] and st.unmodelled and st.deck[-1]["id"] == "DEBT"

    st = toy()  # deferred card offers come back to the caller
    r = EV.play_option(st, "BrainLeech", "Share Knowledge", dr, pick=EV.DEFER)
    assert len(r["offers"]) == 1 and len(r["offers"][0]["cards"]) == 5 and len(st.deck) == len(DECK)
    assert EV.allowed("UnrestSite", toy(hp=40)) and not EV.allowed("UnrestSite", toy(hp=70)) and not EV.allowed("CrystalSphere", toy(gold=500))


def test_ancient_relics_catalog():
    with open(EV.ANCIENT_PATH, encoding="utf-8") as f:
        raw = json.load(f)
    with open(EV.PATH.replace("events.json", "ancients.json"), encoding="utf-8") as f:
        anc = json.load(f)["ancients"]
    offered = {r for a in anc.values() for p in a["pools"].values() for r in p} | {r for a in anc.values() for r in a["relics"]}
    cat = EV.ancient_relics()
    assert set(cat) == offered and len(cat) == len(raw["relics"]) == 102
    ids = {kind: {r["id"] for rows in RM.CAT[kind].values() for r in rows} for kind in ("cards", "relics", "potions")}
    enchantments = {"SWIFT", "IMBUED", "INSTINCT", "GOOPY", "CLONE", "TEZCATARAS_EMBER"}
    for rid, e in cat.items():
        assert rid in ids["relics"] and e["title"] and e["src"].startswith("decomp/MegaCrit.Sts2.Core.Models.Relics/") and e["ancients"], rid
        assert set(e["ancients"]) <= set(anc) and isinstance(e["effects"], list), rid
        for x in walk(e["effects"]):
            kind = next(iter(x))
            assert kind in EV.VOCAB and set(x) - {kind} <= EV.PARAMS.get(kind, set()), (rid, x)
            v = x[kind]
            if kind == "card_add":
                assert all(v.replace("{character}", c) in ids["cards"] for c in EV.CHARACTERS), (rid, x)
            if kind in ("relic", "potion"):
                assert v in ids[kind + "s"], (rid, x)
            if kind in ("card_add_one_of", "relic_one_of"):
                assert set(v) <= ids["cards" if kind == "card_add_one_of" else "relics"], (rid, x)
            if kind == "relic_replace":
                assert set(v) | set(v.values()) <= ids["relics"], (rid, x)
            if kind == "card_transform" and isinstance(v, dict):
                assert set(v.get("map", {})) | set(v.get("map", {}).values()) | ({v["to"]} if "to" in v else set()) <= ids["cards"], (rid, x)
            if kind == "card_enchant":
                assert v["id"] in enchantments, (rid, x)
            if kind == "card_add_random":
                assert v["pool"] in ("character", "colorless", "other", "others") and set(v.get("exclude", ())) <= ids["cards"], (rid, x)
    # every relic applies on every character without error
    for c in EV.CHARACTERS:
        for rid in cat:
            st = toy()
            EV.apply_ancient(st, rid, RM.Draws(random.Random(7), c))
            assert rid in st.relic_ids()


def test_ancient_option():
    rid, eff = EV.ancient_option("Phial Holster: Gain 1 potion slot and procure 2 random Potions.")
    assert rid == "PHIAL_HOLSTER" and eff == [{"potion_slots": 1}, {"potion_random": 2}]
    assert EV.ancient_option("Pael's Tooth: Remove 5 cards from your Deck.")[0] == "PAELS_TOOTH"
    assert EV.ancient_option("Philosopher's Stone")[0] == "PHILOSOPHERS_STONE"
    assert EV.ancient_option("Ironclad Sea Glass: Choose any of 15 cards.")[0] == "SEA_GLASS"
    assert EV.ancient_option("Proceed") == (None, None)


def test_apply_ancient():
    dr = RM.Draws(random.Random(1), "IRONCLAD")
    st = toy()  # Phial Holster: a 4th potion slot, 2 random potions
    r = EV.apply_ancient(st, "PHIAL_HOLSTER", dr)
    assert st.slots == 4 and len(st.potions) == 2 and st.relics[-1] == "PHIAL_HOLSTER" and not r["unmodelled"]

    st = toy()  # Precarious Shears: remove 2 cards (Strikes first), take 16 damage
    EV.apply_ancient(st, "PRECARIOUS_SHEARS", dr)
    assert len(st.deck) == len(DECK) - 2 and st.hp == 24 and sum(c["id"] == "STRIKE_IRONCLAD" for c in st.deck) == 3
    st = toy(hp=10)  # ... which can kill
    r = EV.apply_ancient(st, "PRECARIOUS_SHEARS", dr)
    assert r["dead"] and st.end == (0, "ancient", "PRECARIOUS_SHEARS")

    st = toy()  # Touch of Orobas: the starter becomes its upgrade
    EV.apply_ancient(st, "TOUCH_OF_OROBAS", dr)
    assert st.relics == ["BLACK_BLOOD", "TOUCH_OF_OROBAS"]

    st = toy()  # Archaic Tooth: Bash -> Break; Large Capsule: the character's Strike and Defend plus 2 relics
    EV.apply_ancient(st, "ARCHAIC_TOOTH", dr)
    assert "BREAK" in [c["id"] for c in st.deck] and "BASH" not in [c["id"] for c in st.deck]
    st = toy()
    EV.apply_ancient(st, "LARGE_CAPSULE", dr)
    assert [c["id"] for c in st.deck[-2:]] == ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD"] and len(st.relics) == 4

    st = toy()  # Neow's Talisman upgrades a basic Strike and a basic Defend (not Inflame); Leafy Poultice transforms one of each, -12 max HP
    EV.apply_ancient(st, "NEOWS_TALISMAN", dr)
    assert sorted(c["id"] for c in st.deck if c.get("upgrade")) == ["DEFEND_IRONCLAD", "STRIKE_IRONCLAD"]
    st = toy()
    EV.apply_ancient(st, "LEAFY_POULTICE", dr)
    assert st.max_hp == 68 and sum(c["id"] == "STRIKE_IRONCLAD" for c in st.deck) == 4 and sum(c["id"] == "DEFEND_IRONCLAD" for c in st.deck) == 3

    st = toy()  # Pandora's Box transforms every basic Strike and Defend (not Bash)
    EV.apply_ancient(st, "PANDORAS_BOX", dr)
    assert not any(c["id"].startswith(("STRIKE_", "DEFEND_")) for c in st.deck) and "BASH" in [c["id"] for c in st.deck]

    st = toy()  # Kaleidoscope: 2 offers of 3 cards, each from a different other character's pool
    r = EV.apply_ancient(st, "KALEIDOSCOPE", dr, pick=EV.DEFER)
    pools = {r["id"]: p for p, rows in RM.CAT["cards"].items() for r in rows}
    assert len(r["offers"]) == 2 and all(len({pools[c] for c, _ in o["cards"]}) == 3 and "IRONCLAD" not in {pools[c] for c, _ in o["cards"]}
                                         for o in r["offers"])

    st = toy()  # Neow's Bones: 2 more Neow relics (their pickups fire) and a curse
    EV.apply_ancient(st, "NEOWS_BONES", dr)
    curses = EV.ancient_relics()["NEOWS_BONES"]["effects"][1]["card_add_one_of"]
    assert len(st.relics) >= 4 and any(c["id"] in curses for c in st.deck)

    st = toy()  # Dusty Tome: an upgraded non-transcendence Ancient card of the character
    EV.apply_ancient(st, "DUSTY_TOME", dr)
    assert st.deck[-1] == {"id": "CORRUPTION", "upgrade": 1}
