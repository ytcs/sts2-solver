"""N headless servers in parallel, each playing a seeded run to the end with a random legal policy over the bridge protocol.

usage: python tools/serve_bench.py [--n 8] [--port 15710] [--character ironclad] [--ascension 10] [--seeds S1,S2,..] [--max-steps 4000]
Each server gets its own --state-dir and APPDATA/LOCALAPPDATA/TEMP under target/serve_state/<port>; the game's save folder
(%APPDATA%/SlayTheSpire2) is snapshotted before and after and every file changed meanwhile is listed (the real game may be writing too).
Reports wall time, steps, outcome and peak RSS per instance.
"""
import json, os, random, re, socket, subprocess, sys, threading, time

import psutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DLL = os.environ.get("ORACLE_DLL") or os.path.join(ROOT, "oracle", "combat", "bin", "Release", "net9.0", "OracleCombat.dll")
DOTNET = os.environ.get("DOTNET", os.path.expanduser("~/.dotnet/dotnet.exe" if os.name == "nt" else "~/.dotnet/dotnet"))
NL = chr(10)
SAVES = os.path.join(os.environ.get("APPDATA", os.path.expanduser("~")), "SlayTheSpire2")


def call(port, line):
    with socket.create_connection(("127.0.0.1", port), timeout=120) as s:
        s.sendall((line + "\n").encode())
        out = b""
        while True:
            b = s.recv(65536)
            if not b:
                return out.decode()
            out += b


def options(text):
    return [(int(m.group(1)), m.group(2)) for l in text.split("\n") for m in [re.match(r"^(\d+) (.*)", l)] if m]


def pick(text, rng):
    lines = text.split("\n")
    kind = lines[0].split(" ")[0]
    opts = options(text)
    if kind == "SELECT":
        m = re.match(r"SELECT (\d+)(?:-(\d+))?", lines[0])
        lo, hi = int(m.group(1)), int(m.group(2) or m.group(1))
        n = rng.randint(lo, min(hi, len(opts)))
        return "a " + (" ".join(map(str, sorted(rng.sample([i for i, _ in opts], n)))) if n else "-")
    if kind == "COMBAT":
        enemies = [(int(m.group(2)), int(m.group(1))) for l in lines for m in [re.match(r"^e(\d+) .*? (\d+)/\d+ b", l)] if m and "[untargetable]" not in l]
        plays = [(i, t) for i, t in opts if not t.startswith("(x)") and t != "end turn"]
        end = next(i for i, t in opts if t == "end turn")
        if plays and rng.random() < 0.97:
            i, t = rng.choice(plays)
            return f"a {i}" + (f" e{min(enemies)[1]}" if t.endswith("->e") and enemies else "")
        return f"a {end}"
    if kind == "RESTSITE":
        m = re.search(r"HP (\d+)/(\d+)", text)
        rest = next((i for i, t in opts if t.startswith("Rest")), None)
        if rest is not None and m and int(m.group(1)) < 0.5 * int(m.group(2)):
            return f"a {rest}"
    if kind == "SHOP":
        leave = next(i for i, t in opts if t == "leave shop")
        buy = [i for i, t in opts if "can't afford" not in t and t != "leave shop" and not t.startswith("potion")]
        return f"a {rng.choice(buy)}" if buy and rng.random() < 0.3 else f"a {leave}"
    if kind == "REWARDS":
        take = [i for i, t in opts if not t.startswith("proceed") and "slots full" not in t]
        proceed = [i for i, t in opts if t.startswith("proceed")]
        return f"a {rng.choice(take)}" if take else f"a {proceed[0]}"
    if kind == "CRYSTAL_SPHERE":
        div = [i for i, t in opts if t.startswith("divine")]
        if div:
            return f"a {div[0]} {rng.randrange(11)} {rng.randrange(11)}"
    usable = [i for i, t in opts if not t.startswith("potion")] or [i for i, _ in opts]
    return f"a {rng.choice(usable)}"


def play_one(port, seed, args, rng):
    steps = errs = 0
    s = call(port, "s")
    if s.startswith("GAME_OVER"):
        s = call(port, "a 0")
    s = call(port, f"a 0 {args['character']} {args['ascension']} {seed}")
    while steps < args["max_steps"]:
        kind = s.split(NL)[0].split(" ")[0]
        if kind == "GAME_OVER":
            break
        cmd = pick(s, rng) if "(busy)" not in s.split(NL)[0] else "s"
        r = call(port, cmd)
        steps += 1
        if r.startswith("ERR") or cmd == "s":
            errs += cmd != "s"
            s = call(port, "s")
            if errs > 200:
                break
        else:
            s = r
    hdr = next((l for l in s.split(NL) if re.search(r"A\d+ F\d+", l)), "")
    m = re.search(r"F(\d+)", hdr)
    return steps, errs, s.split(NL)[0], hdr, int(m.group(1)) if m else 0


def play(port, seed, args, res):
    rng = random.Random(seed)
    t0 = time.time()
    runs = []
    for k in range(args["runs"]):
        runs.append(play_one(port, f"{seed}{k}" if args["runs"] > 1 else seed, args, rng))
    res[port] = dict(seed=seed, secs=round(time.time() - t0, 2), runs=len(runs), steps=sum(r[0] for r in runs), errs=sum(r[1] for r in runs),
                     floors=[r[4] for r in runs], end=runs[-1][2], header=runs[-1][3])


def replay_driver(port, rec, live, res):
    """tools/serve_replay.py in its own process (STS2_BRIDGE is per process) against the running server on `port`."""
    out = os.path.join(ROOT, "target", "serve_state", str(port), "replay.jsonl")
    cmd = [sys.executable, os.path.join(ROOT, "tools", "serve_replay.py"), rec, "--port", str(port), "--log", out] + (["--live", live] if live else [])
    t0 = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    try:
        r = json.loads(p.stdout[p.stdout.index("{"):])
    except ValueError:
        r = dict(error=(p.stdout + p.stderr)[-500:])
    r["secs"] = round(time.time() - t0, 2)
    res[port] = dict(seed="replay", runs=1, steps=r.get("done"), errs=len(r.get("stops") or []), floors=[], end=str(r.get("end") or r.get("error")),
                     header=f"identical {r.get('identical')}", secs=r["secs"])


def snapshot(d):
    out = {}
    for base, _, files in os.walk(d):
        for f in files:
            p = os.path.join(base, f)
            try:
                out[p] = os.stat(p).st_mtime_ns
            except OSError:
                pass
    return out


def main():
    a = sys.argv[1:]
    get = lambda k, d: a[a.index(k) + 1] if k in a else d
    n, base = int(get("--n", 8)), int(get("--port", 15710))
    args = dict(character=get("--character", "ironclad"), ascension=int(get("--ascension", 10)), max_steps=int(get("--max-steps", 4000)), runs=int(get("--runs", 1)))
    seeds = get("--seeds", ",".join(f"BENCH{i:07d}" for i in range(n))).split(",")
    before = snapshot(SAVES)
    t_start = time.time()
    procs, peak = {}, {}
    for i in range(n):
        port = base + i
        assert port not in (15555, 15556)
        sd = os.path.join(ROOT, "target", "serve_state", str(port))
        os.makedirs(sd, exist_ok=True)
        env = dict(os.environ, APPDATA=os.path.join(sd, "appdata"), LOCALAPPDATA=os.path.join(sd, "localappdata"), TEMP=os.path.join(sd, "tmp"), TMP=os.path.join(sd, "tmp"))
        for k in ("APPDATA", "LOCALAPPDATA", "TEMP"):
            os.makedirs(env[k], exist_ok=True)
        log = open(os.path.join(sd, "server.log"), "w")
        procs[port] = subprocess.Popen([DOTNET, os.path.abspath(DLL), "serve", "--port", str(port), "--state-dir", os.path.join(sd, "saves")], stdout=log, stderr=log, cwd=sd, env=env)
    for port in procs:
        for _ in range(600):
            try:
                socket.create_connection(("127.0.0.1", port), timeout=1).close()
                break
            except OSError:
                time.sleep(0.1)
    t_up = time.time()
    res = {}
    if "--replay" in a:
        threads = [threading.Thread(target=replay_driver, args=(port, get("--replay", None), get("--live", None), res)) for port in procs]
    else:
        threads = [threading.Thread(target=play, args=(port, seeds[i], args, res)) for i, port in enumerate(procs)]
    stop = False

    def watch():
        while not stop:
            for port, p in procs.items():
                try:
                    peak[port] = max(peak.get(port, 0), psutil.Process(p.pid).memory_info().rss)
                except psutil.Error:
                    pass
            time.sleep(0.2)
    w = threading.Thread(target=watch, daemon=True)
    w.start()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    stop = True
    wall = time.time() - t_start
    for port, p in procs.items():
        try:
            call(port, "shutdown")
        except OSError:
            pass
        try:
            p.wait(5)
        except subprocess.TimeoutExpired:
            p.kill()
    after = snapshot(SAVES)
    changed = sorted(p for p in after if before.get(p) != after[p])
    for port in sorted(res):
        r = res[port]
        print(f"{port} seed {r['seed']}: {r['runs']} runs, floors reached {r['floors']}, last: {r['end']} {r['header']} | {r['steps']} actions, {r['errs']} errors, {r['secs']} s, peak RSS {peak.get(port, 0) / 1e6:.0f} MB")
    rss = [peak.get(p, 0) / 1e6 for p in procs]
    print(f"{n} servers: start-up {t_up - t_start:.1f} s, wall {wall:.1f} s, peak RSS mean {sum(rss) / len(rss):.0f} MB max {max(rss):.0f} MB, sum {sum(rss):.0f} MB")
    stray = [os.path.join(dp, f) for port in procs for dp, _, fs in os.walk(os.path.join(ROOT, "target", "serve_state", str(port))) for f in fs
             if f not in ("server.log", "replay.jsonl") and "_fights" not in dp]
    print(f"files written inside the instance dirs: {len(stray)}" + (f" {stray[:10]}" if stray else ""))
    def ours(p):
        try:
            data = open(p, "rb").read()
        except OSError:
            return "unreadable"
        return "CONTAINS A BENCH SEED" if b"BENCH" in data else "no bench seed"
    print(f"game save folder {SAVES}: {len(changed)} files changed during the bench (the real game writes there too)"
          + "".join(f"\n  {p} [{ours(p)}]" for p in changed[:40]))
    json.dump(dict(results=res, peak_rss_mb={p: peak.get(p, 0) / 1e6 for p in procs}, wall=wall, changed=changed), open(os.path.join(ROOT, "target", "serve_bench.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
