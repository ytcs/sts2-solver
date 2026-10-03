#!/usr/bin/env python3
"""Print oracle trace steps compactly: tools/fuzz_show.py BASE FROM TO   (BASE = path without .scenario.json/.jsonl)"""
import json, sys

base = sys.argv[1]
lo, hi = int(sys.argv[2]), int(sys.argv[3])
t = [json.loads(l) for l in open(base + ".jsonl")]
sc = json.load(open(base + ".scenario.json"))
if lo == 0:
    print({k: v for k, v in sc.items() if k not in ("policy",)})


def cs(cards):
    return [c["id"] + ("+" if c["upgrade"] else "") for c in cards]


for i in range(lo, min(hi + 1, len(t))):
    r = t[i]
    print("STEP", i, r["action"], "choices",
          [([o["id"] + ("+" if o["upgrade"] else "") + "@" + str(o.get("pile")) for o in c["options"]], c["picked"]) for c in r["choices"]])
    print("  energy", r["energy"], "stars", r["stars"], "hp", r["player"]["hp"], "blk", r["player"]["block"], "round", r["round"])
    print("  hand", [(c["id"] + ("+" if c["upgrade"] else ""), c["cost"]) for c in r["hand"]])
    print("  draw", len(r["draw"]), cs(r["draw"][:6]), "disc", cs(r["discard"]), "exh", cs(r["exhaust"]))
    print("  ppow", [(p["id"], p["amount"]) for p in r["player"]["powers"]], "pets", [(p["id"], p["hp"]) for p in r["pets"]])
    print("  enemies", [(e["id"], e["hp"], e["block"], [(p["id"], p["amount"]) for p in e["powers"]], e["next_move"]) for e in r["enemies"]])
    print("  relics", [(x["id"], x.get("counter"), x.get("props")) for x in r["relics"] if x.get("counter") is not None or x.get("props")])
    print("  orbs", [o["id"] for o in r["orbs"]], "potions", r["potions"])
