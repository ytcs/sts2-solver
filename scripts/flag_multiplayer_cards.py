#!/usr/bin/env python3
"""Flags the multiplayer-only cards in data/catalog.json ("multiplayer_only": true): the cards whose decompiled class overrides
`MultiplayerConstraint => CardMultiplayerConstraint.MultiplayerOnly` (decomp/MegaCrit.Sts2.Core.Models.Cards/*.cs). Single-player runs never offer
them, so fight generators, the run model and the plan library leave them out. Rerun after a game update.

  python scripts/flag_multiplayer_cards.py
"""
import glob, json, os, re

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
snake = lambda n: re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", n).upper()  # noqa: E731
mp = {snake(os.path.basename(f)[:-3]) for f in glob.glob(os.path.join(ROOT, "decomp", "MegaCrit.Sts2.Core.Models.Cards", "*.cs"))
      if "CardMultiplayerConstraint.MultiplayerOnly" in open(f, encoding="utf-8").read()}
path = os.path.join(ROOT, "data", "catalog.json")
cat = json.load(open(path))
n = 0
for pool in cat["cards"].values():
    for c in pool:
        c.pop("multiplayer_only", None)
        if c["id"] in mp:
            c["multiplayer_only"] = True
            n += 1
json.dump(cat, open(path, "w"), separators=(",", ":"))
print(f"{n} catalog cards flagged multiplayer-only ({len(mp)} classes)")
