#!/usr/bin/env python3
"""Smoke test for the AgentBridge mod: plays the game with trivial rules through agent.bridge and logs every step.

  python -m agent.autopilot [--steps 2000] [--log target/smoke.log] [--char ironclad --asc 10]

Flags anomalies: ERR replies, busy timeouts, fallback (generic button) screens, and loops (same state 6 times in a row).
"""
import argparse, collections, os, re, sys, time
from agent.bridge import call


def parse(text):
    lines = text.rstrip("\n").split("\n")
    kind = lines[0].split(" ")[0] if lines else "?"
    opts = [(int(m.group(1)), m.group(2)) for m in (re.match(r"^(\d+) (.*)$", l) for l in lines[1:]) if m]
    return kind, opts, lines


def choose(kind, opts, lines, a):
    labels = [o[1] for o in opts]
    def find(pred):
        for i, l in opts:
            if pred(l):
                return i
        return None
    if kind == "SELECT":
        lo = int(lines[0].split(" ")[1].split("-")[0])
        return " ".join(str(i) for i in range(lo)) if lo > 0 else "-"
    if kind == "COMBAT":
        enemies = [(int(m.group(1)), int(m.group(2))) for m in (re.match(r"^e(\d+) .*? (\d+)/\d+ b", l) for l in lines) if m]
        tgt = min(enemies, key=lambda e: e[1])[0] if enemies else 0
        for i, l in opts:
            if l.startswith("(x)") or l.startswith("potion") or l == "end turn":
                continue
            return f"{i} e{tgt}" if l.endswith("->e") else str(i)
        pot = find(lambda l: l.startswith("potion"))
        if pot is not None and "HP" in lines[0]:
            return f"{pot} e{tgt}" if labels[pot].endswith("->e") else str(pot)
        return str(find(lambda l: l == "end turn"))
    if kind == "MENU":
        i = find(lambda l: l.startswith("new run"))
        if i is not None:
            return f"{i} {a.char} {a.asc}"
        return str(find(lambda l: l == "continue run") or 0)
    if kind == "REWARDS":
        i = find(lambda l: not l.startswith("proceed") and "slots full" not in l)
        return str(i if i is not None else find(lambda l: l.startswith("proceed")))
    if kind == "SHOP":
        full = not re.search(r"pots\[[^\]]*-", lines[1] if len(lines) > 1 else "")
        i = find(lambda l: re.match(r"^\d+g ", l) and "can't afford" not in l and not (full and " potion " in l))
        return str(find(lambda l: l == "leave shop")) if i is None or a.no_buy else str(i)
    if kind == "GAME_OVER":
        return str(find(lambda l: l == "continue") if find(lambda l: l == "continue") is not None else 0)
    return "0"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--steps", type=int, default=2000)
    ap.add_argument("--log", default="target/smoke.log")
    ap.add_argument("--char", default="ironclad")
    ap.add_argument("--asc", default="10")
    ap.add_argument("--no-buy", action="store_true")
    ap.add_argument("--stop-at-menu", action="store_true", help="stop when a run ends and the main menu is back")
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.log), exist_ok=True)
    log = open(a.log, "w", encoding="utf-8")
    anomalies = collections.Counter()
    seen = collections.Counter()
    last, same = None, 0
    kinds = collections.Counter()
    t0 = time.time()
    state = call("s")
    for step in range(a.steps):
        kind, opts, lines = parse(state)
        kinds[kind] += 1
        if a.stop_at_menu and kind == "MENU" and step > 0:
            break
        if "(busy)" in lines[0] or not opts:
            anomalies["busy/no options: " + lines[0]] += 1
            time.sleep(1)
            state = call("s")
            continue
        if any(l.startswith("[") for _, l in opts):
            anomalies["fallback buttons on " + kind] += 1
        key = state
        same = same + 1 if key == last else 0
        last = key
        if same >= 6:
            anomalies["loop on " + kind] += 1
            log.write("LOOP\n" + state)
            break
        cmd = choose(kind, opts, lines, a)
        log.write(f"--- step {step} {time.time() - t0:.1f}s\n{state}> a {cmd}\n")
        st = time.time()
        state = call("a " + cmd)
        if state.startswith("ERR"):
            anomalies[f"ERR on {kind}: " + state.split("\n")[0]] += 1
            log.write(state.split("\n")[0] + "\n")
            state = state.split("\n", 1)[1]
        seen[kind] += time.time() - st
    log.write(f"=== done in {time.time() - t0:.1f}s\n")
    log.close()
    print(f"{sum(kinds.values())} decisions in {time.time() - t0:.0f}s; by screen: {dict(kinds)}")
    print("seconds per screen kind:", {k: round(v, 1) for k, v in seen.items()})
    print("anomalies:", dict(anomalies) if anomalies else "none")
    print("last state:\n" + state)


if __name__ == "__main__":
    main()
