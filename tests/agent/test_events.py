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
    assert [o["key"] for o in m] == ["POTION", "GOLD", "POTION_LOCKED"]


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
