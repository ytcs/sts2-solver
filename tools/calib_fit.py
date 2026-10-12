#!/usr/bin/env python3
"""Fit the predictor's recorded-fight calibration (`rl/predictor.py` calibrate) on `tools/fight_calib.py --out` rows: logistic map of
logit P(win) with act / kind terms (L2, IRLS) and, per act x kind (and starting-HP band where >= 40 wins), the quantile map of the HP change
on a win from the predictor's pooled distribution to the realised one; per kind x P(win) band, the share of throwable potions used (the run
model's potion consumption). Two-fold held out by seed, then fit on all -> --out.
usage: tools/calib_fit.py target/review/fights.jsonl --out models/predictor_calib.json"""
import argparse, collections, json, os, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def logistic(X, y, m, l2):
    import numpy as np
    w = np.zeros(X.shape[1]); w[0] = 1.0
    reg = np.full(X.shape[1], l2); reg[0] = 0.0
    for _ in range(50):
        p = 1 / (1 + np.exp(-(X[m] @ w)))
        g = X[m].T @ (p - y[m]) + reg * (w - (np.arange(len(w)) == 0))
        H = (X[m] * (p * (1 - p))[:, None]).T @ X[m] + np.diag(reg + 1e-6)
        w -= np.linalg.solve(H, g)
    return w


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("--out", required=True)
    ap.add_argument("--l2", type=float, default=1.0)
    ap.add_argument("--pot-arms", default="cpull,dpull", help="arms whose potion policy the run model reproduces")
    a = ap.parse_args()
    import numpy as np
    import heads
    from predictor import calib_features, calibrate, hp_band
    from solver import PREDICTOR_CKPT
    C = heads.CENTERS
    QGRID = np.array([0.0, 0.01, 0.03] + list(np.linspace(0.05, 0.95, 19)) + [0.97, 0.99, 1.0])
    R = [json.loads(l) for l in open(a.rows)]
    X = np.stack([calib_features(dict(encounter=r["enc"], act=r["act"]), r["p_win"])[0] for r in R])
    G = [calib_features(dict(encounter=r["enc"], act=r["act"]), r["p_win"])[1] for r in R]
    y = np.array([r["won"] for r in R])
    fold = np.array([zlib.crc32(r["seed"].encode()) % 2 for r in R])
    band = lambda r: hp_band(dict(hp=r["hp"], max_hp=r["mx"]))  # noqa: E731

    def fit(m):
        w = logistic(X, y, m, a.l2)
        pred, real = collections.defaultdict(list), collections.defaultdict(list)
        rng = np.random.default_rng(0)
        for r, g_, mm in zip(R, G, m):
            if mm and r["won"]:
                win = np.asarray(r["dist"][1:], float); win = win / max(win.sum(), 1e-12)
                draws = rng.choice(C, 16, p=win) - r["hp"]
                for key in (g_, f"{g_}:{band(r)}"):
                    pred[key] += list(draws)
                    real[key].append(r["end"] - r["hp"])
        qmap = {}
        for key, v in real.items():
            if len(v) >= (40 if key.count(":") == 2 else 10):
                qp, qr = np.maximum.accumulate(np.quantile(pred[key], QGRID)), np.maximum.accumulate(np.quantile(v, QGRID))
                qmap[key] = [[round(float(x), 2) for x in qp], [round(float(x), 2) for x in qr]]
        return w, qmap

    print("held out (2 folds by seed): per group n, realised, raw predicted -> calibrated; Brier raw -> calibrated")
    pc = np.zeros(len(R))
    hp_q = []
    for f in (0, 1):
        w, qmap = fit(fold != f)
        pc[fold == f] = 1 / (1 + np.exp(-(X[fold == f] @ w)))
        idx = [i for i in range(len(R)) if fold[i] == f and R[i]["won"]]
        Pc = calibrate(np.array([R[i]["dist"] for i in idx]), [dict(encounter=R[i]["enc"], act=R[i]["act"], hp=R[i]["hp"], max_hp=R[i]["mx"]) for i in idx],
                       dict(w=w, hp_qmap=qmap))
        rng = np.random.default_rng(f)
        for i, p in zip(idx, Pc):
            win = p[1:] / max(p[1:].sum(), 1e-12)
            hp_q.append((G[i], band(R[i]), R[i]["end"], float(rng.choice(C, p=win))))
    raw = np.array([r["p_win"] for r in R])
    for g in sorted(set(G)):
        m = np.array([x == g for x in G])
        print(f"  {g:8s} n {m.sum():4d} | realised {y[m].mean():.3f} | {raw[m].mean():.3f} -> {pc[m].mean():.3f} | Brier {np.mean((raw[m] - y[m]) ** 2):.4f} -> {np.mean((pc[m] - y[m]) ** 2):.4f}")
    print(f"  all      Brier {np.mean((raw - y) ** 2):.4f} -> {np.mean((pc - y) ** 2):.4f} (constant {np.mean((y - y.mean()) ** 2):.4f})")
    print("held-out end HP on wins, realised vs sampled from the calibrated distribution: mean, q10, q90 (group, then by starting-HP band)")
    for key in sorted({(h[0],) for h in hp_q} | {(h[0], h[1]) for h in hp_q}):
        a_ = np.array([(e, s) for g, b, e, s in hp_q if (g,) == key or (g, b) == key])
        if len(a_) >= 20:
            print(f"  {':'.join(map(str, key)):10s} n {len(a_):4d} | realised {a_[:, 0].mean():5.1f} {np.quantile(a_[:, 0], 0.1):5.1f} {np.quantile(a_[:, 0], 0.9):5.1f} | "
                  f"calibrated {a_[:, 1].mean():5.1f} {np.quantile(a_[:, 1], 0.1):5.1f} {np.quantile(a_[:, 1], 0.9):5.1f}")
    w, qmap = fit(np.ones(len(R), bool))
    w_search = None
    if all(r.get("p_search") is not None for r in R):
        Xs = np.stack([calib_features(dict(encounter=r["enc"], act=r["act"]), r["p_search"])[0] for r in R])
        ps, sraw = np.zeros(len(R)), np.array([r["p_search"] for r in R])
        for f in (0, 1):
            ps[fold == f] = 1 / (1 + np.exp(-(Xs[fold == f] @ logistic(Xs, y, fold != f, a.l2))))
        print("search playouts (the `eval` instrument), held out by seed: per group n, realised, raw -> calibrated; Brier")
        for g in sorted(set(G)):
            m = np.array([x == g for x in G])
            print(f"  {g:8s} n {m.sum():4d} | realised {y[m].mean():.3f} | {sraw[m].mean():.3f} -> {ps[m].mean():.3f} | Brier {np.mean((sraw[m] - y[m]) ** 2):.4f} -> {np.mean((ps[m] - y[m]) ** 2):.4f}")
        print(f"  all      Brier {np.mean((sraw - y) ** 2):.4f} -> {np.mean((ps - y) ** 2):.4f}")
        w_search = [round(float(v), 5) for v in logistic(Xs, y, np.ones(len(R), bool), a.l2)]
    from predictor import POT_BANDS, pot_key
    use = collections.defaultdict(lambda: [0, 0])
    for r in R:
        if r.get("held") and r["arm"] in a.pot_arms.split(","):
            k = pot_key(r["enc"], r["p_win"])
            use[k][0] += r["used"]; use[k][1] += r["held"]
    pot_use = {k: round(u / h, 3) for k, (u, h) in use.items() if h >= 20}
    print("potion use share by kind x P band:", pot_use)
    json.dump(dict(predictor=os.path.basename(PREDICTOR_CKPT), n=len(R), w=[round(float(v), 5) for v in w], w_search=w_search, hp_qmap=qmap, pot_use=pot_use,
                   features=["logit p", "1", "elite", "boss", "act 2", "act 3", "logit p x elite", "logit p x boss"]), open(a.out, "w"), indent=1)
    print("fit on all ->", a.out, "w", np.round(w, 3), "hp quantile maps", len(qmap))


if __name__ == "__main__":
    main()
