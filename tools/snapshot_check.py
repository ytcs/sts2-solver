#!/usr/bin/env python3
"""Fork gate: a seeded run on the headless server (`serve --history 1`) driven by a fixed pseudo-random policy; then each snapshot is
loaded and must show the screen the original showed after it, and the next --follow commands must reproduce the original replies.
--history DIR checks an existing run's snapshots (e.g. a baseline run's server/saves/history): screens only.
usage: tools/snapshot_check.py [--seed FORKCHECK1] [--steps 400] [--follow 10] [--port 16911] [--history DIR]"""
import argparse, glob, os, random, re, shutil, socket, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DLL = os.environ.get("ORACLE_DLL") or os.path.join(ROOT, "oracle", "combat", "bin", "Release", "net9.0", "OracleCombat.dll")
DOTNET = os.environ.get("DOTNET") or shutil.which("dotnet") or os.path.expanduser("~/.dotnet/dotnet.exe")


def call(port, line):
    s = socket.create_connection(("127.0.0.1", port), timeout=600)
    s.sendall((line + "\n").encode())
    buf = b""
    while True:
        d = s.recv(65536)
        if not d:
            break
        buf += d
    return buf.decode("utf-8", "replace")


def options(screen):
    return [int(m.group(1)) for m in re.finditer(r"^(\d+) ", screen, re.M)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", default="FORKCHECK1")
    ap.add_argument("--steps", type=int, default=400)
    ap.add_argument("--follow", type=int, default=10)
    ap.add_argument("--port", type=int, default=16911)
    ap.add_argument("--history")
    a = ap.parse_args()
    state = tempfile.mkdtemp(prefix="forkcheck_")
    proc = subprocess.Popen([DOTNET, DLL, "serve", "--port", str(a.port), "--state-dir", state, "--history", "1"], stdout=subprocess.DEVNULL, stderr=open(os.path.join(state, "serve.log"), "w"))
    try:
        for _ in range(120):
            try:
                call(a.port, "peek"); break
            except OSError:
                time.sleep(0.5)
        if a.history:
            fs = sorted(f for f in glob.glob(os.path.join(a.history, "*.json")) if os.path.exists(f + ".screen.txt"))
            bad = [f for f in fs if call(a.port, "load " + os.path.abspath(f)).strip() != open(f + ".screen.txt", encoding="utf-8").read().strip()]
            print(f"screens {len(fs) - len(bad)}/{len(fs)}" + "".join(f"\n  DIFFERS {os.path.basename(f)}" for f in bad))
            return
        rng = random.Random(1)
        log = [("a 0 ironclad 10 " + a.seed, call(a.port, "a 0 ironclad 10 " + a.seed))]
        marks = {f: 0 for f in glob.glob(os.path.join(state, "history", "*.json"))}
        for step in range(a.steps):
            screen = log[-1][1]
            if "GAME_OVER" in screen.splitlines()[0]:
                break
            opts = options(screen)
            cmd = f"a {rng.choice(opts)}" if opts else "s"
            log.append((cmd, call(a.port, cmd)))
            for f in sorted(glob.glob(os.path.join(state, "history", "*.json"))):
                marks.setdefault(f, len(log) - 1)
        snaps = sorted(marks)
        print(f"{len(log)} commands, {len(snaps)} snapshots; last screen: {log[-1][1].splitlines()[1] if len(log[-1][1].splitlines()) > 1 else ''}", flush=True)
        ok_screen = ok_follow = 0
        for f in snaps:
            j = marks[f]
            got = call(a.port, "load " + f)
            want = open(f + ".screen.txt", encoding="utf-8").read() if os.path.exists(f + ".screen.txt") else log[j][1]
            same = got.strip() == want.strip()
            ok_screen += same
            n_ok = 0
            for cmd, reply in log[j + 1:j + 1 + a.follow]:
                if call(a.port, cmd).strip() != reply.strip():
                    break
                n_ok += 1
            ok_follow += n_ok == min(a.follow, len(log) - j - 1)
            print(f"{os.path.basename(f)}: screen {'same' if same else 'DIFFERS'}, follow {n_ok}/{min(a.follow, len(log) - j - 1)}" +
                  ("" if same else f"\n  want: {want.strip()[:300]!r}\n  got:  {got.strip()[:300]!r}"), flush=True)
        print(f"screens {ok_screen}/{len(snaps)}, continuations {ok_follow}/{len(snaps)}")
    finally:
        try:
            call(a.port, "shutdown")
        except OSError:
            pass
        proc.wait(timeout=30)


if __name__ == "__main__":
    main()
