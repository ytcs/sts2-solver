#!/usr/bin/env python3
"""Plan gate 2: fit the learned macro value (`agent/value.py`) on logged screens (CARD_REWARD / RESTSITE, every act) with realised outcomes.
Targets per screen: reach_k (run reached act k + 2 or won; screens in act <= k), str_k (row deck_strength at act k's bosses; runs that
reached them), final (row deck_strength 'final deck', every run). Heads: L2 logistic (fractional targets for strengths) on standardised
features. Held out by run (5 folds) and by arm (--test-arm), Brier vs constant and Spearman per head; then the within-screen gate: on the
E85 act-1 boss states, V(deck + card) - V(deck) for the six reference cards vs the measured act-3 boss deltas (sign agreement).
usage: tools/value_fit.py <events dir>=<rows .jsonl> [...] --out models/value.json [--test-arm dpull] [--cache target/review/value_ds.npz]"""
import argparse, glob, json, os, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
HEADS = ("reach_0", "reach_1", "str_0", "str_1", "str_2", "final")


def outcome(path):
    acts, last = [], None
    for l in open(path, encoding="utf-8"):
        e = json.loads(l)
        if e["kind"] == "fight_start":
            acts.append(e.get("act") or 0)
        elif e["kind"] == "fight_end":
            hp = e.get("hp"); hp = json.loads(hp) if isinstance(hp, str) else hp
            last = bool(hp and hp[0] > 0)
    return max(acts or [0]), bool(last)


def dataset(pairs, pred):
    import numpy as np
    from agent import price as PR, value as V
    states, meta = [], []
    for ev_dir, rows_path in pairs:
        rows = {json.loads(l)["seed"]: json.loads(l) for l in open(rows_path)}
        arm = os.path.basename(ev_dir.rstrip("/"))
        for f in sorted(glob.glob(os.path.join(ev_dir, "*", "*", "events.jsonl"))):
            seed = os.path.basename(os.path.dirname(f))
            r = rows.get(seed)
            if r is None:
                continue
            top, won = outcome(f)
            ds = r.get("deck_strength") or {}
            mean = lambda k: float(np.mean(list(ds[k].values()))) if isinstance(ds.get(k), dict) and ds[k] else np.nan  # noqa: E731
            tg = dict(reach_0=float(top > 0 or won), reach_1=float(top > 1 or won), str_0=mean("A1 boss deck"), str_1=mean("A2 boss deck"),
                      str_2=mean("A3 boss deck"), final=mean("final deck"))
            for floor, _state, st, _choice in PR.recorded_screens(f):
                st.abs_floor = floor
                states.append(st)
                meta.append(dict(arm=arm, seed=seed, act=st.act, **tg))
    X = V.features(states, pred)
    return X, meta


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pairs", nargs="+", help="events dir=rows .jsonl")
    ap.add_argument("--out", required=True)
    ap.add_argument("--test-arm", default="dpull")
    ap.add_argument("--cache", default="target/review/value_ds.npz")
    ap.add_argument("--l2", type=float, default=3.0)
    a = ap.parse_args()
    import numpy as np
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    from agent import value as V
    pairs = [p.split("=") for p in a.pairs]
    pred = Predictor(PREDICTOR_CKPT, batch=2048)
    if os.path.exists(a.cache):
        z = np.load(a.cache, allow_pickle=True)
        X, meta = z["X"], list(z["meta"])
    else:
        X, meta = dataset(pairs, pred)
        np.savez(a.cache, X=X, meta=np.array(meta, dtype=object))
    acts = np.array([m["act"] for m in meta])
    runs = np.array([f"{m['arm']}|{m['seed']}" for m in meta])
    print(f"screens {len(meta)} over {len(set(runs))} runs; act 1/2/3 screens {[(acts == k).sum() for k in range(3)]}")

    def rows_for(h):
        y = np.array([m[h] for m in meta], float)
        ok = ~np.isnan(y)
        if h.startswith("reach_") or h.startswith("str_"):
            ok &= acts <= int(h[-1])
        return y, ok

    def fit(Xt, yt):
        mu, sd = Xt.mean(0), Xt.std(0) + 1e-6
        Z = np.column_stack([np.ones(len(Xt)), (Xt - mu) / sd])
        w = np.zeros(Z.shape[1]); reg = np.full(len(w), a.l2); reg[0] = 0.0
        for _ in range(60):
            p = 1 / (1 + np.exp(-(Z @ w)))
            w -= np.linalg.solve((Z * (p * (1 - p))[:, None]).T @ Z + np.diag(reg + 1e-6), Z.T @ (p - yt) + reg * w)
        return w, mu, sd

    def predict(model, Xs):
        w, mu, sd = model
        return 1 / (1 + np.exp(-(np.column_stack([np.ones(len(Xs)), (Xs - mu) / sd]) @ w)))

    def spearman(x, y):
        rx, ry = np.argsort(np.argsort(x)), np.argsort(np.argsort(y))
        return float(np.corrcoef(rx, ry)[0, 1]) if len(x) > 2 else np.nan

    fold = np.array([zlib.crc32(r.encode()) % 5 for r in runs])
    arm = np.array([m["arm"] for m in meta])
    print("head      n_screens n_runs | held out by run: Brier (constant) Spearman | held-out arm: Brier (constant) Spearman")
    model = {}
    for h in HEADS:
        y, ok = rows_for(h)
        pr = np.full(len(y), np.nan)
        for f in range(5):
            tr, te = ok & (fold != f), ok & (fold == f)
            if tr.sum() > 20 and te.any():
                pr[te] = predict(fit(X[tr], y[tr]), X[te])
        trA, teA = ok & (arm != a.test_arm), ok & (arm == a.test_arm)
        prA = predict(fit(X[trA], y[trA]), X[teA]) if trA.sum() > 20 and teA.any() else np.array([])
        m_ = ok & ~np.isnan(pr)
        line = f"{h:8s} {ok.sum():6d} {len(set(runs[ok])):5d} | {np.mean((pr[m_] - y[m_]) ** 2):.4f} ({np.var(y[m_]):.4f}) {spearman(pr[m_], y[m_]):+.2f}"
        if len(prA):
            line += f" | {np.mean((prA - y[teA]) ** 2):.4f} ({np.mean((y[teA] - y[trA].mean()) ** 2):.4f}) {spearman(prA, y[teA]):+.2f}"
        print(line)
        w, mu, sd = fit(X[ok], y[ok])
        model[h] = dict(w=[round(float(v), 5) for v in w], mu=[round(float(v), 5) for v in mu], sd=[round(float(v), 5) for v in sd], n=int(ok.sum()))
    json.dump(dict(features=V.NAMES, heads=model, source=[p[0] for p in pairs]), open(a.out, "w"), indent=1)
    print("->", a.out)

    # within-screen gate: E85 act-1 boss states (arm C), reference cards, V's act-3 terms vs measured act-3 boss deltas (E86 far_check rows)
    from agent import runmodel as R
    val = V.Value(a.out)
    fc = [json.loads(l) for l in open("target/review/far_check.jsonl")]
    cards = list(fc[0]["rungs"]["1"]["surrogate"])
    meas = {c: np.nanmean([r["rungs"]["2"]["measured"][c] for r in fc if "2" in r["rungs"] and r["rungs"]["2"]["measured"]]) for c in cards}
    sts, owner = [], []
    for f in sorted(glob.glob("target/cpull/*/*/events.jsonl")):
        sc = next((json.loads(l)["scenario"] for l in open(f, encoding="utf-8") if '"fight_start"' in l and '_BOSS"' in l and '"act": 0' in l), None)
        if sc is None:
            continue
        name = next((n for n in R.ACT_NAMES[0] if sc["encounter"] in pools_boss(n)), R.ACT_NAMES[0][0])
        for c in [None] + cards:
            deck = [dict(x) for x in sc["deck"]] + ([{"id": c, "upgrade": 0}] if c else [])
            st = R.RunState(sc, 0, name, sc["hp"], sc["max_hp"], sc.get("gold", 0), deck, [r["id"] for r in sc["relics"]], [p["id"] for p in sc.get("potions", [])],
                            sc.get("max_potion_slots", 2), (0, 0, {}, 0), {}, [sc["encounter"]])
            st.abs_floor = sc.get("total_floor", 17)
            sts.append(st); owner.append(c)
    Xg = V.features(sts, pred)
    n = len(cards) + 1
    print("within-screen gate (arm C act-1 boss states): card | mean dV final | mean d str_2 | measured act-3 boss delta (pts)")
    agree = []
    for j, c in enumerate(cards):
        d_final = np.mean([np.log(val.head(Xg[i + j + 1:i + j + 2], "final")[0]) - np.log(val.head(Xg[i:i + 1], "final")[0]) for i in range(0, len(sts), n)])
        d_s2 = np.mean([val.head(Xg[i + j + 1:i + j + 2], "str_2")[0] - val.head(Xg[i:i + 1], "str_2")[0] for i in range(0, len(sts), n)])
        agree.append((np.sign(d_final) == np.sign(meas[c]), np.sign(d_s2) == np.sign(meas[c])))
        print(f"  {c:13s} {d_final:+.4f} | {d_s2:+.4f} | {100 * meas[c]:+.2f}")
    ag = np.array(agree, float)
    print(f"sign agreement: final head {ag[:, 0].mean():.2f}, str_2 head {ag[:, 1].mean():.2f} (gate >= 0.8)")


def pools_boss(name):
    from agent import pools
    return pools.pool(name, "boss")


if __name__ == "__main__":
    main()
