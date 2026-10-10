"""Macro terms DSL (data/macro_rules.json `terms`), shared by runmodel.BasePolicy (price rollouts, scripted screen) and tools/baseline.py.
A term: type adjust (value added to a candidate's score) | margin (value an addition must beat skip by) | flag (named hook, value unused),
on = screens (card_reward, shop, relic, event), when = condition, value = expression. Units: P(win)-points (rollout worth / 2).
Expressions: numbers, strings, variables (VARS), + - * / // %, comparisons, in / not in, and/or/not, x if c else y, min max abs."""
import ast
import functools
import json
import os

PATH = os.path.join(os.path.dirname(__file__), "..", "data", "macro_rules.json")
VARS = {
    "deck_size": "cards in the deck now",
    "size_after": "cards after taking the candidate (deck_size for skip / relics)",
    "act": "act, 1-based",
    "gold": "gold now",
    "hp_frac": "HP / max HP",
    "max_hp": "max HP now",
    "max_hp_delta": "max HP change of an event option's own page effects",
    "removal_cost": "shop removal price (1e9 without a removal offer)",
    "is_card": "1 if the candidate is a card", "is_relic": "1 if the candidate is a relic", "is_skip": "1 for the no-addition variant",
    "core_owned": "other core pieces owned (copies counted) in the candidate's live synergy bundles where it is core (agent/synergy.py)",
    "anti": "owned items with an anti pair to the candidate",
    "se": "paired se of the priced difference (P units; 0 in rollouts)",
    "provides": "the candidate's provided mechanics (data/synergy_candidates.json vocabulary), test with 'energy_gain' in provides",
    "consumes": "the candidate's consumed mechanics",
    "cost": "the candidate card's energy cost (-1 for relics / unknown)",
    "bn_turns": "end-of-turn samples in this act's bottleneck tracker (agent/bottleneck.py)",
    "energy_bound": "share of cards left in hand at end of turn with less energy than their cost, this act",
    "ignored_share": "share of end-of-turn hand slots left unplayed with energy to spare, this act",
    "stranded": "non-starter cards of cost >= 2 left in hand at end of turn for lack of energy, per turn, this act",
}
_FUNCS = {"min": min, "max": max, "abs": abs}
_OK = (ast.Expression, ast.BoolOp, ast.BinOp, ast.UnaryOp, ast.Compare, ast.IfExp, ast.Call, ast.Name, ast.Constant, ast.Load, ast.And, ast.Or,
       ast.Not, ast.USub, ast.UAdd, ast.Add, ast.Sub, ast.Mult, ast.Div, ast.FloorDiv, ast.Mod, ast.Lt, ast.LtE, ast.Gt, ast.GtE, ast.Eq, ast.NotEq, ast.In,
       ast.NotIn)


@functools.lru_cache(maxsize=None)
def compiled(expr):
    tree = ast.parse(str(expr), mode="eval")
    for n in ast.walk(tree):
        if not isinstance(n, _OK) or isinstance(n, ast.Name) and n.id not in VARS and n.id not in _FUNCS or \
                isinstance(n, ast.Call) and not (isinstance(n.func, ast.Name) and n.func.id in _FUNCS and not n.keywords):
            raise ValueError(f"macro term expression not allowed: {expr!r} ({type(n).__name__})")
    return compile(tree, f"<term {expr}>", "eval")


def _eval(expr, ctx):
    return eval(compiled(expr), {"__builtins__": {}, **_FUNCS}, ctx)  # noqa: S307 (whitelisted AST)


def load(path=PATH):
    terms = json.load(open(path, encoding="utf-8"))["terms"]
    for t in terms.values():
        compiled(t.get("when", "True"))
        compiled(t.get("value", "0"))
    return terms


TERMS = {} if os.environ.get("MACRO_RULES") == "off" else load()


def _active(screen, typ, ctx):
    for name, t in TERMS.items():
        if t["type"] == typ and screen in t["on"] and _eval(t.get("when", "True"), ctx):
            yield name, t


def adjust(screen, ctx):
    return sum(float(_eval(t["value"], ctx)) for _, t in _active(screen, "adjust", ctx))


def margin(screen, ctx):
    return sum(float(_eval(t["value"], ctx)) for _, t in _active(screen, "margin", ctx))


def flag(name, screen, ctx):
    t = TERMS.get(name)
    return bool(t and t["type"] == "flag" and screen in t["on"] and _eval(t.get("when", "True"), ctx))
