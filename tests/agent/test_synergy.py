"""data/synergy_candidates.json and tools/synergy.py: known mechanics, anti pairs, consistency."""
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import synergy  # noqa: E402

DB = synergy.load()


def _pair(a, b, kind):
    return any(p["kind"] == kind and {p["a"], p["b"]} == {a, b} for p in DB["pairs"])


def test_known_tags():
    t = DB["tags"]
    assert "strength_gain" in t["INFLAME"]["provides"]
    assert all("multi_hit" in t[c]["provides"] and "strength_gain" in t[c]["consumes"] for c in ("WHIRLWIND", "THRASH", "TWIN_STRIKE"))
    assert "exhaust_enabler" in t["FEEL_NO_PAIN"]["consumes"] and "exhaust_self" in t["FEEL_NO_PAIN"]["consumes"]
    assert "hp_loss_self" in t["RUPTURE"]["consumes"] and "hp_loss_self" in t["INFERNO"]["provides"]
    assert "exhaust_self" in t["TREMBLE"]["provides"] and "strike" in t["PERFECTED_STRIKE"]["consumes"] and "strike" in t["POMMEL_STRIKE"]["provides"]
    assert t["MIDNIGHT"].get("mp_only") and t["WHIRLWIND"]["src"]["multi_hit"].startswith("Cards/Whirlwind.cs:")


def test_pairs():
    assert _pair("TUNGSTEN_ROD", "RUPTURE", "anti") and _pair("TUNGSTEN_ROD", "INFERNO", "anti")
    assert _pair("TUNGSTEN_ROD", "BLOODLETTING", "direct") and not _pair("TUNGSTEN_ROD", "INFERNO", "direct")
    assert _pair("CORRUPTION", "INTIMIDATING_HELMET", "anti") and _pair("FIDDLE", "POMMEL_STRIKE", "anti")
    assert _pair("INFLAME", "TWIN_STRIKE", "direct") and _pair("TRUE_GRIT", "FEEL_NO_PAIN", "direct")
    assert any(p["kind"] == "indirect" and p["a"] == "BLOODLETTING" and p["b"] == "WHIRLWIND" and "RUPTURE" in p["needs"] for p in DB["pairs"])
    assert not any("MIDNIGHT" in (p["a"], p["b"]) for p in DB["pairs"])


def test_consistency():
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json"), encoding="utf-8"))
    ids = {c["id"] for p in ("IRONCLAD", "COLORLESS") for c in cat["cards"][p]} | {r["id"] for p in ("IRONCLAD", "SHARED", "EVENT") for r in cat["relics"][p]}
    assert set(DB["tags"]) == ids
    for p in DB["pairs"]:
        assert p["a"] in ids and p["b"] in ids and p["a"] != p["b"] and p["kind"] in ("direct", "indirect", "anti")
        assert p["kind"] != "anti" or p.get("note")
    names = {b["name"] for b in DB["bundles"]}
    assert {"strength scaling", "exhaust engine", "self-damage strength", "barricade block"} <= names
    for b in DB["bundles"]:
        assert set(b["core"]) <= ids and not set(b["core"]) & set(b["support"])


def test_deck_report():
    rows, anti = synergy.deck_report(DB, ["INFERNO+", "RUPTURE", "SPITE", "BLOODLETTING", "TUNGSTEN_ROD", "STRIKE_IRONCLAD"])
    top = rows[0]
    assert top["name"] == "self-damage strength" and top["core"] == 3 and "TUNGSTEN_ROD" in top["anti"]
    assert ("TUNGSTEN_ROD", "RUPTURE") in {(a, b) for a, b, _ in anti}
