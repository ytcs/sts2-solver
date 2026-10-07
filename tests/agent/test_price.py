"""`price` and the run model's shop policy on a deterministic fake predictor (no network): shop bundles within the budget, the base policy spending
gold, potion swaps on a full belt, single-option screens."""
import random
import re

import numpy as np

from support import MAP_A2, FakeBridge, deck, fight_starts, make_harness, ok, screen

from agent import price as PR
from agent import runmodel as RM

DECK = [{"id": "STRIKE_IRONCLAD", "upgrade": 0}] * 5 + [{"id": "DEFEND_IRONCLAD", "upgrade": 0}] * 4 + [{"id": "BASH", "upgrade": 0}]


class Pred:
    """P(win) from the scenario's content: `bonus` per card id, `pair` when every card of it is in the deck, minus a little per card; wins end at 70% HP."""

    def __init__(self, bonus=None, pair=(), pair_bonus=0.0):
        self.bonus, self.pair, self.pair_bonus, self.calls = bonus or {}, set(pair), pair_bonus, 0

    def fight_start(self, scenarios, shuffles=4):
        import heads as H
        self.calls += 1
        P = np.zeros((len(scenarios), H.NC))
        for k, sc in enumerate(scenarios):
            ids = [c["id"] for c in sc["deck"]] + [r["id"] for r in sc["relics"]]
            p = 0.5 - 0.01 * len(sc["deck"]) + sum(self.bonus.get(i, 0.0) for i in ids) + (self.pair_bonus if self.pair and self.pair <= set(ids) else 0.0)
            p = min(max(p, 0.0), 1.0)
            P[k, 0] = 1 - p
            P[k, min(H.NC - 1, max(1, int(np.ceil(0.7 * sc["hp"] / H.BIN))))] = p
        return P


def toy(gold=150, potions=(), slots=2):
    return RM.RunState({"character": "IRONCLAD", "ascension": 10}, 0, "Overgrowth", 60, 80, gold, DECK, ["BURNING_BLOOD"], list(potions), slots,
                       (0.4, 0.0, {}, 0), bosses=["THE_KIN_BOSS"])


def drive(gen, pred):
    """Runs a run-model generator against the predictor: its return value."""
    try:
        sc = next(gen)
        while True:
            sc = gen.send(pred.fight_start(sc))
    except StopIteration as e:
        return e.value


def test_base_policy_shop_spends_gold():
    items = [("card", "ANGER", 50), ("card", "INFLAME", 75), ("relic", "ANCHOR", 200), ("remove", None, 75)]
    pol, pred = RM.BasePolicy(), Pred({"INFLAME": 0.2, "ANGER": 0.05})
    buys = drive(pol.shop(toy(gold=160), items, random.Random(1)), pred)
    assert [(k, i if k != "remove" else i["id"], p) for k, i, p in buys] == [("remove", "STRIKE_IRONCLAD", 75), ("card", "INFLAME", 75)]
    buys = drive(pol.shop(toy(gold=130), items, random.Random(1)), pred)  # 55 left after the removal: only Anger fits
    assert [k for k, _, _ in buys] == ["remove", "card"] and buys[1][1] == "ANGER"
    buys = drive(pol.shop(toy(gold=100), items, random.Random(1)), pred)  # 25 left: nothing else fits, and the predictor is not asked
    assert [k for k, _, _ in buys] == ["remove"]
    pred = Pred({"ANCHOR": 0.3})
    buys = drive(pol.shop(toy(gold=300), items, random.Random(1)), pred)  # the relic that raises the worth most
    assert buys[1][:2] == ("relic", "ANCHOR")
    buys = drive(pol.shop(toy(gold=300), items, random.Random(1)), Pred())  # no purchase raises the worth: the gold is kept
    assert [k for k, _, _ in buys] == ["remove"]


def test_rollout_shops_through_the_generator():
    """A whole rollout from Act 2 (template acts) with a predictor that always wins: the shops buy inside the batched rollout."""
    st = toy(gold=400)
    st.act, st.act_name, st.bosses = 1, "Hive", []
    won = RM.Rollouts(Pred({"BURNING_BLOOD": 0.9})).run([st], [5])
    assert won[0] == 1 and st.removals >= 1  # a removal at each shop while basics remain


def test_shop_bundles_find_a_pair_no_single_shows():
    s = screen("shop")  # G108: Headbutt 52 + Havoc 48 = 100 fits, a relic does not
    st = toy(gold=108, potions=["FIRE_POTION", "BLOCK_POTION"])  # a full belt: no potion is offered
    items = PR.shop_items(st, s)
    kinds = {k for _, k, _, _ in items}
    assert "potion" not in kinds and "relic" not in kinds and {"card", "remove"} <= kinds
    rich, unpriced = s.replace("(can't afford)", "") + "15 90g relic Philosopher's Stone: Gain 1 energy.\n16 90g relic Made Up Relic: x.\n", []
    names = [n for n, _, _, _ in PR.shop_items(toy(gold=999), rich, unpriced)]
    assert "Philosopher's Stone" in names and "Gambling Chip" in names and unpriced == ["Made Up Relic"]
    pred = Pred(pair=("HEADBUTT", "HAVOC"), pair_bonus=0.3)  # each alone costs a little (one more card), the pair wins more
    opts, note = PR.bundles(st, items, pred)
    labels = [lb for lb, _ in opts]
    assert labels[0] == "nothing" and labels[1] == "Headbutt + Havoc (100g)", labels
    assert 2 <= len(opts) <= 9 and "affordable bundles" in note and pred.calls <= 3  # singles, pairs, triples: one predictor call each
    for lb in labels[1:]:
        assert int(re.search(r"\((\d+)g\)$", lb).group(1)) <= 108 and lb.count("remove") <= 1
    s2 = st.copy()
    opts[1][1](s2, None)
    assert s2.gold == 8 and [c["id"] for c in s2.deck][-2:] == ["HEADBUTT", "HAVOC"]
    st3 = toy(gold=108, potions=["FIRE_POTION"])  # a free slot: potions within it, at most one per bundle
    opts, _ = PR.bundles(st3, PR.shop_items(st3, s), Pred({"BLOCK_POTION": 0.2, "VULNERABLE_POTION": 0.2}))
    assert all(sum(p in lb for p in ("Block Potion", "Vulnerable Potion", "Ashwater")) <= 1 for lb, _ in opts)
    # a pair whose parts are each slightly negative is found behind purchases that are neutral alone (potions, relics)
    st4 = toy(gold=230, potions=["FIRE_POTION"])
    opts, _ = PR.bundles(st4, PR.shop_items(st4, screen("shop_a2")), Pred(pair=("SPITE", "BURNING_PACT"), pair_bonus=0.2))
    assert opts[1][0].startswith("Spite + Burning Pact"), [lb for lb, _ in opts]


def test_rewards_potion_swap_on_a_full_belt():
    rw = "REWARDS\nA1 F7 IRONCLAD A10 HP 60/80 G99 pots[Fire Potion, Block Potion]\n0 12 Gold\n" \
         "1 potion Strength Potion: Gain 2 Strength. (potion slots full: a dp <slot> first)\n2 proceed (skip the rest)\n"
    st = toy(potions=["FIRE_POTION", "BLOCK_POTION"])
    opts = PR.options(st, rw)
    assert [lb for lb, _ in opts] == ["leave Strength Potion", "Strength Potion for FIRE_POTION (a dp 0)", "Strength Potion for BLOCK_POTION (a dp 1)"]
    s = st.copy()
    opts[2][1](s, None)
    assert s.potions == ["FIRE_POTION", "STRENGTH_POTION"] and st.potions == ["FIRE_POTION", "BLOCK_POTION"]
    assert PR.options(toy(potions=["FIRE_POTION"]), rw) == []  # a free slot: taking it costs nothing, no decision


def test_price_single_option_and_shop_bundles(monkeypatch, tmp_path):
    """`price` on a screen with one option prints one line (no predictor, no rollouts); on a shop it prices bundles end to end (paired rollouts
    of the run model, whose shop policy asks the predictor through the generator protocol)."""
    broke = "\n".join(l + " (can't afford)" if re.match(r"^\d+ \d+g ", l) else l for l in screen("shop_a2").split("\n"))
    fake = FakeBridge(broke, deck_json=deck(), map_text=MAP_A2)
    h = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18))
    out = ok(h.handle("price"))
    assert out == "price: one option on this screen (nothing): nothing to compare\n" and getattr(h, "_predictor", None) is None
    fake.screen = screen("shop_a2")
    h._predictor = Pred({"SETUP_STRIKE": 0.1, "FISTICUFFS": 0.1}, pair=("SPITE", "BURNING_PACT"), pair_bonus=0.2)
    out = ok(h.handle("price 4"))
    rows = [l for l in out.split("\n")[1:] if l and not l.startswith("(")]
    assert any(l.startswith("nothing ") for l in rows) and any(" + " in l for l in rows), out
    assert "4 rollouts per option" in out and "affordable bundles" in out
