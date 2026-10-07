"""Public run counters (`agent/tracker.py`) follow the game's code rules."""
from agent import tracker

H = "A1 F2 SILENT A10 HP 56/70 G99 pots[-, -]"
RARITY = {"Adrenaline": "Rare"}.get


def step(t, screen, lines, choice="0", result="MAP"):
    t.screen("\n".join([screen, H] + lines), choice, result, RARITY)


def test_potion_odds_and_elite_bonus():
    t = tracker.Tracker()
    t.fight_start("NIBBITS_WEAK")
    step(t, "REWARDS", ["0 9 Gold", "1 potion Strength Potion: Gain 2 Strength."])
    assert abs(t.potion - 0.30) < 1e-9
    t.fight_start("BYRDONIS_ELITE")
    step(t, "REWARDS", ["0 29 Gold", "1 relic Nunchaku: ..."])
    step(t, "REWARDS", ["0 29 Gold"])  # the same screen again after a pickup: counted once
    assert abs(t.potion - 0.40) < 1e-9
    assert "elite 52%" in t.line() or "elite 53%" in t.line()


def test_rare_offset_steps_and_resets():
    t = tracker.Tracker()
    t.fight_start("NIBBITS_WEAK")
    step(t, "REWARDS", ["0 9 Gold", "1 card: A | B | C"])
    step(t, "CARD_REWARD", ["0 A(1) x", "1 B(1) y", "2 C(1) z", "3 Skip"])
    assert abs(t.offset - (-0.05 + 3 * 0.005)) < 1e-9
    t.fight_start("THE_KIN_BOSS")
    step(t, "REWARDS", ["0 75 Gold", "1 card: Adrenaline | D | E"])
    step(t, "CARD_REWARD", ["0 D(1) x", "1 Adrenaline(0) y", "2 E(1) z", "3 Skip"])
    assert abs(t.offset - (-0.05 + 0.005)) < 1e-9  # D steps, the rare resets, E steps


def test_unknown_room_odds():
    t = tracker.Tracker()
    step(t, "MAP", ["0 Unknown r3c2 -> Mc1"], "0", "EVENT")
    assert abs(t.unknown["monster"] - 0.20) < 1e-9 and abs(t.unknown["shop"] - 0.06) < 1e-9
    step(t, "MAP", ["0 Unknown r5c2 -> Ec2"], "0", "COMBAT")
    assert abs(t.unknown["monster"] - 0.10) < 1e-9 and abs(t.unknown["treasure"] - 0.06) < 1e-9
