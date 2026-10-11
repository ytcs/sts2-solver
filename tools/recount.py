#!/usr/bin/env python3
"""re-price recorded card-reward screens of baseline runs through the current price.decide: old vs new pick per screen.
usage: tools/recount.py <dir with */events.jsonl> [--floors A1] [--n 128] [--out file.jsonl]"""
import argparse, glob, json, os, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--floors", default="")
    ap.add_argument("--n", type=int, default=128)
    ap.add_argument("--out")
    a = ap.parse_args()
    from agent import price as PR
    from agent.engine import Engine
    from predictor import Predictor
    eng = Engine()
    pred = Predictor(eng.solver.net)
    out = open(a.out, "w", encoding="utf-8") if a.out else None
    for f in sorted(glob.glob(os.path.join(a.root, "**", "events.jsonl"), recursive=True)):
        seed = os.path.basename(os.path.dirname(f))
        for floor, state, st, choice, _old in PR.recorded_screens(f, ("CARD_REWARD",)):
            hdr = state.split("\n")[1]
            if a.floors and not hdr.startswith(a.floors):
                continue
            t0 = time.time()
            opts = PR.options(st, state)
            r = PR.decide(st, state, pred, eng, opts, n=a.n, seed=PR.screen_seed("recount", state))
            ck = {k: [round(v[0], 3), round(v[1], 3), {str(x): round(y, 3) for x, y in v[4].items()}] for k, v in r["check"].items()}
            row = dict(seed=seed, floor=hdr[:8], old=choice, new=r["best"], radius=r["radius"], check=ck, s=round(time.time() - t0, 1))
            print(json.dumps(row), flush=True)
            if out:
                out.write(json.dumps(row) + "\n")
                out.flush()


if __name__ == "__main__":
    main()
