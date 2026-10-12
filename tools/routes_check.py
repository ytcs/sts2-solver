#!/usr/bin/env python3
"""routes solves only the jobs its DP can read (routes.need), trusting a job's outcome to depend on (scenario, job seed) alone, not on the
other jobs in the batch. Check it on this device (STS2_DEVICE): at recorded MAP screens, every pruned table and the `routes` text must be
bit-identical to the whole-batch solve. Exit 1 on any difference.
usage: tools/routes_check.py <events.jsonl> <index> [...] [--attempts 1]   (index into the run's MAP screens; --list prints them)"""
import argparse, json, os, re, sys, time, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT]


def map_states(path):
    from agent import pools, runctx
    out, map_text, scen, seen, act = [], None, None, [], None
    for e in (json.loads(x) for x in open(path, encoding="utf-8") if x.strip()):
        if e.get("kind") == "map":
            map_text = e["text"]
        elif e.get("kind") == "fight_start":
            scen = e["scenario"]
            if scen.get("act", 0) != act:
                seen, act = [], scen.get("act", 0)
            seen.append(e["encounter"])
        elif e.get("kind") == "macro" and e.get("screen") == "MAP" and map_text and scen:
            hp, mx = map(int, re.search(r"HP (\d+)/(\d+)", e["state"]).groups())
            bosses = runctx.bosses_from_map(map_text)
            name = next((n for n in pools.act_names(act) if set(pools.pool(n, "boss")) & set(bosses)), pools.act_names(act)[0])
            out.append(dict(deck=dict(scen, hp=hp, max_hp=mx), map=map_text, state=e["state"], ctx=dict(seen=list(seen), bosses=bosses), act=name,
                            seed=zlib.crc32(re.search(r"A\d+ F\d+", e["state"]).group(0).encode()) % 100_000 * 100))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("index", nargs="*", type=int)
    ap.add_argument("--attempts", type=int, default=1)
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    import numpy as np
    from agent import routes
    sts = map_states(a.events)
    if a.list:
        for i, st in enumerate(sts):
            print(i, st["state"].split("\n")[1], "potions", len(st["deck"].get("potions", [])))
        return 0
    from agent.engine import Engine
    eng, bad = Engine(), 0
    for i in a.index:
        st = sts[i]
        eng.table_seed = st["seed"]
        args = (st["deck"], st["map"], st["state"], st["ctx"], st["act"], a.attempts)
        routes._CACHE.clear()
        t = time.perf_counter()
        full = routes.build_tables(eng, st["deck"], st["act"], st["ctx"], a.attempts,
                                   {o[:2] for o in routes._batch(st["deck"], st["act"], st["ctx"], a.attempts)["owners"]})
        text_full, t_full = routes.analyse(eng, *args), time.perf_counter() - t
        routes._CACHE.clear()
        t = time.perf_counter()
        text, t_pruned = routes.analyse(eng, *args), time.perf_counter() - t
        part = next(iter(routes._CACHE.values()))["mats"]
        same = text == text_full and all(np.array_equal(m, full[k]) for k, m in part.items())
        bad += not same
        print(f"{i} {st['state'].split(chr(10))[1]}: tables {len(part)}/{len(full) - 2}, whole batch {t_full:.1f} s, pruned {t_pruned:.1f} s, "
              f"{'identical' if same else 'DIFFERENT'}", flush=True)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
