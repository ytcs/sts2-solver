#!/usr/bin/env python3
import argparse, glob, json, os, random, re, sys, time
from collections import Counter

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))
OUT = os.path.join(ROOT, "data", "bench")
LABEL_CKPT = os.path.join(ROOT, "models", "solver_h128.pt")
ATTEMPTS, PAIR_ATTEMPTS, SHUFFLES = 8, 256, 8
H128_SETS = ("eval", "corpus", "mix", "tail")
SETS = H128_SETS + ("plans",)
PLANS_ATTEMPTS = 16
ACT_FIRST_FLOOR = (2, 19, 35)
POOL_KINDS = ("weak", "regular", "elite", "boss")
RARITY_RANK = {"Rare": 3, "Uncommon": 2, "Common": 1}


def _load(p):
    return json.load(open(os.path.join(ROOT, p)))


def _curriculum(n, seed):
    import subprocess
    out = os.path.join(ROOT, "target", "bench", f"cur_{seed}.json")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if not os.path.exists(out):
        subprocess.run([sys.executable, os.path.join(ROOT, "tools", "gen_curriculum.py"), "--n", str(n), "--seed", str(seed), "--out", out], check=True)
    return json.load(open(out))


def _cid(c):
    return c if isinstance(c, str) else c["id"]


def _deck_ids(sc):
    return [c if isinstance(c, str) else c["id"] for c in sc["deck"]]


def _multiplayer_only():
    cat = _load("data/catalog.json")
    return {c["id"] for pool in cat["cards"].values() for c in pool if c.get("multiplayer_only")}


def _gen_train(n, seed):
    import subprocess
    out = os.path.join(ROOT, "target", "bench", f"train_{seed}.json")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if not os.path.exists(out):
        subprocess.run([sys.executable, os.path.join(ROOT, "tools", "gen_train.py"), "--n", str(n), "--seed", str(seed), "--out", out], check=True)
    return json.load(open(out))


def scenarios(rng):
    mp = _multiplayer_only()
    clean = lambda xs: [x for x in xs if not any(_cid(c) in mp for c in x["deck"])]  # noqa: E731
    s = {"eval": clean(_gen_train(800, 122))[:600], "corpus": clean(_load("data/corpus/fights_holdout.json"))}
    mix = clean(_curriculum(4000, 141))
    rng.shuffle(mix)
    s["mix"] = mix[:600]
    tail = [x for x in clean(_curriculum(20000, 143)) if x["act"] == 2 and len(x["deck"]) >= 28 and x["encounter"].endswith(("_ELITE", "_BOSS"))]
    s["tail"] = tail[:400]
    return s


def pairs(base, rng):
    cat = _load("data/catalog.json")["cards"]
    out = []
    for sc in base:
        kind = rng.choice(["add", "remove", "upgrade", "potion"] if sc.get("potions") else ["add", "remove", "upgrade"])
        v = json.loads(json.dumps(sc))
        if kind == "add":
            pool = [c["id"] for c in cat.get(sc["character"], []) if c["rarity"] in ("Common", "Uncommon", "Rare") and not c.get("multiplayer_only")]
            v["deck"].append(rng.choice(pool))
        elif kind == "remove":
            ok = [i for i, c in enumerate(_deck_ids(sc)) if c not in ("ASCENDERS_BANE",)]
            v["deck"].pop(rng.choice(ok))
        elif kind == "upgrade":
            ok = [i for i, c in enumerate(sc["deck"]) if isinstance(c, str) or not c.get("upgrade")]
            if not ok:
                continue
            i = rng.choice(ok)
            c = v["deck"][i]
            v["deck"][i] = {"id": c, "upgrade": 1} if isinstance(c, str) else {**c, "upgrade": 1}
        else:
            v["potions"].pop(rng.randrange(len(v["potions"])))
        v["name"] = sc["name"] + "_" + kind
        out.append((sc, v, kind))
    return out


def _tag(sc, name, **meta):
    return dict(sc, name=name, meta=meta)


def _act_pool_name(act, encounter):
    from agent import pools
    return next((n for n, d in pools.ACTS.items() if d["act"] == act and any(encounter in pools.pool(n, k) for k in POOL_KINDS)), None)


def _gate_encounters(act_name):
    from agent import pools
    return pools.pool(act_name, "elite") + pools.pool(act_name, "boss")


def _enabler(sc):
    cat = {c["id"]: c for c in _load("data/catalog.json")["cards"].get(sc["character"], [])}
    buckets = _load("data/card_buckets_ironclad.json") if sc["character"] == "IRONCLAD" else {}
    scaling = lambda i: bool({"SD", "SB", "ACC"} & set(buckets.get(i, {}).get("buckets", [])))  # noqa: E731
    cands = [i for i in dict.fromkeys(_deck_ids(sc)) if i in cat and (cat[i]["type"] == "Power" or scaling(i))]
    return max(cands, key=lambda i: (cat[i]["type"] == "Power", RARITY_RANK.get(cat[i]["rarity"], 0), scaling(i)), default=None)


def _without(sc, card):
    i = _deck_ids(sc).index(card)
    return dict(sc, deck=sc["deck"][:i] + sc["deck"][i + 1:])


def plan_targets(p):
    """{(act, hp): encounters}: each threat's act and HP, against the threat and that act's elites and bosses"""
    from agent import plans, pools
    targets = {}
    for t in p["threats"]:
        act, hp = t.get("act", 1), t.get("hp") or plans.STARTERS[p["character"]][2]
        encs = targets.setdefault((act, hp), [])
        for e in [t["id"]] + [e for n in pools.act_names(act) for e in _gate_encounters(n)]:
            if e not in encs:
                encs.append(e)
    return targets


def plan_slice():
    from agent import plans, pools
    mp = _multiplayer_only()
    out = []
    for p in plans.load():
        if mp & set(p["core"] + p.get("support", [])):
            continue
        cores = list(dict.fromkeys(p["core"]))
        versions = [("full", None, None), ("core", None, None)] + [(f"-{c}", "full", c) for c in cores] + [(f"core-{c}", "core", c) for c in cores]
        for (act, hp), encs in plan_targets(p).items():
            for v, base, card in versions:
                for e in encs:
                    out.append(_tag(plans.scenario(p, e, act, hp, v), f"plan:{p['id']}:{v}@{e}", source="plan", character=p["character"], archetype=p["archetype"],
                                    family=f"plan:{p['id']}", deck=f"plan:{p['id']}:{v}:a{act}:hp{hp}", base=base and f"plan:{p['id']}:{base}:a{act}:hp{hp}",
                                    enabler=card, act_name=pools.act_names(act)[0]))
    return out


def _plausible_start(sc, prev_act):
    from agent import plans
    if sc["act"] > 0:
        return sc.get("total_floor") == ACT_FIRST_FLOOR[sc["act"]] or prev_act == sc["act"] - 1
    starter = plans.STARTERS.get(sc["character"])
    relics = {_cid(r) for r in sc["relics"]}
    return (sc.get("total_floor") == ACT_FIRST_FLOOR[0] and starter is not None and starter[1] in relics and len(relics) <= 3
            and len(sc["deck"]) <= len(starter[0]) + 4)


def recorded_slice(runs_dir):
    """act-start decks (first fight of an act) vs their act's elites and bosses, with a minus-enabler twin when the deck has a Power or a
    scaling card; plus every elite and boss fight as recorded, on the same plausible sequence (floors rising, encounters in the act's pools).
    Only runs played on the map: harness test fixtures start in combat"""
    starts, fights = {}, {}
    for path in sorted(glob.glob(os.path.join(runs_dir, "*", "events.jsonl"))):
        run = os.path.basename(os.path.dirname(path))
        events = []
        for raw in open(path, encoding="utf-8"):
            try:
                events.append(json.loads(raw))
            except ValueError:
                continue
        if not any(e.get("kind") == "macro" and e.get("screen") == "MAP" for e in events):
            continue
        seq = None
        for e in events:
            sc = e.get("scenario") if e.get("kind") == "fight_start" else None
            if not sc:
                continue
            act, floor, enc = sc["act"], sc.get("total_floor", 0), e["encounter"]
            name = _act_pool_name(act, enc)
            if name and _plausible_start(sc, seq and seq[0]) and not (seq and seq[0] == act):
                seq = (act, name, floor, f"run:{run}:a{act}")
                key = json.dumps([sc["character"], act, sorted(map(json.dumps, sc["deck"])), sorted(map(json.dumps, sc["relics"])), sc["hp"], sc.get("potions")])
                starts.setdefault(key, (sc, name, seq[3], run))
            elif seq is None or act != seq[0] or name != seq[1] or floor < seq[2]:
                seq = None
                continue
            seq = (act, name, floor, seq[3])
            if enc.endswith(("_ELITE", "_BOSS")):
                key = json.dumps([enc, sorted(map(json.dumps, sc["deck"])), sorted(map(json.dumps, sc["relics"])), sc["hp"], sc.get("potions")])
                fights.setdefault(key, (sc, name, seq[3], run))
    out = []
    for sc, name, deck, run in starts.values():
        base = dict(sc, potions=sc.get("potions", []))
        card = _enabler(base)
        meta = dict(source="recorded", character=sc["character"], archetype="recorded act start", family=deck, act_name=name, run=run)
        for e in _gate_encounters(name):
            out.append(_tag(dict(base, encounter=e), f"{deck}@{e}", deck=deck, base=None, enabler=None, **meta))
            if card:
                out.append(_tag(dict(_without(base, card), encounter=e), f"{deck}:-{card}@{e}", deck=f"{deck}:-{card}", base=deck, enabler=card, **meta))
    for sc, name, deck, run in fights.values():
        out.append(_tag(dict(sc, potions=sc.get("potions", [])), f"{deck}:f{sc['total_floor']}@{sc['encounter']}", source="recorded", character=sc["character"],
                        archetype="recorded fight", family=deck, deck=f"{deck}:f{sc['total_floor']}", base=None, enabler=None, act_name=name, run=run))
    return out


def _expert_card(s):
    m = re.match(r"^([A-Z_]+)(\+?)(?:\((\w+)\))?\??(?: x(\d+))?$", s.strip())
    cid, up, ench, n = m.groups()
    c = {"id": cid, "upgrade": int(bool(up))}
    if ench:
        c["enchantment"] = {"id": ench.upper(), "amount": 1}
    return [c] * int(n or 1)


def _hp(s):
    return tuple(int(x) for x in re.match(r"(\d+)/(\d+)", s).groups())


def expert_slice():
    """the act boundaries that list a deck (next act's start: + the ancient's relic, at the ancient's HP) and the fights with a full deck"""
    import sts2
    from agent import pools
    out = []
    names = set(sts2.names()["relic"]) | set(sts2.names()["potion"])
    clean = lambda xs: [x.rstrip("?") for x in xs if x.rstrip("?") in names]  # noqa: E731
    for path in sorted(glob.glob(os.path.join(ROOT, "data", "expert", "*.json"))):
        d = json.load(open(path))
        who, ch, acts = os.path.splitext(os.path.basename(path))[0], d["meta"]["character"], d["meta"]["acts"]
        base = dict(ascension=d["meta"]["ascension"], character=ch, max_energy=3, base_orb_slots=3 if ch == "DEFECT" else 0, max_potion_slots=2, gold=0, potions=[])
        macro = d["macro"]
        for i, m in enumerate(macro):
            if m["kind"] != "act_boundary" or not m.get("deck"):
                continue
            act = sum(x["kind"] == "act_boundary" for x in macro[:i + 1])
            anc = next((x for x in macro[i + 1:] if x["kind"] == "ancient"), {})
            hp, max_hp = _hp(anc.get("hp") or m["hp"])
            deck = [c for s in m["deck"] for c in _expert_card(s)]
            sc = dict(base, hp=hp, max_hp=max_hp, act=act, total_floor=ACT_FIRST_FLOOR[act], seed=f"{who}-a{act}", deck=deck,
                      relics=clean(m["relics"] + [anc.get("choice", "")]))
            fam = f"expert:{who}:a{act}"
            for e in _gate_encounters(acts[act]):
                out.append(_tag(dict(sc, encounter=e), f"{fam}@{e}", source="expert", character=ch, archetype="expert act start", family=fam, deck=fam,
                                base=None, enabler=None, act_name=acts[act]))
        for f in d["fights"]:
            st = f["start"]
            if not st.get("deck"):
                continue
            enc = f["encounter"].rstrip("?")
            act = next(i for i, a in enumerate(acts) if enc in _gate_encounters(a) + pools.pool(a, "weak") + pools.pool(a, "regular"))
            hp, max_hp = _hp(st["hp"])
            sc = dict(base, encounter=enc, hp=hp, max_hp=max_hp, act=act, total_floor=f["floor"], seed=f"{who}-{f['id']}",
                      deck=[c for s in st["deck"] for c in _expert_card(s)], relics=clean(st["relics"]),
                      potions=[{"id": p, "slot": k} for k, p in enumerate(clean(st.get("potions", [])))])
            fam = f"expert:{who}:f{f['floor']}"
            out.append(_tag(sc, f"{fam}@{enc}", source="expert", character=ch, archetype="expert fight", family=fam, deck=fam, base=None, enabler=None,
                            act_name=acts[act]))
    return out


def plans_slice(runs_dir):
    import sts2
    mp = _multiplayer_only()
    keep, dropped = [], {}
    for sc in plan_slice() + recorded_slice(runs_dir) + expert_slice():
        why = "multiplayer-only card" if any(_cid(c) in mp for c in sc["deck"]) else None
        if why is None:
            try:
                sts2.Sim(json.dumps(sc), 0)
            except Exception as ex:  # noqa: BLE001
                why = f"simulator: {str(ex)[:60]}"
        if why:
            dropped[why] = dropped.get(why, 0) + 1
        else:
            keep.append(sc)
    return keep, dropped


def build_plans(a):
    from solver import DEFAULT_CKPT, Solver
    path = os.path.join(OUT, "plans.json")
    if os.path.exists(path) and not a.force:
        print(f"{path} exists (--force to rebuild)")
        return
    scen, dropped = plans_slice(a.runs)
    print(f"{len(scen)} fights (dropped {dropped}); " + ", ".join(f"{k}: {v}" for k, v in Counter((s["meta"]["source"], s["character"]) for s in scen).items()), flush=True)
    if a.dry:
        return
    root = {}
    groups = [root.setdefault((s["meta"]["base"] or s["meta"]["deck"], s["encounter"]), len(root)) for s in scen]
    part = os.path.join(ROOT, "target", "bench", "plans_partial.json")
    os.makedirs(os.path.dirname(part), exist_ok=True)
    names = [s["name"] for s in scen]
    done = json.load(open(part)) if os.path.exists(part) else {}
    if done.get("names") != names:
        done = {"names": names, "chunks": {}}
    S = Solver(DEFAULT_CKPT, M=5, K=32, cover=True, roots=a.roots, threads=a.threads)
    t0 = time.time()
    for k in range(0, len(scen), a.chunk):
        if str(k) in done["chunks"]:
            continue
        res = S.solve(scen[k:k + a.chunk], attempts=PLANS_ATTEMPTS, seed=11, groups=groups[k:k + a.chunk])
        done["chunks"][str(k)] = [dict(wins=r["wins"], ends=r["ends_abs"]) for r in res]
        json.dump(done, open(part, "w"))
        print(f"  {min(k + a.chunk, len(scen))}/{len(scen)} ({time.time() - t0:.0f}s)", flush=True)
    labels = [r for k in range(0, len(scen), a.chunk) for r in done["chunks"][str(k)]]
    json.dump([dict(scenario=sc, **r) for sc, r in zip(scen, labels)], open(path, "w"))
    print(f"plans: {len(scen)} fights x {PLANS_ATTEMPTS} by {os.path.basename(DEFAULT_CKPT)} 5x32 cover roots {a.roots}, "
          f"win {np.mean([np.mean([w for w in r['wins'] if w is not None]) for r in labels]):.3f}", flush=True)


def relabel(a):
    from solver import DEFAULT_CKPT, Solver
    S = Solver(DEFAULT_CKPT, M=5, K=32, cover=True, roots=a.roots)
    meta_path = os.path.join(OUT, "labels.json")
    meta = json.load(open(meta_path)) if os.path.exists(meta_path) else {}
    for name in a.sets:
        path = os.path.join(OUT, name + ".json")
        rows = json.load(open(path))
        attempts = PLANS_ATTEMPTS if name == "plans" else ATTEMPTS
        t0 = time.time()
        res = S.solve([r["scenario"] for r in rows], attempts=attempts, seed=11)
        json.dump([dict(scenario=r["scenario"], wins=x["wins"], ends=x["ends_abs"]) for r, x in zip(rows, res)], open(path, "w"))
        meta[name] = dict(labeler=os.path.basename(DEFAULT_CKPT), search="5x32 cover", attempts=attempts, seed=11)
        json.dump(meta, open(meta_path, "w"), indent=1)
        print(f"{name}: {len(rows)} fights x {attempts} by {os.path.basename(DEFAULT_CKPT)} in {time.time() - t0:.0f}s, "
              f"win {np.mean([x['win'] for x in res]):.3f}", flush=True)


def build(a):
    from solver import Solver
    rng = random.Random(7)
    os.makedirs(OUT, exist_ok=True)
    S = Solver(LABEL_CKPT, M=5, K=32)
    sets = scenarios(rng)
    for name, scen in sets.items():
        path = os.path.join(OUT, name + ".json")
        if os.path.exists(path) and not a.force:
            continue
        t0 = time.time()
        res = S.solve(scen, attempts=ATTEMPTS, seed=11)
        json.dump([dict(scenario=sc, wins=r["wins"], ends=r["ends_abs"]) for sc, r in zip(scen, res)], open(path, "w"))
        print(f"{name}: {len(scen)} fights x {ATTEMPTS} in {time.time() - t0:.0f}s, win {np.mean([r['win'] for r in res]):.3f}", flush=True)
    path = os.path.join(OUT, "pairs.json")
    if not os.path.exists(path) or a.force:
        base = sets["eval"][:200] + sets["mix"][:150] + sets["tail"][:100] + sets["corpus"][:100]
        tri = pairs(base, rng)[:a.pairs]
        flat = [x for b, v, _ in tri for x in (b, v)]
        groups = [i // 2 for i in range(len(flat))]
        t0 = time.time()
        res = Solver(LABEL_CKPT, M=3, K=8).solve(flat, attempts=PAIR_ATTEMPTS, seed=13, groups=groups)
        rows = []
        for i, (b, v, kind) in enumerate(tri):
            rb, rv = res[2 * i], res[2 * i + 1]
            rows.append(dict(base=b, variant=v, kind=kind, wins=[rb["wins"], rv["wins"]], ends=[rb["ends_abs"], rv["ends_abs"]]))
        json.dump(rows, open(path, "w"))
        print(f"pairs: {len(rows)} x 2 x {PAIR_ATTEMPTS} in {time.time() - t0:.0f}s", flush=True)


def predict(net, scen, shuffles=SHUFFLES):
    from predictor import Predictor
    return Predictor(net).fight_start(scen, shuffles)


def _worth(P, max_hp):
    import heads as H
    c = H.CENTERS
    u = np.concatenate([np.full((len(P), 1), H.LOSS), H.WIN + H.HP_BONUS * np.minimum(c[None, :] / np.asarray(max_hp, float)[:, None], 1.0)], 1)
    return (P * u).sum(1)


def _rps(P, wins, ends):
    import heads as H
    cdf = np.cumsum(P)
    out = []
    for w, e in zip(wins, ends):
        if w is None:
            continue
        k = int(H.end_class(w, e or 0))
        obs = (np.arange(H.NC) >= k).astype(float)
        out.append(((cdf - obs) ** 2).sum() / (H.NC - 1))
    return float(np.mean(out)) if out else float("nan")


def _spearman(a, b):
    r = lambda x: np.argsort(np.argsort(x)).astype(float)  # noqa: E731
    return float(np.corrcoef(r(a), r(b))[0, 1])


def _pearson(a, b):
    return float(np.corrcoef(a, b)[0, 1])


def _plans_report(rows, pw, y, worst=10):
    """per group and per deck (mean over its fights): solver vs predicted P(win); gate = |bias| < 0.05 and deck Spearman >= 0.8. Pairs: deck
    vs the same deck minus one enabler at the same encounter (shared seeds), solver difference vs predicted difference"""
    meta = [r["scenario"]["meta"] for r in rows]
    enc = [r["scenario"]["encounter"] for r in rows]
    for key in ("source", "archetype"):
        line = []
        for g in sorted({m[key] for m in meta}):
            s = np.array([m[key] == g for m in meta])
            line.append(f"{g} n{s.sum()} {y[s].mean():.2f}->{pw[s].mean():.2f} ({pw[s].mean() - y[s].mean():+.3f}, brier {np.mean((pw[s] - y[s]) ** 2):.3f})")
        print("        " + " | ".join(line))
    decks = sorted({m["deck"] for m in meta})
    sel = {d: np.array([m["deck"] == d for m in meta]) for d in decks}
    dy, dp = np.array([y[sel[d]].mean() for d in decks]), np.array([pw[sel[d]].mean() for d in decks])
    multi = np.array([sel[d].sum() >= 3 for d in decks])
    print(f"decks   n{len(decks)} spearman {_spearman(dp, dy):.3f} pearson {_pearson(dp, dy):.3f} |bias| {np.abs(dp - dy).mean():.3f}"
          f"  | {multi.sum()} decks with >= 3 fights: spearman {_spearman(dp[multi], dy[multi]):.3f}  | fights spearman {_spearman(pw, y):.3f}")
    for i in np.argsort(-np.abs(dp - dy))[:worst]:
        print(f"        {decks[i]:52s} n{sel[decks[i]].sum()} solver {dy[i]:.2f} pred {dp[i]:.2f}")
    at = {(m["deck"], e): i for i, (m, e) in enumerate(zip(meta, enc))}
    pairs = [(at[(m["base"], e)], i) for i, (m, e) in enumerate(zip(meta, enc)) if m["base"] and (m["base"], e) in at]
    if not pairs:
        return
    ds, se = [], []
    for b, v in pairs:
        d = np.array([wv - wb for wb, wv in zip(rows[b]["wins"], rows[v]["wins"]) if wb is not None and wv is not None])
        ds.append(d.mean()); se.append(d.std(ddof=1) / len(d) ** 0.5 if len(d) > 1 else 1.0)
    ds, se = np.array(ds), np.array(se)
    dpred = np.array([pw[v] - pw[b] for b, v in pairs])
    for label, s in [("all", np.ones(len(pairs), bool))] + [(src, np.array([meta[v]["source"] == src for _, v in pairs])) for src in sorted({meta[v]["source"] for _, v in pairs})]:
        sig = s & (np.abs(ds) > 2 * se)
        agree = np.mean(np.sign(dpred[sig]) == np.sign(ds[sig])) if sig.any() else float("nan")
        print(f"pairs   {label:8s} n{s.sum():3d} solver d {ds[s].mean():+.3f} (|d| {np.abs(ds[s]).mean():.3f}) pred d {dpred[s].mean():+.3f} (|d| {np.abs(dpred[s]).mean():.3f})"
              f"  sign agreement {agree:.2f} on {sig.sum()} significant  spearman {_spearman(dpred[s], ds[s]):.3f} pearson {_pearson(dpred[s], ds[s]):.3f}")


def score_net(ck):
    from model import load
    net = load(ck, set_version=False)
    print(f"\n== {os.path.basename(ck)}")
    for name in SETS:
        path = os.path.join(OUT, name + ".json")
        if not os.path.exists(path):
            continue
        rows = json.load(open(path))
        scen = [r["scenario"] for r in rows]
        P = predict(net, scen)
        pw = 1 - P[:, 0]
        y = np.array([np.mean([w for w in r["wins"] if w is not None]) for r in rows])
        rps = np.mean([_rps(P[i], r["wins"], r["ends"]) for i, r in enumerate(rows)])
        rel = []
        for lo in (0, 0.2, 0.4, 0.6, 0.8):
            sel = (pw >= lo) & (pw < lo + 0.2 + 1e-9)
            if sel.sum():
                rel.append(f"{lo:.1f}-{lo + .2:.1f}: n{sel.sum()} {pw[sel].mean():.2f}->{y[sel].mean():.2f}")
        print(f"{name:7s} n{len(rows):4d} win {y.mean():.3f} pred {pw.mean():.3f} bias {pw.mean() - y.mean():+.3f} brier {np.mean((pw - y) ** 2):.4f} rps {rps:.4f}")
        print("        " + " | ".join(rel))
        chars = sorted({s["character"] for s in scen})
        print("        " + "  ".join(f"{c[:4]} {np.mean(pw[[s['character'] == c for s in scen]] - y[[s['character'] == c for s in scen]]):+.3f}" for c in chars))
        if name == "plans":
            _plans_report(rows, pw, y)
    path = os.path.join(OUT, "pairs.json")
    if os.path.exists(path):
        rows = json.load(open(path))
        Pb = predict(net, [r["base"] for r in rows])
        Pv = predict(net, [r["variant"] for r in rows])
        mh = [r["base"]["max_hp"] for r in rows]
        dpw = (1 - Pv[:, 0]) - (1 - Pb[:, 0])
        dwo = _worth(Pv, mh) - _worth(Pb, mh)
        ref_w, ref_se, ref_u = [], [], []
        for r in rows:
            wb, wv = r["wins"]
            d = np.array([v - b for b, v in zip(wb, wv) if b is not None and v is not None])
            ref_w.append(d.mean()); ref_se.append(d.std(ddof=1) / len(d) ** 0.5 if len(d) > 1 else 1.0)
            eb, ev = r["ends"]
            ub = [(1 + 0.5 * min((e or 0) / r["base"]["max_hp"], 1)) if w else -1 for w, e in zip(wb, eb) if w is not None]
            uv = [(1 + 0.5 * min((e or 0) / r["base"]["max_hp"], 1)) if w else -1 for w, e in zip(wv, ev) if w is not None]
            ref_u.append(np.mean(uv) - np.mean(ub))
        ref_w, ref_se, ref_u = map(np.array, (ref_w, ref_se, ref_u))
        sig = np.abs(ref_w) > 2 * ref_se
        agree_w = np.mean(np.sign(dpw[sig]) == np.sign(ref_w[sig])) if sig.any() else float("nan")
        big_u = np.abs(ref_u) > 0.02
        agree_u = np.mean(np.sign(dwo[big_u]) == np.sign(ref_u[big_u])) if big_u.any() else float("nan")
        print(f"pairs   n{len(rows)}  P(win) sign agreement {agree_w:.3f} on {sig.sum()} significant  | worth sign agreement {agree_u:.3f} on {big_u.sum()} with |d|>0.02"
              f"  | spearman P(win) {_spearman(dpw, ref_w):.3f} worth {_spearman(dwo, ref_u):.3f}")
        kinds = sorted({r["kind"] for r in rows})
        print("        " + "  ".join(f"{k} {np.mean([np.sign(dwo[i]) == np.sign(ref_u[i]) for i in range(len(rows)) if rows[i]['kind'] == k and big_u[i]]):.2f}" for k in kinds))


def _greedy(net, scen, per_env, seed):
    import sts2
    import heads as H
    from model import DEV
    env = sts2.VecEnv(len(scen), [json.dumps(x) for x in scen], seed=seed, max_steps=600, win=1.0, loss=-1.0, hp_bonus=0.5, round_robin=True, turn_cap=H.TURN_CAP,
                      obs_version=getattr(net, "obs_version", 1))
    obs, mask = env.reset()
    got = np.zeros(len(scen), np.int32)
    out = np.full((len(scen), per_env), np.nan)
    with torch.no_grad():
        while got.min() < per_env:
            lg, _ = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV))
            obs, mask, _r, d, info = env.step(lg.argmax(1).cpu().numpy().astype(np.int32))
            ei = None
            for i in np.nonzero(d)[0]:
                if got[i] < per_env:
                    ei = env.episode_info() if ei is None else ei
                    oc = int(info["outcome"][i])
                    out[int(ei["scenario"][i]), got[i]] = 1.0 if oc == 1 else (0.0 if oc in (-1, 2) else np.nan)
                    got[i] += 1
    return out


def screen(cks, per_env=4, seed=5):
    from model import load
    sets = {n: json.load(open(os.path.join(OUT, n + ".json"))) for n in SETS if os.path.exists(os.path.join(OUT, n + ".json"))}
    base = {}
    for k, ck in enumerate(cks):
        net = load(ck, set_version=False).eval()
        t0 = time.time()
        line = []
        for name, rows in sets.items():
            scen = [r["scenario"] for r in rows]
            w = _greedy(net, scen, per_env, seed)
            P = predict(net, scen)
            y = np.array([np.mean([x for x in r["wins"] if x is not None]) for r in rows])
            pw = 1 - P[:, 0]
            if k == 0:
                base[name] = w
                line.append(f"{name} greedy {np.nanmean(w):.3f} brier {np.mean((pw - y) ** 2):.4f} bias {pw.mean() - y.mean():+.3f}")
            else:
                d = np.nanmean(w - base[name], 1)
                d = d[np.isfinite(d)]
                line.append(f"{name} greedy {np.nanmean(w):.3f} ({d.mean():+.3f} +- {d.std(ddof=1) / len(d) ** 0.5:.3f}) brier {np.mean((pw - y) ** 2):.4f} bias {pw.mean() - y.mean():+.3f}")
        print(f"{os.path.basename(ck):24s} ({time.time() - t0:.0f}s) " + " | ".join(line), flush=True)


def play(ck, roots=None, cover=False):
    from solver import Solver
    S = Solver(ck, M=5, K=32, roots=roots, cover=cover)
    print(f"\n== play {os.path.basename(ck)}{' cover' if cover else ''} (vs the labels, same seeds: data/bench/labels.json)")
    for name in H128_SETS:
        path = os.path.join(OUT, name + ".json")
        if not os.path.exists(path):
            continue
        rows = json.load(open(path))
        res = S.solve([r["scenario"] for r in rows], attempts=ATTEMPTS, seed=11)
        dw = [[b - a for a, b in zip(r["wins"], x["wins"]) if a is not None and b is not None] for r, x in zip(rows, res)]
        dh = [[(b or 0) - (a or 0) for a, b in zip(r["ends"], x["ends_abs"]) if a is not None and b is not None] for r, x in zip(rows, res)]
        d = np.array([np.mean(v) for v in dw if v])
        hp = np.array([np.mean(v) for v in dh if v])
        gone = sum(not v for v in dw)
        print(f"{name:7s} win {np.mean([x['win'] for x in res]):.3f} vs {np.mean([np.mean([w for w in r['wins'] if w is not None]) for r in rows]):.3f}: "
              f"{d.mean():+.3f} +- {d.std(ddof=1) / len(d) ** 0.5:.3f}; end HP {hp.mean():+.2f} +- {hp.std(ddof=1) / len(hp) ** 0.5:.2f}"
              f"{f' ({gone} fights unpaired)' if gone else ''}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build"); b.add_argument("--force", action="store_true"); b.add_argument("--pairs", type=int, default=300)
    bp = sub.add_parser("build-plans", help="the plans slice: plan-library, recorded and expert decks, labelled by the live solver")
    bp.add_argument("--force", action="store_true"); bp.add_argument("--dry", action="store_true", help="assemble and validate only")
    bp.add_argument("--runs", default=os.path.join(ROOT, "runs")); bp.add_argument("--roots", type=int, default=512); bp.add_argument("--threads", type=int, default=None)
    bp.add_argument("--chunk", type=int, default=32, help="fights per solve call; finished chunks resume from target/bench/plans_partial.json")
    s = sub.add_parser("score"); s.add_argument("ckpts", nargs="+")
    pl = sub.add_parser("play"); pl.add_argument("ckpts", nargs="+"); pl.add_argument("--cover", action="store_true")
    rl_ = sub.add_parser("relabel"); rl_.add_argument("--sets", nargs="+", default=list(SETS)); rl_.add_argument("--roots", type=int, default=512)
    sc = sub.add_parser("screen"); sc.add_argument("ckpts", nargs="+", help="the first is the base the others are paired with")
    sc.add_argument("--per-env", type=int, default=4)
    pl.add_argument("--roots", type=int, default=None, help="fights in flight (default 2048 on CUDA); fewer = less host and GPU memory")
    a = ap.parse_args()
    if a.cmd == "build":
        build(a)
    elif a.cmd == "build-plans":
        build_plans(a)
    elif a.cmd == "play":
        for ck in a.ckpts:
            play(ck, a.roots, a.cover)
    elif a.cmd == "screen":
        screen(a.ckpts, a.per_env)
    elif a.cmd == "relabel":
        relabel(a)
    else:
        for ck in a.ckpts:
            score_net(ck)


if __name__ == "__main__":
    main()
