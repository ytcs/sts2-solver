#!/usr/bin/env python3
"""Differential sweep: generate scenarios, record real-game traces with the oracle (random policy), replay in Rust.

  verify/diff_sweep.py TEMPLATE.json [--n 40] [--asc 0,9] [--policy-seed 11] [--keep DIR] [--jobs 8]

Each run overrides `seed` (and `ascension`) of the template. Prints a per-run verdict and a summary; mismatching
runs keep their scenario + trace under DIR (default: $TMPDIR/sweep) so `sts2diff run` can inspect them.
"""
import argparse, json, os, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")


def run_one(args):
    tpl, i, asc, policy, outdir, tag = args
    s = json.load(open(tpl))
    s["seed"] = f"{tag}{i}"
    s["name"] = f"{tag}{i}"
    s["ascension"] = asc
    base = os.path.join(outdir, f"{tag}{i}_a{asc}")
    json.dump(s, open(base + ".scenario.json", "w"))
    r = subprocess.run([ORACLE, "run", base + ".scenario.json", "--random", str(policy + i), "--out", base + ".jsonl"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return base, "oracle-error", (r.stderr or r.stdout)[-300:]
    d = subprocess.run([DIFF, "run", base + ".scenario.json", base + ".jsonl", "--max", "4"], capture_output=True, text=True)
    out = d.stdout.strip()
    if d.returncode == 0:
        os.remove(base + ".scenario.json"); os.remove(base + ".jsonl")
        return base, "ok", ""
    if d.returncode == 3:
        os.remove(base + ".scenario.json"); os.remove(base + ".jsonl")
        return base, "unimplemented", out.strip().splitlines()[-1]
    return base, "mismatch" if d.returncode == 1 else "sim-error", (out or d.stderr)[-600:]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("template")
    ap.add_argument("--n", type=int, default=40)
    ap.add_argument("--asc", default="10")  # max ascension is the only level we care about
    ap.add_argument("--policy-seed", type=int, default=11)
    ap.add_argument("--keep", default=None)
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--tag", default="s")
    a = ap.parse_args()
    outdir = a.keep or tempfile.mkdtemp(prefix="sts2sweep_")  # unique per invocation (concurrent sweeps never collide)
    os.makedirs(outdir, exist_ok=True)
    jobs = [(a.template, i, int(asc), a.policy_seed, outdir, a.tag) for i in range(a.n) for asc in a.asc.split(",")]
    res = {"ok": 0}
    bad = []
    with ThreadPoolExecutor(a.jobs) as ex:
        for base, verdict, msg in ex.map(run_one, jobs):
            res[verdict] = res.get(verdict, 0) + 1
            if verdict != "ok":
                bad.append((base, verdict, msg))
    for base, verdict, msg in bad[:12]:
        if verdict != "unimplemented":
            print(f"--- {verdict}: {base}\n{msg}")
    missing = {}
    for base, verdict, msg in bad:
        if verdict == "unimplemented":
            name = msg.split(" (step")[0].replace("UNIMPLEMENTED ", "")
            missing[name] = missing.get(name, 0) + 1
    if missing:
        print("UNIMPLEMENTED content hit:", ", ".join(f"{k} x{v}" for k, v in sorted(missing.items(), key=lambda t: -t[1])))
    print("SUMMARY", json.dumps(res), f"(artifacts in {outdir})")
    sys.exit(0 if all(v == 'unimplemented' for _, v, _ in bad) else 1)


main()
