#!/usr/bin/env python3
"""Plan gate 1, rollout side: from the first recorded decision screen of each act of each run, run-model rollouts (`BasePolicy`) with the
raw and the calibrated predictor; predicted P(clear this act) vs realised, pooled over arms.
usage: tools/runmodel_calib.py <events dir> [...] [--n 32] [--calib models/predictor_calib.json] [--out target/review/runmodel_calib.jsonl]"""
import argparse, glob, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--n", type=int, default=32)
    ap.add_argument("--calib", default=os.path.join(ROOT, "models", "predictor_calib.json"))
    ap.add_argument("--out", default="target/review/runmodel_calib.jsonl")
    a = ap.parse_args()
    import numpy as np
    from agent import price as PR
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    preds = {"raw": Predictor(PREDICTOR_CKPT, batch=1024), "calibrated": Predictor(PREDICTOR_CKPT, batch=1024, calib=a.calib)}
    rows = []
    with open(a.out, "w") as fh:
        for d in a.dirs:
            for f in sorted(glob.glob(os.path.join(d, "*", "*", "events.jsonl"))):
                seed = os.path.basename(os.path.dirname(f))
                top, won = outcome(f)
                done = set()
                for _floor, _state, st, _choice in PR.recorded_screens(f):
                    if st.act in done:
                        continue
                    done.add(st.act)
                    real = float(top > st.act or (won and st.act == 2))
                    r = dict(arm=os.path.basename(d.rstrip("/")), seed=seed, act=st.act, real=real)
                    for k, p in preds.items():
                        res = PR.price(st, [("nothing", None)], p, n=a.n, seed=1)
                        r[k] = float(res["nothing"]["act"].mean())
                    rows.append(r)
                    fh.write(json.dumps(r) + "\n")
                    fh.flush()
                print(d, seed, len(rows), flush=True)
    for act in (0, 1, 2):
        rs = [r for r in rows if r["act"] == act]
        if not rs:
            continue
        y = np.array([r["real"] for r in rs])
        line = f"A{act + 1} n {len(rs)} realised {y.mean():.3f} +- {y.std() / np.sqrt(len(y)):.3f} (constant Brier {np.mean((y - y.mean()) ** 2):.4f})"
        for k in preds:
            q = np.array([r[k] for r in rs])
            line += f" | {k} {q.mean():.3f} Brier {np.mean((q - y) ** 2):.4f}"
        print(line)


if __name__ == "__main__":
    main()
