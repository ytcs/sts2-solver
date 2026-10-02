#!/usr/bin/env python3
"""Build an oracle/diff scenario template at MAX ASCENSION (A10): 2 potion slots, Ascender's Bane appended to the deck.

  tools/mk_scenario.py --encounter NIBBITS_WEAK --deck STRIKE_IRONCLAD:5,DEFEND_IRONCLAD:4,BASH \
      [--character IRONCLAD] [--relics BURNING_BLOOD,...] [--potions FIRE_POTION] [--hp 80] [--asc 10] [--no-bane] \
      [--starter] > oracle/templates/x.json

`--starter` prefixes the character's starter deck/relic. Card spec: ID[:count][+] (a trailing + means upgraded).
Then: python3 tools/diff_sweep.py oracle/templates/x.json --n 40
"""
import argparse, json, sys

STARTERS = {
    "IRONCLAD": (["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH"], ["BURNING_BLOOD"], 80, 3, 0),
    "SILENT": (["STRIKE_SILENT"] * 5 + ["DEFEND_SILENT"] * 5 + ["NEUTRALIZE", "SURVIVOR"], ["RING_OF_THE_SNAKE"], 70, 3, 0),
    "DEFECT": (["STRIKE_DEFECT"] * 4 + ["DEFEND_DEFECT"] * 4 + ["ZAP", "DUALCAST"], ["CRACKED_CORE"], 75, 3, 3),
    "NECROBINDER": (["STRIKE_NECROBINDER"] * 4 + ["DEFEND_NECROBINDER"] * 4 + ["BODYGUARD", "UNLEASH"], ["BOUND_PHYLACTERY"], 66, 3, 0),
    "REGENT": (["STRIKE_REGENT"] * 4 + ["DEFEND_REGENT"] * 4 + ["FALLING_STAR", "VENERATE"], ["DIVINE_RIGHT"], 75, 3, 0),
}


def cards(spec):
    out = []
    for tok in filter(None, spec.split(",")):
        up = tok.endswith("+")
        tok = tok.rstrip("+")
        cid, _, n = tok.partition(":")
        for _ in range(int(n or 1)):
            out.append({"id": cid, "upgrade": 1} if up else cid)
    return out


ap = argparse.ArgumentParser()
ap.add_argument("--encounter", required=True)
ap.add_argument("--character", default="IRONCLAD")
ap.add_argument("--deck", default="")
ap.add_argument("--starter", action="store_true")
ap.add_argument("--relics", default="")
ap.add_argument("--potions", default="")
ap.add_argument("--hp", type=int)
ap.add_argument("--asc", type=int, default=10)
ap.add_argument("--no-bane", action="store_true")
ap.add_argument("--floor", type=int, default=1)
ap.add_argument("--act", type=int, default=0)
a = ap.parse_args()
deck_s, relics_s, hp, energy, orbs = STARTERS[a.character]
deck = (list(deck_s) if a.starter else []) + cards(a.deck)
relics = (list(relics_s) if a.starter else []) + [r for r in a.relics.split(",") if r]
if a.asc >= 5 and not a.no_bane:
    deck.append("ASCENDERS_BANE")
slots = 3 - (1 if a.asc >= 4 else 0)
potions = [p for p in a.potions.split(",") if p]
assert len(potions) <= slots, f"at A{a.asc} there are only {slots} potion slots"
json.dump({"name": f"{a.character.lower()}_{a.encounter.lower()}", "ascension": a.asc, "encounter": a.encounter,
           "character": a.character, "hp": a.hp or hp, "max_hp": a.hp or hp, "max_energy": energy, "base_orb_slots": orbs,
           "max_potion_slots": slots, "seed": "1", "total_floor": a.floor, "act": a.act,
           "deck": deck, "relics": relics, "potions": potions}, sys.stdout, indent=1)
print()
