"""Replay a fight record through agent.fight.Replayer (as hindsight does) and report sync stats per decision."""
import json, sys
sys.path.insert(0, r"C:\Users\steve\sts2\sts2-solver")
from agent.fight import Replayer

rec = json.load(open(sys.argv[1]))
fight = rec["fight"]
rp = Replayer(fight, seed=1)
ok = rp.advance(fight)
print("advance ok:", ok, "outcome", rp.sim.outcome(), "stage", rp.sim.stage())
print(dict(rp.stats))
for k, v in rp.examples.items():
    print(" ", k, v[:2])
print("errors", rp.errors[:3])
