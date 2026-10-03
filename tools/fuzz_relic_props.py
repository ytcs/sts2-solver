#!/usr/bin/env python3
"""Discover the saved properties (counters / flags) of every relic through the oracle and cache them in tools/fuzz_relic_props.json,
so tools/fuzz_gen.py can start relics with non-default counters (a relic carried through a run has accumulated state).

  tools/fuzz_relic_props.py        # runs one scenario per relic through `oracle.sh batch` and reads record 0
"""
import json, os, shutil, subprocess, sys, tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
OUT = os.path.join(ROOT, "tools/fuzz_relic_props.json")


def main():
    pools = json.load(open(os.path.join(ROOT, "tools/fuzz_pools.json")))
    ids = sorted({r["id"] for p in ("IroncladRelicPool", "SilentRelicPool", "SharedRelicPool", "EventRelicPool") for r in pools["relics"][p]
                  if r["rarity"] not in ("Starter", "None")})
    d = tempfile.mkdtemp(prefix="relicprops_")
    for rid in ids:
        sc = {"name": rid, "ascension": 10, "encounter": "NIBBITS_WEAK", "character": "IRONCLAD", "hp": 80, "max_hp": 80, "max_energy": 3,
              "max_potion_slots": 2, "seed": "p1", "total_floor": 3, "act": 0,
              "deck": ["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH", "ASCENDERS_BANE"], "relics": ["BURNING_BLOOD", rid],
              "potions": [], "script": []}
        json.dump(sc, open(os.path.join(d, rid + ".scenario.json"), "w"))
    subprocess.run([ORACLE, "batch", d], capture_output=True, text=True)
    res = {}
    for rid in ids:
        p = os.path.join(d, rid + ".jsonl")
        if not os.path.exists(p) or os.path.exists(os.path.join(d, rid + ".err")):
            continue
        rec = json.loads(open(p).readline())
        for r in rec["relics"]:
            if r["id"] == rid and r.get("props"):
                res[rid] = r["props"]
    json.dump(res, open(OUT, "w"), indent=1, sort_keys=True)
    shutil.rmtree(d)
    print(len(res), "relics with saved properties ->", OUT)


main()
