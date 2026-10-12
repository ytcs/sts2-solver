#!/usr/bin/env python3
"""Death bench gate (promotion): the value head at states whose value hides HP behind an enemy's death (Test Subject revives, Waterfall Giant's
blast; data/bench/leaf_values.json, states and play-out targets from tools/leaf_bench.py build) against the play-out win rate.
Pass: the candidate within 2 se of the target at every state (se floored at 1/n) and its mean |error| below the reference's, and within 2 se
at >= --holdout-share of the states that have targets in each held-out file (data/bench/contrast_holdout.json: community decks;
data/bench/contrast_weak.json: weak bot decks; tools/contrast.py). Exit 1 on fail.
usage: python tools/death_gate.py --ckpt CAND [--ref models/solver_r9.pt] [--bench data/bench/leaf_values.json] [--holdout FILE ...] --out FILE"""
import argparse, json, os, sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]


def errors(ckpt, items):
    from leaf_bench import build_state, describe, net_values
    from model import load
    net = load(ckpt)
    sims = [build_state(it) for it in items]
    for it, s in zip(items, sims):
        assert describe(s) == it["state"], (it["name"], describe(s), it["state"])
    pw = np.array([net_values(net, [s], [it["scenario"]["max_hp"]])[0][0] for it, s in zip(items, sims)])
    t = np.array([it["target"]["win"] for it in items])
    se = np.array([max(it["target"]["win_se"], 1.0 / it["target"]["n"]) for it in items])
    return pw, t, se


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--ref", default=os.path.join(ROOT, "models", "solver_r9.pt"))
    ap.add_argument("--bench", default=os.path.join(ROOT, "data", "bench", "leaf_values.json"))
    ap.add_argument("--holdout", nargs="+", default=[os.path.join(ROOT, "data", "bench", f) for f in ("contrast_holdout.json", "contrast_weak.json")])
    ap.add_argument("--holdout-share", type=float, default=0.8)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    items = json.load(open(a.bench))["items"]
    pc, t, se = errors(a.ckpt, items)
    pr = errors(a.ref, items)[0]
    within = np.abs(pc - t) <= 2 * se
    mae_c, mae_r = float(np.abs(pc - t).mean()), float(np.abs(pr - t).mean())
    rows = [dict(state=it["name"], target=round(float(x), 4), se=round(float(s), 4), cand=round(float(c), 4), ref=round(float(r), 4), within=bool(w))
            for it, x, s, c, r, w in zip(items, t, se, pc, pr, within)]
    for r in rows:
        print(f"{'ok  ' if r['within'] else 'OFF '} {r['state']:40s} target {r['target']:.3f} +- {r['se']:.3f} | cand {r['cand']:.3f} ref {r['ref']:.3f}")
    holdouts = []
    for f in a.holdout:
        hold = [it for it in json.load(open(f))["items"] if "target" in it] if os.path.exists(f) else []
        hrows, share, hm = [], 1.0, {}
        if hold:
            hc, ht, hs = errors(a.ckpt, hold)
            hr = errors(a.ref, hold)[0]
            hw = np.abs(hc - ht) <= 2 * hs
            share = float(hw.mean())
            hm = dict(cand=float(np.abs(hc - ht).mean()), ref=float(np.abs(hr - ht).mean()), ref_within=int((np.abs(hr - ht) <= 2 * hs).sum()))
            hrows = [dict(state=it["name"], target=round(float(x), 4), se=round(float(s), 4), cand=round(float(c), 4), ref=round(float(r), 4), within=bool(w))
                     for it, x, s, c, r, w in zip(hold, ht, hs, hc, hr, hw)]
        name = os.path.basename(f)
        print(f"{name}: {sum(r['within'] for r in hrows)}/{len(hrows)} within 2 se (need {a.holdout_share:.0%}; ref {hm['ref_within']}); mean |error| cand "
              f"{hm['cand']:.4f} ref {hm['ref']:.4f}" if hrows else f"{name}: no targets")
        holdouts.append(dict(file=f, share=share, states=len(hrows), mae=hm, rows=hrows))
    ok = bool(within.all() and mae_c < mae_r and all(h["share"] >= a.holdout_share for h in holdouts))
    print(f"{int(within.sum())}/{len(rows)} within 2 se; mean |error| cand {mae_c:.4f} vs ref {mae_r:.4f}\nDEATH GATE {'PASS' if ok else 'FAIL'}")
    json.dump(dict(ckpt=a.ckpt, ref=a.ref, bench=a.bench, mae=dict(cand=mae_c, ref=mae_r), within=int(within.sum()), states=len(rows), passed=ok, rows=rows,
                   holdouts=holdouts), open(a.out, "w"), indent=1)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
