#!/usr/bin/env python3
"""Fixed expert-gap check: the live player (Engine: models/current.json, cover, exact turn search on; Engine.decide at the live 2 s
budget) on the game states where the expert's move beat it (hMrQSndDvPc divergences, confirmed by paired playouts or an exact
enumeration). States come from the replay log by compact index k (`tools/expert.py build <compact>` writes them). Per state and seed:
the move, and whether it falls in the expert's action class (gap closed). FAIL only when a must-win state does not play a winning
first move (exact value > 1).

usage: python tools/exact_turn_check.py [--seeds 4] [--budget 2.0] [--cap]
"""
import argparse, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tools"))
import torch  # noqa: E402
import expert as X  # noqa: E402
from agent import reenact as RE  # noqa: E402

RECORD = os.path.join(ROOT, "data", "expert", "baalorlord", "hMrQSndDvPc.compact.jsonl")
# (k, must): "win" = must play a winning first move; "gap" = confirmed our gap, reported
STATES = ((184, "win"), (204, "gap"), (438, "gap"), (460, "gap"), (477, "gap"), (479, "gap"), (517, "gap"), (626, "gap"), (627, "gap"), (640, "gap"),
          (659, "gap"))


def state_at(rec, k):
    x = RE.flatten(rec, built=None)[k]
    built = RE.built_record(rec, x["fight"])
    if built is None:
        X.build_from_replay(rec, x["fight"])
        built = RE.built_record(rec, x["fight"])
    fight = built["fight"]
    sim, j = next((sim, j) for i, sim, j in X.decision_states(fight) if i == x["i"])
    return x, fight["scenario"], sim, j


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", type=int, default=4)
    ap.add_argument("--budget", type=float, default=2.0)
    ap.add_argument("--no-cap", action="store_true", help="leaf values not capped at the win-now value (FastSearch hp_cap off)")
    a = ap.parse_args()
    torch.set_num_threads(4)
    from agent.engine import Engine
    eng = Engine(exact_turn=True, hp_cap=not a.no_cap)
    rec = RE.load(RECORD)
    ok, closed = True, 0
    for k, must in STATES:
        x, sc, sim, j = state_at(rec, k)
        cls = X.classes(sim)
        his = {cls[m] for m in X.match(sim, j)}
        text = dict(sim.legal())
        print(f"\nk{k} {x['fight'][12:]} #{x['i']} ({x.get('t')}): his {RE.describe(x)}")
        hits = 0
        for s in range(a.seeds):
            r = eng.decide(sc, sim.copy(), a.budget, seed=1000 + s, tol_hp=0.5, keep_potions=True)
            q = {o["text"]: o["q"] for o in r["options"]}
            mine = cls.get(r["action"]) in his
            hits += mine
            print(f"  seed {s}: {r['text']:28s} {'= his class' if mine else ''} rounds {r['rounds']} {r['seconds']}s  {q}")
            if must == "win":
                same = [v for t_, v in q.items() if v is not None and re.sub(r" #\d+", "", t_) == re.sub(r" #\d+", "", text[r["action"]])]
                if not (same and max(same) > 1.0):
                    ok = False
        closed += hits == a.seeds
        print(f"  his class on {hits}/{a.seeds} seeds")
    print(f"\n{closed}/{len(STATES)} states play his class on every seed")
    print("PASS" if ok else "FAIL: a must-win state did not play a winning first move")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
