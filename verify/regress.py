#!/usr/bin/env python3
"""Whole-corpus regression: run every oracle template (small n) through the differential sweep.

  verify/regress.py [--n 3] [--jobs 10] [--only SUBSTR] [--out DIR]

Exit status 0 iff there are no mismatches/errors (UNIMPLEMENTED hits are reported but tolerated).
"""
import argparse, glob, json, os, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ap = argparse.ArgumentParser()
ap.add_argument("--n", type=int, default=3)
ap.add_argument("--jobs", type=int, default=10)
ap.add_argument("--only", default="")
ap.add_argument("--out", default=None)
a = ap.parse_args()
out = a.out or tempfile.mkdtemp(prefix="sts2regress_")
tpls = sorted(p for p in glob.glob(os.path.join(ROOT, "oracle/templates/**/*.json"), recursive=True) if a.only in p)


def run(t):
    keep = os.path.join(out, os.path.basename(t)[:-5])
    os.makedirs(keep, exist_ok=True)
    r = subprocess.run([sys.executable, os.path.join(ROOT, "verify/diff_sweep.py"), t, "--n", str(a.n), "--jobs", "1", "--keep", keep],
                       capture_output=True, text=True)
    summ = [l for l in r.stdout.splitlines() if l.startswith("SUMMARY")]
    if not summ:
        return t, "error", r.stdout[-300:] + r.stderr[-300:]
    d = json.loads(summ[0].split(" ", 1)[1].rsplit(" (", 1)[0])
    bad = {k: v for k, v in d.items() if k not in ("ok", "unimplemented")}
    miss = [l for l in r.stdout.splitlines() if l.startswith("UNIMPLEMENTED")]
    if bad:
        return t, "FAIL", r.stdout[-500:]
    return t, "ok", (miss[0] if miss else "")


tot = {"ok": 0, "FAIL": 0, "error": 0}
unimpl = {}
fails = []
with ThreadPoolExecutor(a.jobs) as ex:
    for t, v, msg in ex.map(run, tpls):
        tot[v] += 1
        if v != "ok":
            fails.append((t, v, msg))
        elif msg:
            unimpl[os.path.basename(t)] = msg
for t, v, msg in fails:
    print(f"--- {v}: {os.path.relpath(t, ROOT)}\n{msg}")
print(f"templates: {len(tpls)}  ok: {tot['ok']}  FAIL: {tot['FAIL']}  error: {tot['error']}  with-unimplemented: {len(unimpl)}  (artifacts {out})")
sys.exit(1 if fails else 0)
