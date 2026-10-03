#!/usr/bin/env python3
"""Delta-debug a failing fuzz scenario: try dropping relics / potions / deck cards (one group at a time) and report which
variants still mismatch (so the culprit set shrinks).

  tools/fuzz_variants.py FUZZ_DIR/fm_X_N [--seeds 1]       relics are tried one at a time; then deck cards; then potions

Each variant is run through the oracle (batch) and sts2diff; a variant "keeps failing" if the verdict is still a mismatch.
The minimal scenario is written to BASE.min.scenario.json (with its trace BASE.min.jsonl).
"""
import argparse, json, os, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")


def verdict(sc, base):
    p = base + ".scenario.json"
    json.dump(sc, open(p, "w"))
    lst = base + ".list"
    open(lst, "w").write(p + "\n")
    for ext in (".error.txt", ".done", ".jsonl"):
        try: os.remove(base + ext)
        except OSError: pass
    subprocess.run([ORACLE, "batch", lst], capture_output=True, text=True)
    if os.path.exists(base + ".error.txt"):
        return "oracle-error"
    d = subprocess.run([DIFF, "run", p, base + ".jsonl", "--max", "2"], capture_output=True, text=True)
    return {0: "ok", 1: "mismatch", 3: "unimplemented"}.get(d.returncode, "sim-error")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    a = ap.parse_args()
    sc = json.load(open(a.base + ".scenario.json"))
    work = a.base + ".min"
    print("original:", verdict(sc, work))
    for key in ("relics", "potions", "deck"):
        i = 0
        while i < len(sc.get(key, [])):
            trial = json.loads(json.dumps(sc))
            removed = trial[key].pop(i)
            v = verdict(trial, work)
            if v == "mismatch":
                sc = trial
                print(f"  dropped {key}[{i}] = {removed if isinstance(removed, str) else removed.get('id')} -> still mismatch")
            else:
                i += 1
    print("final:", verdict(sc, work))
    print("relics:", sc["relics"], "potions:", sc["potions"], "deck:", len(sc["deck"]))
    print("written", work + ".scenario.json")


main()
