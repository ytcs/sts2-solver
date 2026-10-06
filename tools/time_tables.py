#!/usr/bin/env python3
"""Wall time of the harness calculators with the real engine, on the offline test screens (no game needed): the numbers behind `sts2-harness`
"Cost of the calculators". Each command runs twice: cold (first at this deck) and again (the caches of `routes` / `pickplan`).

  STS2_DEVICE=cuda python tools/time_tables.py [leaf_turns]
"""
import os, sys, tempfile, time

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tests", "agent"))
from support import MAP_A2, MAP_SCREEN_A2, FakeBridge, MonkeyPatch, deck, fight_starts, make_harness, screen  # noqa: E402

CMDS = [("reward", "card_reward_a2"), ("routes", MAP_SCREEN_A2), ("rmcalc", "shop_a2"), ('eval --boss --smooth --v "up|upgrade=BASH"', "shop_a2"),
        ("eval --elites", "shop_a2"), ("pickplan", "card_reward_a2")]


def main():
    if len(sys.argv) > 1:  # search depth override (LEAF_TURNS) for a comparison
        sys.path.insert(0, os.path.join(ROOT, "rl"))
        import fastsearch
        fastsearch.LEAF_TURNS = int(sys.argv[1])
    from agent.engine import Engine
    t = time.perf_counter()
    eng = Engine()
    print(f"engine load {time.perf_counter() - t:.1f}s", flush=True)
    mp = MonkeyPatch()
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        for cmd, scr_name in CMDS:
            text = scr_name if "\n" in scr_name else screen(scr_name)
            h = make_harness(mp, tmp, FakeBridge(text, deck_json=deck(), map_text=MAP_A2), events=fight_starts(upto=18), engine=eng)
            times = []
            for _ in range(2):
                t = time.perf_counter()
                out = h.handle(cmd)
                times.append(time.perf_counter() - t)
            bad = [l for l in out.splitlines() if l.startswith(("ERR", "REFUSED"))]
            print(f"{cmd:45s} {times[0]:6.1f}s  again {times[1]:5.1f}s" + (f"  {bad[0][:80]}" if bad else ""), flush=True)
    mp.undo()


if __name__ == "__main__":
    main()
