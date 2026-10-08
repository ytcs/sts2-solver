#!/usr/bin/env python3
"""The predictor benchmark (`docs/rebuild.md` S2): frozen fight sets labelled by search play, and the scores of a predictor on them.

  tools/bench.py build                 # freeze the sets into data/bench/ and label them (search at live width, h128; ~1 h on one GPU)
  tools/bench.py score CKPT [CKPT ...] # calibration and ranking of each network's fight-start prediction
  tools/bench.py play CKPT             # the network as the search's policy: win and end HP paired with the labels (same seeds)
  tools/bench.py screen BASE CKPT ...  # cheap screen (~1 min each): greedy-policy win per set, paired with BASE (same env seeds), and the predictor's
                                       # bias / Brier; play only what the screen does not rule out (a play-check is ~10 min and its se ~0.004)

Sets (`data/bench/<set>.json`: scenarios plus per-attempt labels):
  eval    600 generated fights (tools/gen_train.py seed 122: 5 characters, 3 acts)
  corpus  real-run fights (data/corpus/fights_holdout.json)
  mix     cross-character cards, ancient relics, belts up to 8 (tools/gen_curriculum.py, held-out seed)
  tail    late game: Act 3 (act index 2) elites and bosses with decks of 28+ cards (tools/gen_curriculum.py, held-out seed)
  pairs   ranking: (base, variant) fights, the variant adds a pool card, removes a card, upgrades a card or drops a potion; labelled with
          common random numbers so the reference difference is paired, 256 attempts at search width 3x8 (32 attempts left the reference agreeing
          with itself on only 66% of signs: E3)

Labels: `Solver(h128, M=5, K=32)`, the search's live width without adaptive rounds; potions are free to use (the allowed set is the whole belt).
Scores of a predictor's fight-start prediction (the outcome head averaged over SHUFFLES opening shuffles):
  calibration  bias and Brier of P(win) against the label mean, a reliability table, the ranked probability score of the end distribution (loss,
               then 2-HP end bins) against each labelled attempt, per set and per character
  ranking      on pairs whose reference difference exceeds 2 paired se: share where the predicted difference has the same sign (P(win), and the
               linear worth the search maximises), and the Spearman correlation of predicted vs reference differences
"""
import argparse, json, os, random, sys, time

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))
OUT = os.path.join(ROOT, "data", "bench")
LABEL_CKPT = os.path.join(ROOT, "models", "solver_h128.pt")
ATTEMPTS, PAIR_ATTEMPTS, SHUFFLES = 8, 256, 8
SETS = ("eval", "corpus", "mix", "tail")


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
    """The frozen scenario sets (deterministic given the seeds). v2 (2026-10-07): no multiplayer-only card anywhere (single-player runs never
    offer them; v1 had them in ~62% of fights, E11)."""
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
    """(base, variant, kind) triples: one variant of each base fight."""
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
        groups = [i // 2 for i in range(len(flat))]  # base and variant share the RNG streams and search seeds of every attempt
        t0 = time.time()
        # card effects are a few points against fight-to-fight variance: the reference needs many paired attempts at a cheaper width (E3)
        res = Solver(LABEL_CKPT, M=3, K=8).solve(flat, attempts=PAIR_ATTEMPTS, seed=13, groups=groups)
        rows = []
        for i, (b, v, kind) in enumerate(tri):
            rb, rv = res[2 * i], res[2 * i + 1]
            rows.append(dict(base=b, variant=v, kind=kind, wins=[rb["wins"], rv["wins"]], ends=[rb["ends_abs"], rv["ends_abs"]]))
        json.dump(rows, open(path, "w"))
        print(f"pairs: {len(rows)} x 2 x {PAIR_ATTEMPTS} in {time.time() - t0:.0f}s", flush=True)


# ---------------------------------------------------------------------------------------------------------------------------- scoring

def predict(net, scen, shuffles=SHUFFLES):
    """Fight-start prediction averaged over opening shuffles: class probabilities [S, NC] (`rl/predictor.py`)."""
    from predictor import Predictor
    return Predictor(net).fight_start(scen, shuffles)


def _worth(P, max_hp):
    import heads as H
    c = H.centers().numpy()
    u = np.concatenate([np.full((len(P), 1), H.LOSS), H.WIN + H.HP_BONUS * np.minimum(c[None, :] / np.asarray(max_hp, float)[:, None], 1.0)], 1)
    return (P * u).sum(1)


def _rps(P, wins, ends):
    """Mean ranked probability score of the ordinal end distribution (loss < end HP bins) against each labelled attempt."""
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
    r = lambda x: np.argsort(np.argsort(x)).astype(float)  # noqa: E731  (ties broken by order: fine for continuous differences)
    return float(np.corrcoef(r(a), r(b))[0, 1])


def score_net(ck):
    from model import load
    net = load(ck, set_version=False)  # the envs take the network's observation version: checkpoints of both versions score in one run
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
    """Greedy-policy fights (no search): every scenario `per_env` times on fixed env seeds (round robin); [S, per_env] win (1/0, NaN if aborted)."""
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
    """Greedy win per set for each checkpoint, paired with the first (same env seeds), plus the fight-start predictor's bias and Brier."""
    from model import load
    sets = {n: json.load(open(os.path.join(OUT, n + ".json"))) for n in SETS if os.path.exists(os.path.join(OUT, n + ".json"))}
    base = {}
    for k, ck in enumerate(cks):
        net = load(ck, set_version=False).eval()  # each env takes its network's observation version: v1 and v2 checkpoints pair in one run
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


def play(ck, roots=None):
    """A network as the search's policy and evaluator at live width on the frozen sets, on the labels' seeds: win and end HP paired with the labels."""
    from solver import Solver
    S = Solver(ck, M=5, K=32, roots=roots)
    print(f"\n== play {os.path.basename(ck)} (search 5x32 vs the labels' h128 5x32, same seeds)")
    for name in SETS:
        path = os.path.join(OUT, name + ".json")
        if not os.path.exists(path):
            continue
        rows = json.load(open(path))
        res = S.solve([r["scenario"] for r in rows], attempts=ATTEMPTS, seed=11)
        # a fight with no attempt finished on both sides (an aborted play-out is None) has no paired difference: left out and counted
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
    s = sub.add_parser("score"); s.add_argument("ckpts", nargs="+")
    pl = sub.add_parser("play"); pl.add_argument("ckpts", nargs="+")
    sc = sub.add_parser("screen"); sc.add_argument("ckpts", nargs="+", help="the first is the base the others are paired with")
    sc.add_argument("--per-env", type=int, default=4)
    pl.add_argument("--roots", type=int, default=None, help="fights in flight (default 2048 on CUDA); fewer = less host and GPU memory")
    a = ap.parse_args()
    if a.cmd == "build":
        build(a)
    elif a.cmd == "play":
        for ck in a.ckpts:
            play(ck, a.roots)
    elif a.cmd == "screen":
        screen(a.ckpts, a.per_env)
    else:
        for ck in a.ckpts:
            score_net(ck)


if __name__ == "__main__":
    main()
