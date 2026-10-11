#!/usr/bin/env python3
"""Plan gate 1, fight side: per recorded fight, search-predicted (`tools/card_timing.py` base rows) and predictor-predicted win / end HP
vs the realised outcome in the events; Brier vs a constant, calibration bins, HP bias by act x kind, act clear as the product per run.
usage: tools/fight_calib.py <events dir> --timing target/review/card_timing.jsonl"""
import argparse, collections, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def kind(enc):
    return "boss" if enc.endswith("_BOSS") else "elite" if "ELITE" in enc else "hall"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--timing", required=True)
    a = ap.parse_args()
    import numpy as np
    import predictor as PR
    from solver import PREDICTOR_CKPT
    timing = {(r["seed"], r["i"]): r for r in map(json.loads, open(a.timing))}
    F = []
    for f in sorted(glob.glob(os.path.join(a.events, "*", "*", "events.jsonl"))):
        seed = os.path.basename(os.path.dirname(f))
        cur, i = None, -1
        for l in open(f, encoding="utf-8"):
            e = json.loads(l)
            if e["kind"] == "fight_start":
                if cur:
                    F.append(cur)
                i += 1
                cur = dict(seed=seed, i=i, enc=e["encounter"], act=e.get("act"), hp=int(e["hp"]), mx=int(e["max_hp"]), sc=e["scenario"], end=None)
            elif e["kind"] == "fight_end" and cur:
                hp = e.get("hp"); hp = json.loads(hp) if isinstance(hp, str) else hp
                cur["end"] = int(hp[0]) if hp else 0
                F.append(cur); cur = None
        if cur:
            F.append(dict(cur, end=0))
    F = [x for x in F if (x["seed"], x["i"]) in timing and x["end"] is not None]
    P = PR.Predictor(PREDICTOR_CKPT, batch=1024).fight_start([x["sc"] for x in F], 8)
    pw, pe = PR.p_win(P), PR.end_hp(P)
    for x, w, h in zip(F, pw, pe):
        t = timing[(x["seed"], x["i"])]["base"]
        x.update(won=float(x["end"] > 0), s_win=t["win"], s_end=x["hp"] - t["lost"] * x["mx"] if t["lost"] is not None else np.nan, p_win=float(w), p_end=float(h))
    y = np.array([x["won"] for x in F])
    print(f"fights {len(F)}, deaths {int((1 - y).sum())}")
    base = np.mean((y - y.mean()) ** 2)
    for src in ("s", "p"):
        q = np.array([x[f"{src}_win"] for x in F])
        print(f"{'search' if src == 's' else 'predictor'}: Brier {np.mean((q - y) ** 2):.4f} vs constant {base:.4f}; mean predicted deaths {np.sum(1 - q):.1f}")
        for lo, hi in ((0, .5), (.5, .8), (.8, .95), (.95, .99), (.99, 1.01)):
            m = (q >= lo) & (q < hi)
            if m.any():
                print(f"   P in [{lo:.2f}, {min(hi, 1):.2f}): n {m.sum():3d} predicted {q[m].mean():.3f} realised {y[m].mean():.3f}")
    print("\nend HP (wins only), mean realised - predicted, in HP; by act x kind")
    g = collections.defaultdict(list)
    for x in F:
        if x["won"]:
            g[(x["act"], kind(x["enc"]))].append((x["end"] - x["s_end"], x["end"] - x["p_end"], x["end"] - x["hp"]))
    for k in sorted(g):
        v = np.array(g[k])
        print(f"A{k[0] + 1} {k[1]:5s} n {len(v):3d} | realised change {v[:, 2].mean():+5.1f} | search bias {np.nanmean(v[:, 0]):+5.1f} | predictor bias {v[:, 1].mean():+5.1f}")
    print("\nact clear per run (product of per-fight P(win) over the act's recorded fights) vs realised")
    runs = collections.defaultdict(list)
    for x in F:
        runs[(x["seed"], x["act"])].append(x)
    for act in (0, 1, 2):
        rs = [v for (s, ac), v in runs.items() if ac == act]
        real = np.array([min(z["won"] for z in v) for v in rs])
        for src in ("s", "p"):
            pr = np.array([np.prod([z[f"{src}_win"] for z in v]) for v in rs])
            print(f"A{act + 1} runs {len(rs)} | {'search' if src == 's' else 'predictor'} {pr.mean():.3f} vs realised {real.mean():.3f} | Brier {np.mean((pr - real) ** 2):.4f} vs constant {np.mean((real - real.mean()) ** 2):.4f}")


if __name__ == "__main__":
    main()
