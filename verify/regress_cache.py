#!/usr/bin/env python3
"""Cached whole-corpus regression: record the real game's traces ONCE, replay them against the Rust sim many times.

  verify/regress_cache.py record [--n 4] [--jobs 8] [--dir DIR] [--only SUBSTR]   # slow: runs the oracle
  verify/regress_cache.py check  [--jobs 14] [--dir DIR] [--only SUBSTR]          # fast: sts2diff on the cached pairs

The oracle trace does not depend on the Rust code, so after an engine/perf change `check` is the quick equivalent of
`verify/regress.py` (same scenarios, same random policies). STS2DIFF selects the sts2diff binary (default target/debug/sts2diff).
Exit status 0 iff no mismatches / sim errors (UNIMPLEMENTED runs are tolerated, like regress.py).
"""
import argparse, glob, json, os, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")
ap = argparse.ArgumentParser()
ap.add_argument("mode", choices=["record", "check"])
ap.add_argument("--n", type=int, default=4)
ap.add_argument("--jobs", type=int, default=8)
ap.add_argument("--dir", default=os.path.join(ROOT, "target/regress_cache"))
ap.add_argument("--only", default="")
ap.add_argument("--policy-seed", type=int, default=11)
a = ap.parse_args()
os.makedirs(a.dir, exist_ok=True)


def record(job):
    tpl, i = job
    name = os.path.basename(tpl)[:-5]
    base = os.path.join(a.dir, f"{name}_s{i}")
    if os.path.exists(base + ".jsonl"):
        return base, "cached", ""
    s = json.load(open(tpl))
    s["seed"] = f"s{i}"
    s["name"] = f"s{i}"
    s["ascension"] = 10
    json.dump(s, open(base + ".scenario.json", "w"))
    r = subprocess.run([ORACLE, "run", base + ".scenario.json", "--random", str(a.policy_seed + i), "--out", base + ".jsonl.tmp"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return base, "oracle-error", (r.stderr or r.stdout)[-200:]
    os.rename(base + ".jsonl.tmp", base + ".jsonl")
    return base, "recorded", ""


def check(base):
    d = subprocess.run([DIFF, "run", base + ".scenario.json", base + ".jsonl", "--max", "4"], capture_output=True, text=True)
    out = d.stdout.strip()
    if d.returncode == 0:
        return base, "ok", ""
    if d.returncode == 3:
        return base, "unimplemented", out.splitlines()[-1] if out else ""
    return base, "mismatch" if d.returncode == 1 else "sim-error", (out or d.stderr)[-600:]


tpls = sorted(p for p in glob.glob(os.path.join(ROOT, "oracle/templates/**/*.json"), recursive=True) if a.only in p)
res = {}
bad = []
if a.mode == "record":
    jobs = [(t, i) for i in range(a.n) for t in tpls]  # breadth first
    with ThreadPoolExecutor(a.jobs) as ex:
        for base, v, msg in ex.map(record, jobs):
            res[v] = res.get(v, 0) + 1
            if v == "oracle-error":
                bad.append((base, v, msg))
else:
    bases = sorted(p[: -len(".jsonl")] for p in glob.glob(os.path.join(a.dir, "*.jsonl")) if a.only in p)
    with ThreadPoolExecutor(a.jobs) as ex:
        for base, v, msg in ex.map(check, bases):
            res[v] = res.get(v, 0) + 1
            if v not in ("ok", "unimplemented"):
                bad.append((base, v, msg))
for base, v, msg in bad[:15]:
    print(f"--- {v}: {base}\n{msg}")
print("SUMMARY", json.dumps(res))
sys.exit(1 if bad else 0)
