#!/usr/bin/env python3
"""Promote a fuzz finding to a committed regression: oracle/regression/NAME.{scenario.json,jsonl}.

  tools/fuzz_keep.py FUZZ_DIR/fm_X_N NAME [--upto STEP] [--note "what it covers"]

The trace is truncated after STEP (default: whole trace); `crates/sts2diff/tests/regression.rs` replays every pair and requires a
perfect match against the recorded real-game states (so keep only traces recorded from the oracle, never edited ones).
"""
import argparse, json, os

ap = argparse.ArgumentParser()
ap.add_argument("base")
ap.add_argument("name")
ap.add_argument("--upto", type=int)
ap.add_argument("--note", default="")
a = ap.parse_args()
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "oracle", "regression")
os.makedirs(root, exist_ok=True)
sc = json.load(open(a.base + ".scenario.json"))
sc.pop("policy", None)
sc["name"] = a.name
if a.note:
    sc["note"] = a.note
lines = open(a.base + ".jsonl").read().splitlines()
if a.upto is not None:
    lines = lines[: a.upto + 1]
json.dump(sc, open(os.path.join(root, a.name + ".scenario.json"), "w"), separators=(",", ":"))
open(os.path.join(root, a.name + ".jsonl"), "w").write("\n".join(lines) + "\n")
print("kept", a.name, len(lines), "records")
