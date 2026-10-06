#!/usr/bin/env python3
"""M2 gate probe (`docs/rl_redesign.md`): adding a relic with no combat effect must not move a fight. For each probe relic, the eval scenarios are played
with and without it (same job seeds), with the relic mask on (the default) and off; reports the paired win / HP-lost differences and how many fights
changed at all.

  STS2_DEVICE=cuda python tools/probe_macro_relics.py [--n 100] [--attempts 8] [--relics LAVA_LAMP,PLANISPHERE,...] [--out evals/probe_macro_relics.json]
"""
import argparse, json, os, sys
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--relics", default="")
    ap.add_argument("--count", type=int, default=8, help="probe relics drawn from the macro-only class when --relics is not given")
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "probe_macro_relics.json"))
    a = ap.parse_args()
    import sts2
    from solver import Solver
    cls = json.load(open(os.path.join(ROOT, "data", "relic_classes.json")))["relics"]
    macro = sorted(k for k, v in cls.items() if v["class"] == "macro_only" and k != "VAKUU_CARD_SELECTOR")
    probe = a.relics.split(",") if a.relics else ["LAVA_LAMP", "PLANISPHERE"] + list(np.random.default_rng(3).choice([m for m in macro if m not in ("LAVA_LAMP", "PLANISPHERE")], a.count - 2, replace=False))
    base = json.load(open(os.path.join(ROOT, "target", "train", "eval.json")))[:a.n]
    S = Solver(value_ckpts=None)
    out = {}
    for mask in (True, False):
        sts2.set_relic_mask(mask)
        r0 = S.solve(base, attempts=a.attempts, seed=21, groups=list(range(len(base))))
        w0 = np.array([x["win"] for x in r0]); h0 = np.array([x["hp_lost"] for x in r0], float)
        for rel in probe:
            var = [dict(s, relics=list(s.get("relics", [])) + [rel]) for s in base]
            r1 = S.solve(var, attempts=a.attempts, seed=21, groups=list(range(len(base))))
            w1 = np.array([x["win"] for x in r1]); h1 = np.array([x["hp_lost"] for x in r1], float)
            dw, dh = w1 - w0, h1 - h0
            k = len(dw) ** 0.5
            changed = int(sum(any(a_ != b_ for a_, b_ in zip(x["wins"], y["wins"])) or x["ends"] != y["ends"] for x, y in zip(r0, r1)))
            row = dict(win=float(dw.mean()), win_se=float(dw.std(ddof=1) / k), hp_lost=float(np.nanmean(dh)), hp_lost_se=float(np.nanstd(dh, ddof=1) / k),
                       max_abs_win=float(np.abs(dw).max()), fights_changed=changed)
            out.setdefault("mask_on" if mask else "mask_off", {})[rel] = row
            print(f"mask {'on ' if mask else 'off'} {rel:28s} win {row['win']:+.4f} +- {row['win_se']:.4f}  HP lost {row['hp_lost']:+.4f}  max |dwin| {row['max_abs_win']:.3f}  fights changed {changed}/{len(base)}", flush=True)
    sts2.set_relic_mask(True)
    json.dump(dict(n=a.n, attempts=a.attempts, probe=probe, results=out), open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
