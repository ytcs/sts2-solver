#!/usr/bin/env python3
"""Freeze a fuzz finding as a regression: copy NAME.scenario.json + the oracle trace (truncated after record --upto) into
oracle/regression/. `cargo test -p sts2diff --test fuzz_orb_pet_regression` replays every pair against the simulator.

  tools/mk_regression.py DIR/BASE NAME --upto STEP --note "what broke / why"
"""
import argparse, json, os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ap = argparse.ArgumentParser()
ap.add_argument("base")
ap.add_argument("name")
ap.add_argument("--upto", type=int, required=True)
ap.add_argument("--note", default="")
a = ap.parse_args()
out = os.path.join(ROOT, "oracle/regression")
os.makedirs(out, exist_ok=True)
s = json.load(open(a.base + ".scenario.json"))
s["name"] = a.name
if a.note:
    s["note"] = a.note
json.dump(s, open(os.path.join(out, a.name + ".scenario.json"), "w"), indent=1)
with open(a.base + ".jsonl") as f, open(os.path.join(out, a.name + ".jsonl"), "w") as g:
    for i, line in enumerate(f):
        if i > a.upto:
            break
        g.write(line)
print("saved", a.name)
