"""The live combat loop on a recorded fight (Fabricator, 34 actions in): `turn` / `combat`, the potion gate and its answers, `adv`, slot translation.
The engine's decisions are scripted (FakeEngine.decide); the simulator is the real one, aligned by the Replayer."""
import json

import support
from support import MAP_A2, FakeBridge, FakeEngine, deck, events, fight_starts, fixture_json, golden, make_harness, ok

COMBAT = """COMBAT
A3 F38 IRONCLAD A10 HP 41/72 G150 pots[Strength Potion, -]
T6 E3/3 draw5 disc8 exh1
you b0
e0 Guardbot 18/18 b0 -> block
play: a <i> [e<target>]
0 Bolas(0) Deal 3 damage. ->e
6 potion Strength Potion: Gain 2 Strength.
7 end turn
"""
REWARDS = "REWARDS\nA3 F38 IRONCLAD A10 HP 41/72 G165 pots[Strength Potion, -]\n0 15 Gold\n1 proceed (skip the rest)\n"


def live_fight():
    d = fixture_json("fabricator_34.json")
    return dict(d, state=d["states"][34])


def decision(sim, chosen, qs):
    """A decide() result choosing the legal action whose text is `chosen`, with the given q per action text (others unsearched)."""
    legal = dict((t, a) for a, t in sim.legal())
    opts = [dict(action=a, text=t, p=0.1, q=qs.get(t)) for a, t in sim.legal()]
    a = legal[chosen]
    return dict(action=a, json=sim.action_json(a), text=chosen, searched=True, rounds=4, seconds=0.1, options=opts)


def setup(monkeypatch, tmp_path, script, fight=None, screen_text=COMBAT):
    """script(call_no, sim, keep_potions) -> (chosen text, {text: q})."""
    n = [0]

    def decide(scenario, sim, budget, keep_potions):
        n[0] += 1
        chosen, qs = script(n[0], sim, keep_potions)
        return decision(sim, chosen, qs)
    eng = FakeEngine(decide=decide)
    f = fight or live_fight()
    fake = FakeBridge(screen_text, deck_json=deck())
    fake.fight = lambda: f if fake.screen.startswith("COMBAT") else None
    fake.on_action = lambda line: REWARDS if '"end_turn"' in line and fake.end_after_turn else screen_text
    fake.end_after_turn = False
    h = make_harness(monkeypatch, tmp_path, fake, engine=eng)
    return h, fake, eng


def test_turn_plays_until_end_turn(monkeypatch, tmp_path):
    def script(i, sim, kp):
        if i == 1:
            return "play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.5, "end turn": 0.2, "play SQUASH #2 -> e0": 0.3}
        return "end turn", {"end turn": 0.4, "play BOLAS #0 -> e1": 0.1}
    h, fake, eng = setup(monkeypatch, tmp_path, script)
    out = ok(h.handle("turn !"))
    assert out == "  play BOLAS #0 -> e1 q0.5   [alt: play SQUASH #2 -> e0 q0.3; end turn q0.2]\n  end turn q0.4   [alt: play BOLAS #0 -> e1 q0.1; play BOLAS #0 -> e0 qNone]\n" + COMBAT
    assert fake.actions() == ['do {"play":{"hand_pos":0,"target":1}}', 'do {"end_turn":true}']
    assert [c["keep_potions"] for c in eng.decide_calls] == [True, True]  # potions are my call: the search plans without them
    ev = events(h)
    st = [e for e in ev if e["kind"] == "fight_start"][-1]
    assert st["encounter"] == "FABRICATOR_NORMAL" and st["keep_potions"] is True and st["util"] is None
    assert st["util_why"] == "linear (the adopted networks were not trained with the HP-worth input)"
    assert [e["text"] for e in ev if e["kind"] == "action"] == ["play BOLAS #0 -> e1", "end turn"]
    assert h.fight_actions == 2
    if h.drive[0] == "manual":
        assert h.handle("turn") == "REFUSED: this fight is MANUAL.\n" + h._drive_line()


def _alerting(monkeypatch):
    """potion_price.now_vs_hold stubbed: throwing the Strength Potion now adds 8 win points and 9 HP over the rest of the fight."""
    from agent import potion_price
    row = dict(i=0, id="STRENGTH_POTION", action="potion 0", win_now=0.7, win_hold=0.62, hp_now=30.0, hp_hold=21.0, d_win=0.08, d_hp=9.0, se_win=0.02, se_hp=2.0,
               attempts=32, alert=True)
    calls = []
    monkeypatch.setattr(potion_price, "now_vs_hold", lambda eng, sc, sim, skip=(), **kw: calls.append(sorted(skip)) or ([] if 0 in skip else [dict(row)]))
    return calls


def test_potion_alert_stops_once_per_turn(monkeypatch, tmp_path):
    """Potions are my call: the search plans without them, the turn's check alerts (and stops `turn` / `combat`) when throwing one now saves HP or win;
    a second `turn` on the same turn plays on without a new alert."""
    calls = _alerting(monkeypatch)
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("end turn", {"end turn": 0.3}))
    out = ok(h.handle("combat !"))
    golden("potion_gate.txt", out)
    assert out.startswith("POTION ALERT (turn 7): throwing now saves HP or win over the rest of this fight\n  STRENGTH_POTION: throw NOW (potion 0)")
    assert "\n  HP 41/72 now; belt: Strength Potion, -\n" in out
    assert fake.actions() == [] and calls == [[]]
    out = ok(h.handle("turn !"))
    assert fake.actions() == ['do {"end_turn":true}'] and calls == [[]]  # no second check this turn
    assert [c["keep_potions"] for c in eng.decide_calls] == [True]
    assert [e["kind"] for e in events(h) if e["kind"].startswith("potion")] == ["potion_check", "potion_price"]


def test_potion_allow(monkeypatch, tmp_path):
    """`potion allow <name>` lets the search use that potion for the rest of the fight; it is no longer checked (the search times it)."""
    calls = _alerting(monkeypatch)

    def script(i, sim, kp):
        return ("potion 0", {"potion 0": 0.6, "end turn": 0.1}) if kp != True and i == 1 else ("end turn", {"end turn": 0.3})  # noqa: E712
    h, fake, eng = setup(monkeypatch, tmp_path, script)
    h.sync()
    assert h.handle("potion allow strength potion") == "the search may use this fight: ['STRENGTH_POTION']\n"
    out = ok(h.handle("turn !"))
    assert fake.actions() == ['do {"use_potion": {"slot": 0}}', 'do {"end_turn":true}'] and h.potions_used == 1
    assert calls == [] or calls == [[0]]
    assert h.handle("potion deny all") == "the search may use this fight: no potion\n"


def test_advice(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.5, "end turn": 0.2}))
    out = ok(h.handle("adv 2"))
    assert out.startswith(COMBAT.rstrip("\n") + "\n")
    assert "\ne1 plan: +1 FABRICATE summon [summons 2] (50%) | FABRICATING_STRIKE 22 + summon [summons 1] (50%)" in out
    assert "advice: play BOLAS #0 -> e1 q0.5   [alt: end turn q0.2; play BOLAS #0 -> e0 qNone]   (4 rounds, 0.1s)\noutlook (expected damage, next 3 turns): " in out
    assert eng.decide_calls[-1]["budget"] == 2.0 and eng.decide_calls[-1]["tol_hp"] == 0.0
    assert fake.actions() == []
    adv = [e for e in events(h) if e["kind"] == "advice"][-1]
    assert adv["text"] == "play BOLAS #0 -> e1" and adv["fight"] == h.fight_id


def test_potions_command(monkeypatch, tmp_path):
    """`potions`: the turn's check on demand (no alert needed), with the real paired play-outs (FakeEngine.play_on) and every potion priced."""
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("end turn", {"end turn": 0.3}))
    out = ok(h.handle("potions"))
    golden("potions_cmd.txt", out)
    assert out.startswith("POTION ALERT (turn 7): asked\n  STRENGTH_POTION: throw NOW (potion 0) then none vs never this fight")
    assert [e for e in eng.log if "play_on" in e][0]["play_on"] == 32


def test_potion_slot_translation(monkeypatch, tmp_path):
    """Bug fix: the simulator's packed potion index is sent to the game as the game's slot (a lone potion in the second slot is slot 1)."""
    f = live_fight()
    f["scenario"] = dict(f["scenario"], potions=[dict(id="STRENGTH_POTION", slot=1)])
    _alerting(monkeypatch)

    def script(i, sim, kp):
        return ("potion 0", {"potion 0": 0.6}) if kp != True and i == 1 else ("end turn", {"end turn": 0.3})  # noqa: E712
    h, fake, eng = setup(monkeypatch, tmp_path, script, fight=f, screen_text=COMBAT.replace("pots[Strength Potion, -]", "pots[-, Strength Potion]"))
    h.sync()
    h.handle("potion allow all")
    ok(h.handle("turn !"))
    assert fake.actions()[0] == 'do {"use_potion": {"slot": 1}}'  # the game's slot
    assert h._potion_name("potion 0 -> e1") == "Strength Potion"


def test_fight_util_inputs(monkeypatch, tmp_path):
    """The HP-worth curve of a fight (networks trained for it): the route DP gets the priced deck (held potions out), the map, the narrowing context and
    the act's pool variant; a boss and a missing run are linear."""
    from agent import routes

    class UtilEngine(FakeEngine):
        util_trained = True
    got = []
    monkeypatch.setattr(routes, "continuation_util", lambda eng, deck, map_text, ctx, act, *a, **kw: got.append((deck, map_text, ctx, act)) or (None, "recorded"))
    fake = FakeBridge(support.screen("shop_a2"), deck_json=dict(deck(), potions=[dict(id="POWER_POTION", slot=0), dict(id="FIRE_POTION", slot=1)]), map_text=MAP_A2)
    h = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18), engine=UtilEngine())
    h.handle("hold FIRE_POTION")
    assert h._fight_util(dict(encounter="MYTES_NORMAL")) == (None, "recorded")
    d, m, ctx, act = got[-1]
    assert [p["id"] for p in d["potions"]] == ["POWER_POTION"] and d["max_hp"] == deck()["max_hp"]
    assert m == MAP_A2 and act == "Hive"
    assert ctx == dict(seen=["TUNNELER_WEAK", "EXOSKELETONS_WEAK", "OVICOPTER_NORMAL", "MYTES_NORMAL", "LOUSE_PROGENITOR_NORMAL"], bosses=["KNOWLEDGE_DEMON_BOSS"])
    assert h._fight_util(dict(encounter="KNOWLEDGE_DEMON_BOSS")) == (None, "linear (a boss: no route after it)")
    fake.deck = None
    assert h._fight_util(dict(encounter="MYTES_NORMAL")) == (None, "linear (no run)")
    assert len(got) == 1


def test_engine_held_potions():
    """`Engine.decide` with held potions: they leave the searched copy (`without_potions`, by the simulator's packed index) and are never chosen."""
    try:
        from agent.engine import Engine
    except Exception as e:  # noqa: BLE001  torch / rl not importable here
        support.skip(f"agent.engine not importable: {e}")

    class Sim:
        def __init__(self, dropped=()):
            self.dropped = list(dropped)

        def legal(self):
            return [(0, "end turn"), (5, "potion 0"), (6, "potion 1 -> e0"), (7, "discard potion 1"), (9, "play STRIKE #0 -> e0")]

        def without_potions(self, drop):
            return Sim(drop)

        def action_json(self, a):
            return json.dumps({"a": a})

        def snapshot(self):
            return json.dumps(dict(potions=[dict(id="FIRE_POTION"), dict(id="BLOCK_POTION")]))

    class FS:
        def __init__(self):
            self.seen = []

        def set_util(self, u):
            pass

        def decide(self, scenario, sim, seed):
            self.seen.append(sim.dropped)
            opts = [0, 5, 6, 7, 9]
            return dict(searched=True, opts=opts, q=[0.1, 0.9, 0.8, 0.95, 0.2], legal=[True] * 5, p=[0.2] * 5, action=5)
    eng = object.__new__(Engine)
    eng.fs, eng.seed = FS(), 0
    sc = dict(max_hp=80, potions=[dict(id="FIRE_POTION", slot=0), dict(id="BLOCK_POTION", slot=2)])
    d = eng.decide(sc, Sim(), budget=0.0, keep_potions={"BLOCK_POTION"})
    assert eng.fs.seen[0] == [1]  # BLOCK_POTION is the simulator's potion 1 (game slot 2)
    assert d["text"] == "potion 0"  # the discard (0.95) and the held potion 1 (0.8) are never chosen
    assert [o["q"] for o in d["options"] if o["text"] in ("potion 1 -> e0", "discard potion 1")] == [None, None]
    eng.fs.seen.clear()
    d = eng.decide(sc, Sim(), budget=0.0, keep_potions=True)
    assert eng.fs.seen[0] == [0, 1] and d["text"] == "play STRIKE #0 -> e0"
    eng.fs.seen.clear()
    d = eng.decide(sc, Sim(), budget=0.0, keep_potions=set())
    assert eng.fs.seen[0] == [] and d["text"] == "potion 0"


def test_game_json_slots(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("end turn", {}))
    h.sync()
    h.rp.scenario = dict(h.rp.scenario, potions=[dict(id="A", slot=1)])
    assert json.loads(h._game_json('{"use_potion":{"slot":0}}')) == {"use_potion": {"slot": 1}}
    h.rp.scenario = dict(h.rp.scenario, potions=[dict(id="A", slot=0), dict(id="B", slot=2)])
    assert json.loads(h._game_json('{"use_potion":{"slot":1,"target":0}}')) == {"use_potion": {"slot": 2, "target": 0}}
    assert json.loads(h._game_json('{"use_potion":{"slot":4}}')) == {"use_potion": {"slot": 4}}
    assert h._game_json('{"end_turn":true}') == '{"end_turn":true}'


def test_potion_price_units(monkeypatch, tmp_path):
    """agent.potion_price: both arms on the same futures and seeds, a loss worth 0, the ending weighed by the route DP's V with the potion spent / kept;
    exactly one copy of the potion allowed in the 'only it' arm."""
    import numpy as np
    from agent import potion_price
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("end turn", {"end turn": 0.3}))
    h.sync()
    sim, sc = h.rp.sim, h.rp.scenario
    mx = sc["max_hp"]
    calls = []

    def act_values(spend):
        calls.append(spend)
        return [np.linspace(0, 0.5, mx + 1), np.linspace(0, 0.6, mx + 1)], "win"
    p = potion_price.price(eng, sc, sim, 0, deck(), act_values=act_values, attempts=16, seed=3)
    assert calls == [(("STRENGTH_POTION",), ())]
    plays = [e for e in eng.log if "play_on" in e]
    assert [e["potions"] for e in plays] == [[1], [0]]  # only it, then none
    assert p["kind"] == "act" and 0 <= p["hold"] <= 0.6 and 0 <= p["throw"] <= 0.5
    assert abs(p["diff"] - (p["throw"] - p["hold"])) < 1e-12
    golden("potion_price.txt", "\n".join(potion_price.text(p)) + "\n")
    q = potion_price.price(eng, sc, sim, 0, deck(), next_boss=lambda drop: 0.4 if drop else 0.5, attempts=16, seed=3)
    assert q["kind"] == "next act" and abs(q["hold"] - q["win_hold"] * 0.5) < 1e-12 and abs(q["throw"] - q["win_throw"] * 0.4) < 1e-12
    r = potion_price.price(eng, sc, sim, 0, deck(), next_boss=lambda drop: 0.01, attempts=16, seed=3)  # the next boss out of reach: this fight's win alone
    assert r["kind"] == "this fight" and abs(r["diff"] - (r["win_throw"] - r["win_hold"])) < 1e-12
