import random
import re

import numpy as np

from support import MAP_A2, FakeBridge, deck, fight_starts, make_harness, ok, screen

from agent import price as PR
from agent import runmodel as RM

DECK = [{"id": "STRIKE_IRONCLAD", "upgrade": 0}] * 5 + [{"id": "DEFEND_IRONCLAD", "upgrade": 0}] * 4 + [{"id": "BASH", "upgrade": 0}]


class Pred:
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
    buys = drive(pol.shop(toy(gold=130), items, random.Random(1)), pred)
    assert [k for k, _, _ in buys] == ["remove", "card"] and buys[1][1] == "ANGER"
    buys = drive(pol.shop(toy(gold=100), items, random.Random(1)), pred)
    assert [k for k, _, _ in buys] == ["remove"]
    pred = Pred({"ANCHOR": 0.3})
    buys = drive(pol.shop(toy(gold=300), items, random.Random(1)), pred)
    assert buys[1][:2] == ("relic", "ANCHOR")
    buys = drive(pol.shop(toy(gold=300), items, random.Random(1)), Pred())
    assert [k for k, _, _ in buys] == ["remove"]


def test_rollout_shops_through_the_generator():
    st = toy(gold=400)
    st.act, st.act_name, st.bosses = 1, "Hive", []
    won = RM.Rollouts(Pred({"BURNING_BLOOD": 0.9})).run([st], [5])
    assert won[0] == 1 and st.removals >= 1


def test_shop_bundles_find_a_pair_no_single_shows():
    s = screen("shop")
    st = toy(gold=108, potions=["FIRE_POTION", "BLOCK_POTION"])
    items = PR.shop_items(st, s)
    kinds = {k for _, k, _, _ in items}
    assert "potion" not in kinds and "relic" not in kinds and {"card", "remove"} <= kinds
    rich, unpriced = s.replace("(can't afford)", "") + "15 90g relic Philosopher's Stone: Gain 1 energy.\n16 90g relic Made Up Relic: x.\n", []
    names = [n for n, _, _, _ in PR.shop_items(toy(gold=999), rich, unpriced)]
    assert "Philosopher's Stone" in names and "Gambling Chip" in names and unpriced == ["Made Up Relic"]
    pred = Pred(pair=("HEADBUTT", "HAVOC"), pair_bonus=0.3)
    opts, note = PR.bundles(st, items, pred)
    labels = [lb for lb, _ in opts]
    assert labels[0] == "nothing" and labels[1] == "Headbutt + Havoc (100g)", labels
    assert 2 <= len(opts) <= 9 and "affordable bundles" in note and pred.calls <= 3
    for lb in labels[1:]:
        assert int(re.search(r"\((\d+)g\)$", lb).group(1)) <= 108 and lb.count("remove") <= 1
    s2 = st.copy()
    opts[1][1](s2, None)
    assert s2.gold == 8 and [c["id"] for c in s2.deck][-2:] == ["HEADBUTT", "HAVOC"]
    st3 = toy(gold=108, potions=["FIRE_POTION"])
    opts, _ = PR.bundles(st3, PR.shop_items(st3, s), Pred({"BLOCK_POTION": 0.2, "VULNERABLE_POTION": 0.2}))
    assert all(sum(p in lb for p in ("Block Potion", "Vulnerable Potion", "Ashwater")) <= 1 for lb, _ in opts)
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
    assert PR.options(toy(potions=["FIRE_POTION"]), rw) == []


def test_price_single_option_and_shop_bundles(monkeypatch, tmp_path):
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
    assert out.startswith("ranked by: ") and "next act ready" in out


class ActPred:
    def __init__(self, nxt, b=10, boss=0.4, elite=0.8, b2=20, act0=1.0):
        from agent import pools
        self.bosses, self.elites = set(pools.pool(nxt, "boss")), set(pools.pool(nxt, "elite"))
        self.nxt_act = pools.ACTS[nxt]["act"]
        self.b, self.boss, self.elite, self.b2, self.act0, self.asked, self.batches = b, boss, elite, b2, act0, [], []

    def fight_start(self, scenarios, shuffles=4):
        import heads as H
        P = np.zeros((len(scenarios), H.NC))
        self.batches.append(list(scenarios))
        for k, sc in enumerate(scenarios):
            self.asked.append(sc)
            if sc["act"] < self.nxt_act:
                p, b = self.act0, self.b
            else:
                p = self.boss if sc["encounter"] in self.bosses else self.elite if sc["encounter"] in self.elites else 0.0
                b = self.b2
            P[k, 0], P[k, b] = 1 - p, p
        return P


def _res(**cols):
    labels = list(next(iter(cols.values())))
    return {lb: {k: np.asarray(v[lb], float) for k, v in cols.items()} for lb in labels}


def test_ladder_picks_the_longest_estimable_unsaturated_horizon():
    rng = np.random.default_rng(0)
    n = 64
    zero = np.zeros(n)

    def noise(p):
        return (rng.random(n) < p).astype(float)
    act_hi = noise(0.97)
    floors = {"a": np.full(n, 30.0), "b": np.full(n, 30.0)}
    w = _res(win={"a": np.r_[np.ones(20), np.zeros(n - 20)], "b": zero}, act={"a": act_hi, "b": act_hi}, ready={"a": zero, "b": zero}, floors=floors)
    assert PR.ladder(w)[0] == "win"
    act_mid = noise(0.6)
    a = _res(win={"a": zero, "b": zero}, act={"a": act_mid, "b": act_mid}, ready={"a": zero, "b": zero + 0.1}, floors=floors)
    assert PR.ladder(a)[0] == "act" and "not saturated" in PR.ladder(a)[1]
    ones, sat = np.ones(n), np.r_[np.zeros(8), np.ones(n - 8)]
    s = _res(win={"a": zero, "b": zero}, act={"a": ones, "b": sat}, ready={"a": zero + 0.2, "b": zero + 0.5}, floors=floors)
    assert PR.ladder(s)[0] == "act" and "separates" in PR.ladder(s)[1]
    r = _res(win={"a": zero, "b": zero}, act={"a": act_hi, "b": act_hi}, ready={"a": act_hi * 0.3, "b": act_hi * 0.5}, floors=floors)
    assert PR.ladder(r)[0] == "ready"
    out = PR.table(r)
    head, cols, first, second = out.split("\n")[:4]
    assert head.startswith("ranked by: next act ready (") and "saturated" in head
    assert "*next act ready" in cols and "P(win run)" in cols and "P(clear act)" in cols and "floors" in cols
    assert first.startswith("b ") and second.startswith("a ") and "*ready -0.19" in second and "win +0.000" in second and "act +0.000" in second
    assert act_mid.mean() > 0.4 and PR.ladder(a, saturated=0.4)[0] == "ready"
    f = _res(win={"a": zero, "b": zero}, act={"a": act_hi, "b": act_hi}, floors={"a": np.full(n, 31.0), "b": np.full(n, 30.0)})
    assert PR.ladder(f)[0] == "floors" and "not estimable" in PR.ladder(f)[1]
    assert "next act ready" not in PR.table(f) and PR.table(f).split("\n")[2].startswith("a ")
    one = _res(win={"a": [1.0], "b": [0.0]}, act={"a": [1.0], "b": [1.0]}, ready={"a": [0.4], "b": [0.3]}, floors={"a": [3.0], "b": [3.0]})
    assert PR.ladder(one)[0] == "ready"


def test_readiness_is_priced_after_the_ancient_heal():
    import heads as H
    from agent import pools
    pred = ActPred("Hive", b=10, boss=0.4, elite=0.8)
    st = toy(gold=0)
    won = RM.Rollouts(pred).run([st], [3])
    end = int(H.centers()[10 - 1].item())
    healed = end + int(RM.HEAL_ANCIENT * (st.max_hp - end))
    assert healed == 67
    hive = [sc for sc in pred.asked if sc["act"] == 1]
    ready = hive[:len(pools.pool("Hive", "boss")) + len(pools.pool("Hive", "elite"))]
    assert {sc["encounter"] for sc in ready} == set(pools.pool("Hive", "boss")) | set(pools.pool("Hive", "elite"))
    assert {sc["hp"] for sc in ready} == {healed}
    assert won[0] == 0 and st.end[0] == 1 and st.end[1] == "hallway"
    assert abs(st.ready - (0.5 * 0.4 + 0.5 * 0.8)) < 1e-9
    c = H.centers()[20 - 1].item()
    w_boss, w_elite = 0.4 * (1 + 0.5 * c / 80) - 0.6, 0.8 * (1 + 0.5 * c / 80) - 0.2
    assert abs(st.ready_worth - (0.5 * w_boss + 0.5 * w_elite)) < 1e-6
    boss_fight = [b[0] for b in pred.batches if len(b) == 1 and b[0]["act"] == 0 and b[0]["encounter"] == "THE_KIN_BOSS"][-1]
    assert ready[0]["gold"] == boss_fight["gold"] + int(RM.GOLD["boss"][0] * RM.POVERTY)


def test_readiness_glory_double_boss_and_deaths_count_zero():
    import heads as H
    from agent import pools
    st = toy(potions=["FIRE_POTION"])
    st.act, st.act_name, st.hp = 2, "Glory", 50
    pred = ActPred("Glory", boss=0.5, elite=0.9, b2=15)
    rdy, _w = drive(RM.readiness(st, RM.BasePolicy()), pred)
    nb, ne = len(pools.pool("Glory", "boss")), len(pools.pool("Glory", "elite"))
    second = pred.asked[nb + ne:]
    assert len(second) == nb * (nb - 1) and {sc["hp"] for sc in second} == {round(H.centers()[14].item())}
    assert all(not sc["potions"] for sc in second) and all(sc["potions"] for sc in pred.asked[:nb])
    assert abs(rdy - (0.5 * 0.25 + 0.5 * 0.9)) < 1e-9
    opts = [("rest", lambda s, _d: setattr(s, "hp", s.max_hp)), ("nothing", None)]
    res = PR.price(toy(), opts, ActPred("Hive", act0=0.0), n=6)
    assert all((r["ready"] == 0).all() and (r["ready_worth"] == -1).all() and (r["act"] == 0).all() for r in res.values())
    g = toy()
    g.act, g.act_name, g.bosses = 2, "Glory", []
    res = PR.price(g, opts, ActPred("Glory", act0=0.0, boss=0.0, elite=0.0), n=3)
    assert all("ready" not in r for r in res.values()) and "next act ready" not in PR.table(res)
