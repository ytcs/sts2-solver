#!/usr/bin/env python3
"""Robustness test of the harness: the solver plays many different encounters (dev-console `fight`, HP topped up) through the daemon.

  python -m agent.selftest --pool Overgrowth:elite --pool Hive:regular:4 [--enc A,B]

Per encounter: outcome, actions played, HP lost, and every rejected action / divergence the harness logged. Dev-console use is allowed here: this tests the
harness, it is not a scored run.
"""
import argparse
import json
import re
import sys

from agent import pools
from agent.__main__ import ask, start_daemon, up


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pool", action="append", default=[], help="Act:kind[:n]")
    ap.add_argument("--enc", default="")
    a = ap.parse_args()
    encs = [e for e in a.enc.split(",") if e]
    for p in a.pool:
        act, kind, *n = p.split(":")
        encs += pools.pool(act, kind)[: int(n[0]) if n else None]
    if not up():
        start_daemon()
    rows = []
    for enc in encs:
        st = ask("s")
        if st.startswith("GAME_OVER") or st.startswith("MENU"):
            print("run ended; start a new run before continuing:", st.split("\n")[0])
            break
        ask("x heal 999")
        r = ask("x fight " + enc)
        out = ask("combat")
        end = out.rstrip().split("\n")
        kind = next((l for l in reversed(end) if re.match(r"^[A-Z_]+$", l)), "?")
        hp = re.findall(r"HP (\d+)/(\d+)", out)
        status = ask("status")
        bad = "none" if "divergences: none" in status else status.split("divergences:")[-1].strip()[:140]
        err = [l for l in out.split("\n") if l.startswith("ERR")][:1]
        rows.append((enc, kind, hp[-1] if hp else "?", bad, err))
        print(f"{enc:34s} {kind:10s} HP {hp[-1][0] + '/' + hp[-1][1] if hp else '?':7s} divergences: {bad}" + (f"  {err[0][:100]}" if err else ""), flush=True)
        if kind == "GAME_OVER":
            print("died; stopping (start a new run to continue)")
            break
    print(f"\n{len(rows)} encounters; clean: {sum(1 for r in rows if r[3] == 'none' and not r[4])}")


if __name__ == "__main__":
    main()
