"""Offline analysis of a saved fidelity fight (evals/fidelity_fights/*.json): replays it in the simulator and prints, for the divergences that are not benign, the
action, the card played, the simulator's hand and piles before it, and what the game showed afterwards.

    python -m agent.fidelity_trace evals/fidelity_fights/009_KNIGHTS_ELITE.json [--only 'diff .energy']
"""
import json
import sys

import sts2

from agent.fight import Replayer, RANDOM_PREFIXES, category


class Tracer(Replayer):
    def __init__(self, f):
        super().__init__(f)
        self.events = []
        self._before = None

    def _card_at(self, act):
        self._before = self.sim.snapshot()
        return super()._card_at(act)

    def _end_turn(self, state):
        self._before = self.sim.snapshot()
        return super()._end_turn(state)

    def _compare_and_sync(self, state, action):
        if self.sim.stage() == "play":
            lines = [l for l in self.sim.diff(json.dumps(state)) if not l.startswith(RANDOM_PREFIXES) and "props.Skin" not in l]
            if lines:
                self.events.append(dict(step=self.applied, action=action, played=getattr(self, "_played", ""), before=self._before, sim_after=self.sim.snapshot(), game_after=state, diff=lines))
        super()._compare_and_sync(state, action)


def brief(sn):
    if sn is None:
        return ""
    s = json.loads(sn) if isinstance(sn, str) else sn
    ids = lambda cs: sorted(c["id"] for c in cs)  # noqa: E731
    return (f"E{s.get('energy')} hp{s['player']['hp']} b{s['player']['block']} hand{ids(s['hand'])} draw{len(s['draw'])} disc{ids(s['discard'])} exh{ids(s['exhaust'])} "
            f"enemies{[(e['id'], e['hp'], e['block']) for e in s['enemies'] if e['alive']]} powers{[(p['id'], p['amount']) for p in s['player']['powers']]}")


def main():
    path = sys.argv[1]
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""
    d = json.load(open(path))
    f = d["fight"]
    rp = Tracer(f)
    rp.advance(f)
    for e in rp.events:
        if only and not any(only in category(l) or only in l for l in e["diff"]):
            continue
        print(f"-- step {e['step']} {e['action']} [{e['played']}]")
        print("   diff:", "; ".join(l[:160] for l in e["diff"][:4]))
        print("   before(sim):", brief(e["before"])[:400])
        print("   after(sim): ", brief(e["sim_after"])[:400])
        print("   after(game):", brief(e["game_after"])[:400])


if __name__ == "__main__":
    main()
