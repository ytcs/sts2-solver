"""Re-enact an expert record against a headless `OracleCombat serve` and diff its replay log with a live one, decision by decision.

usage: python tools/serve_replay_check.py RECORD.compact.jsonl LIVE_REPLAY.jsonl [--port 15700] [--keep]
Starts a fresh server on --port (never 15555), points the harness at it (STS2_BRIDGE), runs the harness's `replay` (Harness() built
directly, as sweeps do), then compares every `decision` row's screen, deck and combat state with the live log row of the same k.
"""
import json, os, re, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DLL = os.environ.get("ORACLE_DLL") or os.path.join(ROOT, "oracle", "combat", "bin", "Release", "net9.0", "OracleCombat.dll")
DOTNET = os.environ.get("DOTNET", os.path.expanduser("~/.dotnet/dotnet.exe" if os.name == "nt" else "~/.dotnet/dotnet"))


def start_server(port, log):
    assert port not in (15555, 15556)
    p = subprocess.Popen([DOTNET, DLL, "serve", "--port", str(port)], stdout=log, stderr=log, cwd=ROOT)
    import socket
    for _ in range(100):
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            return p
        except OSError:
            time.sleep(0.1)
    p.kill()
    raise SystemExit("server did not start")


PROFILE = re.compile(r"Test Subject #C\d+")  # the name counts the profile's kills (SaveManager progress); the server's fixed profile has none


def norm_screen(s, profile=False):
    s = (s or "").rstrip("\n")
    return PROFILE.sub("Test Subject #C?", s) if profile else s


def norm_state(st):
    return json.dumps(st, sort_keys=True) if st is not None else None


def compare(mine, live):
    M = {r["k"]: r for r in mine if r.get("event") == "decision"}
    L = {r["k"]: r for r in live if r.get("event") == "decision"}
    keys = sorted(set(M) & set(L))
    bad = {"screen": [], "screen modulo profile counters": [], "deck": [], "state": []}
    for k in keys:
        a, b = M[k], L[k]
        if norm_screen(a["screen"]) != norm_screen(b["screen"]):
            bad["screen"].append(k)
        if norm_screen(a["screen"], True) != norm_screen(b["screen"], True):
            bad["screen modulo profile counters"].append(k)
        if "deck" in b and a.get("deck") != b.get("deck"):
            bad["deck"].append(k)
        if "state" in b and norm_state(a.get("state")) != norm_state(b.get("state")):
            bad["state"].append(k)
    return keys, bad, M, L


def main():
    args = sys.argv[1:]
    rec, live_path = args[0], args[1]
    port = int(args[args.index("--port") + 1]) if "--port" in args else 15700
    os.environ["STS2_BRIDGE"] = f"127.0.0.1:{port}"
    sys.path.insert(0, ROOT)
    from agent import reenact
    from agent.harness import Harness
    r = reenact.load(rec)
    log_path = os.path.join(reenact.creator_dir(r), "replay", r["video"]["id"] + ".jsonl")
    if os.path.exists(log_path):
        os.remove(log_path)
    os.makedirs(os.path.join(ROOT, "target"), exist_ok=True)
    slog = open(os.path.join(ROOT, "target", f"serve_{port}.log"), "w")
    srv = start_server(port, slog)
    t0 = time.time()
    try:
        h = Harness()
        steps = f" --steps {args[args.index('--steps') + 1]}" if "--steps" in args else ""
        out = h.handle("replay " + os.path.abspath(rec).replace(os.sep, "/") + steps)
        secs = time.time() - t0
    finally:
        if "--keep" not in args:
            srv.kill()
    print(out.split("\n")[-2] if out.count("\n") > 1 else out)
    print("\n".join(l for l in out.split("\n") if l.startswith(("STOP", "record replayed", "cursor", "opening")) and "matches" not in l))
    mine = [json.loads(l) for l in open(log_path, encoding="utf-8")]
    live = [json.loads(l) for l in open(live_path, encoding="utf-8")]
    keys, bad, M, L = compare(mine, live)
    done = [x["k"] for x in mine if x.get("event") == "done"]
    print(f"replay: {len(done)} steps done in {secs:.1f} s; decisions compared {len(keys)} (live {len(L)}, server {len(M)})")
    for what, ks in bad.items():
        print(f"{what}: {len(keys) - len(ks)}/{len(keys)} identical" + (f"; first differing k {ks[:8]}" if ks else ""))
    real = bad["screen modulo profile counters"]
    if real:
        k = real[0]
        print(f"--- live k={k}\n{L[k]['screen']}\n--- server k={k}\n{M[k]['screen']}")
    ops = [x for x in mine if x.get("event") == "opening"]
    print(f"fight openings: {sum(1 for o in ops if not o['diff'])}/{len(ops)} match the record")
    return 0 if not (real or bad["deck"] or bad["state"]) else 1


if __name__ == "__main__":
    sys.exit(main())
