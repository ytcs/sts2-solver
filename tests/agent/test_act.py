"""`a` chains, the map guard, the decision guards and the pick guard, the skill gate in `_handle`, PRICING bookkeeping, hold / held potions."""
import json
import os

from support import FakeBridge, bare, deck, events, make_harness, ok, screen

from agent import harness, skillgate


def low_hp(text, hp="40/80"):
    import re
    return re.sub(r"HP \d+/\d+", f"HP {hp}", text, count=1)


def no_header(text):
    lines = text.split("\n")
    return "\n".join([lines[0]] + lines[2:])


# ---------------------------------------------------------------------------------------------------------------- chains

def test_single_step_logs_macro(monkeypatch, tmp_path):
    fake = FakeBridge(screen("rewards"), on_action=[screen("card_reward")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = ok(h.handle("a ~card -- look at the cards"))
    assert bare(out) == screen("card_reward")
    assert fake.actions() == ["a 1"]
    e = [x for x in events(h) if x["kind"] == "macro"][-1]
    assert e["screen"] == "REWARDS" and e["choice"] == "1" and e["why"] == "look at the cards" and e["result"] == "CARD_REWARD"
    assert e["state"] == screen("rewards")[:1500]
    assert h.last_state == screen("card_reward")


def test_chain_runs_steps_in_order(monkeypatch, tmp_path):
    after_gold = screen("rewards").replace("0 9 Gold\n1 card", "0 card").replace("2 proceed", "1 proceed")
    fake = FakeBridge(screen("rewards"), on_action=[after_gold, screen("map_single")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = ok(h.handle("a ~gold; ~proceed -- why"))
    assert fake.actions() == ["a 0", "a 1"]
    assert bare(out) == screen("map_single")
    assert [e["choice"] for e in events(h) if e["kind"] == "macro"] == ["0", "1"]


def test_map_click_must_be_last(monkeypatch, tmp_path):
    fake = FakeBridge(screen("map_a1"), on_action=[])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 1; ~gold")
    assert out == "REFUSED: a map choice must be the last step of a chain.\n" + screen("map_a1")
    assert fake.actions() == []


def test_bare_number_after_a_step_refused(monkeypatch, tmp_path):
    fake = FakeBridge(screen("rewards"), on_action=[screen("rewards")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 0; 1")
    assert bare(out) == screen("rewards") + "REFUSED: `1` is an option number after an earlier step of the same chain: the list shifted when that step ran. Name the option (`~text`) or send it as its own call after reading the screen.\n"
    assert fake.actions() == ["a 0"]


def test_chain_stops_at_combat_select_menu(monkeypatch, tmp_path):
    fake = FakeBridge(screen("map_single"), on_action=[screen("combat")])
    fake.fight = None
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 0")
    assert bare(out) == screen("combat")  # no fight export: no drive line, no combat info
    fake = FakeBridge(screen("rewards"), on_action=[screen("combat")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 0; ~strike")
    assert bare(out) == screen("combat") + "[chain stopped before `~strike`: COMBAT]\n"
    fake = FakeBridge(screen("restsite"), on_action=[screen("select")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 1; 0")
    assert bare(out) == screen("select") + "[chain stopped before `0`: SELECT]\n"
    fake = FakeBridge(screen("restsite"), on_action=[screen("select"), screen("restsite")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = ok(h.handle("a 1; ~pommel"))  # a selection step named by text goes through
    assert fake.actions() == ["a 1", "a 0"]
    fake = FakeBridge(screen("game_over"), on_action=[screen("menu")])
    h = make_harness(monkeypatch, tmp_path, fake)
    out = h.handle("a 0; 0 ironclad 10")
    assert bare(out) == screen("menu") + "[chain stopped before `0 ironclad 10`: MENU]\n"


def test_chain_errors(monkeypatch, tmp_path):
    fake = FakeBridge(screen("rewards"), on_action=[])
    h = make_harness(monkeypatch, tmp_path, fake)
    assert h.handle("a ~nothing") == "ERR no option matching `nothing`\n" + screen("rewards")
    fake = FakeBridge(screen("rewards"), on_action=["ERR no such option\n"])
    h = make_harness(monkeypatch, tmp_path, fake)
    assert h.handle("a 7; ~gold") == "ERR no such option\n"
    assert fake.actions() == ["a 7"]


def test_busy_screen_settles_first(monkeypatch, tmp_path):
    fake = FakeBridge("REWARDS (busy)\n", on_action=[screen("card_reward")])
    seq = ["REWARDS (busy)\n", screen("rewards")]
    fake.extra["peek"] = lambda line: seq.pop(0) if len(seq) > 1 else seq[0]
    fake.extra["s"] = lambda line: screen("rewards")
    h = make_harness(monkeypatch, tmp_path, fake)
    ok(h.handle("a ~card"))
    assert fake.actions() == ["a 1"]
    assert "s" in fake.calls


def test_menu_new_run_resets(monkeypatch, tmp_path):
    fake = FakeBridge(screen("menu"), on_action=[screen("event_neow")])
    h = make_harness(monkeypatch, tmp_path, fake)
    old = h.log.run_id
    h.hold = {"FIRE_POTION"}
    h.priced = {"routes": "A1 F1"}
    h.reward_screen = ("A1 F2", ("X",))
    ok(h.handle("a 0 ironclad 10"))
    assert h.log.run_id != old
    assert h.hold == set() and h.priced == {} and h.reward_screen is None
    fake = FakeBridge(screen("menu"), on_action=[screen("menu")])
    h = make_harness(monkeypatch, tmp_path, fake, run_id="r2")
    ok(h.handle("a 0"))  # a bare `a 0` on the menu (no character) is not a new run
    assert h.log.run_id == "r2"


# ---------------------------------------------------------------------------------------------------------------- the map guard

def test_map_guard(monkeypatch, tmp_path):
    fake = FakeBridge(screen("map_elite"), on_action=[screen("combat")])
    h = make_harness(monkeypatch, tmp_path, fake)
    st = low_hp(screen("map_elite"))
    assert h._map_guard(screen("map_elite"), "1") is None  # 78/80
    assert h._map_guard(st, "1") == "REFUSED: `1 Elite r6c2 -> $c2` at 40/80 HP. Heal first, or confirm with `a 1 !` if this is deliberate (check `route` first).\n" + st
    assert h._map_guard(st, "1 !") is None
    assert h._map_guard(st, "0") is None
    assert h._map_guard(low_hp(screen("map_elite"), "48/80"), "1") is None  # 60% exactly is fine
    assert h._map_guard(low_hp(screen("map_elite"), "47/80"), "1") is not None
    boss = "MAP\nA1 F16 IRONCLAD A10 HP 30/80 G10 pots[-, -]\nfull map: m\n0 Boss r16c3 -> \n"
    assert h._map_guard(boss, "0").startswith("REFUSED: `0 Boss r16c3 -> ` at 30/80 HP.")
    assert h._map_guard(screen("shop"), "1") is None
    assert h._map_guard(st, "~elite") is None  # unresolved text is not a number
    fake.screen = st
    assert h.handle("a 1").startswith("REFUSED: `1 Elite")
    assert fake.actions() == []
    ok(h.handle("a 1 !"))
    assert fake.actions() == ["a 1 !"]


# ---------------------------------------------------------------------------------------------------------------- decision guards

def gated(monkeypatch, tmp_path, fake, **kw):
    h = make_harness(monkeypatch, tmp_path, fake, **kw)
    h.gate = True
    monkeypatch.setattr(skillgate, "gate_message", lambda session, state=None: None)
    monkeypatch.setattr(skillgate, "active", lambda: "test-session")
    return h


ROUTES_REFUSAL = "REFUSED: run `routes` on this floor before a fork choice (sts2-pathing procedure, sts2-harness: routes at every fork).\n"
NEOW_REFUSAL = "REFUSED: run `routes` on this floor before a Neow / ancient choice (sts2-pathing procedure, sts2-harness: routes at every fork).\n"
REST_REFUSAL = ("REFUSED: rest or smith is priced over the rest of the act (sts2-deckbuilding section 6): `routes --hp <HP after the rest>` and `routes` at "
                "the HP now, plus `eval --smooth --boss --next` upgrade variants, on this floor.\n")
WHY_REFUSAL = "REFUSED: the `-- why` records the decision: numbers: <what the calculators said> ; judgment: <what decided it>. Missing: "


def test_decision_guard_map_and_neow(monkeypatch, tmp_path):
    h = gated(monkeypatch, tmp_path, FakeBridge(""))
    m = screen("map_a1")
    assert h._decision_guard(m, "1", None) == ROUTES_REFUSAL + m
    h.priced = {"routes": "A1 F2"}
    assert h._decision_guard(m, "1", None) == ROUTES_REFUSAL + m  # priced on another floor
    h.priced = {"routes": "A1 F1"}
    assert h._decision_guard(m, "1", None) is None
    h.priced = {"route": "A1 F1"}
    assert h._decision_guard(m, "1", None) is None
    h.priced = {"eval": "A1 F1"}
    assert h._decision_guard(m, "1", None) == ROUTES_REFUSAL + m
    assert h._decision_guard(screen("map_single"), "0", None) is None  # no fork
    assert h._decision_guard(m, "~monster", None) is None
    nh = no_header(m)
    h.priced = {"routes": "A1 F1"}
    assert h._decision_guard(nh, "1", None) == ROUTES_REFUSAL + nh  # no header: nothing counts as priced
    n = screen("event_neow")
    h.priced = {}
    assert h._decision_guard(n, "2", None) == NEOW_REFUSAL + n
    h.priced = {"routes": "A1 F1"}
    assert h._decision_guard(n, "2", None) is None
    assert h._decision_guard(screen("event_late"), "0", None) is None
    assert h._decision_guard(screen("rewards"), "0", None) is None
    assert h._decision_guard(screen("treasure"), "0", None) is None


def test_decision_guard_rest(monkeypatch, tmp_path):
    h = gated(monkeypatch, tmp_path, FakeBridge(""))
    r = screen("restsite")
    assert h._decision_guard(r, "0", "numbers: x; judgment: y") == REST_REFUSAL + r
    h.priced = {"routes": "A1 F11"}
    assert h._decision_guard(r, "0", "numbers: x; judgment: y") == REST_REFUSAL + r
    h.priced = {"routes": "A1 F11", "eval": "A1 F11"}
    assert h._decision_guard(r, "0", "numbers: x") == WHY_REFUSAL + "judgment:.\n"
    assert h._decision_guard(r, "0", None) == WHY_REFUSAL + "numbers:, judgment:.\n"
    assert h._decision_guard(r, "0", "Numbers: x; JUDGMENT: y") is None
    proceed = r.rstrip("\n") + "\n2 Proceed\n"
    h.priced = {}
    assert h._decision_guard(proceed, "2", None) is None


def test_decision_guard_shop(monkeypatch, tmp_path):
    h = gated(monkeypatch, tmp_path, FakeBridge(""))
    s = screen("shop")
    want = "REFUSED: price this shop decision first (`eval` variants, `rmcalc`, `routes`; sts2-deckbuilding section 1 / 6), on this floor.\n" + s
    assert h._decision_guard(s, "4", "numbers: a; judgment: b") == want
    assert h._decision_guard(s, "14", None) is None  # leave shop
    for calc in ("eval", "rmcalc", "routes", "pickplan"):
        h.priced = {calc: "A1 F3"}
        assert h._decision_guard(s, "4", "numbers: a; judgment: b") is None, calc
        assert h._decision_guard(s, "4", "judgment: b") == WHY_REFUSAL + "numbers:.\n"
    h.priced = {"reward": "A1 F3", "route": "A1 F3"}
    assert h._decision_guard(s, "4", "numbers: a; judgment: b") == want


def test_pick_guard(monkeypatch, tmp_path):
    h = gated(monkeypatch, tmp_path, FakeBridge(""))
    c = screen("card_reward")
    need = "REFUSED: run `reward` on this card reward first (sts2-deckbuilding section 1, step 2), then pick with the section-3 bar in the `-- why`.\n" + c
    full = "buckets: FD7 open SD; weakest: Vantom; numbers: best +0.1; judgment: plan"
    assert h._decision_guard(c, "2", full) == need
    assert h._pick_guard(c, "2", full) == need
    h.reward_screen = ("A1 F3", ("Armaments", "Headbutt", "Perfected Strike"))
    assert h._pick_guard(c, "2", full) == need  # a table of another floor
    h.reward_screen = ("A1 F2", ("Armaments", "Headbutt", "Bash"))
    assert h._pick_guard(c, "2", full) == need  # another screen's cards
    h.reward_screen = ("A1 F2", ("Armaments", "Headbutt", "Perfected Strike"))
    assert h._pick_guard(c, "2", full) is None
    assert h._pick_guard(c, "3", full) is None  # skip needs the same record
    rec = ("REFUSED: the `-- why` of a card pick records each input of the decision (sts2-deckbuilding section 1): buckets: <five-bucket line, which are open>; "
           "weakest: <weakest fight and what it asks>; numbers: <table: best option and gain, the chosen card's section-3 bar status>; "
           "judgment: <plan fit, density, future problems, synergies: why this choice>. Missing: ")
    assert h._pick_guard(c, "2", None) == rec + "buckets:, weakest:, numbers:, judgment:.\n"
    assert h._pick_guard(c, "3", "buckets: x; numbers: y") == rec + "weakest:, judgment:.\n"
    assert h._pick_guard(c, "~armaments", None) is None
    assert h._pick_guard(screen("shop"), "1", None) is None
    assert h._pick_guard("CARD_REWARD\nA1 F2 X\n0 Skip\n", "0", None) is None  # no card on the screen


def test_guards_through_act(monkeypatch, tmp_path):
    fake = FakeBridge(screen("map_a1"), on_action=[screen("combat")])
    h = gated(monkeypatch, tmp_path, fake)
    assert h.handle("a ~r1c3") == ROUTES_REFUSAL + screen("map_a1")
    assert fake.actions() == []
    h.priced["routes"] = "A1 F1"
    ok(h.handle("a ~r1c3 -- numbers: x"))
    assert fake.actions() == ["a 1"]
    h = make_harness(monkeypatch, tmp_path, FakeBridge(screen("map_a1"), on_action=[screen("combat")]), run_id="ungated")
    ok(h.handle("a 1"))  # gate off (bare Harness): the decision guards do not apply


def test_skill_gate_in_handle(monkeypatch, tmp_path):
    fake = FakeBridge(screen("rewards"), on_action=[screen("card_reward"), screen("rewards")])
    h = make_harness(monkeypatch, tmp_path, fake)
    h.gate = True
    monkeypatch.setattr(skillgate, "active", lambda: "sess")
    msgs = {"REWARDS": "skills not loaded in this session: sts2-deckbuilding. Invoke them with the Skill tool and read them, then repeat the command. No game action happens before that."}
    monkeypatch.setattr(skillgate, "gate_message", lambda session, state=None: msgs.get(harness._kind(state)) if state is not None else None)
    assert h.handle("a 0") == "REFUSED: " + msgs["REWARDS"] + "\n"
    assert h.handle("turn") == "REFUSED: " + msgs["REWARDS"] + "\n"
    assert fake.actions() == []
    assert bare(h.handle("s")) == screen("rewards")  # read-only
    fake.screen = screen("treasure")
    msgs["CARD_REWARD"] = "skills not loaded in this session: sts2-x. Invoke them with the Skill tool and read them, then repeat the command. No game action happens before that."
    h.priced["routes"] = "A1 F2"
    out = h.handle("a 0; ~armaments")
    assert bare(out) == screen("card_reward") + "[chain stopped before `~armaments`: " + msgs["CARD_REWARD"] + "]\n"
    fake.screen = screen("shop")
    assert h.handle("do {\"end_turn\":true}") == "REFUSED: `do` sends a raw action past the harness's guards; use `a <i>`, `turn` or `combat`.\n"


# ---------------------------------------------------------------------------------------------------------------- PRICING bookkeeping

def test_pricing_records_floor(monkeypatch, tmp_path):
    fake = FakeBridge(screen("shop_a2"), deck_json=None)
    h = make_harness(monkeypatch, tmp_path, fake)
    calls = []
    for cmd, meth in harness.PRICING.items():
        monkeypatch.setattr(h, meth, lambda rest, cmd=cmd: calls.append((cmd, rest)) or f"{cmd} table\n")
    for cmd in harness.PRICING:
        assert h.handle(cmd + " --attempts 8") == f"{cmd} table\n"
    assert calls == [(c, "--attempts 8") for c in harness.PRICING]
    assert h.priced == {c: "A2 F23" for c in harness.PRICING}
    fake.screen = screen("restsite")
    for i, bad in enumerate(("ERR x", "REFUSED: x", "reward: not a card reward screen", "no run in progress", "need --enc IDS", "routes: no act map",
                             "routes: boss unknown", "eval: x")):
        monkeypatch.setattr(h, "evaluate", lambda rest, bad=bad: bad)
        assert h.handle("eval") == bad
        assert h.priced["eval"] == "A2 F23", bad
    monkeypatch.setattr(h, "routes", lambda rest: "routes: no node left before the boss: the next fight is the boss")
    h.handle("routes")
    assert h.priced["routes"] == "A1 F11"  # that one did price
    fake.screen = no_header(screen("shop"))
    monkeypatch.setattr(h, "rmcalc", lambda rest: "table\n")
    h.handle("rmcalc")
    assert h.priced["rmcalc"] == "A2 F23"  # no header: not priced


def test_pricing_failures_real_methods(monkeypatch, tmp_path):
    fake = FakeBridge(screen("shop"), deck_json=None, map_text="no map\n")
    h = make_harness(monkeypatch, tmp_path, fake)
    assert h.handle("reward") == "reward: not a card reward screen (SHOP); use eval\n"
    assert h.handle("eval --v x") == "need --enc IDS or --pool Act:kind[:n]"
    assert h.handle("eval --enc NIBBITS_WEAK") == "no run in progress"
    assert h.handle("route M E") == "no run in progress"
    assert h.handle("routes") == "no run in progress"
    assert h.handle("rmcalc") == "no run in progress\n"
    assert h.handle("pickplan") == "no run in progress\n"
    assert h.handle("brief") == screen("shop")
    fake.screen = screen("card_reward")
    assert h.handle("reward") == "no run in progress\n"
    assert h.priced == {}
    fake.deck = deck()
    fake.screen = screen("shop")
    assert h.handle("routes") == "routes: no act map"
    assert h.priced == {}


# ---------------------------------------------------------------------------------------------------------------- hold / held potions

def test_hold_and_held_potions(monkeypatch, tmp_path):
    d = deck()
    d["potions"] = [dict(id="POWER_POTION", slot=0), dict(id="FIRE_POTION", slot=1)]
    fake = FakeBridge(screen("shop_a2"), deck_json=d)
    h = make_harness(monkeypatch, tmp_path, fake)
    assert json.loads(h._deck_raw()) == d
    assert h.handle("hold power potion, Fire_Potion") == "held (out of the potion check, the search and every table): ['FIRE_POTION', 'POWER_POTION']\n"
    with open(os.path.join(h.log.dir, "hold.json"), encoding="utf-8") as f:
        assert json.load(f) == ["FIRE_POTION", "POWER_POTION"]
    h.handle("hold POWER_POTION")
    assert [p["id"] for p in json.loads(h._deck_raw())["potions"]] == ["FIRE_POTION"]
    assert {k: v for k, v in json.loads(h._deck_raw()).items() if k != "potions"} == {k: v for k, v in d.items() if k != "potions"}
    assert h._kp() is True  # no fight: nothing to use
    from agent import potions
    belt = ["POWER_POTION", "FIRE_POTION"]
    assert potions.search_keep({"POWER_POTION"}, set(), belt) is True  # default: the search plans without potions
    assert potions.search_keep({"POWER_POTION"}, {"FIRE_POTION", "POWER_POTION"}, belt) == {"POWER_POTION"}  # allowed, but a held one never
    assert potions.search_keep(set(), set(belt), belt) == set()

    class Sim:  # slot 0 already thrown: the simulator's `potion N` keeps the fight's slot numbers
        def legal(self):
            return [(1, "potion 1"), (2, "discard potion 1"), (3, "end turn")]
    sc = dict(potions=[dict(id="WEAK_POTION", slot=0), dict(id="SPEED_POTION", slot=1)])
    assert potions.live_slots(sc, Sim()) == [(1, "SPEED_POTION")]
    assert h.status().endswith("potions held back from the solver: ['POWER_POTION']")
    from agent.harness import Harness
    h2 = Harness()  # a daemon restart keeps the hold
    assert h2.hold == {"POWER_POTION"}
    assert h.handle("hold none") == "held (out of the potion check, the search and every table): nothing held\n"
    assert h.handle("hold") == "held (out of the potion check, the search and every table): nothing held\n"
    assert json.loads(h._deck_raw()) == d
    fake.deck = None
    assert h._deck_raw() == "null"


def test_misc_commands(monkeypatch, tmp_path):
    fake = FakeBridge(screen("shop"))
    h = make_harness(monkeypatch, tmp_path, fake)
    assert h.handle("budget") == "search budget auto (this fight: 1.0s) per combat decision\n"
    assert h.handle("budget 3") == "search budget 3.0s per combat decision\n"
    assert h.handle("budget auto") == "search budget auto (this fight: 1.0s) per combat decision\n"
    assert h.handle("note hello there") == "noted\n"
    assert events(h)[-1]["kind"] == "note" and events(h)[-1]["text"] == "hello there"
    assert h.handle("status") == (f"run {h.log.run_id}  engine loaded  fight None  actions 0\n"
                                  "replay: 0 enemy turns matched, 0 unmatched; divergences: none\npotions held back from the solver: none\n")
    fake.extra["snap"] = "null\n"
    assert h.handle("relics") == "not in combat\n"
    fake.extra["snap"] = json.dumps(dict(relics=[dict(id="PEN_NIB", counter=3), dict(id="ANCHOR"), dict(id="X", props={"a": 1})])) + "\n"
    assert h.handle("relics") == "PEN_NIB counter 3\nANCHOR\nX {'a': 1}\n"
    fake.extra["d"] = "deck: ...\n"
    assert h.handle("d") == "deck: ...\n"
    out = h.handle("newrun")
    assert out == f"run {h.log.run_id}\n"
    fake.extra["fight"] = "null\n"
    fake.extra["peek"] = screen("shop")
    assert h.handle("adv") == screen("shop") + "advice: not in combat\n"
    assert h.handle("potions") == "not in combat\n"
    assert h.handle("turn") .startswith("-- combat over")


def test_run_end_logged_once(monkeypatch, tmp_path):
    fake = FakeBridge(screen("game_over"))
    h = make_harness(monkeypatch, tmp_path, fake)
    h.handle("s")
    h.handle("s")
    ends = [e for e in events(h) if e["kind"] == "run_end"]
    assert len(ends) == 1
    assert ends[0]["screen"] == "GAME_OVER" and ends[0]["header"] == "A3 F48 IRONCLAD A10 HP 0/72 G125 pots[-, -]"
