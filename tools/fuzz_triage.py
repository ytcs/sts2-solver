#!/usr/bin/env python3
"""Re-run `sts2diff` on every kept scenario of a fuzz dir and print a one-block summary each.

  tools/fuzz_triage.py DIR [--lines 3] [--only SUBSTR]

Entries whose replay is now OK are listed as `fixed` (so a fix can be checked against the whole backlog at once).
"""
import argparse, glob, json, os, subprocess

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")
ap = argparse.ArgumentParser()
ap.add_argument("dir")
ap.add_argument("--lines", type=int, default=3)
ap.add_argument("--only", default="")
a = ap.parse_args()
fixed = []
for sc in sorted(glob.glob(os.path.join(a.dir, "*.scenario.json"))):
    if a.only not in sc:
        continue
    base = sc[: -len(".scenario.json")]
    if not os.path.exists(base + ".jsonl"):
        err = open(base + ".error.txt").read()[:200] if os.path.exists(base + ".error.txt") else "no trace"
        print(f"== {os.path.basename(base)}: ORACLE {err!r}")
        continue
    s = json.load(open(sc))
    r = subprocess.run([DIFF, "run", sc, base + ".jsonl", "--max", "4"], capture_output=True, text=True)
    out = (r.stdout + r.stderr).strip().splitlines()
    if r.returncode == 0:
        fixed.append(os.path.basename(base))
        continue
    head = f"[{s['character']} {s['encounter']} {s.get('meta')}]"
    print(f"== {os.path.basename(base)} {head} rc={r.returncode}")
    for l in out[: a.lines]:
        print("   ", l[:230])
print("fixed:", " ".join(fixed))
