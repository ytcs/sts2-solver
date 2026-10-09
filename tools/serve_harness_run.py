"""The harness plays a fresh seeded run end to end against a headless `OracleCombat serve`: `combat` for every fight, `a <i>` macro
commands with a fixed simple policy elsewhere (first map option, take rewards, first card, rest below 60% HP else smith, leave shops).

usage: python tools/serve_harness_run.py [--seed S] [--character ironclad] [--ascension 10] [--port 15720] [--budget 1]
Starts the server (never port 15555), sets STS2_BRIDGE for this process, builds Harness() directly (sweep mode, no skill gate).
"""
import os, re, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tools"))
from serve_replay_check import start_server  # noqa: E402


def macro_choice(s):
    from agent import screen as scr
    kind = scr.kind(s)
    opts = scr.options(s)
    lab = [(int(n), t) for n, t in opts]
    first = lambda pred: next((i for i, t in lab if pred(t)), None)  # noqa: E731
    if kind == "MAP":
        return f"{lab[0][0]} !"
    if kind == "REWARDS":
        i = first(lambda t: not t.startswith("proceed") and "slots full" not in t)
        return str(i if i is not None else first(lambda t: t.startswith("proceed")))
    if kind == "RESTSITE":
        hp = scr.hp(s)
        if hp and hp[0] < 0.6 * hp[1] and first(lambda t: t.startswith("Rest")) is not None:
            return str(first(lambda t: t.startswith("Rest")))
        i = first(lambda t: t.startswith("Smith"))
        return str(i if i is not None else first(lambda t: t == "proceed") if first(lambda t: t == "proceed") is not None else lab[0][0])
    if kind == "SHOP":
        return str(first(lambda t: t == "leave shop"))
    if kind == "SELECT":
        m = re.match(r"SELECT (\d+)", s)
        k = int(m.group(1)) if m else 1
        return " ".join(str(i) for i, _ in lab[:k]) if k else "-"
    if kind == "CRYSTAL_SPHERE":
        div = first(lambda t: t.startswith("divine"))
        if div is not None:
            hidden = [(x, y) for y, row in enumerate(l for l in s.split("\n") if re.match(r"^\s*\d+  [#.RPCXg?]+$", l)) for x, c in enumerate(row.split()[-1]) if c == "#"]
            x, y = hidden[len(hidden) // 2] if hidden else (5, 5)
            return f"{div} {x} {y}"
    usable = [i for i, t in lab if not t.startswith("potion")] or [i for i, _ in lab]
    return str(usable[0])


def main():
    a = sys.argv[1:]
    get = lambda k, d: a[a.index(k) + 1] if k in a else d  # noqa: E731
    port = int(get("--port", 15720))
    seed, ch, asc = get("--seed", "HARNESS01"), get("--character", "ironclad"), int(get("--ascension", 10))
    os.environ["STS2_BRIDGE"] = f"127.0.0.1:{port}"
    from agent import screen as scr
    from agent.harness import Harness
    log = open(os.path.join(ROOT, "target", f"serve_{port}.log"), "w")
    srv = start_server(port, log)
    t0 = time.time()
    fights = macros = 0
    try:
        h = Harness()
        print(h.handle(f"budget {get('--budget', '1')}").strip())
        s = h.handle("s")
        print(h.handle(f"a 0 {ch} {asc} {seed}").split("\n")[0], f"{ch} A{asc} seed {seed}")
        stuck = 0
        for step in range(3000):
            s = h.handle("s")
            kind = scr.kind(s)
            if kind == "GAME_OVER":
                break
            if scr.busy(s):
                stuck += 1
                if stuck > 50:
                    print("STUCK busy:\n" + s)
                    break
                time.sleep(0.05)
                continue
            if kind in ("COMBAT",) or (kind == "SELECT" and h.sync() is not None):
                out = h.handle("combat !")
                fights += out.count("-- combat over")
                stop = [l for l in out.split("\n") if l.startswith(("SIMULATOR", "POTION PROPOSAL", "REFUSED", "ERR"))]
                if stop:
                    print(f"[{scr.floor_key(s)}] combat stopped: {stop[0][:160]}")
                    if scr.kind(h.handle("s")) == "COMBAT" and "POTION PROPOSAL" not in stop[0]:
                        nxt = h.handle("s")
                        end = next((n for n, t in scr.options(nxt) if t == "end turn"), None)
                        if end is not None:
                            h.handle(f"a {end}")
                continue
            choice = macro_choice(s)
            r = h.handle("a " + choice)
            macros += 1
            if r.startswith(("ERR", "REFUSED")):
                stuck += 1
                print(f"[{scr.floor_key(s)}] {kind} `a {choice}` -> {r.splitlines()[0][:160]}")
                if stuck > 50:
                    break
            else:
                stuck = 0
        final = h.handle("s")
        print(final)
        print(f"harness run: {fights} fights via `combat`, {macros} macro commands, {time.time() - t0:.0f} s; run log {h.log.dir}")
    finally:
        srv.kill()


if __name__ == "__main__":
    main()
