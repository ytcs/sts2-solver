#!/usr/bin/env python3
"""Plan gate 1, fight side: per recorded fight, the predictor's win / end HP vs the realised outcome, pooled over arms. The scenario
gets the potions live play could throw (none when a `potion_reserve` policy event kept them). Brier vs a constant, calibration bins,
HP bias by act x kind, act clear as the product per run vs realised, divergence events by encounter. --out keeps the per-fight rows.
usage: tools/fight_calib.py <events dir> [...] [--out target/review/fights.jsonl]"""
import argparse, collections, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def kind(enc):
    return "boss" if enc.endswith("_BOSS") else "elite" if "ELITE" in enc else "hall"


def fights(dirs):
    F, div = [], collections.Counter()
    for d in dirs:
        seen = set()
        for f in sorted(glob.glob(os.path.join(d, "*", "*", "events.jsonl"))):
            seed = os.path.basename(os.path.dirname(f))
            if seed in seen:
                continue
            seen.add(seed)
            cur, prev, i = None, None, -1
            for l in open(f, encoding="utf-8"):
                e = json.loads(l)
                if e["kind"] == "fight_start":
                    if cur:
                        F.append(cur)
                    i += 1
                    sc = e["scenario"]
                    kept = bool(prev and prev.get("kind") == "policy" and prev.get("component") == "potion_reserve" and (prev.get("detail") or {}).get("keep"))
                    cur = dict(arm=os.path.basename(d.rstrip("/")), seed=seed, i=i, enc=e["encounter"], act=e.get("act"), hp=int(e["hp"]), mx=int(e["max_hp"]),
                               kept=kept, sc=dict(sc, potions=[] if kept else sc.get("potions", [])), end=None, held=0 if kept else len(sc.get("potions", [])), used=0)
                elif e["kind"] == "fight_end" and cur:
                    hp = e.get("hp"); hp = json.loads(hp) if isinstance(hp, str) else hp
                    cur["end"] = int(hp[0]) if hp else 0
                    cur["used"] = int(e.get("potions_used") or 0)
                    F.append(cur); cur = None
                elif e["kind"] == "divergence" and cur:
                    div[cur["enc"]] += 1
                prev = e
            if cur:
                F.append(dict(cur, end=0))
    return [x for x in F if x["end"] is not None], div


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--out")
    a = ap.parse_args()
    import numpy as np
    import predictor as PR
    from solver import PREDICTOR_CKPT
    F, div = fights(a.dirs)
    P = PR.Predictor(PREDICTOR_CKPT, batch=1024).fight_start([x["sc"] for x in F], 8)
    for x, w, h, p in zip(F, PR.p_win(P), PR.end_hp(P), P):
        x.update(won=float(x["end"] > 0), p_win=float(w), p_end=float(h), dist=[round(float(v), 5) for v in p])
    if a.out:
        with open(a.out, "w") as fh:
            for x in F:
                fh.write(json.dumps({k: v for k, v in x.items() if k != "sc"}) + "\n")
    y = np.array([x["won"] for x in F]); q = np.array([x["p_win"] for x in F])
    print(f"fights {len(F)} over {len({(x['arm'], x['seed']) for x in F})} runs, deaths {int((1 - y).sum())}, potions kept by policy in {sum(x['kept'] for x in F)}")
    print(f"predictor: Brier {np.mean((q - y) ** 2):.4f} vs constant {np.mean((y - y.mean()) ** 2):.4f}; predicted deaths {np.sum(1 - q):.1f}")
    for lo, hi in ((0, .5), (.5, .8), (.8, .95), (.95, .99), (.99, 1.01)):
        m = (q >= lo) & (q < hi)
        if m.any():
            print(f"   P in [{lo:.2f}, {min(hi, 1):.2f}): n {m.sum():4d} predicted {q[m].mean():.3f} realised {y[m].mean():.3f}")
    print("\nend HP on wins, realised minus predicted (HP), by act x kind")
    g = collections.defaultdict(list)
    for x in F:
        if x["won"]:
            g[(x["act"], kind(x["enc"]))].append((x["end"] - x["p_end"], x["end"] - x["hp"]))
    for k in sorted(g):
        v = np.array(g[k])
        print(f"A{k[0] + 1} {k[1]:5s} n {len(v):4d} | realised change {v[:, 1].mean():+5.1f} | predictor bias {v[:, 0].mean():+5.1f}")
    print("\nact clear per run (product over the act's recorded fights) vs realised")
    runs = collections.defaultdict(list)
    for x in F:
        runs[(x["arm"], x["seed"], x["act"])].append(x)
    for act in (0, 1, 2):
        rs = [v for k, v in runs.items() if k[2] == act]
        real = np.array([min(z["won"] for z in v) for v in rs]); pr = np.array([np.prod([z["p_win"] for z in v]) for v in rs])
        print(f"A{act + 1} runs {len(rs)} | predictor {pr.mean():.3f} vs realised {real.mean():.3f} +- {real.std() / np.sqrt(len(real)):.3f} | Brier {np.mean((pr - real) ** 2):.4f} vs constant {np.mean((real - real.mean()) ** 2):.4f}")
    print("\ndivergence events by encounter (top 10):", div.most_common(10))


if __name__ == "__main__":
    main()
