"""The live combat loop on a recorded fight (Fabricator, 34 actions in): `turn` / `combat`, the potion gate and its answers, `adv`, slot translation.
The engine's decisions are scripted (FakeEngine.decide); the simulator is the real one, aligned by the Replayer."""
import json

from support import FakeBridge, FakeEngine, deck, events, fixture_json, golden, make_harness, ok

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
    assert [c["keep_potions"] for c in eng.decide_calls] == [set(), set()]
    ev = events(h)
    st = [e for e in ev if e["kind"] == "fight_start"][-1]
    assert st["encounter"] == "FABRICATOR_NORMAL" and st["keep_potions"] == [] and st["util"] is None
    assert st["util_why"] == "linear (the adopted networks were not trained with the HP-worth input)"
    assert [e["text"] for e in ev if e["kind"] == "action"] == ["play BOLAS #0 -> e1", "end turn"]
    assert h.fight_actions == 2
    if h.drive[0] == "manual":
        assert h.handle("turn") == "REFUSED: this fight is MANUAL.\n" + h._drive_line()


def test_potion_gate_stops_then_ok(monkeypatch, tmp_path):
    def script(i, sim, kp):
        if kp is True:  # the gate's no-potion search
            return "play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.45, "end turn": 0.1}
        if i <= 3:
            return "potion 0", {"potion 0": 0.6, "play BOLAS #0 -> e1": 0.5, "end turn": 0.1}
        return "end turn", {"end turn": 0.3}
    h, fake, eng = setup(monkeypatch, tmp_path, script)
    out = ok(h.handle("combat !"))
    golden("potion_gate.txt", out)
    assert out.startswith("POTION (your call): the solver wants `potion 0` (Strength Potion) now.\n")
    assert fake.actions() == []
    fake.end_after_turn = True
    out = ok(h.handle("combat ok !"))
    assert fake.actions() == ['do {"use_potion": {"slot": 0}}', 'do {"end_turn":true}']
    assert h.potions_used == 1
    assert out.split("\n")[0] == "  potion 0 q0.6   [alt: play BOLAS #0 -> e1 q0.5; end turn q0.1]"
    assert "-- combat over" in out and out.endswith(REWARDS)
    ends = [e for e in events(h) if e["kind"] == "fight_end"]
    assert len(ends) == 1 and ends[0]["hp"] == [41, 72] and ends[0]["potions_used"] == 1 and ends[0]["screen"] == "REWARDS"


def test_potion_gate_skip_and_go(monkeypatch, tmp_path):
    def script(i, sim, kp):
        if i == 1:
            return "potion 0", {"potion 0": 0.6, "play BOLAS #0 -> e1": 0.5, "end turn": 0.1}
        return "end turn", {"end turn": 0.3}
    h, fake, eng = setup(monkeypatch, tmp_path, script)
    out = ok(h.handle("turn skip !"))
    assert fake.actions() == ['do {"play":{"hand_pos":0,"target":1}}', 'do {"end_turn":true}']
    assert out.split("\n")[0] == "  play BOLAS #0 -> e1 q0.5   [alt: potion 0 q0.6; end turn q0.1]"
    assert h.potions_used == 0

    def script2(i, sim, kp):
        return ("potion 0", {"potion 0": 0.6, "end turn": 0.1}) if i == 1 else ("end turn", {"end turn": 0.3})
    h, fake, eng = setup(monkeypatch, tmp_path, script2)
    ok(h.handle("turn go !"))
    ok(h.handle("turn !"))  # the decline lasts the fight
    assert h._decline_fight == h.fight_id and h._kp() is True
    assert [c["keep_potions"] for c in eng.decide_calls] == [True, True]
    assert fake.actions() == ['do {"end_turn":true}', 'do {"end_turn":true}']


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
    def script(i, sim, kp):
        return ("play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.45}) if kp is True else ("potion 0", {"potion 0": 0.6})
    h, fake, eng = setup(monkeypatch, tmp_path, script)
    out = ok(h.handle("potions"))
    assert out == ("belt: Strength Potion, -\nbest line with no potion: play BOLAS #0 -> e1 (value 0.45); with potions allowed: potion 0 (Strength Potion, value 0.60; "
                   "+0.1 is about +5% win or +16 HP)\n")


def test_potion_name_by_game_slot(monkeypatch, tmp_path):
    """Bug fix: the gate named the potion by indexing the belt (game slots) with the simulator's packed index: a lone potion in the second slot printed `-`."""
    f = live_fight()
    f["scenario"] = dict(f["scenario"], potions=[dict(id="STRENGTH_POTION", slot=1)])
    def script(i, sim, kp):
        if kp is True:
            return "play BOLAS #0 -> e1", {"play BOLAS #0 -> e1": 0.45}
        return ("potion 0", {"potion 0": 0.6, "play BOLAS #0 -> e1": 0.5}) if i <= 3 else ("end turn", {"end turn": 0.3})
    h, fake, eng = setup(monkeypatch, tmp_path, script, fight=f, screen_text=COMBAT.replace("pots[Strength Potion, -]", "pots[-, Strength Potion]"))
    out = ok(h.handle("turn !"))
    assert out.startswith("POTION (your call): the solver wants `potion 0` (Strength Potion) now.\n"), out
    assert "potions in the belt: -, Strength Potion" in out
    ok(h.handle("turn ok !"))
    assert fake.actions()[0] == 'do {"use_potion": {"slot": 1}}'  # the game's slot
    assert h._potion_name("potion 0 -> e1") == "Strength Potion"


def test_engine_held_potions():
    """`Engine.decide` with held potions: they leave the searched copy (`without_potions`, by the simulator's packed index) and are never chosen."""
    try:
        from agent.engine import Engine
    except Exception as e:  # noqa: BLE001  torch / rl not importable here
        raise type("Skip", (Exception,), {})(f"agent.engine not importable: {e}")

    class Sim:
        def __init__(self, dropped=()):
            self.dropped = list(dropped)

        def legal(self):
            return [(0, "end turn"), (5, "potion 0"), (6, "potion 1 -> e0"), (7, "discard potion 1"), (9, "play STRIKE #0 -> e0")]

        def without_potions(self, drop):
            return Sim(drop)

        def action_json(self, a):
            return json.dumps({"a": a})

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
