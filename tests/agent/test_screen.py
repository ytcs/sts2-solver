"""Screen-text parsing: kind, act / floor, HP, gold, belt, option lines, card options, map offers, `~text` resolution."""
from support import MAP_A2, MAP_SCREEN_A2, FakeBridge, make_harness, screen

from agent import harness, macro, routes, skillgate

ALL = ["card_reward", "card_reward_a2", "combat", "event_late", "event_neow", "game_over", "map_a1", "map_elite", "map_single", "menu", "restsite",
       "rewards", "select", "shop", "shop_a2", "treasure"]


def test_kind():
    want = dict(card_reward="CARD_REWARD", card_reward_a2="CARD_REWARD", combat="COMBAT", event_late="EVENT", event_neow="EVENT", game_over="GAME_OVER",
                map_a1="MAP", map_elite="MAP", map_single="MAP", menu="MENU", restsite="RESTSITE", rewards="REWARDS", select="SELECT", shop="SHOP",
                shop_a2="SHOP", treasure="TREASURE")
    for n in ALL:
        assert harness._kind(screen(n)) == want[n], n
    assert harness._kind("") == "?"
    assert harness._kind(None) == "?"
    assert harness._kind("ERR bridge down: the game is not running") == "ERR"
    assert harness._kind("SELECT 1\nA1 F9 X") == "SELECT"
    assert harness._kind("COMBAT (busy)\n") == "COMBAT"


def test_floor_act_hp():
    assert harness._floor(screen("shop_a2")) == "A2 F23"
    assert harness._floor(screen("card_reward")) == "A1 F2"
    assert harness._floor(screen("game_over")) == "A3 F48"
    assert harness._floor(screen("menu")) is None
    assert harness._floor("") is None and harness._floor(None) is None
    assert harness._act_index(screen("shop_a2")) == 1
    assert harness._act_index(screen("event_neow")) == 0
    assert harness._act_index(screen("game_over")) == 2
    assert harness._act_index(screen("menu")) is None and harness._act_index(None) is None
    assert harness._hp(screen("shop_a2")) == (36, 80)
    assert harness._hp(screen("game_over")) == (0, 72)
    assert harness._hp(screen("combat")) == (78, 80)
    assert harness._hp(screen("menu")) is None


def test_belt_and_potion_name(monkeypatch, tmp_path):
    fake = FakeBridge(screen("combat"))
    h = make_harness(monkeypatch, tmp_path, fake)
    assert h._belt() == ["Explosive Ampoule", "Blood Potion"]
    assert h._potion_name("potion 1 -> e0") == "Blood Potion"
    assert h._potion_name("potion 5") == "?"
    assert h._potion_name("play STRIKE #0") == "?"
    fake.screen = screen("shop_a2")
    assert h._belt() == ["Power Potion", "Vulnerable Potion"]
    fake.screen = screen("menu")
    assert h._belt() == []
    fake.screen = MAP_SCREEN_A2
    assert h._belt() == ["Power Potion", "-"]


def test_card_options():
    opts, skip = macro.parse_card_options(screen("card_reward"))
    assert opts == [(0, "Armaments", "ARMAMENTS", 0), (1, "Headbutt", "HEADBUTT", 0), (2, "Perfected Strike", "PERFECTED_STRIKE", 0)]
    assert skip == 3
    opts, skip = macro.parse_card_options(screen("card_reward_a2"))
    assert [o[2] for o in opts] == ["HEMOKINESIS", "CRUELTY", "STONE_ARMOR"] and skip == 3
    synth = "CARD_REWARD\nA1 F5 IRONCLAD A10 HP 50/80 G10 pots[-, -]\n0 Bash+(2) Deal 10 damage.\n1 Not A Real Card(1) Text.\n2 Resonance(1/2*) Text.\n3 Skip\n"
    opts, skip = macro.parse_card_options(synth)
    assert opts == [(0, "Bash+", "BASH", 1), (1, "Not A Real Card", None, 0), (2, "Resonance", macro.card_from_name("Resonance")[0], 0)]
    assert skip == 3
    assert macro.parse_card_options(screen("rewards")) == ([], None)
    assert macro.card_from_name("Setup Strike") == ("SETUP_STRIKE", 0)
    assert macro.card_from_name("Ashen Strike+") == ("ASHEN_STRIKE", 1)


def test_map_offers_and_parse():
    assert routes.offered(screen("map_a1")) == [(1, 1), (1, 3), (1, 5)]
    assert routes.offered(screen("map_elite")) == [(6, 1), (6, 2), (6, 3)]
    assert routes.offered(MAP_SCREEN_A2) == [(2, 0), (2, 2)]
    assert routes.offered(screen("shop")) == []
    nodes, boss_row = routes.parse_map(MAP_A2)
    assert boss_row == 8
    assert (0, 3) not in nodes  # the ancient (A) is not a priced node
    assert nodes[(1, 1)] == dict(type="M", children=[(2, 0), (2, 2)], visited=True)
    assert nodes[(2, 3)] == dict(type="$", children=[(3, 4)], visited=False)
    assert nodes[(7, 2)] == dict(type="R", children=[], visited=False)
    assert len(nodes) == 22


def test_brief_header():
    d = dict(deck=[dict(id="STRIKE_IRONCLAD"), dict(id="STRIKE_IRONCLAD"), dict(id="BASH", upgrade=1)], relics=[dict(id="BURNING_BLOOD")], potions=[])
    hz = dict(boss=["VANTOM_BOSS"], elites=["BYRDONIS_ELITE"], next=[], ctx=dict(seen=["NIBBITS_WEAK"], bosses=["VANTOM_BOSS"]))
    t = macro.brief_text(screen("shop"), d, hz)
    lines = t.split("\n")
    assert lines[0] == "A1 F3 IRONCLAD A10 HP 68/80 G108 pots[-, -]"
    assert lines[1] == "deck (3): BASH+, STRIKE_IRONCLADx2"
    assert lines[2] == "relics: BURNING_BLOOD" and lines[3] == "potions: -"
    assert lines[-1] == "boss: VANTOM_BOSS | elites that can still appear: BYRDONIS_ELITE | met this act: NIBBITS_WEAK"
    assert macro.brief_text("MENU\n0 new run", d, hz).split("\n")[0] == "MENU"


def test_skillgate_screen_skills(monkeypatch):
    monkeypatch.setattr(skillgate, "exists", lambda s: True)
    assert skillgate.screen_skills(screen("map_a1")) == ["sts2-ironclad", "sts2-ironclad-act1", "sts2-pathing"]
    assert skillgate.screen_skills(screen("event_late")) == ["sts2-ironclad", "sts2-ironclad-act1", "sts2-pathing", "sts2-mechanics"]
    assert skillgate.screen_skills(screen("event_neow")) == ["sts2-ironclad", "sts2-ironclad-act1", "sts2-pathing"]
    assert skillgate.screen_skills(screen("shop_a2")) == ["sts2-ironclad", "sts2-ironclad-act2", "sts2-deckbuilding"]
    assert skillgate.screen_skills(screen("select")) == ["sts2-ironclad", "sts2-ironclad-act1", "sts2-deckbuilding"]
    assert skillgate.screen_skills(screen("combat")) == ["sts2-ironclad", "sts2-ironclad-act1"]
    assert skillgate.screen_skills(screen("menu")) == []


def test_resolve():
    r = harness.Harness._resolve
    rw, rest, sel, cmb, shop = screen("rewards"), screen("restsite"), screen("select"), screen("combat"), screen("shop")
    assert r(rw, "3 e1") == "3 e1"  # not a `~` step: unchanged
    assert r(rw, "~gold") == "0"
    assert r(rw, "~card") == "1"  # the line that starts with `card`
    assert r(rw, "~proceed") == "2"
    assert r(rw, "~PROCEED") == "2"
    assert r(rest, "~rest") == "0"
    assert r(rest, "~smith") == "1"
    assert r(rest, "~upgrade a card") == "1"
    assert r(sel, "~pommel") == "0"  # identical options: either is the same action
    assert r(cmb, "~defend") == "1"
    assert r(cmb, "~strike e0") == "4 e0"
    assert r(cmb, "~potion blood !") == "6 !"
    assert r(sel, "~pommel strike 1") == "0 1"
    assert r(cmb, "~potion") == "ERR `potion` matches options 5, 6: name it more exactly, or use the number in its own call"
    assert r(shop, "~card") == "ERR `card` matches options 0, 1, 2, 3, 4, 5, 6, 7, 12, 13: name it more exactly, or use the number in its own call"
    assert r(shop, "~stampede") == "4"
    assert r(shop, "~leave") == "14"
    assert r(rw, "~nothing here") == "ERR no option matching `nothing here`"
    assert r(rw, "~9") == "0"  # a lone number is the text itself: `9 Gold`
    two = rw.replace("1 card:", "1 25 Gold\n3 card:")  # Amethyst Aubergine: a second gold reward; every one is collected, so `~gold` takes the first
    assert r(two, "~gold") == "0" and r(two, "~25") == "1"
    assert r(cmb.replace("1 Defend(1)", "1 Defend(0)"), "~defend").startswith("ERR")  # other options differing in a number stay ambiguous
    assert r(screen("map_a1"), "~r1c3") == "1"
