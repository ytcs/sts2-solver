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
    legal = dict((t, a) for a, t in sim.legal())
    opts = [dict(action=a, text=t, p=0.1, q=qs.get(t)) for a, t in sim.legal()]
    a = legal[chosen]
    return dict(action=a, json=sim.action_json(a), text=chosen, searched=True, rounds=4, seconds=0.1, options=opts)


def setup(monkeypatch, tmp_path, script, fight=None, screen_text=COMBAT):
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
    assert [c["keep_potions"] for c in eng.decide_calls] == [True, True]
    assert [c["worth"] for c in eng.decide_calls] == ["linear", "linear"]
    ev = events(h)
    st = [e for e in ev if e["kind"] == "fight_start"][-1]
    assert st["encounter"] == "FABRICATOR_NORMAL" and st["worth"] == "linear" and st["predicted"]["potions"] == "none"
    assert st["objective"].startswith("linear (win +1 + 0.5 x end HP / max HP, loss -1)")
    assert [e["text"] for e in ev if e["kind"] == "action"] == ["play BOLAS #0 -> e1", "end turn"]
    assert h.fight_actions == 2
    if h.drive[0] == "manual":
        assert h.handle("turn") == "REFUSED: this fight is MANUAL.\n" + h._drive_line()


def test_advice(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.5, "end turn": 0.2}))
    out = ok(h.handle("adv 2"))
    assert out.startswith(COMBAT.rstrip("\n") + "\n")
    assert "\ne1 plan: +1 DISINTEGRATE 14  +2 DISINTEGRATE 14" in out
    assert "advice: play BOLAS #0 -> e1 q0.5   [alt: end turn q0.2; play BOLAS #0 -> e0 qNone]   (4 rounds, 0.1s)\noutlook (expected damage, next 4 turns): " in out
    assert eng.decide_calls[-1]["budget"] == 2.0 and eng.decide_calls[-1]["tol_hp"] == 0.0
    assert fake.actions() == []
    adv = [e for e in events(h) if e["kind"] == "advice"][-1]
    assert adv["text"] == "play BOLAS #0 -> e1" and adv["fight"] == h.fight_id


def test_game_json_slots(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, lambda i, sim, kp: ("end turn", {}))
    h.sync()
    h.rp.scenario = dict(h.rp.scenario, potions=[dict(id="A", slot=1)])
    assert json.loads(h._game_json('{"use_potion":{"slot":0}}')) == {"use_potion": {"slot": 1}}
    h.rp.scenario = dict(h.rp.scenario, potions=[dict(id="A", slot=0), dict(id="B", slot=2)])
    assert json.loads(h._game_json('{"use_potion":{"slot":1,"target":0}}')) == {"use_potion": {"slot": 2, "target": 0}}
    assert json.loads(h._game_json('{"use_potion":{"slot":4}}')) == {"use_potion": {"slot": 4}}
    assert h._game_json('{"end_turn":true}') == '{"end_turn":true}'


def test_choice_resync_random_offer():
    import types
    from agent.live import Live

    class Sim:
        def __init__(self, cands):
            self.cands, self.calls = cands, []

        def legal(self):
            return [(i, f"pick {i} ({c})") for i, c in enumerate(self.cands)] + [(9, "skip")]

        def sync_choice(self, real, opts):
            self.calls.append(opts)
            self.cands = [c for c, _ in opts]
            return True

    def live(cands, hand):
        lv = Live.__new__(Live)
        lv.rp = types.SimpleNamespace(sim=Sim(cands))
        lv._last_f = {"state": {"hand": hand}}
        lv.log = types.SimpleNamespace(event=lambda *a, **k: None)
        return lv
    sel = "SELECT 1\nA1 F5 IRONCLAD A10 HP 50/80 G99 pots[-, -]\nChoose a card.\n0 Inflame(1) Gain 2 Strength.\n1 Demon Form+(3) At the start ...\n2 Barricade(3) Block stays.\n"
    lv = live(["CORRUPTION", "JUGGERNAUT", "RUPTURE"], [{"id": "STRIKE_IRONCLAD", "upgrade": 0}])
    assert lv._choice_mismatch(sel) is None
    assert lv.rp.sim.calls == [[("INFLAME", 0), ("DEMON_FORM", 1), ("BARRICADE", 0)]]
    hand = "SELECT 1\nA1 F5 IRONCLAD A10 HP 50/80 G99 pots[-, -]\nDiscard a card.\n0 Strike(1) Deal 6 damage.\n1 Defend+(1) Gain 8 Block.\n"
    lv = live(["BASH", "ANGER"], [{"id": "DEFEND_IRONCLAD", "upgrade": 1}, {"id": "STRIKE_IRONCLAD", "upgrade": 0}])
    assert lv._choice_mismatch(hand) is None
    assert lv.rp.sim.calls == [[("STRIKE_IRONCLAD", 0), ("DEFEND_IRONCLAD", 1)]]
