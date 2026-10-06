"""Screen text as the bridge renders it (`s` / `peek` / the reply of `a`): one parser for everything the harness reads off a screen. Stdlib only.

  KIND [extra]                                   first line: MAP, SHOP, REWARDS, CARD_REWARD, RESTSITE, EVENT, SELECT 1, COMBAT, GAME_OVER ... (`(busy)` mid-transition)
  A2 F23 IRONCLAD A10 HP 36/80 G230 pots[Power Potion, -]     the header (absent on the menu and right after a fight or an act change)
  0 52g card Headbutt(1) Deal 9 damage. ...      option lines: `<i> <label>`

(`agent/skillgate.py` keeps its own header regex: the hook loads it standalone, by file path.)
"""
import re

HEADER = re.compile(r"\bA(\d+) F(\d+)\b")
OPTION = re.compile(r"^(\d+) (.*)")
BARE_NUMBER = re.compile(r"^\d+(\s|$)")


def kind(text):
    """The screen kind (first word of the first line); '?' for no text."""
    return text.split("\n", 1)[0].split(" ")[0] if text else "?"


def busy(text):
    return text.split("\n")[0].endswith("(busy)")


def floor_key(text):
    """'A1 F5' from a screen header (the key the decision guards compare against), or None."""
    m = HEADER.search(text or "")
    return f"A{m.group(1)} F{m.group(2)}" if m else None


def act_index(text):
    """0-based act from a state header ("A2 F20 IRONCLAD ..."), None when the screen has no header."""
    m = re.search(r"A(\d+) F\d+", text or "")
    return int(m.group(1)) - 1 if m else None


def header_line(text, default=None):
    """The header line (the first line with `A<act> F<floor>`), else `default`."""
    return next((l for l in (text or "").split("\n") if re.search(r"A\d+ F\d+", l)), default)


def character(text):
    """'IRONCLAD' from the header, or None."""
    m = re.search(r"\bA\d+ F\d+\s+([A-Z]+)", text or "")
    return m.group(1) if m else None


def hp(text):
    """(HP, max HP) from the header, or None."""
    m = re.search(r"HP (\d+)/(\d+)", text or "")
    return (int(m.group(1)), int(m.group(2))) if m else None


def gold(text):
    m = re.search(r"\bG(\d+)\b", text or "")
    return int(m.group(1)) if m else None


def belt(text):
    """The potions in the game's belt, slot order (the header's `pots[...]`; `-` = empty slot)."""
    m = re.search(r"pots\[([^\]]*)\]", text or "")
    return [p.strip() for p in m.group(1).split(",")] if m else []


def options(text):
    """The option lines [(number as printed, label)]."""
    return [(m.group(1), m.group(2)) for l in (text or "").split("\n") for m in [OPTION.match(l)] if m]


def option_line(text, num):
    """The whole line of option `num` (as typed: '3'), '' when there is none."""
    return next((l for l in (text or "").split("\n") if l.startswith(f"{num} ")), "")


def is_bare_number(step):
    """An option given by number: after an earlier action in the same chain or batch the list has shifted, so a number may name another option."""
    return bool(BARE_NUMBER.match(step))


def resolve(state, step):
    """`~text [args]` -> `<i> [args]` for the first option line whose label contains text (case-insensitive); text may be several words (args are `eN` or `!`).
    A label that STARTS with the text wins over one that only contains it (`~card` is the card reward, not a potion whose text mentions cards); identical
    options (two copies of Strike) are the same action. Anything else is an `ERR ...` string."""
    if not step.startswith("~"):
        return step
    words = step[1:].split()
    args = []
    while words and (re.fullmatch(r"e\d+|!|\d+", words[-1]) and len(words) > 1):
        args.insert(0, words.pop())
    want = " ".join(words).lower()
    opts = [(n, t.lower()) for n, t in options(state)]
    starts = [n for n, t in opts if t.startswith(want)]
    hits = starts if len(starts) == 1 else [n for n, t in opts if want in t]
    if len(hits) > 1 and len({t for n, t in opts if n in hits}) == 1:
        hits = hits[:1]
    if len(hits) == 1:
        return " ".join([hits[0]] + args)
    if len(hits) > 1:
        return f"ERR `{want}` matches options {', '.join(hits)}: name it more exactly, or use the number in its own call"
    return f"ERR no option matching `{want}`"
