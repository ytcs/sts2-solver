"""Live-player column for a boss fight with the harness's boss objective (proposal.fight_objective: win only for an act-1 boss)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, r"C:\Users\steve\sts2\sts2-solver")
from evaluate import classes, decision_states, load_spec, match  # noqa: E402
from agent import proposal  # noqa: E402
from agent.engine import Engine  # noqa: E402

sc, dec = decision_states(load_spec(sys.argv[1]))
worth, why = proposal.fight_objective(sc)
print("objective:", why)
eng = Engine()
for i, d in enumerate(dec):
    sim = d["sim"]
    if len(sim.legal()) < 2:
        continue
    cls = classes(sim)
    mine = {cls[x] for x in match(sim, d["action"])}
    r = eng.decide(sc, sim.copy(), 2.0, tol_hp=0.5, keep_potions=True, worth=worth)
    print(i, d["where"], "live(win-only):", r["text"], "agree" if cls.get(r["action"]) in mine else "DISAGREE", flush=True)
