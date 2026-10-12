#!/usr/bin/env python3
"""Deck sources for a combat round (collector scenario lists, `meta.source`): training-only seeds only.
arms:   fight_start scenarios of the scripted-arm logs (target/*pull/*/BASE00xx/events.jsonl), seeds BASE0001-0040 only (BASE0041+ are eval seeds).
codex:  community runs (target/codex/a10.jsonl.gz, `codex.walk`): deck, relics and HP entering each act's first floor after its ancient (acts 2
        and 3, meta.at "start") and entering each act's boss (meta.at "boss"), potions sampled as gen_curriculum does; encounter = one of that
        act's bosses.
strong: decks scored by the predictor against their act's bosses at full HP without potions; decks at or above --min-p paired with that act's
        hallway and elite fights at shaped HP.
usage: tools/round_decks.py arms --out target/r10/decks/arms.json
       tools/round_decks.py codex --out target/r10/decks/codex.json
       tools/round_decks.py strong --decks target/r10/decks/arms.json target/r10/decks/codex.json --n 30000 --out target/r10/decks/strong.json"""
import argparse, collections, glob, hashlib, json, os, random, re, sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
TRAIN_SEEDS = range(1, 41)
SHAPED_HP = (0.45, 1.0)


def _main_target():
    d = ROOT
    while d != os.path.dirname(d):
        if glob.glob(os.path.join(d, "target", "*pull")):
            return os.path.join(d, "target")
        d = os.path.dirname(d)
    sys.exit("no target/*pull above " + ROOT)


def _key(sc):
    deck = sorted((c["id"], c.get("upgrade", 0), c.get("enchant") or "") if isinstance(c, dict) else (c, 0, "") for c in sc["deck"])
    rel = sorted(x["id"] if isinstance(x, dict) else x for x in sc["relics"])
    return hashlib.sha1(json.dumps([deck, rel, sc["act"]]).encode()).hexdigest()[:12]


def _act_name(scs, act):
    from agent import pools
    names = pools.act_names(act)
    encs = {s["encounter"] for s in scs}
    return next((n for n in names if encs & {e for k in ("weak", "regular", "elite", "boss") for e in pools.pool(n, k)}), names[0])


def _valid(scs, why):
    from round_pool import constructs
    return [sc for sc in scs if constructs(sc, why)]


def arms(a):
    out, seen, why = [], set(), collections.Counter()
    by = collections.Counter()
    for p in sorted(glob.glob(os.path.join(_main_target(), "*pull", "*", "BASE*", "events.jsonl"))):
        seed = os.path.basename(os.path.dirname(p))
        if int(re.fullmatch(r"BASE(\d+)", seed).group(1)) not in TRAIN_SEEDS:
            continue
        pull = os.path.relpath(p, _main_target()).split(os.sep)[0]
        scs = [json.loads(l)["scenario"] for l in open(p, encoding="utf-8") if '"fight_start"' in l]
        names = {act: _act_name([s for s in scs if s["act"] == act], act) for act in {s["act"] for s in scs}}
        for i, sc in enumerate(scs):
            k = _key(sc)
            if k in seen:
                continue
            seen.add(k)
            by[(pull, sc["act"])] += 1
            out.append(dict(sc, name=f"arms_{seed}_{k}", seed=k, meta=dict(source="arms", seed=seed, pull=pull, act_name=names[sc["act"]], floor=sc["total_floor"])))
    assert all(int(sc["meta"]["seed"][4:]) in TRAIN_SEEDS for sc in out), "an eval seed entered the training decks"
    out = _valid(out, why)
    _dump(out, a.out, why)
    print("per pull / act: " + ", ".join(f"{p} a{act} {n}" for (p, act), n in sorted(by.items())))


def codex_decks(a):
    import codex, sts2
    import gen_curriculum as gc
    from agent import pools
    C = gc.Curriculum(os.path.join(ROOT, "data", "catalog.json"))
    cards, relics = set(sts2.names()["card"]), set(sts2.names()["relic"])
    path = os.path.join(os.path.dirname(_main_target()), "target", "codex", "a10.jsonl.gz")
    acts = {"OVERGROWTH": "Overgrowth", "UNDERDOCKS": "Underdocks", "HIVE": "Hive", "GLORY": "Glory"}
    out, why, n_runs = [], collections.Counter(), 0
    for r in codex.rows(path):
        n_runs += 1
        ch = r["char"].upper()
        if ch not in gc.CHAR_W:
            why["character"] += 1
            continue
        prev = None
        for ai, i, g, f, now, rel, up in codex.walk(r):
            at = "start" if ai in (1, 2) and i == 1 else "boss" if f["t"] == "boss" else None
            if at and ai < len(r["acts"]) and r["acts"][ai] in acts and prev and prev["mx"]:
                if not (set(now) <= cards and rel <= relics):
                    why["unknown card / relic"] += 1
                else:
                    rng = random.Random(f"{r['hash']}/{ai}/{at}")
                    name = acts[r["acts"][ai]]
                    _, _, _, energy, orbs = gc.fg.STARTERS[ch]
                    pots, slots = C.potions(rng, ch, sorted(rel), 8, False)
                    out.append({"name": f"codex_{r['hash']}_{ai}_{at}", "ascension": 10, "encounter": rng.choice(pools.pool(name, "boss")), "character": ch,
                                "hp": max(1, prev["hp"] or 1), "max_hp": prev["mx"], "max_energy": energy, "base_orb_slots": orbs, "max_potion_slots": slots,
                                "gold": prev["gold"] or 0, "seed": f"codex{r['hash']}{ai}{at}", "total_floor": g, "act": ai,
                                "deck": [{"id": c, "upgrade": int(k < up[c])} for c in sorted(now) for k in range(now[c])], "relics": sorted(rel), "potions": pots,
                                "meta": dict(source="codex", at=at, run=r["hash"], win=r["win"], character=ch, act_name=name, floor=g)})
            prev = f
    out = _valid(out, why)
    _dump(out, a.out, why)
    print(f"{n_runs} community runs; per act / at: {dict(collections.Counter((s['act'], s['meta']['at']) for s in out))}; per character: {dict(collections.Counter(s['character'] for s in out))}; "
          f"won runs {np.mean([s['meta']['win'] for s in out]):.2f}")


def strong(a):
    from agent import pools
    from predictor import Predictor, p_win
    decks = [sc for fn in a.decks for sc in json.load(open(fn))]
    rng = random.Random(a.seed)
    if a.max_decks and len(decks) > a.max_decks:
        decks = rng.sample(decks, a.max_decks)
    bosses = [pools.pool(d["meta"]["act_name"], "boss") for d in decks]
    scs = [dict(d, encounter=b, hp=d["max_hp"], potions=[]) for d, bl in zip(decks, bosses) for b in bl]
    calib = None if a.raw else os.path.join(ROOT, "models", "predictor_calib.json")
    P = Predictor(a.ckpt, calib=calib)
    p = p_win(P.fight_start(scs, shuffles=a.shuffles))
    off = np.cumsum([0] + [len(bl) for bl in bosses])
    strength = np.array([p[off[k]:off[k + 1]].mean() for k in range(len(decks))])
    bands = [0, .2, .4, .6, .7, .8, .9, 1.01]
    for src in sorted({(d["meta"]["source"], d["meta"].get("at", "")) for d in decks}):
        ix = [k for k, d in enumerate(decks) if (d["meta"]["source"], d["meta"].get("at", "")) == src]
        for act in (0, 1, 2):
            jx = [k for k in ix if decks[k]["act"] == act]
            if jx:
                h = np.histogram(strength[jx], bands)[0] / len(jx)
                print(f"{' '.join(src):11s} act {act} n {len(jx):6d} strength mean {strength[jx].mean():.3f} | " + " ".join(f"[{lo:.1f},{hi:.1f}) {x:.2f}" for lo, hi, x in zip(bands, bands[1:], h)))
    keep = [k for k in range(len(decks)) if strength[k] >= a.min_p]
    print(f"{len(keep)} of {len(decks)} decks at strength >= {a.min_p} ({'raw' if a.raw else 'calibrated'} predictor {os.path.basename(a.ckpt)})")
    out = []
    for i in range(a.n if keep else 0):
        k = keep[i % len(keep)]
        d = decks[k]
        name = d["meta"]["act_name"]
        enc = rng.choice(pools.pool(name, "regular") + pools.pool(name, "elite"))
        out.append(dict(d, name=f"strong_{i}_{d['name']}", encounter=enc, hp=max(1, round(d["max_hp"] * rng.uniform(*SHAPED_HP))),
                        meta=dict(d["meta"], source="strong", deck_source=d["meta"]["source"], strength=round(float(strength[k]), 4))))
    _dump(out, a.out, collections.Counter())


def _dump(out, path, why):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    json.dump(out, open(path, "w"))
    print(f"{len(out)} scenarios -> {path}; rejected {sum(why.values())} {dict(why.most_common(5))}")


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    sp.add_parser("arms").add_argument("--out", required=True)
    sp.add_parser("codex").add_argument("--out", required=True)
    s = sp.add_parser("strong")
    s.add_argument("--decks", nargs="+", required=True)
    s.add_argument("--out", required=True)
    s.add_argument("--n", type=int, required=True, help="fights in total, cycling over the kept decks")
    s.add_argument("--min-p", type=float, default=0.8)
    s.add_argument("--max-decks", type=int, default=0, help="score a random sample of this many decks (0 = all)")
    s.add_argument("--ckpt", default=os.path.join(ROOT, "models", json.load(open(os.path.join(ROOT, "models", "current.json")))["predictor"]))
    s.add_argument("--raw", action="store_true", help="the raw predictor (default: with models/predictor_calib.json)")
    s.add_argument("--shuffles", type=int, default=4)
    s.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    dict(arms=arms, codex=codex_decks, strong=strong)[a.cmd](a)


if __name__ == "__main__":
    main()
