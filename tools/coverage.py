#!/usr/bin/env python3
"""Content coverage: which entities have a Rust implementation (scans src/content/**/*.rs like build.rs does).

  tools/coverage.py [--missing cards,powers,...] [--root DIR]
"""
import argparse, glob, os, re, sys

here = os.path.dirname(os.path.abspath(__file__))
ap = argparse.ArgumentParser()
ap.add_argument("--root", default=os.path.join(here, ".."))
ap.add_argument("--missing", default="")
a = ap.parse_args()
root = os.path.abspath(a.root)
src = os.path.join(root, "crates/sts2sim/src")

ids = open(os.path.join(src, "ids.rs")).read()
mods = {}
for m in re.finditer(r"pub mod (\w+) \{(.*?)\n\}\n", ids, flags=re.S):
    mods[m.group(1)] = re.findall(r'^\s+"([A-Z0-9_]+)",$', m.group(2).split("CLASS_NAMES")[0], flags=re.M)


def slugify(name):
    out, i, last = [], 0, -1
    while i < len(name):
        if i + 1 < len(name) and name[i].isascii() and name[i].isalnum() and "A" <= name[i + 1] <= "Z":
            out.append(name[i] + "_" + name[i + 1]); i += 2; last = i; continue
        if i == last and i != 0 and "A" <= name[i] <= "Z":
            out.append("_" + name[i]); i += 1; last = i; continue
        out.append(name[i]); i += 1
    return "".join(out).upper()


def scan(cat, pat):
    found = set()
    for f in glob.glob(os.path.join(src, "content", cat, "*.rs")):
        found |= set(re.findall(pat, open(f).read()))
    return found


have = {
    "card": {slugify(n) for n in scan("cards", r"listener!\(\s*(\w+)\s*\{")},
    "power": {slugify(n) for n in scan("powers", r"listener!\(\s*(\w+)\s*\{")},
    "relic": {slugify(n) for n in scan("relics", r"listener!\(\s*(\w+)\s*\{")},
    "potion": {slugify(n) for n in scan("potions", r"listener!\(\s*(\w+)\s*\{")},
    "monster": {n[:-4] for n in scan("monsters", r"pub static (\w+_DEF): MonsterDef")},
    "encounter": {n.upper() for n in scan("encounters", r"pub fn spawn_(\w+)\(")},
}
want = set(a.missing.split(",")) if a.missing else set()
tot_have = tot = 0
for cat, names in have.items():
    allv = mods[cat]
    h = [n for n in allv if n in names]
    print(f"{cat:10s} {len(h):4d} / {len(allv):4d}  ({100*len(h)//len(allv)}%)")
    tot_have += len(h); tot += len(allv)
    if cat + "s" in want or cat in want:
        print("   missing:", " ".join(n for n in allv if n not in names))
print(f"{'total':10s} {tot_have:4d} / {tot:4d}")
