#!/usr/bin/env python3
"""Stage-A gate (docs/plan.md): replay the measured decisions of data/bench/decision_gate.json through price.decide (the shared live /
scripted pricing) and report pass / fail per item. usage: tools/decision_gate.py [--n 128] [--no-search]"""
import argparse, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=128)
    ap.add_argument("--no-search", action="store_true")
    a = ap.parse_args()
    from agent import price as PR
    from agent.engine import Engine
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    spec = json.load(open(os.path.join(ROOT, "data", "bench", "decision_gate.json")))
    eng = None if a.no_search else Engine()
    pred = Predictor(eng.solver.net if eng else PREDICTOR_CKPT)
    passed = 0
    for it in spec["items"]:
        hit = None
        for floor, state, st, choice, _old in PR.recorded_screens(os.path.join(ROOT, it["run"]), (it["kind"],)):
            if it["floor"] in state.split("\n")[1] and (not it.get("page") or it["page"] in state):
                hit = (state, st)
                break
        if hit is None:
            print(f"MISSING {it['floor']} {it['kind']} in {it['run']}")
            continue
        state, st = hit
        if it.get("bosses"):
            st.bosses = list(it["bosses"])
        opts = PR.options(st, state)
        r = PR.decide(st, state, pred, eng, opts, n=a.n, seed=PR.screen_seed("gate", state))
        best = r["best"]
        ok = (not it.get("want") or best in it["want"]) and best not in it.get("avoid", [])
        passed += ok
        chk = ", ".join(f"{k} {v[0]:+.3f}+-{v[1]:.3f}" for k, v in sorted(r["check"].items(), key=lambda x: -x[1][0]))
        print(f"{'PASS' if ok else 'FAIL'} {it['floor']} {it['kind']}: best {best!r} (want {it.get('want')}, avoid {it.get('avoid')}) | search {chk}", flush=True)
    print(f"{passed}/{len(spec['items'])} passed")


if __name__ == "__main__":
    main()
