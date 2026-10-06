#!/usr/bin/env python3
"""Paired non-inferiority gate between solver configurations (`docs/rl_redesign.md` M1): every configuration plays the same fights with the same job
seeds (search on, `Solver`), per set; differences against the reference are fight-clustered (per scenario, mean over its attempts) with their se.

  STS2_DEVICE=cuda python tools/gate_m1.py --ref control=target/m1/control/ckpt.pt --cand heads=target/m1/heads/ckpt.pt \
      [--values models/solver_c128.pt,models/solver_d128.pt] [--n 600] [--attempts 2] [--out evals/gate_m1.json]

Configurations: each of ref / cand alone, and with `--values` (extra value nets averaged in, as `adopt` would install). A checkpoint written
`path@nomask` runs with the relic mask off (observations show every relic, as before M2). PASS (per candidate config vs
the same ref config, every set): win >= -0.01 and HP lost <= +0.01 of max (loss counts the whole start HP), both on the point estimate.
"""
import argparse, json, os, sys, time

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, os.path.join(ROOT, "rl"))

SEED = int(os.environ.get("GATE_SEED", "7"))
SETS = {"eval": "target/train/eval.json", "eval_energy": "target/train/eval_energy.json", "corpus": "target/train/corpus_all.json"}  # corpus: fights of real runs
SETS = {k: v for k, v in SETS.items() if os.path.exists(os.path.join(ROOT, v))}


def run(ckpt, values, sets, n, attempts):
    from solver import Solver
    import sts2
    nomask = ckpt.endswith("@nomask")
    ckpt = ckpt[:-len("@nomask")] if nomask else ckpt
    prev = sts2.set_relic_mask(not nomask)
    S = Solver(ckpt, value_ckpts=values or None)
    out = {}
    for name, path in sets.items():
        scen = json.load(open(os.path.join(ROOT, path)))[:n]
        t = time.time()
        res = S.solve(scen, attempts=attempts, seed=SEED)
        out[name] = dict(win=[r["win"] for r in res], hp_lost=[r["hp_lost"] if r["hp_lost"] is not None else np.nan for r in res], secs=time.time() - t)
    del S
    sts2.set_relic_mask(prev)
    import torch
    torch.cuda.empty_cache()
    return out


def paired(a, b):
    d = np.asarray(b, float) - np.asarray(a, float)
    d = d[~np.isnan(d)]
    return float(d.mean()), float(d.std(ddof=1) / len(d) ** 0.5)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True, action="append", help="name=ckpt; repeat for several references")
    ap.add_argument("--cand", required=True)
    ap.add_argument("--values", default="")
    ap.add_argument("--n", type=int, default=600)
    ap.add_argument("--attempts", type=int, default=2)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "gate_m1.json"))
    ap.add_argument("--pair", action="append", default=[], help="extra paired comparison cand_conf:ref_conf (e.g. heads:current+cd)")
    ap.add_argument("--set", action="append", default=[], help="extra scenario set name=path (reported; PASS still needs every set)")
    ap.add_argument("--only", help="comma list: run only these sets")
    a = ap.parse_args()
    global SETS
    SETS = dict(SETS, **dict(x.split("=", 1) for x in a.set))
    if a.only:
        SETS = {k: v for k, v in SETS.items() if k in a.only.split(",")}
    vals = [v for v in a.values.split(",") if v]
    refs = [r.split("=", 1) for r in a.ref]
    cn, cp = a.cand.split("=", 1)
    confs = [(n_, c, []) for n_, c in refs] + [(cn, cp, [])]
    if vals:
        confs += [(n_ + "+cd", c, vals) for n_, c in refs] + [(cn + "+cd", cp, vals)]
    res = {}
    for name, ck, vl in confs:
        res[name] = run(ck, vl, SETS, a.n, a.attempts)
        print(name, {s: (round(float(np.mean(r["win"])), 4), round(float(np.nanmean(r["hp_lost"])), 4), round(r["secs"])) for s, r in res[name].items()}, flush=True)
    verdict = {}
    pairs = [(rn, cn) for rn, _ in refs] + ([(rn + "+cd", cn + "+cd") for rn, _ in refs] if vals else []) + [tuple(p.split(":")[::-1]) for p in a.pair]
    for ref, cand in pairs:
        rows = {}
        ok = True
        for s in SETS:
            dw, sw = paired(res[ref][s]["win"], res[cand][s]["win"])
            dh, sh = paired(res[ref][s]["hp_lost"], res[cand][s]["hp_lost"])
            rows[s] = dict(win=round(dw, 4), win_se=round(sw, 4), hp_lost=round(dh, 4), hp_lost_se=round(sh, 4))
            ok &= dw >= -0.01 and dh <= 0.01
            print(f"{cand} - {ref} [{s}]: win {dw:+.4f} +- {sw:.4f}   HP lost {dh:+.4f} +- {sh:.4f}", flush=True)
        verdict[f"{cand} vs {ref}"] = dict(sets=rows, PASS=bool(ok))
        print(f"{cand} vs {ref}: {'PASS' if ok else 'FAIL'}", flush=True)
    json.dump(dict(confs=[(n_, c, v) for n_, c, v in confs], n=a.n, attempts=a.attempts, verdict=verdict,
                   means={k: {s: dict(win=float(np.mean(r["win"])), hp_lost=float(np.nanmean(r["hp_lost"]))) for s, r in v.items()} for k, v in res.items()},
                   per_scenario={k: {s: dict(win=r["win"], hp_lost=[None if np.isnan(x) else x for x in r["hp_lost"]]) for s, r in v.items()} for k, v in res.items()}, seed=SEED),
              open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
