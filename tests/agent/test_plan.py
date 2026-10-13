"""Plan nudges (target deck: missing pieces, no-target nudge, passed-over warning; never refusals) and the community prior lookup."""
import json
import os

from support import FakeBridge, make_harness

from agent import macro

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
REWARD = ("CARD_REWARD\nA2 F20 IRONCLAD A10 HP 60/80 G99 pots[-, -]\n0 Inflame(1) Gain 2 Strength.\n1 Barricade(3) Block is not removed.\n"
          "2 Anger(0) Deal 6 damage.\n3 Skip\n")
DECK = dict(character="IRONCLAD", hp=60, max_hp=80, gold=99, deck=[{"id": "STRIKE_IRONCLAD", "upgrade": 0}] * 4 + [{"id": "BASH", "upgrade": 0}],
            relics=[{"id": "BURNING_BLOOD"}], potions=[], max_potion_slots=2)


def test_target_left_counts_against_the_deck_it_was_set_on():
    deck = dict(deck=[{"id": "INFLAME"}, {"id": "STRIKE_IRONCLAD"}, {"id": "STRIKE_IRONCLAD"}], relics=[])
    v = dict(name="target", add=["INFLAME", "BARRICADE"], remove=["STRIKE_IRONCLAD"], upgrade=["INFLAME"])
    base = macro.deck_counts(deck["deck"])
    assert macro.target_left(v, deck, base) == v | {"add": ["INFLAME", "BARRICADE"]}  # a second Inflame wanted
    later = dict(deck=[{"id": "INFLAME", "upgrade": 1}, {"id": "INFLAME"}, {"id": "STRIKE_IRONCLAD"}], relics=[])
    assert macro.pieces(macro.target_left(v, later, base)) == "add BARRICADE"
    assert macro.target_left(v, deck)["add"] == ["BARRICADE"]  # no base: an add is done once the deck holds it


def test_brief_and_pick_nudge_but_never_refuse(monkeypatch, tmp_path):
    monkeypatch.setenv("STS2_SKILL_GATE", "off")
    monkeypatch.setenv("STS2_DECISION_GUARDS", "off")
    fake = FakeBridge(REWARD, deck_json=DECK, on_action=["REWARDS\nA2 F20 IRONCLAD A10 HP 60/80 G99 pots[-, -]\n0 proceed\n"])
    h = make_harness(monkeypatch, tmp_path, fake)
    h.gate = True
    assert "no target deck for act 2" in h.handle("brief")
    h.handle("target add=BARRICADE,INFLAME|remove=STRIKE_IRONCLAD")
    out = h.handle("brief")
    assert out.startswith("target missing: add BARRICADE, INFLAME; remove STRIKE_IRONCLAD\n") and "no target deck" not in out, out
    out = h.handle("a 2 -- rules: new: test ; numbers: none ; judgment: Anger")
    assert out.startswith("REWARDS") and out.rstrip().endswith("update it with `target`.") and "passed over BARRICADE, INFLAME" in out, out


def test_prior_lines_from_the_table():
    import importlib.util
    spec = importlib.util.spec_from_file_location("community_lut", os.path.join(ROOT, "tools", "community_lut.py"))
    lut = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(lut)
    table = {"chars": {"ironclad": {"AEONGLASS_BOSS": {"n": [81, 100], "c": {"BARRICADE": [18, 20]}, "r": {}}}}}
    out = lut.lines("IRONCLAD", "aeonglass", ["Barricade", "Bludgeon"], lut=table)
    assert out.startswith(lut.TAG) and "0.81 of 100" in out
    row = next(l for l in out.splitlines() if "BARRICADE" in l).split()
    assert row[1:6] == ["0.90", "20", "0.79", "80", "+0.11"], row
    assert "BLUDGEON" in out and "not in the table" in out
    assert "unknown" in lut.lines("ironclad", "nope", lut=table)
    if os.path.exists(lut.LUT):
        assert json.dumps(lut.load()["chars"]["ironclad"]["AEONGLASS_BOSS"]["n"]) == "[2064, 2552]"  # the reference: 0.81 of 2552
