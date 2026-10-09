#!/usr/bin/env python3
"""Fixed check of the exact turn search (FastSearch exact_turn) on the three E34 pilot states: Skulking Colony #18 must play a
winning first move (exact value > 1 = a win); Phantasmal Gardeners #1 and Lagavulin Matriarch #0 are reported. The live player's search
(Engine: models/current.json, M=5 K=32 cover) decides each state with exact_turn off and on, one round per seed, then Engine.decide (2 s).

usage: python tools/exact_turn_check.py [--seeds 4]
"""
import argparse, json, os, sys, time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tools"))
import torch  # noqa: E402
import sts2  # noqa: E402
from expert import decision_states  # noqa: E402

FIGHTS = os.path.join(ROOT, "data", "expert", "baalorlord", "fights")
STATES = (("hMrQSndDvPc_F12_SKULKING_COLONY_ELITE.json", 18, True), ("hMrQSndDvPc_F15_PHANTASMAL_GARDENERS_ELITE.json", 1, False),
          ("hMrQSndDvPc_F17_LAGAVULIN_MATRIARCH_BOSS.json", 0, False))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", type=int, default=4)
    a = ap.parse_args()
    torch.set_num_threads(4)
    from agent.engine import Engine
    print("sts2:", sts2.__file__)
    eng = Engine(exact_turn=True)
    fs, cfg = eng.fs, eng.fs.exact
    ok = True
    for name, i, must_win in STATES:
        fight = json.load(open(os.path.join(FIGHTS, name)))["fight"]
        sc = fight["scenario"]
        d = dict(sim=next(sim for k, sim, _j in decision_states(fight) if k == i))
        text = dict(d["sim"].legal())
        print(f"\n{name} #{i}")
        for s in range(a.seeds):
            for on in (False, True):
                fs.exact = cfg if on else None
                t = time.perf_counter()
                r = fs.decide(sc, d["sim"].copy(), seed=1000 + s)
                t = time.perf_counter() - t
                q = {text[o]: round(v, 3) for o, v, lg in zip(r["opts"], r["q"], r["legal"]) if lg}
                print(f"  seed {s} exact {'on ' if on else 'off'}: {text[r['action']]:30s} triggered={r['exact']} {t:.2f}s  {q}")
                if on and must_win and not q.get(text[r["action"]], -9) > 1.0:
                    ok = False
        fs.exact = cfg
        r = eng.decide(sc, d["sim"].copy(), 2.0, tol_hp=0.5, keep_potions=True)
        print(f"  live player (Engine.decide 2 s, exact on): {r['text']}  rounds {r['rounds']} in {r['seconds']}s  {[(o['text'], o['q']) for o in r['options']]}")
        if must_win and ("DEFEND" in r["text"] or r["text"] == "end turn"):
            ok = False
    print("\nPASS" if ok else "\nFAIL: Skulking Colony #18 did not play a winning first move")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
