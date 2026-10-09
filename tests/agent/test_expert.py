import importlib.util
import json
import os
import shutil

from support import ROOT

from agent import reenact as RE

FIX = os.path.join(ROOT, "tests", "agent", "fixtures", "expert")
REC = os.path.join(FIX, "hMrQSndDvPc.compact.jsonl")
TARGETED = {"STRIKE_SILENT", "NEUTRALIZE", "DASH", "SNAKEBITE", "SHIV", "STRANGLE"}
HEAD = "A1 F9 SILENT A10 HP 35/70 G79 pots[Dexterity Potion, Cunning Potion]"
NACTS = 37


def _expert():
    spec = importlib.util.spec_from_file_location("expert_tool", os.path.join(ROOT, "tools", "expert.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def title(c):
    t = RE.base(c["id"]).replace("_", " ").title().replace("Ascenders", "Ascender's")
    return t + ("+" if c.get("upgrade") else "")


def combat_screen(state, potions=()):
    lines = ["COMBAT", HEAD, f"T1 E{state.get('energy', 3)}/3 draw0 disc0 exh0", "you b0"]
    for i, e in enumerate(state.get("enemies", [])):
        if e.get("alive", True):
            lines.append(f"e{i} {e['id'].title()} {e['hp']}/{e['max_hp']} b0 -> atk 5")
    lines.append("play: a <i> [e<target>]")
    opts = [f"{title(c)}(1) text" + (" ->e" if c["id"] in TARGETED else "") for c in state.get("hand", [])]
    opts += [f"potion {p}: text" for p in potions]
    opts.append("end turn")
    return "\n".join(lines + [f"{i} {o}" for i, o in enumerate(opts)]) + "\n"


def select_screen(hand, n):
    return "\n".join([f"SELECT {n}", HEAD, "Choose a card to discard.", "answer: a <i> [<j> ...]"] + [f"{i} {title(c)}(1) text" for i, c in enumerate(hand)]) + "\n"


def _play(card, up=0, ench=False, pos=None, target=None, expect=None):
    return dict(kind="play", card=card, upgrade=up, enchanted=ench, pos=pos, target=target, expect=expect)


def test_play_targets_and_enchantment():
    hand = [{"id": "DEFEND_SILENT", "upgrade": 0, "enchantment": {"id": "SPIRAL", "amount": 1}}, {"id": "DEFEND_SILENT", "upgrade": 0},
            {"id": "STRIKE_SILENT", "upgrade": 0}, {"id": "DAGGER_SPRAY", "upgrade": 1, "enchantment": {"id": "GLAM", "amount": 1}}]
    st = dict(hand=hand, enemies=[dict(id="TERROR_EEL", hp=150, max_hp=150, index=0)])
    scr = combat_screen(st)
    assert RE.combat_command(_play("DEFEND_SILENT", ench=True), st, scr) == ("a 0", None)
    assert RE.combat_command(_play("DEFEND_SILENT"), st, scr) == ("a 1", None)
    assert RE.combat_command(_play("DAGGER_SPRAY", up=1), st, scr) == ("a 3", None)
    assert RE.combat_command(_play("STRIKE_SILENT", target=0), st, scr) == ("a 2 e0", None)
    assert RE.combat_command(_play("STRIKE_SILENT"), st, scr) == ("a 2", None)
    cmd, err = RE.combat_command(_play("NEUTRALIZE", target=0), st, scr)
    assert cmd is None and "no such card" in err
    cmd, err = RE.combat_command(_play("STRIKE_SILENT", pos=1, target=0), st, scr)
    assert cmd is None


def test_unplayable_and_target_remap():
    hand = [{"id": "CLUMSY", "upgrade": 0}, {"id": "STRIKE_SILENT", "upgrade": 0}]
    st = dict(hand=hand, enemies=[dict(id="GREMLIN_MERC", hp=0, max_hp=53, alive=False, index=0), dict(id="SNEAKY_GREMLIN", hp=12, max_hp=12, index=1),
                                  dict(id="FAT_GREMLIN", hp=18, max_hp=18, index=2)])
    scr = combat_screen(st).replace("0 Clumsy(1)", "0 (x) Clumsy(1)")
    cmd, err = RE.combat_command(_play("CLUMSY"), st, scr)
    assert cmd is None and "unplayable" in err
    assert RE.combat_command(_play("STRIKE_SILENT", target=1, expect="FAT_GREMLIN"), st, scr) == ("a 1 e2", None)
    cmd, err = RE.combat_command(_play("STRIKE_SILENT"), st, scr)
    assert cmd is None and "needs a target" in err


def test_potion_end_and_choose():
    st = dict(hand=[{"id": "STRIKE_SILENT", "upgrade": 0}], enemies=[dict(id="TERROR_EEL", hp=150, max_hp=150, index=0)])
    scr = combat_screen(st, potions=["Dexterity Potion", "Cunning Potion"])
    pot = dict(kind="potion", slot=1, potion="CUNNING_POTION", target=None)
    assert RE.combat_command(pot, st, scr) == ("a 2", None)
    assert RE.to_bridge(pot, "a 2") == {"use_potion": {"slot": 1}}
    end = dict(kind="end")
    assert RE.combat_command(end, st, scr) == ("a 3", None)
    assert RE.to_bridge(end, "a 3") == {"end_turn": True}
    hand = [{"id": "STRIKE_SILENT", "upgrade": 0}, {"id": "DEFEND_SILENT", "upgrade": 0}, {"id": "STRIKE_SILENT", "upgrade": 0}]
    ch = dict(kind="choose", cards=[["DEFEND_SILENT", 0], ["STRIKE_SILENT", 0]])
    assert RE.combat_command(ch, st, select_screen(hand, 2)) == ("a 1 0", None)
    assert RE.to_bridge(ch, "a 1 0") == {"choose": [1, 0]}
    cmd, err = RE.combat_command(ch, st, select_screen(hand, 1))
    assert cmd is None and "SELECT 1" in err


NEOW = """EVENT
A1 F1 SILENT A10 HP 56/70 G99 pots[-, -]
Neow:
0 Lead Paperweight: Choose 1 of 2 Colorless cards to add to your Deck.
1 Booming Conch: At the start of Elite combats, draw 2 additional cards.
2 Silken Tress: Lose all Gold. Your first card reward is Glam.
"""
MAP = """MAP
A1 F1 SILENT A10 HP 56/70 G0 pots[-, -]
full map: m
0 Monster r1c1 -> Mc0,Mc2
1 Unknown r1c3 -> $c3
2 Monster r1c5 -> Mc6
"""
REWARD = """CARD_REWARD
A1 F2 SILENT A10 HP 56/70 G9 pots[-, -]
0 Dagger Spray(1) Deal 4 damage to ALL enemies twice.
1 Calculated Gamble(0) Discard your Hand, then draw that many cards.
2 Prepared(0) Draw 1 card. Discard 1 card.
3 Skip
"""
REST = """RESTSITE
A1 F8 SILENT A10 HP 35/70 G20 pots[-, -]
0 Rest: Heal for 30% of your Max HP (21).
1 Smith: Upgrade a card in your Deck.
"""


def test_macro_screens():
    step = dict(screen="EVENT", seen=["Lead Paperweight", "Booming Conch", "Silken Tress"], pick="Silken Tress")
    assert RE.macro_command(step, NEOW) == ("a 2", None)
    cmd, err = RE.macro_command(dict(step, seen=["Arcane Scroll"]), NEOW)
    assert cmd is None and "diverged" in err
    assert RE.macro_command(dict(screen="MAP", pick="r1c5"), MAP) == ("a 2", None)
    assert RE.macro_command(dict(screen="MAP", pick={"room": "Unknown"}), MAP) == ("a 1", None)
    cmd, err = RE.macro_command(dict(screen="MAP", pick={"room": "Monster"}), MAP)
    assert cmd is None and "matches 2" in err
    assert RE.macro_command(dict(screen="CARD_REWARD", pick="Dagger Spray"), REWARD) == ("a 0", None)
    assert RE.macro_command(dict(screen="CARD_REWARD", pick=None), REWARD) == ("a 3", None)
    cmd, err = RE.macro_command(dict(screen="CARD_REWARD", pick="Dagger Spray+"), REWARD)
    assert cmd is None
    assert RE.macro_command(dict(screen="RESTSITE", pick="Smith"), REST) == ("a 1", None)
    cmd, err = RE.macro_command(dict(screen="REWARDS", pick="gold"), REST)
    assert cmd is None and "expects REWARDS" in err
    smith = "SELECT 1\nA1 F8 SILENT A10 HP 35/70 G20 pots[-, -]\nChoose a card to upgrade.\n0 Strike(1) x\n1 Dagger Spray(1) x\n"
    assert RE.macro_command(dict(screen="SELECT", pick=["Dagger Spray"]), smith) == ("a 1", None)


def test_map_paths():
    text = "rows bottom->top\nr1: Mc0>0 ?c2>2\nr2: Ec0> Mc2>\nboss: 3 LAGAVULIN_MATRIARCH_BOSS\n"
    paths, boss = RE.map_paths(text, {2: "Monster", 3: "Monster"})
    assert paths == [[(1, 2), (2, 2)]] and boss == (3, ["LAGAVULIN_MATRIARCH_BOSS"])
    nodes, _ = RE.parse_map(text)
    assert RE.consistent_from(nodes, {2: "Monster", 3: "Elite"}, 1, 0) and not RE.consistent_from(nodes, {2: "Monster", 3: "Elite"}, 1, 2)


def _fixture_dir(tmp_path):
    shutil.copytree(os.path.join(FIX, "replay"), tmp_path / "replay")
    return str(tmp_path)


def _write(rec, path):
    path.write_text("".join(json.dumps(x) + "\n" for x in [{k: v for k, v in rec.items() if k != "steps"}] + rec["steps"]), encoding="utf-8")
    return str(path)


def test_pilot_record_translates_to_bridge_actions(monkeypatch):
    monkeypatch.setattr(RE, "creator_dir", lambda rec: FIX)
    rec = RE.load(REC)
    assert RE.build_ok(rec)
    flat = RE.flatten(rec)
    n = 0
    for fid in sorted({a["fight"] for a in flat if a.get("fight")}):
        built = RE.built_record(rec, fid)
        states, log = built["fight"]["states"], built["fight"]["log"]
        pots = [p["id"].replace("_", " ").title() for p in built["scenario"].get("potions", [])]
        acts = [a for a in flat if a.get("fight") == fid]
        assert len(acts) == len(log), fid
        for a, want in zip(acts, log):
            st = states[a["i"]]
            if a["kind"] == "choose":
                scr = select_screen(st["hand"], len(a["cards"]))
            else:
                scr = combat_screen(st, pots)
            cmd, err = RE.combat_command(a, st, scr)
            assert err is None, (fid, a["i"], err)
            got = RE.to_bridge(a, cmd)
            if "play" in want:
                j, k = got["play"]["hand_pos"], want["play"]["hand_pos"]
                assert (st["hand"][j]["id"], st["hand"][j]["upgrade"]) == (st["hand"][k]["id"], st["hand"][k]["upgrade"]), (fid, a["i"])
                assert got["play"].get("target") == want["play"].get("target"), (fid, a["i"])
            elif "choose" in want:
                assert sorted(st["hand"][x]["id"] for x in got["choose"]) == sorted(st["hand"][x]["id"] for x in want["choose"]), (fid, a["i"])
            elif "use_potion" in want:
                assert got["use_potion"]["slot"] == want["use_potion"]["slot"] and got["use_potion"].get("target") == want["use_potion"].get("target")
            else:
                assert got == want
            n += 1
    assert n == NACTS


def test_record_validates():
    ex = _expert()
    rec = RE.load(REC)
    errs, warns = ex.check_record(rec)
    assert errs == [] and not any("gap" in w for w in warns), (errs, warns)
    rec["steps"].insert(3, {"floor": 2, "screen": "COMBAT", "gap": "test gap"})
    errs, warns = ex.check_record(rec)
    assert errs == [] and any("gap" in w and "step 3" in w for w in warns), warns
    bad = RE.load(REC)
    bad["seed"] = "YMY1KELG18SO"
    bad["steps"][1]["pick"] = "somewhere"
    errs, _ = ex.check_record(bad)
    assert any("alphabet" in e for e in errs) and any("map pick" in e for e in errs)


MENU = "MENU\n0 new run  (a <i> <character> [ascension] [seed]; characters: ironclad silent defect regent necrobinder)\n1 custom run  (a <i> ...)\n"
MAP_TEXT = "rows bottom->top; point = <type>c<col>><child cols>; * = visited\nr1: Mc1> ?c3> Ec5>\nboss: 16 LAGAVULIN_MATRIARCH_BOSS\n"
FIGHT = dict(id=1, scenario=dict(encounter="SLUDGE_SPINNER_WEAK"), log=[], state=dict(
    player=dict(hp=56), enemies=[dict(id="SLUDGE_SPINNER", hp=41, max_hp=41, index=0, intents=[dict(type="Attack", damage=9, hits=1), dict(type="Debuff")])],
    hand=[{"id": c, "upgrade": 0} for c in ("STRIKE_SILENT", "DEFEND_SILENT", "DEFEND_SILENT", "STRIKE_SILENT", "STRIKE_SILENT", "DEFEND_SILENT", "DEFEND_SILENT")]))
COMBAT_SCREEN = "COMBAT\nA1 F2 SILENT A10 HP 56/70 G0 pots[-, -]\nT1 E3/3 draw10 disc0 exh0\nyou b0\ne0 Sludge Spinner 40/40 b0 -> atk 5\nplay: a <i> [e<target>]\n0 end turn\n"


class FakeLog:
    run_id = "test"

    def event(self, *a, **kw):
        pass


class FakeHarness:
    def __init__(self):
        self.gate, self.log, self.sent = False, FakeLog(), []
        self.screen = MENU

    def _send(self, cmd):
        self.sent.append(cmd)
        if self.screen is MENU:
            self.screen = NEOW.replace("SILENT A10 HP 56/70 G99", "SILENT A10 HP 56/70 G99")
        elif self.screen.startswith("EVENT"):
            self.screen = MAP.replace("2 Monster r1c5 -> Mc6", "2 Elite r1c5 -> Mc6")
        elif self.screen.startswith("MAP"):
            self.screen = COMBAT_SCREEN
        return self.screen

    def sync(self):
        return FIGHT if self.screen.startswith("COMBAT") else None

    def _new_run(self):
        pass

    def _skill_refusal(self, s):
        return None


def _unread_map(tmp_path):
    rec = RE.load(REC)
    assert rec["steps"][1]["screen"] == "MAP" and rec["steps"][2]["fight"]["encounter"] == "SLUDGE_SPINNER_WEAK"
    rec["steps"][1]["pick"] = {}
    return rec, _write(rec, tmp_path / "unread_map.jsonl")


def test_seedcheck_flow_then_replay_refuses_guessed_run(monkeypatch, tmp_path):
    h = FakeHarness()
    monkeypatch.setattr(RE, "creator_dir", lambda rec: str(tmp_path))
    monkeypatch.setattr(RE, "call", lambda cmd: MAP_TEXT if cmd == "m" else ("null" if cmd in ("deck.json", "fight") else h.screen))
    _rec, path = _unread_map(tmp_path)
    out = RE.Reenactor(h, path).run(seedcheck=True)
    assert h.sent[:2] == ["a 0 silent 10 YMY1KELG18SC", "a 2"], h.sent
    assert h.sent[2] in ("a 0", "a 1"), h.sent
    assert "boss LAGAVULIN_MATRIARCH_BOSS (record LAGAVULIN_MATRIARCH_BOSS: ok)" in out
    assert "opening of hMrQSndDvPc_F02_SLUDGE_SPINNER_WEAK (floor 2): matches the record" in out, out
    out = RE.Reenactor(h, path).run()
    assert out.startswith("REFUSED: seedcheck walked this run with a guessed map node")
    rows = [json.loads(x) for x in open(tmp_path / "replay" / "hMrQSndDvPc.jsonl", encoding="utf-8")]
    assert [r["event"] for r in rows if r["event"] in ("start", "guess", "opening")] == ["start", "guess", "opening"]


def test_replay_stops_on_ambiguous_map_and_at_gaps(monkeypatch, tmp_path):
    h = FakeHarness()
    monkeypatch.setattr(RE, "creator_dir", lambda rec: str(tmp_path))
    monkeypatch.setattr(RE, "call", lambda cmd: MAP_TEXT if cmd == "m" else ("null" if cmd in ("deck.json", "fight") else h.screen))
    rec, path = _unread_map(tmp_path)
    out = RE.Reenactor(h, path).run()
    assert "STOP at step 1" in out and "Monster r1c1" in out and "Unknown r1c3" in out and "read the map frame" in out
    assert h.sent == ["a 0 silent 10 YMY1KELG18SC", "a 2"]
    rec["steps"][1]["pick"] = "r1c3"
    rec["steps"][2] = {"floor": 2, "screen": "COMBAT", "encounter": "SLUDGE_SPINNER_WEAK", "gap": "test gap"}
    out = RE.Reenactor(h, _write(rec, tmp_path / "rec.jsonl")).run()
    assert h.sent[-1] == "a 1" and "STOP at step 2, floor 2: record gap" in out


class FightGame(FakeHarness):
    def __init__(self, built, acts, screen_of):
        super().__init__()
        self.built, self.acts, self.screen_of, self.i, self.ended = built, acts, screen_of, None, False

    def _view(self):
        st = self.built["fight"]["states"][self.i]
        return self.screen_of(self.acts[self.i], st, self.built["scenario"]).replace("COMBAT\n", "COMBAT\nA1 F2 SILENT A10 HP 56/70 G0 pots[-, -]\n", 1)

    def _send(self, cmd):
        self.sent.append(cmd)
        self.i = 0 if self.i is None else self.i + 1
        self.screen = self._view() if self.i < len(self.acts) else "REWARDS\nA1 F2 SILENT A10 HP 56/70 G0 pots[-, -]\n0 proceed (skip the rest)\n"
        return self.screen

    def sync(self):
        if self.i is None or self.i >= len(self.acts):
            return None
        self._last_f = dict(id=7, scenario=self.built["scenario"], log=self.built["fight"]["log"][: self.i], state=self.built["fight"]["states"][self.i])
        return self._last_f

    def _fight_end(self, reply):
        self.ended = True


def test_replay_drives_a_recorded_fight(monkeypatch, tmp_path):
    monkeypatch.setattr(RE, "creator_dir", lambda r: _fixture_dir(tmp_path) if not (tmp_path / "replay").exists() else str(tmp_path))
    rec = RE.load(REC)
    k = next(i for i, st in enumerate(rec["steps"]) if st.get("fight", {}).get("id", "").endswith("F02_SLUDGE_SPINNER_WEAK"))
    rec["steps"] = rec["steps"][k:k + 1]
    p = _write(rec, tmp_path / "rec.jsonl")
    built = RE.built_record(rec, rec["steps"][0]["fight"]["id"])
    acts = [a for a in RE.flatten(rec) if a.get("fight")]
    h = FightGame(built, acts, _expert().recorded_screen)
    monkeypatch.setattr(RE, "call", lambda cmd: "null" if cmd in ("deck.json", "fight", "m") else h.screen)
    out = RE.Reenactor(h, p).run()
    assert "opening of hMrQSndDvPc_F02_SLUDGE_SPINNER_WEAK (floor 2): matches the record" in out, out
    assert "record replayed to its end" in out and h.ended, out
    assert len(h.sent) == 1 + len(acts)
    assert [RE.to_bridge(a, c)["play"]["hand_pos"] for a, c in zip(acts, h.sent[1:]) if a["kind"] == "play"][:3] == [1, 1, 0]
    saved = RE.load(str(tmp_path / "replay" / "hMrQSndDvPc" / (acts[0]["fight"] + ".json")))
    assert saved["source"] == "replay" and saved["scenario"]["run_seed"] == "YMY1KELG18SC"


def test_replay_refuses_other_builds(monkeypatch, tmp_path):
    rec = RE.load(REC)
    rec["build"] = "v0.112.0 (2026.09.01)"
    monkeypatch.setattr(RE, "creator_dir", lambda rec: str(tmp_path))
    assert RE.Reenactor(FakeHarness(), _write(rec, tmp_path / "rec.jsonl")).run().startswith("REFUSED: record build")


def test_verdict_rule_reproduces_pilot_calls():
    v = _expert().verdict
    m = 70
    assert v({"r3": dict(d=-0.0110, se=0.0053, n=24), "r2": dict(d=-0.0132, se=0.0232), "r1": dict(d=-0.0979, se=0.0027)}, m)[:2] == ("expert error", "r3")
    assert v({"r2": dict(d=-0.0170, se=0.0036), "r1": dict(d=0.0030, se=0.0009)}, m)[0] == "unresolved"
    assert v({"exact": dict(his_lb=1.0643, his_ub=1.0643, live_lb=1.0643, live_ub=1.0643), "r1": dict(d=-1.0321, se=0.2665)}, m)[:2] == ("tie", "exact")
    assert v({"exact": dict(his_lb=1.0643, his_ub=None, live_lb=-1.0, live_ub=-1.0), "r1": dict(d=0.0, se=0.0)}, m)[:2] == ("our gap", "exact")
    assert v({"r3": dict(d=0.0620, se=0.0048, n=16), "r2": dict(d=0.0387, se=0.0181), "r1": dict(d=-0.0183, se=0.0064)}, m)[:2] == ("our gap", "r3")
    assert v({"r3": dict(d=0.185, se=0.178, n=12), "r2": dict(d=0.1607, se=0.0506), "r1": dict(d=-0.0058, se=0.0040)}, m)[:2] == ("our gap", "r2")
    assert v({"r2": dict(d=-0.0035, se=0.0013), "r1": dict(d=-0.0082, se=0.0008)}, m)[:2] == ("tie", "r2")
    assert v({"r2": dict(d=0.0018, se=0.0003), "r1": dict(d=0.0482, se=0.0115)}, m)[:2] == ("tie", "r2")
    assert v({"r2": dict(d=0.0, se=0.0)}, m)[0] == "tie"
    assert v({"r1": dict(d=0.0066, se=0.0181)}, m)[0] == "unresolved"
    win_now = dict(his_lb=1.3714, his_ub=1.3714, live_lb=1.3714, live_ub=None)
    assert v({"exact": win_now, "r2": dict(d=-0.0170, se=0.0011), "cap": 1.3714}, m)[:2] == ("tie", "exact")
    assert v({"exact": dict(win_now, live_lb=None), "r2": dict(d=-0.0170, se=0.0011), "cap": 1.3714}, m)[:2] == ("unresolved", "exact")
