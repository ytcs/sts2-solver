#!/usr/bin/env python3
"""Reward values in P(win) units (E103): an elite / treasure relic and a card reward (elite vs hallway offer) added to Ironclad decks met at act 1 / 2 / 3
elites, scored by the search vs that act's boss pool and vs the act-3 boss pool (full HP, no potions; paired seeds per boss). Decks: community runs
(target/codex/a10.jsonl.gz, `codex.walk`, stratified won / lost) and the operator runs' elite fight_start scenarios. Relics and offers are drawn from the
community floor records of the same act (the elite relic rarity mix matches RelicFactory.RollRarity 50/33/17). A card offer is valued by cross-fitting:
the best of {cards, skip} chosen on half the attempts is scored on the other half.
usage: tools/reward_values.py run [--per-act 40] [--relics 5] [--attempts 32] [--out target/review/reward_values.jsonl]
       tools/reward_values.py report [--out ...] [--json data/reward_values.json]"""
import argparse, collections, glob, gzip, json, os, random, sys

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
CODEX = os.path.join(ROOT, "target", "codex", "a10.jsonl.gz")
RUNS = ("20261011-211805", "20261011-231853")
ACT = {1: "Hive", 2: "Glory"}
FRUIT = {"STRAWBERRY": 7, "PEAR": 10, "MANGO": 14}
PICKUP_UP = {"WHETSTONE": "Attack", "WAR_PAINT": "Skill"}


def _runs_dir():
    d = ROOT
    while d != os.path.dirname(d):
        if os.path.isdir(os.path.join(d, "runs", RUNS[0])):
            return os.path.join(d, "runs")
        d = os.path.dirname(d)
    sys.exit("runs/ not found above " + ROOT)


def _codex_path():
    d = ROOT
    while d != os.path.dirname(d):
        p = os.path.join(d, "target", "codex", "a10.jsonl.gz")
        if os.path.exists(p):
            return p
        d = os.path.dirname(d)
    sys.exit("target/codex/a10.jsonl.gz not found")


def frame():
    """one pass over the community Ironclad runs: per act the elite / treasure relic draws, elite / hallway card offers, candidate elite decks,
    skip shares by room, card rewards per completed act"""
    import sts2, codex
    from agent import runmodel as R
    cards, sim_relics = set(sts2.names()["card"]), set(sts2.names()["relic"])
    relics = {r["id"] for r in R.pool("relics", "IRONCLAD") if r["rarity"] in ("Common", "Uncommon", "Rare")}
    F = dict(relic={a: [] for a in range(3)}, treasure={a: [] for a in range(3)}, offer={(a, k): [] for a in range(3) for k in ("elite", "monster")},
             decks={a: {True: [], False: []} for a in range(3)}, skip=collections.Counter(), offers=collections.Counter(), per_act={a: [] for a in range(3)})
    for r in codex.rows(_codex_path(), "ironclad"):
        if len(r["acts"]) < 3:
            continue
        cand, mx = {a: [] for a in range(3)}, 80
        for ai, i, g, f, now, rel, up in codex.walk(r):
            if ai > 2:
                break
            if f["t"] in ("elite", "treasure"):
                F["relic" if f["t"] == "elite" else "treasure"][ai] += [x for x, _ in f["relics"] if x in relics]
            if f["offer"] and f["t"] in ("elite", "monster", "boss"):
                F["offers"][(ai, f["t"])] += 1
                F["skip"][(ai, f["t"])] += not any(p for _, _, p in f["offer"])
                if f["t"] != "boss" and 3 <= len(f["offer"]) <= 5 and all(c in cards for c, _, _ in f["offer"]):
                    F["offer"][(ai, f["t"])].append([[c, u] for c, u, _ in f["offer"]])
            if f["t"] == "elite" and now and all(c in cards for c in now):
                deck = [[c, int(k < up[c])] for c in sorted(now) for k in range(now[c])]
                cand[ai].append(dict(deck=deck, relics=sorted(rel & sim_relics), max_hp=mx, floor=g))
            mx = f["mx"] or mx
        for ai, act in enumerate(r["floors"][:3]):
            if ai < len(r["floors"]) - 1 or r["win"]:
                F["per_act"][ai].append(sum(1 for f in act if f["offer"] and f["t"] in ("elite", "monster", "boss")))
            if cand[ai]:
                F["decks"][ai][r["win"]].append(dict(random.Random(r["hash"]).choice(cand[ai]), act_name=r["acts"][0].capitalize() if ai == 0 else ACT[ai], src="won" if r["win"] else "lost"))
    return F


def operator_decks():
    from agent import pools
    out, rd = [], _runs_dir()
    for run in RUNS:
        sc = [json.loads(l)["scenario"] for l in open(os.path.join(rd, run, "events.jsonl"), encoding="utf-8") if '"fight_start"' in l]
        v = next(n for n in ("Overgrowth", "Underdocks") if any(s["encounter"] in pools.pool(n, "boss") for s in sc))
        for s in sc:
            if s["encounter"].endswith("_ELITE"):
                out.append(dict(deck=[[c["id"], c.get("upgrade", 0)] for c in s["deck"]], relics=[x["id"] for x in s["relics"]], max_hp=s["max_hp"], act=s["act"],
                                act_name=v if s["act"] == 0 else ACT[s["act"]], src="operator", floor=s["total_floor"], enc=s["encounter"]))
    return out


def _template():
    rd = _runs_dir()
    return next(json.loads(l)["scenario"] for l in open(os.path.join(rd, RUNS[0], "events.jsonl"), encoding="utf-8") if '"fight_start"' in l)


def _with_relic(sc, rid, rng):
    from agent import runmodel as R
    sc = dict(sc, relics=sc["relics"] + [{"id": rid}], deck=[dict(c) for c in sc["deck"]])
    if rid in FRUIT:
        sc["max_hp"] += FRUIT[rid]
        sc["hp"] = sc["max_hp"]
    if rid in PICKUP_UP:
        info = {c["id"]: c for rows in R.CAT["cards"].values() for c in rows}
        idx = [i for i, c in enumerate(sc["deck"]) if info.get(c["id"], {}).get("type") == PICKUP_UP[rid] and c["upgrade"] < info[c["id"]].get("max_upgrade", 1)]
        for i in rng.sample(idx, min(2, len(idx))):
            sc["deck"][i]["upgrade"] += 1
    return sc


def measure(eng, d, F, n_relics, attempts, seed):
    """one deck: base, n_relics relic draws, one elite and one hallway offer, vs this act's bosses and the act-3 bosses"""
    import sts2
    from agent import pools
    cls = json.load(open(os.path.join(ROOT, "data", "relic_classes.json")))["relics"]
    rng, act = random.Random(seed), d["act"]
    base = dict(_template(), deck=[{"id": c, "upgrade": u} for c, u in d["deck"]], relics=[{"id": x} for x in d["relics"]], hp=d["max_hp"], max_hp=d["max_hp"],
                potions=[], act=act, gold=0)
    hz = {"act": pools.pool(d["act_name"], "boss")}
    if act < 2:
        hz["pair"] = pools.pool("Glory", "boss")
    bosses = [b for h in hz.values() for b in h]
    pool = [x for x in F["relic"][act] if x not in d["relics"]]
    rel = []
    while len(rel) < n_relics:
        x = rng.choice(pool)
        if x not in rel:
            rel.append(x)
    offers = [("elite", rng.choice(F["offer"][(act, "elite")])), ("monster", rng.choice(F["offer"][(act, "monster")]))]
    variants = [base] + [_with_relic(base, x, rng) for x in rel] + [dict(base, deck=base["deck"] + [{"id": c, "upgrade": u}]) for _, o in offers for c, u in o]
    scs = [dict(v, encounter=b, act=2 if b in pools.pool("Glory", "boss") else act, seed="rv") for v in variants for b in bosses]
    res = eng.solve(scs, attempts=attempts, seed=seed, groups=[i % len(bosses) for i in range(len(scs))])
    W = np.array([[np.nan if x is None else x for x in q["wins"]] for q in res], float).reshape(len(variants), len(bosses), -1)
    out = {}
    for h, bl in hz.items():
        j = [bosses.index(b) for b in bl]
        w = W[:, j, :]
        g = lambda sl: (np.nanmean(w[:, :, sl], (1, 2)) - np.nanmean(w[0, :, sl])).tolist()
        out[h] = dict(base=float(np.nanmean(w[0])), g=g(slice(None)), gA=g(slice(0, None, 2)), gB=g(slice(1, None, 2)))
    k = 1 + len(rel)
    return dict({x: d[x] for x in ("src", "act", "act_name", "floor", "max_hp") if x in d}, enc=d.get("enc"), n_cards=len(d["deck"]), relics_owned=d["relics"],
                relics=[dict(id=x, cls=cls.get(x, {}).get("class"), modelled=cls.get(x, {}).get("class") == "combat" or x in FRUIT or x in PICKUP_UP) for x in rel],
                offers=[dict(kind=kd, cards=o, span=[k + sum(len(p) for _, p in offers[:i]), k + sum(len(p) for _, p in offers[:i + 1])]) for i, (kd, o) in enumerate(offers)],
                hz=out, attempts=attempts, seed=seed)


def run(a):
    from agent.engine import Engine
    F = frame()
    meta = dict(meta=True, skip={f"{k[0]}:{k[1]}": [F["skip"][k], F["offers"][k]] for k in F["offers"]}, per_act={a_: [len(v), float(np.mean(v))] for a_, v in F["per_act"].items()},
                relic_draws={a_: len(v) for a_, v in F["relic"].items()}, offer_draws={f"{k[0]}:{k[1]}": len(v) for k, v in F["offer"].items()},
                decks={a_: {str(w): len(v) for w, v in x.items()} for a_, x in F["decks"].items()})
    print(json.dumps(meta), flush=True)
    done = set()
    if os.path.exists(a.out):
        done = {json.loads(l)["seed"] for l in open(a.out) if '"seed"' in l}
    else:
        open(a.out, "w").write(json.dumps(meta) + "\n")
    rng = random.Random(0)
    todo = [dict(d, act=d["act"]) for d in operator_decks()]
    for act in range(3):
        for w in (True, False):
            pool = F["decks"][act][w]
            todo += [dict(d, act=act) for d in rng.sample(pool, min(a.per_act // 2, len(pool)))]
    eng = Engine()
    for i, d in enumerate(todo):
        seed = 1000 + i
        if seed in done:
            continue
        row = measure(eng, d, F, a.relics, a.attempts, seed)
        with open(a.out, "a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"{i + 1}/{len(todo)} act {d['act'] + 1} {d['src']} base {row['hz']['act']['base']:.2f}", flush=True)


def _cross(gA, gB):
    """value of choosing the best of {cards, skip} on one half of the attempts, scored on the other (both ways); share of offers taken"""
    v, t = 0.0, 0.0
    for a, b in ((gA, gB), (gB, gA)):
        j = int(np.argmax(a))
        if a[j] > 0:
            v, t = v + b[j] / 2, t + 0.5
    return v, t


def _ms(x):
    x = np.asarray(x, float)
    return (float(x.mean()), float(x.std(ddof=1) / len(x) ** 0.5) if len(x) > 1 else float("nan"), len(x))


def per_deck(row, h):
    z = row["hz"][h]
    rel = [z["g"][1 + i] for i in range(len(row["relics"]))]
    out = dict(base=z["base"], relic=float(np.mean(rel)), relic_draws=rel)
    for o in row["offers"]:
        s, e = o["span"]
        v, t = _cross(z["gA"][s:e], z["gB"][s:e])
        out["card_" + o["kind"]], out["take_" + o["kind"]], out["naive_" + o["kind"]] = v, t, max(0.0, max(z["g"][s:e]))
    out["elite"] = out["relic"] + out["card_elite"] - out["card_monster"]
    return out


def operator_skip():
    n = s = 0
    for run in RUNS:
        for l in open(os.path.join(_runs_dir(), run, "events.jsonl"), encoding="utf-8"):
            e = json.loads(l)
            if e["kind"] == "macro" and e.get("screen") == "CARD_REWARD":
                ch = str(e.get("choice", "")).split()[0]
                n += 1
                s += any(x.startswith(ch + " Skip") for x in e["state"].splitlines())
    return s, n


def report(a):
    rows = [json.loads(l) for l in open(a.out)]
    meta, rows = rows[0], rows[1:]
    L, J = [], dict(E="E103", note="search vs the boss pool at full HP, no potions; P(win) points per reward; lower bound for relics (out-of-combat effects score 0)", acts={})
    keys = ("relic", "card_elite", "card_monster", "elite")
    L.append(f"reward values (E103): gain in P(win) vs the boss pool, full HP, no potions; mean +- se over decks (n decks; {rows[0]['attempts']} attempts per boss, paired)")
    for act in range(3):
        J["acts"][str(act + 1)] = {}
        for h in ("act", "pair"):
            for src, sel in (("community", lambda r: r["src"] in ("won", "lost")), ("operator", lambda r: r["src"] == "operator")):
                R = [r for r in rows if r["act"] == act and sel(r) and h in r["hz"]]
                if act == 2 and h == "pair":
                    R = [r for r in rows if r["act"] == act and sel(r)]
                    hh = "act"
                else:
                    hh = h
                if not R:
                    continue
                P = [per_deck(r, hh) for r in R]
                st = {k: _ms([p[k] for p in P]) for k in keys}
                draws = [x for r in R for x in r["relics"]]
                zero = sum(not x["modelled"] for x in draws) / len(draws)
                L.append(f"act {act + 1} {'this act boss' if h == 'act' else 'act-3 bosses'} {src:9s} n {len(R):3d} base {np.mean([p['base'] for p in P]):.3f} (floored < 0.02: {np.mean([p['base'] < 0.02 for p in P]):.0%}): "
                         + "  ".join(f"{k} {m:+.4f}+-{s:.4f}" for k, (m, s, _) in st.items())
                         + f"  | cross-fit policy takes (half-sample best > 0) elite {np.mean([p['take_elite'] for p in P]):.2f} hallway {np.mean([p['take_monster'] for p in P]):.2f}; naive max elite {np.mean([p['naive_elite'] for p in P]):+.4f}"
                         + f"; relic draws {len(draws)}, scored 0 by construction {zero:.0%}")
                if src == "community":
                    J["acts"][str(act + 1)][h] = {k: [round(m, 4), round(s, 4), n] for k, (m, s, n) in st.items()}
                    if len(P) >= 9:
                        S = sorted(P, key=lambda p: p["base"])
                        T = [S[len(S) * i // 3:len(S) * (i + 1) // 3] for i in range(3)]
                        L.append("    by base strength tertile (base range / relic / card elite / elite +- se): " + "; ".join(
                            f"{t[0]['base']:.2f}-{t[-1]['base']:.2f} / {np.mean([p['relic'] for p in t]):+.3f} / {np.mean([p['card_elite'] for p in t]):+.3f} / "
                            f"{_ms([p['elite'] for p in t])[0]:+.3f} +- {_ms([p['elite'] for p in t])[1]:.3f} (n {len(t)})" for t in T))
                        J["acts"][str(act + 1)][h]["elite_by_base"] = [[round(t[-1]["base"], 3)] + [round(x, 4) for x in _ms([p["elite"] for p in t])[:2]] + [len(t)] for t in T]
                    for w in ("won", "lost"):
                        Pw = [per_deck(r, hh) for r in R if r["src"] == w]
                        if Pw:
                            L.append(f"    {w:4s} n {len(Pw):3d} base {np.mean([p['base'] for p in Pw]):.3f} relic {np.mean([p['relic'] for p in Pw]):+.4f} card elite {np.mean([p['card_elite'] for p in Pw]):+.4f} elite {np.mean([p['elite'] for p in Pw]):+.4f}")
    J["acts"] = {k: {"act": x["act"]} for k, x in J["acts"].items()}
    by = collections.defaultdict(list)
    for r in (r for r in rows if r["act"] == 2):
        for i, x in enumerate(r["relics"]):
            by[x["id"]].append(r["hz"]["act"]["g"][1 + i])
    top = sorted(((np.mean(v), k, len(v)) for k, v in by.items() if len(v) >= 3), reverse=True)
    L.append("relics with >= 3 draws on act-3 decks, mean gain vs the act-3 bosses (high / low; share scored 0 by construction above): " + ", ".join(f"{k} {m:+.3f} ({n})" for m, k, n in top[:8]) + " ... " + ", ".join(f"{k} {m:+.3f} ({n})" for m, k, n in top[-4:]))
    sk = {k: v for k, v in meta["skip"].items()}
    osk = operator_skip()
    J["skip"] = dict(community={k: round(v[0] / v[1], 3) for k, v in sk.items()}, operator=[osk[0], osk[1]])
    J["card_rewards_per_act"] = [round(meta["per_act"][str(a_)][1], 2) for a_ in range(3)]
    L.append("community card-reward skip share (act:room n): " + ", ".join(f"{k} {v[0] / v[1]:.2f} ({v[1]})" for k, v in sorted(sk.items()) if v[1] > 100)
             + f"; operator runs 1-2: {osk[0]}/{osk[1]} = {osk[0] / max(osk[1], 1):.2f}")
    L.append("card rewards per completed act (community, incl. boss): " + ", ".join(f"act {int(k) + 1} {v[1]:.1f} (n {v[0]})" for k, v in sorted(meta["per_act"].items())))
    text = "\n".join(L) + "\n"
    print(text)
    open(os.path.splitext(a.out)[0] + ".txt", "w").write(text)
    if a.json:
        json.dump(J, open(a.json, "w"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report"])
    ap.add_argument("--per-act", type=int, default=40)
    ap.add_argument("--relics", type=int, default=5)
    ap.add_argument("--attempts", type=int, default=32)
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "review", "reward_values.jsonl"))
    ap.add_argument("--json")
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    (run if a.cmd == "run" else report)(a)


if __name__ == "__main__":
    main()
