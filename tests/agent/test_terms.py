"""data/macro_rules.json terms DSL (agent/terms.py) as used by runmodel.pick_with_terms / take_relic."""
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]
from agent import runmodel as R, terms as TM  # noqa: E402


class _St:
    act, gold, hp, max_hp, relics = 1, 100, 50, 80, ["BURNING_BLOOD"]

    def __init__(self, ids):
        self.deck = [{"id": i} for i in ids]


BASE = ["STRIKE_IRONCLAD"] * 5 + ["INFLAME", "TWIN_STRIKE", "SWORD_BOOMERANG", "INFERNO"]


def test_rules_compile_and_reject_unsafe():
    assert TM.load()
    for bad in ("__import__('os')", "deck_size.real", "open('x')", "unknown_var > 1"):
        try:
            TM.compiled(bad)
        except ValueError:
            continue
        raise AssertionError(bad)


def test_margin_only_past_density_target():
    small, big = _St(BASE), _St(BASE + ["DEFEND_IRONCLAD"] * 12)
    assert R.pick_with_terms("card_reward", small, [0.5, 0.501], [(None, None), ("card", "ANGER")]) == 1
    assert R.pick_with_terms("card_reward", big, [0.5, 0.515], [(None, None), ("card", "ANGER")]) == 0
    assert R.pick_with_terms("card_reward", big, [0.0, 0.03], [(None, None), ("card", "ANGER")], se=0.02) == 0


def test_plan_terms():
    st = _St(BASE)
    assert R.term_ctx(st, "card", "DEMON_FORM")["core_owned"] >= 2
    assert not R.take_relic(st, "TUNGSTEN_ROD") and R.take_relic(st, "VAJRA")
