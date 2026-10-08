import json
import random

import support
from support import FakeBridge, FakeEngine, deck, events, golden, make_harness, ok
from test_live import COMBAT, REWARDS, decision, live_fight

from agent import proposal


class ArmEngine(FakeEngine):
    def __init__(self, arms, decide=None):
        super().__init__(decide=decide)
        self.arms = arms
        self.seen = []

    def outcome(self, key, sd):
        n, st, turn = key
        arm = "keep" if n == 1 else ("now" if st >= 3 else "save")
        self.seen.append((arm, turn, sd))
        return self.arms[arm](random.Random(sd))


def win(fr):
    return lambda rng: (1, fr + 0.1 * rng.random())


def risky(p, fr):
    return lambda rng: (1, fr + 0.1 * rng.random()) if rng.random() < p else (-1, 0.0)


NOW_WINS = dict(now=win(0.7), keep=win(0.5), save=win(0.4))
KEEP_WINS = dict(now=win(0.5), keep=win(0.7), save=win(0.4))
TIE = dict(now=win(0.5), keep=win(0.5), save=win(0.5))
STAKE = dict(now=risky(0.95, 0.4), keep=risky(0.95, 0.42), save=risky(0.5, 0.4))


def setup(monkeypatch, tmp_path, arms, fight=None, screen_text=COMBAT):
    eng = ArmEngine(arms, decide=lambda sc, sim, budget, kp: decision(sim, "end turn", {"end turn": 0.3}))
    f = fight or live_fight()
    fake = FakeBridge(screen_text, deck_json=deck())
    fake.fight = lambda: f if fake.screen.startswith("COMBAT") else None
    fake.on_action = lambda line: REWARDS if '"end_turn"' in line and fake.end_after_turn else screen_text
    fake.end_after_turn = False
    h = make_harness(monkeypatch, tmp_path, fake, engine=eng)
    return h, fake, eng


def proposals(h):
    return [e for e in events(h) if e["kind"] == "potion_proposal"]


def test_fight_objective():
    sc = dict(max_hp=80, ascension=10)
    lin = proposal.fight_objective(dict(sc, encounter="NIBBITS_WEAK", act=0))
    assert lin[0] is None and lin[1].startswith("linear (win +1 + 0.5 x end HP / max HP, loss -1)")
    assert proposal.fight_objective(dict(sc, encounter="BYRDONIS_ELITE", act=0))[0] is None
    for act, boss in ((0, "THE_KIN_BOSS"), (1, "KNOWLEDGE_DEMON_BOSS")):
        w, why = proposal.fight_objective(dict(sc, encounter=boss, act=act))
        assert w["kind"] == "win only" and why.startswith(f"win only (act {act + 1} boss, the ancient's heal follows")
    w, why = proposal.fight_objective(dict(sc, encounter="QUEEN_BOSS", act=2), bosses=["QUEEN_BOSS", "AEONGLASS_BOSS"])
    assert w is None and why == "linear (the first of the two final bosses: HP carries to the second)"
    w, why = proposal.fight_objective(dict(sc, encounter="AEONGLASS_BOSS", act=2), bosses=["QUEEN_BOSS", "AEONGLASS_BOSS"])
    assert w["kind"] == "win only" and why.startswith("win only (the run's final boss")
    assert proposal.fight_objective(dict(sc, encounter="QUEEN_BOSS", act=2))[0] is None
    assert proposal.fight_objective(dict(sc, encounter="QUEEN_BOSS", act=2), seen=["KNIGHTS_ELITE", "AEONGLASS_BOSS"])[0]["kind"] == "win only"
    assert proposal.fight_objective(dict(sc, encounter="QUEEN_BOSS", act=2, ascension=5))[0]["kind"] == "win only"
    assert proposal.fight_objective(dict(encounter="QUEEN_BOSS", max_hp=80))[0] is None


def test_win_only_worth():
    u = proposal.win_only_worth(80)["u"]
    assert len(u) == proposal.HEAD_NC == 76 and u[0] == 0.0
    assert all(b > a for a, b in zip(u[1:41], u[2:41])) and all(abs(x - 1.01) < 1e-12 for x in u[41:])
    assert 1.0 < u[1] < 1.0 + 1e-3
    w = proposal.win_only_worth(80)
    assert proposal.score((1, 0.5, 40), 80, w) == 1.0 and proposal.score((-1, 0.0, 0), 80, w) == 0.0
    assert proposal.score((1, 0.5, 40), 80, dict(u=u)) == u[20]
    assert proposal.score((1, 0.5, 40), 80, None) == 1.25 and proposal.score((-1, 0.0), 80, None) == -1.0
    try:
        import sys
        sys.path.insert(0, support.ROOT + "/rl")
        import heads
    except Exception as e:  # noqa: BLE001
        support.skip(f"rl/heads not importable: {e}")
    assert (heads.BIN, heads.NC) == (proposal.HEAD_BIN, proposal.HEAD_NC)
    assert [int(heads.end_class(True, hp)) for hp in (1, 2, 3, 40, 200)] == [proposal.end_class(True, hp) for hp in (1, 2, 3, 40, 200)]


def test_arms_are_paired(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, NOW_WINS)
    h.sync()
    rows = proposal.price(eng, h.rp.scenario, h.rp.sim, attempts=8, seed=5)
    assert [r["id"] for r in rows] == ["STRENGTH_POTION"] and rows[0]["text"] == "potion 0" and rows[0]["replay_failed"] == 0
    by = {}
    for arm, turn, sd in eng.seen:
        by.setdefault(arm, []).append((turn, sd))
    assert sorted(by) == ["keep", "now", "save"]
    assert [sd for _, sd in by["now"]] == [sd for _, sd in by["save"]] == [sd for _, sd in by["keep"]]
    assert {t for t, _ in by["save"]} == {t for t, _ in by["now"]} == {7} and {t for t, _ in by["keep"]} == {8}
    plays = [e for e in eng.log if "play_on" in e]
    assert [e["play_on"] for e in plays] == [8, 16] and [e["potions"] for e in plays] == [[0], [0]]


def test_proposal_stops_turn_once(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, NOW_WINS)
    out = ok(h.handle("turn !"))
    golden("potion_proposal.txt", out)
    assert "\nPOTION PROPOSAL (turn 7): STRENGTH_POTION now (potion 0): using it now beats keep and save beyond noise.\n" in out
    assert fake.actions() == []
    p = proposals(h)[-1]
    assert p["proposed"] == "STRENGTH_POTION" and p["rows"][0]["verdict"] == "use now" and p["objective"].startswith("linear")
    ok(h.handle("turn !"))
    assert fake.actions() == ['do {"end_turn":true}'] and len(proposals(h)) == 1
    assert [c["keep_potions"] for c in eng.decide_calls] == [True]


def test_no_stop_when_keep_or_save_is_as_good(monkeypatch, tmp_path):
    for arms, verdict in ((KEEP_WINS, "keep (worth using later this fight)"), (TIE, "save (no gain in this fight beyond noise)")):
        mp = support.MonkeyPatch()
        try:
            h, fake, eng = setup(mp, tmp_path / verdict[:4], arms)
            out = ok(h.handle("turn !"))
            assert "POTION PROPOSAL" not in out and fake.actions() == ['do {"end_turn":true}'], verdict
            p = proposals(h)[-1]
            assert p["proposed"] is None and p["rows"][0]["verdict"] == verdict
        finally:
            mp.undo()


def test_win_at_stake_stops(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, STAKE)
    out = ok(h.handle("combat !"))
    assert "POTION PROPOSAL (turn 7): STRENGTH_POTION now (potion 0): the win is at stake" in out and fake.actions() == []
    r = proposals(h)[-1]["rows"][0]
    assert not r["beats"] and r["stake"]


def test_aside_for_the_boss(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, NOW_WINS)
    h.handle("potion aside strength potion")
    out = ok(h.handle("turn !"))
    assert "POTION PROPOSAL" not in out and fake.actions() == ['do {"end_turn":true}']
    assert proposals(h)[-1]["proposed"] is None
    h2, fake2, eng2 = setup(monkeypatch, tmp_path / "boss", NOW_WINS)
    h2.handle("potion aside strength potion")
    h2._is_boss = lambda: True
    assert "POTION PROPOSAL (turn 7): STRENGTH_POTION now" in ok(h2.handle("turn !")) and fake2.actions() == []


def test_commit_one_potion(monkeypatch, tmp_path):
    f = live_fight()
    f["scenario"] = dict(f["scenario"], potions=[dict(id="STRENGTH_POTION", slot=1)])
    h, fake, eng = setup(monkeypatch, tmp_path, NOW_WINS, fight=f, screen_text=COMBAT.replace("pots[Strength Potion, -]", "pots[-, Strength Potion]"))
    ok(h.handle("turn !"))
    assert h.handle("potion use fire potion").startswith("ERR name one usable potion: STRENGTH_POTION")
    out = ok(h.handle("potion use strength potion"))
    assert out == fake.screen and fake.actions() == ['do {"use_potion": {"slot": 1}}']
    assert h.potions_used == 1
    c = [e for e in events(h) if e["kind"] == "potion_commit"][-1]
    assert c["id"] == "STRENGTH_POTION" and c["priced"] and c["verdict"] == "use now"


def test_potions_command_and_advice(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, TIE)
    out = ok(h.handle("potions"))
    assert out.startswith("POTIONS (turn 7; objective: linear") and "no potion proposed: none beats keep and save beyond noise" in out
    assert fake.actions() == []
    h2, fake2, eng2 = setup(monkeypatch, tmp_path / "adv", TIE)
    out = ok(h2.handle("adv 1"))
    assert "\nPOTIONS (turn 7; objective: linear" in out and out.rstrip().endswith("no potion proposed this turn")


def test_boss_objective_reaches_every_search(monkeypatch, tmp_path):
    monkeypatch.setattr(proposal, "fight_objective", lambda sc, bosses=(), seen=(): (proposal.win_only_worth(sc["max_hp"]), "win only (test boss)"))
    h, fake, eng = setup(monkeypatch, tmp_path, NOW_WINS)
    out = ok(h.handle("turn !"))
    assert "POTION PROPOSAL" not in out and fake.actions() == ['do {"end_turn":true}']
    r = proposals(h)[-1]["rows"][0]
    assert all(r["arms"][a]["score"] == 1.0 for a in proposal.ARMS) and not r["beats"] and proposals(h)[-1]["proposed"] is None
    assert proposals(h)[-1]["objective"] == "win only (test boss)"
    st = [e for e in events(h) if e["kind"] == "fight_start"][-1]
    assert st["worth"] == "win only" and st["objective"] == "win only (test boss)"
    assert [e.get("worth") for e in eng.log if "attempts" in e] == ["win only"]
    assert {e["worth"] for e in eng.log if "play_on" in e} == {"win only"}
    assert [c["worth"] for c in eng.decide_calls] == ["win only"]
    h2, fake2, eng2 = setup(monkeypatch, tmp_path / "stake", STAKE)
    assert "POTION PROPOSAL (turn 7): STRENGTH_POTION now (potion 0): the win is at stake" in ok(h2.handle("turn !"))
    assert "objective: win only (test boss)" in ok(h2.handle("potions"))


def test_networks_without_head_stay_linear(monkeypatch, tmp_path):
    h, fake, eng = setup(monkeypatch, tmp_path, TIE)
    eng.worth_ok = False
    w, why = h._fight_objective(dict(encounter="THE_KIN_BOSS", act=0, max_hp=80))
    assert w is None and why.startswith("linear (the adopted networks have no outcome head")


def test_engine_worth_and_potions():
    try:
        from agent.engine import Engine
    except Exception as e:  # noqa: BLE001
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

    class FS:
        def __init__(self):
            self.seen = []

        def decide(self, scenario, sim, seed, worth=None):
            self.seen.append((sim.dropped, worth))
            return dict(searched=True, opts=[0, 5, 6, 7, 9], q=[0.1, 0.9, 0.8, 0.95, 0.2], legal=[True] * 5, p=[0.2] * 5, action=5)
    eng = object.__new__(Engine)
    eng.fs, eng.seed = FS(), 0
    sc = dict(max_hp=80, potions=[dict(id="FIRE_POTION", slot=0), dict(id="BLOCK_POTION", slot=2)])
    w = proposal.win_only_worth(80)
    d = eng.decide(sc, Sim(), budget=0.0, keep_potions=True, worth=w)
    assert eng.fs.seen[0] == ([0, 1], w) and d["text"] == "play STRIKE #0 -> e0"
    eng.fs.seen.clear()
    d = eng.decide(sc, Sim(), budget=0.0, keep_potions={"BLOCK_POTION"})
    assert eng.fs.seen[0] == ([1], None) and d["text"] == "potion 0"


def test_commit_targeted_potion(monkeypatch, tmp_path):
    f = live_fight()
    f["scenario"] = dict(f["scenario"], potions=[dict(id="FIRE_POTION", slot=0)])
    f["state"] = dict(f["state"], potions=[dict(id="FIRE_POTION", slot=0)])
    h, fake, eng = setup(monkeypatch, tmp_path, TIE, fight=f, screen_text=COMBAT.replace("Strength Potion", "Fire Potion"))
    assert h.handle("potion use fire potion").startswith("ERR FIRE_POTION needs a target and no proposal priced one this turn")
    ok(h.handle("potions"))
    r = proposals(h)[-1]["rows"][0]
    assert r["id"] == "FIRE_POTION" and r["text"].startswith("potion 0 -> e")
    ok(h.handle("potion use fire potion"))
    target = int(r["text"].rsplit("e", 1)[1])
    assert [json.loads(x[3:]) for x in fake.actions()] == [{"use_potion": {"slot": 0, "target": target}}]
