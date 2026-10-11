#!/usr/bin/env python3
"""Fit the predictor's recorded-fight calibration (`rl/predictor.py` calibrate) on `tools/fight_calib.py --out` rows: logistic map of
logit P(win) with act / kind terms (L2, IRLS) and the mean end-HP bias on wins per act x kind. Two-fold held out by run, then fit on
all -> --out.
usage: tools/calib_fit.py target/review/fights.jsonl --out models/predictor_calib.json"""
import argparse, collections, json, os, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rows")
    ap.add_argument("--out", required=True)
    ap.add_argument("--l2", type=float, default=1.0)
    a = ap.parse_args()
    import numpy as np
    from predictor import calib_features
    from solver import PREDICTOR_CKPT
    R = [json.loads(l) for l in open(a.rows)]
    X = np.stack([calib_features(dict(encounter=r["enc"], act=r["act"]), r["p_win"])[0] for r in R])
    G = [calib_features(dict(encounter=r["enc"], act=r["act"]), r["p_win"])[1] for r in R]
    y = np.array([r["won"] for r in R])
    fold = np.array([zlib.crc32(f"{r['arm']}|{r['seed']}".encode()) % 2 for r in R])

    def fit(m):
        w = np.zeros(X.shape[1]); w[0] = 1.0
        reg = np.full(X.shape[1], a.l2); reg[0] = 0.0
        for _ in range(50):
            p = 1 / (1 + np.exp(-(X[m] @ w)))
            g = X[m].T @ (p - y[m]) + reg * (w - (np.arange(len(w)) == 0))
            H = (X[m] * (p * (1 - p))[:, None]).T @ X[m] + np.diag(reg + 1e-6)
            w -= np.linalg.solve(H, g)
        shift = collections.defaultdict(list)
        for r, g_, mm in zip(R, G, m):
            if mm and r["won"]:
                shift[g_].append((r["hp"] / r["mx"], r["end"] - r["p_end"]))
        out = {}
        for k, v in shift.items():
            if len(v) >= 10:
                h, d = np.array(v).T
                b = float(np.cov(h, d, bias=True)[0, 1] / max(np.var(h), 1e-6)) if np.var(h) > 1e-3 and len(v) >= 80 else 0.0
                out[k] = (float(d.mean() - b * h.mean()), b)
        return w, out

    print("held out (2 folds by run): per group n, realised, raw predicted -> calibrated; Brier raw -> calibrated")
    pc = np.zeros(len(R))
    for f in (0, 1):
        w, _ = fit(fold != f)
        pc[fold == f] = 1 / (1 + np.exp(-(X[fold == f] @ w)))
    raw = np.array([r["p_win"] for r in R])
    for g in sorted(set(G)):
        m = np.array([x == g for x in G])
        print(f"  {g:8s} n {m.sum():4d} | realised {y[m].mean():.3f} | {raw[m].mean():.3f} -> {pc[m].mean():.3f} | Brier {np.mean((raw[m] - y[m]) ** 2):.4f} -> {np.mean((pc[m] - y[m]) ** 2):.4f}")
    print(f"  all      Brier {np.mean((raw - y) ** 2):.4f} -> {np.mean((pc - y) ** 2):.4f} (constant {np.mean((y - y.mean()) ** 2):.4f})")
    w, shift = fit(np.ones(len(R), bool))
    json.dump(dict(predictor=os.path.basename(PREDICTOR_CKPT), n=len(R), w=[round(float(v), 5) for v in w], hp_shift={k: [round(v[0], 2), round(v[1], 2)] for k, v in shift.items()},
                   features=["logit p", "1", "elite", "boss", "act 2", "act 3", "logit p x elite", "logit p x boss"]), open(a.out, "w"), indent=1)
    print("fit on all ->", a.out, "w", np.round(w, 3), "hp shift a + b x hp fraction", {k: (round(v[0], 1), round(v[1], 1)) for k, v in shift.items()})


if __name__ == "__main__":
    main()
