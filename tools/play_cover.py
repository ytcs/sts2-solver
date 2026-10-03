#!/usr/bin/env python3
"""Coverage of a sweep template: which cards the random policy actually PLAYED (and how often), per oracle trace.

  tools/play_cover.py TEMPLATE.json [--n 20] [--asc 10] [--keep DIR]

Runs the real-game oracle with the same seeds/policy as tools/diff_sweep.py (tag `cv`), then counts plays by card id (the
card id is read from the hand of the previous record). Use it to make sure a sweep really exercises the cards you ported.
"""
import argparse, collections, json, os, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")


def run(args):
    tpl, i, asc, policy, outdir = args
    s = json.load(open(tpl))
    s["seed"] = f"cv{i}"
    s["name"] = f"cv{i}"
    s["ascension"] = asc
    base = os.path.join(outdir, f"cv{i}_a{asc}")
    json.dump(s, open(base + ".scenario.json", "w"))
    r = subprocess.run([ORACLE, "run", base + ".scenario.json", "--random", str(policy + i), "--out", base + ".jsonl"], capture_output=True, text=True)
    cnt = collections.Counter()
    if r.returncode != 0:
        return cnt, (r.stderr or r.stdout)[-200:]
    prev = None
    for line in open(base + ".jsonl"):
        rec = json.loads(line)
        a = rec.get("action")
        if prev is not None and isinstance(a, dict) and "play" in a:
            hp = a["play"]["hand_pos"]
            hand = prev.get("hand", [])
            if hp < len(hand):
                cnt[hand[hp]["id"] + ("+" if hand[hp].get("upgrade") else "")] += 1
        prev = rec
    os.remove(base + ".scenario.json")
    os.remove(base + ".jsonl")
    return cnt, ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("template")
    ap.add_argument("--n", type=int, default=20)
    ap.add_argument("--asc", type=int, default=10)
    ap.add_argument("--policy-seed", type=int, default=11)
    ap.add_argument("--keep", default=None)
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    outdir = a.keep or tempfile.mkdtemp(prefix="cover")
    os.makedirs(outdir, exist_ok=True)
    tot = collections.Counter()
    with ThreadPoolExecutor(a.jobs) as ex:
        for cnt, err in ex.map(run, [(a.template, i, a.asc, a.policy_seed, outdir) for i in range(a.n)]):
            tot.update(cnt)
            if err:
                print("oracle error:", err, file=sys.stderr)
    for k, v in sorted(tot.items()):
        print(f"{v:6d} {k}")


if __name__ == "__main__":
    main()
