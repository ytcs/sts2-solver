import json
import os

from support import ROOT

from agent.fight import Replayer

FIGHT = os.path.join(ROOT, "tests", "agent", "fixtures", "fights", "knights_thrash_dampen.json")


def test_replayer_rerolls_thrash_exhaust_pick_to_the_games():
    fight = json.load(open(FIGHT))
    for seed in range(6):
        rp = Replayer(fight, seed=seed)
        rp.advance(fight)
        bad = {k: v for k, v in rp.stats.items() if k.startswith(("diff", "residual", "action failed", "intent unmatched"))}
        assert not rp.errors and not bad, (seed, rp.errors, bad)
