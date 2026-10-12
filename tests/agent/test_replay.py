import json
import os

from support import FIX

from agent.fight import RANDOM_PREFIXES, Replayer
from agent.live import Live


def _fight(name):
    with open(os.path.join(FIX, "fights", name), encoding="utf-8") as f:
        return json.load(f)


# run 4: Mayhem / Cascade auto-play the top of the hidden draw pile (the Skulking Colony's Hardened Shell cap was spent by Mayhem's Bludgeon;
# the Thieving Hopper's Armaments select came out of a Cascade); the replay must re-match every observed state, including mid-card selections
def test_replay_rematches_draw_order_auto_plays():
    for name in ("skulking_colony_mayhem.json", "thieving_hopper_cascade.json"):
        f = _fight(name)
        for seed in (1, 2):
            rp = Replayer(f, seed=seed)
            for k in range(1, len(f["log"]) + 1):
                state = f["states"][k] if k < len(f["states"]) and f["states"][k] is not None else f["state"]
                rp.advance(dict(f, log=f["log"][:k], states=f["states"][:k + 1], state=state))
                assert not rp.errors, (name, seed, k, rp.errors)
                bad = [l for l in rp.sim.diff(json.dumps(state)) if not l.startswith(RANDOM_PREFIXES)]
                assert not bad, (name, seed, k, f["log"][k - 1], bad[:2])


def test_selection_labels_and_game_indices():
    assert Live._base("Wither+1") == "WITHER" and Live._base("Pommel Strike+") == "POMMEL_STRIKE"
    screen = "SELECT 1\nChoose a card to put on top of your Draw Pile.\n0 Strike(1) Deal 6 damage.\n1 Wither+1(-1) Unplayable.\n2 Jackpot(3) Deal 25 damage.\n"
    assert Live._game_picks(Live, [0], ["JACKPOT"], screen) == [2]
