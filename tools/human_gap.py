#!/usr/bin/env python3
"""Community humans vs our combat player per encounter, on matched deck / relics / entry HP (spire-codex export, target/codex/a10.jsonl.gz).
Each fight floor -> scenario: deck from `codex.walk`; final-deck enchantments (modal amount per id) and upgrades no recorded upgrade explains are placed
from the card's acquisition floor; upgrades capped at 1; relics owned in pickup order (fresh counters); HP / max HP / gold from the previous floor;
potions either none or the ones the human used in that fight (potions held are not in the export). Human outcome: lost iff the run died on that floor; end HP = the floor's HP (post-combat heals included, as the simulator's end HP).
predict: predictor (raw no potions, raw used potions, calibrated used potions) per row -> P(win), E[end HP | win], PIT of the human outcome.
search: search playouts (`Engine.solve`, used potions) on boss / elite encounters with human-better calibrated gaps + named targets + a random control.
usage: tools/human_gap.py predict | search [--per 120] [--attempts 64] | report"""
import argparse, collections, gzip, json, os, random, sys

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
OUT = os.path.join(ROOT, "target", "review", "human_gap")
CHARS = ("ironclad", "silent", "defect", "necrobinder", "regent")
NAMED = ("TEST_SUBJECT_BOSS", "WATERFALL_GIANT_BOSS", "PHROG_PARASITE_ELITE", "GREMLIN_MERC_NORMAL", "AXEBOTS_NORMAL")
VARS = ("raw0", "raw", "cal")
ENCH = {"NIMBLE": 2, "CLONE": 4, "SWIFT": 2, "SHARP": 2, "ADROIT": 3, "MOMENTUM": 5, "VIGOROUS": 8}


def kind(enc):
    return "boss" if enc.endswith("_BOSS") else "elite" if "ELITE" in enc else "hall"


def _codex():
    from reward_values import _codex_path
    return _codex_path()


def fights(r, N):
    """(row, scenario) per fight floor of a run, or a drop reason for the whole run"""
    import codex
    if not all(x in N["relic"] for x, _ in r["relics"]) or not all(c[0] in N["card"] for c in r["deck"]):
        return "modded"
    seq = [f for a in r["floors"] for f in a]
    if any(e not in N["encounter"] and f["t"] in ("monster", "elite", "boss") for f in seq for e in f["enc"]) or any(p not in N["potion"] for f in seq for p in f["pot_used"]) \
            or any(c not in N["card"] for f in seq for c in f["gained"]):
        return "modded"
    W = list(codex.walk(r))
    f, now, up = W[-1][3], W[-1][4], W[-1][6]
    now = +(now + collections.Counter(f["gained"]) - collections.Counter(f["removed"]))
    up = up + collections.Counter(f["upgraded"]) + collections.Counter(c for c, u, pk in f["offer"] if u and pk)
    fin, finu = collections.Counter(c[0] for c in r["deck"]), collections.Counter(c[0] for c in r["deck"] if c[1] > 0)
    extra = sum((now - fin).values())
    unexpl = {c: finu[c] - min(up[c], fin[c]) for c in finu if finu[c] > min(up[c], fin[c])}
    out, prev = [], None
    for ai, i, g, f, now, rl, up in W:
        if prev and f["enc"] and f["enc"][0] in N["encounter"] and (f["t"] in ("monster", "elite", "boss") or f["turns"]):
            enc = f["enc"][0]
            dead = g == len(seq) and not r["win"]
            if dead and r["killed_by"] not in f["enc"]:
                prev = f
                continue
            acq = collections.Counter(c[0] for c in r["deck"] if c[1] > 0 and (c[3] or 0) < g)
            ench = collections.defaultdict(list)
            for c in r["deck"]:
                if c[2] and (c[3] or 0) < g:
                    ench[c[0]].append(c[2])
            deck = []
            for c in sorted(now):
                u = min(now[c], up[c] + min(unexpl.get(c, 0), acq[c]))
                for k in range(now[c]):
                    deck.append({"id": c, "upgrade": int(k < u)})
                    if k < len(ench[c]):
                        deck[-1]["enchantment"] = {"id": ench[c][k], "amount": ENCH.get(ench[c][k], 1)}
            rel = [x for x, fx in r["relics"] if (fx or 0) < g]
            if len(deck) <= 80 and len(rel) <= 40 and prev["hp"] and prev["mx"]:
                mxp = prev["mx"]
                sc = dict(name="hg", ascension=10, encounter=enc, character=r["char"].upper(), hp=min(prev["hp"], mxp), max_hp=mxp, max_energy=3, gold=prev["gold"] or 0,
                          max_potion_slots=max(2 + 2 * ("POTION_BELT" in rel), len(f["pot_used"])), base_orb_slots=3 if r["char"] == "defect" else 0,
                          seed=f"hg{r['hash']}{g}", total_floor=g, act=ai, deck=deck, relics=[{"id": x} for x in rel], potions=[])
                out.append((dict(char=r["char"], hash=r["hash"], g=g, act=ai, kind=kind(enc), enc=enc, room=f["t"], hp=sc["hp"], mx=mxp, won=int(not dead),
                                 end=0 if dead else int(f["hp"] or 0), dmg=f["dmg"], turns=f["turns"], pots=f["pot_used"], n=len(deck),
                                 ench=sum(len(v) for v in ench.values()), upi=int(any(min(unexpl.get(c, 0), acq[c]) for c in now)), extra=extra,
                                 mxchg=int((f["mx"] or mxp) != mxp)), sc))
        prev = f
    return out


def _pit(P, cls, rng):
    cum = np.cumsum(P, -1)
    below = np.where(cls > 0, cum[np.arange(len(P)), np.maximum(cls - 1, 0)], 0.0)
    return below + rng.random(len(P)) * P[np.arange(len(P)), cls]


def _summ(P, rows, rng):
    import predictor as PR, heads as H
    won = np.array([x["won"] for x in rows], bool)
    cls = np.where(won, H.end_class(True, [max(x["end"], 1) for x in rows]), 0)
    W = P[:, 1:] / np.maximum(P[:, 1:].sum(1, keepdims=True), 1e-12)
    pitw = _pit(np.concatenate([np.zeros((len(P), 1)), W], 1), cls, rng)
    return PR.p_win(P), PR.end_hp(P), _pit(P, cls, rng), np.where(won, pitw, np.nan)


def predict(a):
    import sts2, predictor as PR, codex
    from solver import PREDICTOR_CKPT
    N = {k: set(v) for k, v in sts2.names().items() if isinstance(v, list)}
    pr = PR.Predictor(PREDICTOR_CKPT, batch=4096)
    calib = json.load(open(os.path.join(ROOT, "models", "predictor_calib.json")))
    rng, drop, n = np.random.default_rng(0), collections.Counter(), 0
    with gzip.open(OUT + ".jsonl.gz", "wt") as g:
        def flush(buf):
            rows, scs = [x for x, _ in buf], [s for _, s in buf]
            P0 = pr.fight_start(scs, 8)
            P1 = pr.fight_start([dict(s, potions=[{"id": p, "slot": i} for i, p in enumerate(x["pots"][:8])]) for x, s in buf], 8)
            for v, P in zip(VARS, (P0, P1, PR.calibrate(P1, scs, calib))):
                for x, w, e, pit, pitw in zip(rows, *_summ(P, rows, rng)):
                    x[v] = [round(float(w), 4), round(float(e), 1), round(float(pit), 3), None if np.isnan(pitw) else round(float(pitw), 3)]
            for x in rows:
                g.write(json.dumps(x) + "\n")
        buf = []
        for r in codex.rows(_codex()):
            if r["char"] not in CHARS:
                drop["character"] += 1
                continue
            f = fights(r, N)
            if isinstance(f, str):
                drop[f] += 1
                continue
            try:
                sts2.FightStarts([dict(s, potions=[{"id": p, "slot": i} for i, p in enumerate(x["pots"][:8])]) for x, s in f])
            except ValueError as e:
                drop[str(e)[:60]] += 1
                continue
            buf += f
            if len(buf) >= 4096:
                n += len(buf)
                flush(buf)
                buf = []
                print(f"  {n} rows", flush=True)
        if buf:
            n += len(buf)
            flush(buf)
    print(f"{n} fight rows; runs dropped: {dict(drop)}")


def load():
    with gzip.open(OUT + ".jsonl.gz", "rt") as f:
        return [json.loads(l) for l in f]


def _z(y, p):
    y, p = np.asarray(y, float), np.asarray(p, float)
    return float((y - p).sum() / max(np.sqrt((p * (1 - p)).sum()), 1e-9))


def groups(R, key):
    G = collections.defaultdict(list)
    for x in R:
        G[key(x)].append(x)
    return G


def stats(v):
    y = np.array([x["won"] for x in v], float)
    s = dict(n=len(v), loss=int((1 - y).sum()), wh=float(y.mean()))
    for k in VARS:
        p = np.array([x[k][0] for x in v])
        s["w_" + k], s["z_" + k] = float(p.mean()), _z(y, p)
        s["pit_" + k] = float(np.mean([x[k][2] for x in v]))
        w = [x for x in v if x["won"]]
        s["dend_" + k] = float(np.mean([x["end"] - x[k][1] for x in w])) if w else float("nan")
        s["pitw_" + k] = float(np.mean([x[k][3] for x in w])) if w else float("nan")
    s["pots"] = float(np.mean([len(x["pots"]) for x in v]))
    return s


def clean(x):
    return not (x["extra"] or x["mxchg"])


def search(a):
    R = load()
    rng = random.Random(1)
    G = groups([x for x in R if x["kind"] in ("boss", "elite")], lambda x: (x["char"], x["enc"]))
    pick = []
    for ch in CHARS:
        top = sorted(((stats(v), k) for k, v in G.items() if k[0] == ch and len(v) >= 50), key=lambda t: -(t[0]["wh"] - t[0]["w_cal"]))
        sel = [k for s, k in top if s["z_cal"] >= 2][:a.top if ch == "ironclad" else max(a.top // 2, 1)]
        sel += [k for k in groups(R, lambda x: (x["char"], x["enc"])) if k[0] == ch and k[1] in NAMED and k not in sel]
        for k in sel:
            v = [x for x in R if (x["char"], x["enc"]) == k and clean(x)]
            pick += [dict(x, stratum="gap" if k[1] not in NAMED else "named") for x in rng.sample(v, min(a.per if ch == "ironclad" else a.per // 2, len(v)))]
    ctrl = [x for x in R if x["kind"] in ("boss", "elite") and clean(x)]
    have = {(x["hash"], x["g"]) for x in pick}
    pick += [dict(x, stratum="control") for x in rng.sample(ctrl, a.control) if (x["hash"], x["g"]) not in have]
    import sts2, codex
    from agent.engine import Engine
    N = {k: set(v) for k, v in sts2.names().items() if isinstance(v, list)}
    want = collections.defaultdict(dict)
    for x in pick:
        want[x["hash"]][x["g"]] = x
    path = OUT + "_search.jsonl"
    done = {(d["hash"], d["g"]) for d in map(json.loads, open(path))} if os.path.exists(path) else set()
    todo = []
    for r in codex.rows(_codex()):
        if r["hash"] in want:
            for row, sc in fights(r, N):
                x = want[r["hash"]].get(row["g"])
                if x and (x["hash"], x["g"]) not in done:
                    todo.append((x, dict(sc, potions=[{"id": p, "slot": i} for i, p in enumerate(x["pots"][:8])])))
    print(f"{len(pick)} rows picked, {len(todo)} to search", flush=True)
    eng = Engine()
    for k in range(0, len(todo), a.batch):
        B = todo[k:k + a.batch]
        res = eng.solve([sc for _, sc in B], attempts=a.attempts, seed=104729)
        with open(path, "a") as f:
            for (x, _), q in zip(B, res):
                f.write(json.dumps(dict(x, ps=q["win"], ps_end=q["hp_left_on_win"], att=q["attempts"])) + "\n")
        print(f"  searched {min(k + a.batch, len(todo))}/{len(todo)}", flush=True)


def report(a):
    R = load()
    L = []
    flags = {k: np.mean([bool(x[k]) for x in R]) for k in ("ench", "upi", "extra", "mxchg")}
    L.append(f"human_gap: {len(R)} fight rows, {len({x['hash'] for x in R})} runs; row shares flagged: " + ", ".join(f"{k} {v:.3f}" for k, v in flags.items())
             + f"; clean {np.mean([clean(x) for x in R]):.3f}; rows with a potion used {np.mean([bool(x['pots']) for x in R]):.3f}")
    L.append("columns: n, human losses, human win | predicted win raw no potions / raw used potions / calibrated used potions | z (sum(y-p)/sqrt(sum p(1-p))) raw / cal | "
             "loss ratio pred cal / human | end HP on human wins, human minus predicted E[end|win] (raw) | PIT all / PIT on wins (raw; 0.5 calibrated, > 0.5 humans better)")
    hdr = f"{'':34s} {'n':>6s} {'loss':>5s} {'w_h':>6s} | {'raw0':>6s} {'raw':>6s} {'cal':>6s} | {'z_raw':>6s} {'z_cal':>6s} | {'lr':>5s} | {'dEnd':>6s} | {'PIT':>5s} {'PITw':>5s}"

    def line(name, s):
        lr = (1 - s["w_cal"]) / max(1 - s["wh"], 1e-3)
        return (f"{name:34s} {s['n']:6d} {s['loss']:5d} {s['wh']:6.3f} | {s['w_raw0']:6.3f} {s['w_raw']:6.3f} {s['w_cal']:6.3f} | {s['z_raw']:+6.1f} {s['z_cal']:+6.1f} | "
                f"{lr:5.2f} | {s['dend_raw']:+6.1f} | {s['pit_raw']:5.3f} {s['pitw_raw']:5.3f}")
    for ch in CHARS:
        Rc = [x for x in R if x["char"] == ch]
        L.append(f"\n== {ch} ({len(Rc)} rows) by act x kind\n" + hdr)
        for k, v in sorted(groups(Rc, lambda x: (x["act"], x["kind"])).items()):
            L.append(line(f"A{k[0] + 1} {k[1]}", stats(v)))
        L.append("clean rows only: " + "  ".join(f"A{k[0] + 1} {k[1]} {s['wh']:.3f}/{s['w_cal']:.3f}" for k, s in
                                                ((k, stats(v)) for k, v in sorted(groups([x for x in Rc if clean(x)], lambda x: (x["act"], x["kind"])).items()))))
        L.append(f"-- {ch} boss / elite encounters (n >= 50) by calibrated gap w_h - w_cal, plus named targets\n" + hdr)
        G = groups(Rc, lambda x: x["enc"])
        S = sorted(((stats(v), e) for e, v in G.items() if (kind(e) != "hall" and len(v) >= 50) or e in NAMED), key=lambda t: -(t[0]["wh"] - t[0]["w_cal"]))
        for s, e in S:
            L.append(line(f"{e}", s) + (" *" if e in NAMED else ""))
    sp = OUT + "_search.jsonl"
    if os.path.exists(sp):
        Sx = [json.loads(l) for l in open(sp)]
        L.append(f"\n== search playouts ({Sx[0]['att']} attempts, used potions; clean rows): w_h human, w_raw / w_cal predictor, w_s search playouts; d = w_s - w_h "
                 "(se paired per row), d0 on rows without a potion used; verdict: search ~ human (|d| < 2 se) / search worse / search better; "
                 "predictor pessimistic when w_s - w_cal > 2 se; endH / endS: HP left on the human's won rows, human / search mean on its wins")
        L.append(f"{'':38s} {'n':>5s} {'w_h':>6s} {'w_raw':>6s} {'w_cal':>6s} {'w_s':>6s} {'d':>7s} {'se':>6s} {'d0':>7s} {'endH':>6s} {'endS':>6s}  verdict")
        J = []
        for k, v in sorted(groups(Sx, lambda x: (x["char"], x["enc"] if x["stratum"] != "control" else "~control " + x["kind"])).items()):
            y = np.array([x["won"] for x in v], float)
            ps = np.array([x["ps"] for x in v])
            pc = np.array([x["cal"][0] for x in v])
            d = ps - y
            se = float(d.std(ddof=1) / np.sqrt(len(d))) if len(d) > 1 else float("nan")
            sp_ = float((ps - pc).std(ddof=1) / np.sqrt(len(d))) if len(d) > 1 else float("nan")
            z0 = [x["ps"] - x["won"] for x in v if not x["pots"]]
            verdict = ("search ~ human" if abs(d.mean()) < 2 * se else "search worse" if d.mean() < 0 else "search better") + \
                      ("; predictor pessimistic" if (ps - pc).mean() > 2 * sp_ else "")
            ws = [x for x in v if x["won"]]
            eh = np.mean([x["end"] for x in ws]) if ws else float("nan")
            es = np.mean([x["ps_end"] for x in ws]) if ws else float("nan")
            row = dict(char=k[0], enc=k[1], n=len(v), wh=float(y.mean()), w_raw=float(np.mean([x["raw"][0] for x in v])), w_cal=float(pc.mean()), ws=float(ps.mean()),
                       d=float(d.mean()), se=se, d0=float(np.mean(z0)) if z0 else float("nan"), n0=len(z0), end_h=float(eh), end_s=float(es), verdict=verdict)
            J.append(row)
            L.append(f"{k[0][:4]} {k[1]:33s} {len(v):5d} {row['wh']:6.3f} {row['w_raw']:6.3f} {row['w_cal']:6.3f} {row['ws']:6.3f} {row['d']:+7.3f} {se:6.3f} {row['d0']:+7.3f} "
                     f"{eh:6.1f} {es:6.1f}  {verdict}" + (" *" if k[1] in NAMED else ""))
        L.append("\n== r10 curriculum ranking (searched encounters by d = w_s - w_h ascending; n_pop = all rows of the encounter)")
        pop = {k: len(v) for k, v in groups(R, lambda x: (x["char"], x["enc"])).items()}
        for j in sorted((j for j in J if not j["enc"].startswith("~")), key=lambda j: j["d"]):
            L.append(f"{j['char'][:4]} {j['enc']:33s} n_pop {pop.get((j['char'], j['enc']), 0):5d}  w_h {j['wh']:.3f}  w_cal {j['w_cal']:.3f}  w_s {j['ws']:.3f}  "
                     f"d {j['d']:+.3f} +- {j['se']:.3f}  {j['verdict']}" + (" *" if j["enc"] in NAMED else ""))
        json.dump(J, open(OUT + "_search.json", "w"), indent=0)
    text = "\n".join(L) + "\n"
    open(OUT + ".txt", "w").write(text)
    print(text)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["predict", "search", "report"])
    ap.add_argument("--top", type=int, default=10)
    ap.add_argument("--per", type=int, default=120)
    ap.add_argument("--control", type=int, default=600)
    ap.add_argument("--attempts", type=int, default=64)
    ap.add_argument("--batch", type=int, default=256)
    a = ap.parse_args()
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    dict(predict=predict, search=search, report=report)[a.cmd](a)


if __name__ == "__main__":
    main()
