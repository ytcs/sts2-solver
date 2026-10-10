import re

from agent import macro
from agent import screen as scr

PICK_RECORD = {"buckets:": "<five-bucket line, which are open>", "weakest:": "<weakest fight and what it asks>",
               "numbers:": "<table: best option and gain, the chosen card's section-3 bar status>",
               "judgment:": "<plan fit, density, future problems, synergies: why this choice>"}


def _option(step):
    toks = step.split()
    return toks[0] if toks and toks[0].isdigit() else None


def map_guard(state, step):
    if scr.kind(state) != "MAP":
        return None
    num = _option(step)
    if num is None or "!" in step.split():
        return None
    hp = scr.hp(state)
    line = scr.option_line(state, num)
    if hp and hp[0] < 0.6 * hp[1] and re.match(rf"^{num} (Elite|Boss)", line):
        return f"REFUSED: `{line}` at {hp[0]}/{hp[1]} HP. Heal first, or confirm with `a {num} !` if this is deliberate (check `route` first).\n" + state
    return None


def decision_guard(state, step, why, priced, reward_screen):
    kind = scr.kind(state)
    num = _option(step)
    if num is None:
        return None
    line = scr.option_line(state, num)
    n_opts = len(scr.options(state))
    here = scr.floor_key(state)
    ran = lambda *c: here is not None and any(priced.get(x) == here for x in c)  # noqa: E731
    lw = (why or "").lower()
    if kind == "CARD_REWARD":
        return pick_guard(state, step, why, reward_screen)
    if (kind == "MAP" and n_opts >= 2) or (kind == "EVENT" and here and here.endswith(" F1") and n_opts >= 2):
        if not ran("routes", "route"):
            return f"REFUSED: run `routes` on this floor before a {'fork' if kind == 'MAP' else 'Neow / ancient'} choice (sts2-pathing procedure, sts2-harness: routes at every fork).\n" + state
        return None
    if kind == "RESTSITE" and not re.search(r"(?i)proceed", line) and not (ran("routes") and ran("eval")):
        return ("REFUSED: rest or smith is priced over the rest of the act (sts2-deckbuilding section 6): `routes --hp <HP after the rest>` and `routes` at "
                "the HP now, plus `eval --smooth --boss --next` upgrade variants, on this floor.\n" + state)
    if (kind == "SHOP" and not re.search(r"(?i)leave", line)) or (kind == "RESTSITE" and not re.search(r"(?i)proceed", line)):
        if not ran("eval", "rmcalc", "routes", "pickplan", "price"):
            return f"REFUSED: price this {kind.lower()} decision first (`price`, `eval` variants, `rmcalc`, `routes`; sts2-deckbuilding section 1 / 6), on this floor.\n" + state
        miss = [k for k in ("numbers:", "judgment:") if k not in lw]
        if miss:
            return f"REFUSED: the `-- why` records the decision: numbers: <what the calculators said> ; judgment: <what decided it>. Missing: {', '.join(miss)}.\n"
    return None


def reward_key(state):
    opts, _skip = macro.parse_card_options(state)
    return scr.floor_key(state), tuple(o[1] for o in opts)


def pick_guard(state, step, why, reward_screen):
    if scr.kind(state) != "CARD_REWARD" or _option(step) is None:
        return None
    key = reward_key(state)
    if not key[1]:
        return None
    if reward_screen != key:
        return "REFUSED: run `reward` on this card reward first (sts2-deckbuilding section 1, step 2), then pick with the section-3 bar in the `-- why`.\n" + state
    missing = [k for k in PICK_RECORD if k not in (why or "").lower()]
    if missing:
        return ("REFUSED: the `-- why` of a card pick records each input of the decision (sts2-deckbuilding section 1): "
                + "; ".join(f"{k} {PICK_RECORD[k]}" for k in PICK_RECORD) + f". Missing: {', '.join(missing)}.\n")
    return None
