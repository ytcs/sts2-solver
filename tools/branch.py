#!/usr/bin/env python3
"""Branch a recorded headless run for diagnosis (not a scored run): start `OracleCombat serve` on the run's seed, re-send its logged game
commands up to the first event at floor --until (e.g. "A2 F27"; the screen before that floor's first decision), leave the server running.
Then drive it live: STS2_BRIDGE=127.0.0.1:<port> python -m agent ... (own daemon). Same seed + same commands = same game.
usage: tools/branch.py <run dir with events.jsonl> --until "A2 F27" [--port 15950] [--out target/branch/<name>]"""
import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run")
    ap.add_argument("--until", required=True)
    ap.add_argument("--port", type=int, default=15950)
    ap.add_argument("--out")
    ap.add_argument("--seed")
    a = ap.parse_args()
    import baseline as B
    from postmortem import replay_prefix
    from agent import bridge, screen as scr
    ev = [json.loads(l) for l in open(os.path.join(a.run, "events.jsonl"), encoding="utf-8")]
    seed = a.seed or os.path.basename(os.path.normpath(a.run))
    want = re.match(r"A(\d+) F(\d+)", a.until)
    act, floor = int(want.group(1)), int(want.group(2))

    def at(e):
        m = re.search(r"A(\d+) F(\d+)", (e.get("state") or "").split("\n")[0] if e["kind"] == "macro" else str(e.get("floor") or ""))
        return m and (int(m.group(1)), int(m.group(2))) >= (act, floor)
    stop = next((i for i, e in enumerate(ev) if e["kind"] in ("macro", "fight_start") and at(e)), len(ev))
    out = a.out or os.path.join(ROOT, "target", "branch", f"{seed}_{a.until.replace(' ', '')}")
    os.makedirs(out, exist_ok=True)
    sd = os.path.join(out, "server")
    env = dict(os.environ, APPDATA=os.path.join(sd, "appdata"), LOCALAPPDATA=os.path.join(sd, "localappdata"), TEMP=os.path.join(sd, "tmp"),
               TMP=os.path.join(sd, "tmp"))
    for k in ("APPDATA", "LOCALAPPDATA", "TEMP"):
        os.makedirs(env[k], exist_ok=True)
    import socket, subprocess, time
    log = open(os.path.join(sd, "server.log"), "w")
    flags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0) | getattr(subprocess, "DETACHED_PROCESS", 0)
    p = subprocess.Popen([B.DOTNET, os.path.abspath(B.DLL), "serve", "--port", str(a.port), "--state-dir", os.path.join(sd, "saves"), "--seed", seed,
                          "--character", "ironclad", "--ascension", "10"], stdout=log, stderr=log, cwd=sd, env=env, creationflags=flags)
    for _ in range(600):
        try:
            socket.create_connection(("127.0.0.1", a.port), timeout=1).close()
            break
        except OSError:
            time.sleep(0.1)
    bridge.OVERRIDE = f"127.0.0.1:{a.port}"
    s, bad = replay_prefix(ev, stop)
    print(f"server pid {p.pid} on 127.0.0.1:{a.port}, replayed {stop} events, header mismatches {len(bad)}{': ' + str(bad[:2]) if bad else ''}")
    print(scr.header_line(s, "") or s.split("\n")[0])
    print(f"drive: STS2_BRIDGE=127.0.0.1:{a.port} .venv/Scripts/python.exe -m agent s")


if __name__ == "__main__":
    main()
