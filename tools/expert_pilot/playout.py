"""Full-fight playouts from a decision: arm H plays his action, arm L the live player's, then the live player (Engine, small
budget) finishes the fight. Paired by determinization seed. Utility = win 1 + 0.5 hp/max_hp, loss -1 (the search's units).

usage: python playout.py spec.py i --n 24 --budget 0.3
"""
import argparse, json, math, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, r"C:\Users\steve\sts2\sts2-solver")
import numpy as np  # noqa: E402
from evaluate import classes, decision_states, load_spec, match  # noqa: E402


def utility(sim, max_hp):
    o = sim.outcome()
    hp = json.loads(sim.snapshot())["player"]["hp"]
    return (1.0 + 0.5 * min(hp / max_hp, 1.0)) if o == 1 else -1.0, o, hp


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("spec")
    ap.add_argument("i", type=int)
    ap.add_argument("--n", type=int, default=24)
    ap.add_argument("--budget", type=float, default=0.3)
    ap.add_argument("--live-file", default=None)
    a = ap.parse_args()
    spec = load_spec(a.spec)
    sc, dec = decision_states(spec)
    d = dec[a.i]
    sim0 = d["sim"]
    from agent.engine import Engine
    eng = Engine()
    his = match(sim0, d["action"])[0]
    live = eng.decide(sc, sim0.copy(), 2.0, tol_hp=0.5, keep_potions=True)["action"]
    cls = classes(sim0)
    text = dict(sim0.legal())
    if cls[live] == cls[his]:
        print("live agrees with him at this decision; nothing to compare")
        return
    res = {"H": [], "L": []}
    for s in range(a.n):
        for arm, first in (("H", his), ("L", live)):
            sim = sim0.copy()
            sim.determinize(100003 + s)
            sim.step(first)
            steps = 0
            while sim.stage() != "over" and sim.outcome() == 0 and steps < 300:
                r = eng.decide(sc, sim, 0.2 if sim.stage() == "choice" else a.budget, seed=7 * s + steps, tol_hp=0.5, keep_potions=False)
                sim.step(r["action"])
                steps += 1
            res[arm].append(utility(sim, sc["max_hp"]))
        h, l = res["H"][-1], res["L"][-1]
        print(f"seed {s}: H {h}  L {l}", flush=True)
    H = np.array([x[0] for x in res["H"]])
    L = np.array([x[0] for x in res["L"]])
    dif = H - L
    se = dif.std(ddof=1) / math.sqrt(len(dif))
    out = dict(spec=os.path.basename(a.spec), i=a.i, where=d["where"], his=text[his], live=text[live], n=a.n,
               H_mean=float(H.mean()), L_mean=float(L.mean()), H_win=float(np.mean([x[1] == 1 for x in res["H"]])),
               L_win=float(np.mean([x[1] == 1 for x in res["L"]])), H_hp=float(np.mean([x[2] for x in res["H"]])),
               L_hp=float(np.mean([x[2] for x in res["L"]])), his_minus_live=float(dif.mean()), se=float(se))
    print(json.dumps(out))
    with open(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "out", "playout.jsonl"), "a") as f:
        f.write(json.dumps(out) + chr(10))


if __name__ == "__main__":
    main()
