#!/usr/bin/env python3
"""Synthetic contrast states at death effects for a combat round. Test Subject: phase 1 / 1d / 2 / 2d / 3 ("d": killed, revive pending) x
player HP x Test Subject HP on community act-3 boss-entry decks. Waterfall Giant: pre-kill / killed / explode-pending x blast (Steam Eruption
stacks) vs player HP, HP - blast in -36..+10 so that HP + the block of the turns left straddles the blast (at -10 the solver still wins
0.80), on community Underdocks boss-entry decks. Weak-deck axis (`--source bot`): Test Subject only, the same grid on act-3 fight-start
decks of the scripted-arm logs (target/*pull/*/BASE*/events.jsonl, --seeds, default BASE0041-0120; BASE0122, the death bench's seed, never),
held-out bucket = whole seeds (number % 8 == 0). A state is a tools/leaf_bench.py recipe (deck scenario, build seed, ops). `roots`:
collection roots for `rl/exit.py collect --starts` ({"build", "seed"}; hidden piles and RNG resampled per job). `holdout`: the held-out
bench (leaf_values format, live-search play-out targets, saved after each state) from decks of runs in the held-out bucket only, which
`roots` and `round_decks.py boss --exclude-runs` never use; build seeds disjoint; no state equals a data/bench/leaf_values.json state.
usage: python tools/contrast.py roots --n 20000 --out-fights target/r11/contrast_fights.json --out-starts target/r11/contrast_starts.json
       python tools/contrast.py roots --source bot --n 15000 --out-fights target/r12/weak_fights.json --out-starts target/r12/weak_starts.json
       python tools/contrast.py holdout [--n 60] [--playouts 256] [--states-only] [--retarget] [--only giant] [--out data/bench/contrast_holdout.json]
       python tools/contrast.py holdout --source bot --n 30   # -> data/bench/contrast_weak.json"""
import argparse, collections, json, os, random, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
from leaf_bench import BENCH, END, build_state, describe  # noqa: E402

HOLDOUT = os.path.join(ROOT, "data", "bench", "contrast_holdout.json")
WEAK = os.path.join(ROOT, "data", "bench", "contrast_weak.json")
BENCH_SEED = "BASE0122"
TS_PHASES = ("1", "1d", "2", "2d", "3")
TS_MAX = {1: 111, 2: 212, 3: 313}
PLAYER_HP = (0.08, 0.15, 0.25, 0.35, 0.5, 0.7, 0.9)
TS_HP = (0.05, 0.15, 0.3, 0.5, 0.75, 1.0)
GIANT_STATES = ("pre", "killed", "explode")
GIANT_HP = (3, 8, 15, 25, 40)
MARGINS = (-36, -30, -24, -20, -16, -12, -8, -4, 0, 4, 10)
HOLDOUT_SEED = 1_000_000
WEAK_SEED = 2_000_000
SETTLE = ["settle"]
DOC = {"codex": "Held-out contrast bench (tools/contrast.py holdout): synthetic Test Subject phase / Waterfall Giant blast states on community "
                "boss-entry decks of held-out runs (run hash ending in 0; never in contrast roots or boss slices).",
       "bot": "Held-out weak-deck bench (tools/contrast.py holdout --source bot): synthetic Test Subject phase states on act-3 decks of scripted-arm "
              "runs on held-out seeds (number % 8 == 0; never in weak-deck roots; BASE0122 never)."}
DOC_TARGET = (" target: live-search play-outs (tools/leaf_bench.py playouts; lower bounds; E113). death_gate: >= 80% within 2 se (se floored "
              "at 1/n).")


def held_out(run):
    return run[-1] == "0"


def held_out_seed(seed):
    return int(seed[4:]) % 8 == 0


def decks(path):
    out = collections.defaultdict(list)
    for d in json.load(open(path)):
        if d["meta"].get("at") == "boss":
            kind = "ts" if d["act"] == 2 else "giant" if d["meta"]["act_name"] == "Underdocks" else None
            if kind:
                out[kind, held_out(d["meta"]["run"])].append(d)
    return out


def bot_decks(seeds):
    """act-3 fight-start decks (distinct deck + relics) of the scripted-arm logs on `seeds`"""
    import glob
    from round_decks import _key, _main_target, _valid
    tgt, out, seen, why = _main_target(), [], set(), collections.Counter()
    for p in sorted(glob.glob(os.path.join(tgt, "*pull", "*", "BASE*", "events.jsonl"))):
        seed = os.path.basename(os.path.dirname(p))
        if seed == BENCH_SEED or int(seed[4:]) not in seeds:
            continue
        run = "/".join(os.path.relpath(os.path.dirname(p), tgt).split(os.sep))
        for line in open(p, encoding="utf-8"):
            if '"fight_start"' in line:
                sc = json.loads(line)["scenario"]
                k = _key(sc)
                if sc["act"] == 2 and k not in seen:
                    seen.add(k)
                    out.append(dict(sc, name=f"bot_{seed}_{k}", seed=k, meta=dict(source="bot", run=run, seed=seed, floor=sc["total_floor"])))
    out = _valid(out, why)
    assert all(d["meta"]["seed"] != BENCH_SEED for d in out)
    res = collections.defaultdict(list)
    for d in out:
        res["ts", held_out_seed(d["meta"]["seed"])].append(d)
    return res


def pool_of(a):
    if a.source == "bot":
        lo, hi = map(int, a.seeds.split("-"))
        return bot_decks(range(lo, hi + 1))
    from round_decks import _main_target
    return decks(a.decks or os.path.join(_main_target(), "r10", "decks", "codex.json"))


def recipe(r, kind, d):
    """-> (encounter, ops, labels)"""
    extra = [END, SETTLE] * r.choice((0, 0, 1, 2))
    if kind == "ts":
        ph = r.choice(TS_PHASES)
        p, hp = int(ph[0]), max(1, round(d["max_hp"] * r.choice(PLAYER_HP)))
        sync = {"player_hp": hp} | ({} if ph.endswith("d") else {"enemy_hp": [max(1, round(TS_MAX[p] * r.choice(TS_HP)))]})
        ops = [SETTLE] + [["kill", 0], END, SETTLE] * (p - 1) + extra + [["sync", sync]] + ([["kill", 0]] if ph.endswith("d") else [])
        return "TEST_SUBJECT_BOSS", ops, dict(phase=ph, hp=hp, enemy_hp=sync.get("enemy_hp", [0])[0])
    st, steam = r.choice(GIANT_STATES), r.randint(20, 60)
    hp = min(d["max_hp"], max(1, steam + r.choice(MARGINS)))
    pw = ["set_power", "enemy", 0, "STEAM_ERUPTION_POWER", steam]
    if st == "explode":
        ops = [SETTLE] + extra + [pw, ["kill", 0], END, SETTLE, ["sync", {"player_hp": hp}]]
    else:
        ops = [SETTLE] + extra + [pw, ["sync", {"player_hp": hp, "enemy_hp": [r.choice(GIANT_HP)]}]] + ([["kill", 0]] if st == "killed" else [])
    return "WATERFALL_GIANT_BOSS", ops, dict(phase=st, hp=hp, steam=steam, margin=hp - steam)


def state(r, kind, pool, seed, bench, name):
    """a recipe that builds to a live player decision, else None"""
    d = r.choice(pool)
    enc, ops, lab = recipe(r, kind, d)
    sc = dict(d, encounter=enc, hp=d["max_hp"], name=name, meta=dict(d["meta"], source="contrast", deck_source=d["meta"]["source"], kind=kind, **lab))
    it = dict(name=name, scenario=sc, seed=seed, ops=ops)
    try:
        s = build_state(it)
    except (ValueError, AssertionError):
        return None
    if s.stage() != "play" or s.outcome() != 0 or describe(s) in bench:
        return None
    if kind == "ts" and json.loads(s.snapshot())["enemies"][0]["max_hp"] != TS_MAX[int(lab["phase"][0])]:
        return None
    return it


def roots(a):
    pool = pool_of(a)
    bench = {it["state"] for it in json.load(open(BENCH))["items"]}
    tag = "contrast" if a.source == "codex" else "weak"
    fights, starts, n = [], [], 0
    while len(fights) < a.n:
        r = random.Random(f"{tag}/{a.seed}/{n}")
        kind = "ts" if a.source == "bot" or r.random() < a.ts_share else "giant"
        it = state(r, kind, pool[kind, False], n, bench, f"{tag}:{kind}:{n}")
        n += 1
        if it:
            fights.append(it["scenario"])
            starts.append(dict(build=it["ops"], seed=it["seed"]))
    os.makedirs(os.path.dirname(os.path.abspath(a.out_fights)), exist_ok=True)
    json.dump(fights, open(a.out_fights, "w"))
    json.dump(starts, open(a.out_starts, "w"))
    c = collections.Counter((sc["meta"]["kind"], sc["meta"]["phase"]) for sc in fights)
    for held in (False, True):
        ds = [d for k, v in pool.items() if k[1] == held for d in v]
        print(f"{'held-out' if held else 'training'} decks {len(ds)} from {len({d['meta']['run'] for d in ds})} runs")
    print(f"{len(fights)} {tag} roots ({n - len(fights)} recipes rejected): {dict(sorted(c.items()))} -> {a.out_fights}, {a.out_starts}")


def holdout(a):
    model = json.load(open(os.path.join(ROOT, "models", "current.json")))["policy"]
    bot = a.source == "bot"
    if os.path.exists(a.out):
        doc = json.load(open(a.out))
        if a.retarget:
            for it in doc["items"]:
                it.pop("target", None)
                it.pop("net_at_build", None)
            doc.update(_doc=DOC[a.source] + DOC_TARGET, model=model)
            json.dump(doc, open(a.out, "w"))
    else:
        pool = pool_of(a)
        bench = {it["state"] for it in json.load(open(BENCH))["items"]}
        items, n = [], 0
        while len(items) < a.n:
            k = len(items)
            kind = "ts" if bot or k % 2 == 0 else "giant"
            tag = "weak" if bot else "holdout"
            r = random.Random(f"{tag}/{a.seed}/{n}")
            it = state(r, kind, pool[kind, True], (WEAK_SEED if bot else HOLDOUT_SEED) + n, bench, f"{tag}_{kind}_{k}")
            n += 1
            j = k if bot else k // 2
            if it and it["scenario"]["meta"]["phase"] == (TS_PHASES if kind == "ts" else GIANT_STATES)[j % (5 if kind == "ts" else 3)]:
                it["state"] = describe(build_state(it))
                items.append(it)
        doc = dict(_doc=DOC[a.source] + DOC_TARGET, model=model, runs=sorted({it["scenario"]["meta"]["run"] for it in items}), items=items)
        json.dump(doc, open(a.out, "w"))
    print(f"{len(doc['items'])} held-out states, {sum('target' in it for it in doc['items'])} with targets -> {a.out}")
    if a.states_only:
        return
    os.environ.setdefault("STS2_DEVICE", "cpu")
    import torch
    torch.set_num_threads(a.threads)
    from agent.engine import Engine
    from leaf_bench import playouts
    eng = Engine()
    for it in doc["items"]:
        if "target" not in it and a.only in it["name"]:
            playouts(eng, it, a.playouts)
            json.dump(doc, open(a.out, "w"))


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("roots", "holdout"):
        s = sub.add_parser(name)
        s.add_argument("--source", choices=("codex", "bot"), default="codex")
        s.add_argument("--decks", default=None, help="codex deck file (default target/r10/decks/codex.json)")
        s.add_argument("--seeds", default="41-120", help="bot source: BASE seed numbers lo-hi")
        s.add_argument("--seed", type=int, default=0)
    s = sub.choices["roots"]
    s.add_argument("--n", type=int, default=20000)
    s.add_argument("--ts-share", type=float, default=0.6)
    s.add_argument("--out-fights", required=True)
    s.add_argument("--out-starts", required=True)
    s = sub.choices["holdout"]
    s.add_argument("--n", type=int, default=60)
    s.add_argument("--playouts", type=int, default=256)
    s.add_argument("--threads", type=int, default=6)
    s.add_argument("--states-only", action="store_true")
    s.add_argument("--retarget", action="store_true", help="drop the stored targets (states and build seeds kept) and play them out again")
    s.add_argument("--only", default="", help="play out the states whose name contains this (e.g. giant)")
    s.add_argument("--out", default=None)
    a = ap.parse_args()
    if a.cmd == "holdout" and a.out is None:
        a.out = WEAK if a.source == "bot" else HOLDOUT
    dict(roots=roots, holdout=holdout)[a.cmd](a)


if __name__ == "__main__":
    main()
