import re

from agent import macro
from agent import screen as scr

PICK_RECORD = {"rules:": "<rule IDs (sts2 decision loop) or new: <claim>>", "numbers:": "<`price` / `eval`: best option and its margin>",
               "judgment:": "<plan fit: what this changes in the run plan>"}
RULE_ID = re.compile(r"rules:\s*(new:|[A-Z][A-Z0-9]*-?\d+)")


def _cited(why):
    return [] if "rules:" not in (why or "").lower() or RULE_ID.search(why or "") else ["a rule ID or new: after rules:"]


def _option(step):
    toks = step.split()
    return toks[0] if toks and toks[0].isdigit() else None


def map_guard(state, step):
    if scr.kind(state) != "MAP":
        return None
    num = _option(step)
    if num is None or "!" in step.split() or len(scr.options(state)) < 2:
        return None
    hp = scr.hp(state)
    line = scr.option_line(state, num)
    if hp and hp[0] < 0.6 * hp[1] and re.match(rf"^{num} (Elite|Boss)", line):
        return f"REFUSED: `{line}` at {hp[0]}/{hp[1]} HP. Heal first, or confirm with `a {num} !` if this is deliberate (check `routes` first).\n" + state
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
        if not ran("routes"):
            return f"REFUSED: run `routes` on this floor before a {'fork' if kind == 'MAP' else 'Neow / ancient'} choice (sts2-pathing procedure).\n" + state
        return None
    hp = scr.hp(state)
    near_full = bool(hp) and hp[0] >= 0.95 * hp[1]
    if kind == "RESTSITE" and not re.search(r"(?i)proceed", line) and not near_full and not ran("restcalc") and not (ran("routes") and ran("eval")):
        return ("REFUSED: rest or smith is priced at the next binding fight (R3, P4): `restcalc` (boss win at the HP now vs after the rest), "
                "or `routes --hp <after>` + `routes` + `eval --smooth --boss --next` upgrade variants, on this floor.\n" + state)
    if (kind == "SHOP" and not re.search(r"(?i)leave", line)) or (kind == "RESTSITE" and not re.search(r"(?i)proceed", line)):
        if not (kind == "RESTSITE" and near_full) and not ran("eval", "routes", "price", "restcalc"):
            return f"REFUSED: price this {kind.lower()} decision first (`price`, `eval` variants, `routes`; sts2-deckbuilding protocol), on this floor.\n" + state
        miss = [k for k in ("rules:", "numbers:", "judgment:") if k not in lw] + _cited(why)
        if miss:
            return f"REFUSED: the `-- why` records the decision: rules: <IDs or new: claim> ; numbers: <what the calculators said> ; judgment: <what decided it>. Missing: {', '.join(miss)}.\n"
    return None


def _card_of(label):
    m = re.match(r"^(?:\d+ )?(?:\d+g card )?(.+?)\(", label)
    return macro.card_from_name(m.group(1))[0] if m and "can't afford" not in label else None


def target_warning(state, step, missing):
    """a nudge, never a refusal: a card reward pick or a shop exit that passes over a card the target deck still lacks"""
    kind, num = scr.kind(state), _option(step)
    if kind not in ("CARD_REWARD", "SHOP") or num is None or not missing:
        return None
    line = scr.option_line(state, num)
    if kind == "SHOP" and not re.search(r"(?i)leave", line):
        return None
    passed = sorted(({_card_of(label) for _, label in scr.options(state)} & set(missing)) - {_card_of(line)})
    return f"WARNING: passed over {', '.join(passed)}, on the target deck's add list (sts2-plan); if the plan changed, update it with `target`.\n" if passed else None


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
        return "REFUSED: run `price` on this card reward first (sts2-deckbuilding protocol), then pick with `rules:`, `numbers:` and `judgment:` in the `-- why`.\n" + state
    missing = [k for k in PICK_RECORD if k not in (why or "").lower()] + _cited(why)
    if missing:
        return ("REFUSED: the `-- why` of a card pick records each input of the decision (sts2 decision loop): "
                + "; ".join(f"{k} {PICK_RECORD[k]}" for k in PICK_RECORD) + f". Missing: {', '.join(missing)}.\n")
    chosen = next((o[1] for o in macro.parse_card_options(state)[0] if str(o[0]) == _option(step)), None)
    if chosen and chosen.lower().rstrip("+") not in (why or "").lower():
        return f"REFUSED: option {_option(step)} is {chosen}, which the `-- why` does not name (index slip?). Pick by name: `a ~{chosen}`.\n" + state
    return None
