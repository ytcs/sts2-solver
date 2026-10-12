#!/usr/bin/env python3
"""Iteration 0 of the macro policy loop: clone the live policy's route and rest decisions from logged runs (MAP / RESTSITE `macro`
events): conditional logit over map options (room type x [1, HP fraction, act 2, act 3] + elites / rests among its children) and a
logistic rest-vs-smith rule (HP fraction, act). Held-out (2 folds by run) accuracy vs `BasePolicy`, then fit on all -> --out.
usage: tools/bc_fit.py <events dir> [...] --out models/bc_policy.json"""
import argparse, glob, json, os, re, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
TYPES = {"Monster": "M", "Elite": "E", "RestSite": "R", "Shop": "$", "Treasure": "T", "Unknown": "?"}
ORDER = ("M", "E", "R", "$", "T", "?")
HEAD = re.compile(r"^A(\d) F(\d+) \S+ A\d+ HP (\d+)/(\d+)", re.M)


def map_features(t, hp_frac, act, kids):
    x = []
    for o in ORDER:
        on = float(t == o)
        x += [on, on * hp_frac, on * (act == 1), on * (act == 2)]
    return x + [sum(k == "E" for k in kids), sum(k == "R" for k in kids), sum(k == "E" for k in kids) * hp_frac]


def rest_features(hp_frac, act):
    return [1.0, hp_frac, float(act == 1), float(act == 2)]


def decisions(dirs):
    maps, rests = [], []
    for d in dirs:
        for f in sorted(glob.glob(os.path.join(d, "*", "*", "events.jsonl"))):
            run = f"{d}|{os.path.basename(os.path.dirname(f))}"
            for l in open(f, encoding="utf-8"):
                e = json.loads(l)
                if e.get("kind") != "macro" or e.get("screen") not in ("MAP", "RESTSITE"):
                    continue
                h = HEAD.search(e["state"])
                if not h:
                    continue
                act, hp_frac = int(h.group(1)) - 1, int(h.group(3)) / max(int(h.group(4)), 1)
                try:
                    pick = int(str(e.get("choice", "")).split()[0])
                except (ValueError, IndexError):
                    continue
                if e["screen"] == "MAP":
                    opts = []
                    for m in re.finditer(r"^(\d+) (\w+) r\d+c\d+(?: -> (\S+))?$", e["state"], re.M):
                        kids = [k[0] for k in (m.group(3) or "").split(",") if re.match(r"^[MER$?T]", k)]
                        opts.append((int(m.group(1)), TYPES.get(m.group(2), "?"), kids))
                    if len(opts) >= 2 and pick in [o[0] for o in opts]:
                        maps.append((run, [map_features(t, hp_frac, act, k) for _, t, k in opts], [o[0] for o in opts].index(pick), [t for _, t, _ in opts], hp_frac))
                else:
                    labels = dict((int(m.group(1)), m.group(2)) for m in re.finditer(r"^(\d+) (\w+)", e["state"], re.M))
                    if "Rest" in labels.values() and "Smith" in labels.values() and labels.get(pick) in ("Rest", "Smith"):
                        rests.append((run, rest_features(hp_frac, act), float(labels[pick] == "Rest"), hp_frac))
    return maps, rests


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--l2", type=float, default=1.0)
    a = ap.parse_args()
    import numpy as np
    maps, rests = decisions(a.dirs)

    def fit_map(rows):
        w = np.zeros(len(rows[0][1][0]))
        for _ in range(60):
            g, Hm = a.l2 * w, a.l2 * np.eye(len(w))
            for _r, X, y, _t, _h in rows:
                X = np.asarray(X); u = X @ w; p = np.exp(u - u.max()); p /= p.sum()
                mu = p @ X
                g += mu - X[y]
                Hm += (X * p[:, None]).T @ X - np.outer(mu, mu)
            w -= np.linalg.solve(Hm, g)
        return w

    def fit_rest(rows):
        X = np.array([r[1] for r in rows]); y = np.array([r[2] for r in rows]); w = np.zeros(X.shape[1])
        for _ in range(50):
            p = 1 / (1 + np.exp(-(X @ w)))
            w -= np.linalg.solve((X * (p * (1 - p))[:, None]).T @ X + a.l2 * np.eye(len(w)), X.T @ (p - y) + a.l2 * w)
        return w

    base_rank = lambda t, f: ({"R": 0, "?": 1, "T": 1, "$": 2, "M": 3, "E": 9} if f < 0.45 else {"T": 0, "?": 1, "M": 2, "R": 3, "$": 4, "E": 9}).get(t, 5)  # noqa: E731
    fold = lambda run: zlib.crc32(run.split("|")[-1].encode()) % 2  # noqa: E731
    acc = {"cloned": [], "base": []}
    racc = {"cloned": [], "base": []}
    for f in (0, 1):
        tr = [m for m in maps if fold(m[0]) != f]; te = [m for m in maps if fold(m[0]) == f]
        w = fit_map(tr)
        for _r, X, y, types, h in te:
            acc["cloned"].append(int(np.argmax(np.asarray(X) @ w) == y))
            acc["base"].append(int(min(range(len(types)), key=lambda i: base_rank(types[i], h)) == y))
        rtr = [r for r in rests if fold(r[0]) != f]; rte = [r for r in rests if fold(r[0]) == f]
        wr = fit_rest(rtr)
        for _r, x, y, h in rte:
            racc["cloned"].append(int((np.asarray(x) @ wr > 0) == bool(y)))
            racc["base"].append(int((h < 0.5) == bool(y)))
    print(f"map decisions {len(maps)} (>= 2 options), rest/smith {len(rests)} (rest share {np.mean([r[2] for r in rests]):.2f})")
    print(f"held-out agreement with the live choice: map cloned {np.mean(acc['cloned']):.3f} vs BasePolicy {np.mean(acc['base']):.3f}; "
          f"rest cloned {np.mean(racc['cloned']):.3f} vs BasePolicy {np.mean(racc['base']):.3f}")
    w, wr = fit_map(maps), fit_rest(rests)
    json.dump(dict(order=ORDER, map=[round(float(v), 4) for v in w], rest=[round(float(v), 4) for v in wr], n_map=len(maps), n_rest=len(rests),
                   source=[os.path.basename(d.rstrip("/")) for d in a.dirs]), open(a.out, "w"), indent=1)
    print("->", a.out)


if __name__ == "__main__":
    main()
