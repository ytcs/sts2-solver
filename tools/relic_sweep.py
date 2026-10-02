#!/usr/bin/env python3
"""Differential sweep for relics: builds an A10 scenario (Ironclad + a mix of already-ported cards) carrying the relics
under test (optionally with injected saved properties) and runs tools/diff_sweep.py on it.

  tools/relic_sweep.py KUNAI,ANCHOR [--n 40] [--encounter NIBBITS_WEAK] [--potions FIRE_POTION] [--hp 80]
                       [--props KUNAI:AttacksPlayed=2,NUNCHAKU:AttacksPlayed=9] [--deck CARD:n,CARD+,...] [--starter-relic]
                       [--character IRONCLAD] [--max-energy 3] [--keep DIR]

`--props` injects `[SavedProperty]` values (name=int|true|false) into the named relic (scenario `relics[].props`).
By default the starter relic is dropped so Burning Blood does not interfere (use --starter-relic to keep it).
"""
import argparse, json, os, subprocess, sys, tempfile

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DECK = ("STRIKE_IRONCLAD:3,DEFEND_IRONCLAD:3,BASH,ANGER,ARMAMENTS,TRUE_GRIT,HEADBUTT,BURNING_PACT,THUNDERCLAP,"
        "SWORD_BOOMERANG,CINDER,BLOODLETTING,INFLAME,TWIN_STRIKE,IRON_WAVE,SHRUG_IT_OFF,POMMEL_STRIKE")

ap = argparse.ArgumentParser()
ap.add_argument("relics")
ap.add_argument("--n", type=int, default=40)
ap.add_argument("--encounter", default="NIBBITS_WEAK")
ap.add_argument("--potions", default="")
ap.add_argument("--hp", type=int)
ap.add_argument("--props", default="")
ap.add_argument("--deck", default=DECK)
ap.add_argument("--starter-relic", action="store_true")
ap.add_argument("--character", default="IRONCLAD")
ap.add_argument("--max-energy", type=int)
ap.add_argument("--keep")
ap.add_argument("--jobs", type=int, default=4)
ap.add_argument("--asc", default="10")
ap.add_argument("--policy-seed", type=int, default=11)
ap.add_argument("--tag", default="r")
ap.add_argument("--floor", type=int, default=1)
a = ap.parse_args()

cmd = [sys.executable, os.path.join(ROOT, "tools/mk_scenario.py"), "--encounter", a.encounter, "--character", a.character,
       "--deck", a.deck, "--relics", a.relics, "--floor", str(a.floor)]
if a.starter_relic:
    cmd.append("--starter")
if a.potions:
    cmd += ["--potions", a.potions]
if a.hp:
    cmd += ["--hp", str(a.hp)]
sc = json.loads(subprocess.run(cmd, capture_output=True, text=True, check=True).stdout)
if a.max_energy:
    sc["max_energy"] = a.max_energy
if a.props:
    byrelic = {}
    for tok in a.props.split(","):
        rel, _, kv = tok.partition(":")
        k, _, v = kv.partition("=")
        val = True if v == "true" else False if v == "false" else int(v)
        byrelic.setdefault(rel, {})[k] = val
    out = []
    for r in sc["relics"]:
        rid = r if isinstance(r, str) else r["id"]
        out.append({"id": rid, "props": byrelic[rid]} if rid in byrelic else r)
    sc["relics"] = out
if not a.starter_relic and a.character == "IRONCLAD":
    pass
d = a.keep or tempfile.mkdtemp(prefix="relicsweep_")
os.makedirs(d, exist_ok=True)
tpl = os.path.join(d, "template.json")
json.dump(sc, open(tpl, "w"), indent=1)
r = subprocess.run([sys.executable, os.path.join(ROOT, "tools/diff_sweep.py"), tpl, "--n", str(a.n), "--jobs", str(a.jobs), "--keep", d,
                    "--asc", a.asc, "--policy-seed", str(a.policy_seed), "--tag", a.tag])
sys.exit(r.returncode)
