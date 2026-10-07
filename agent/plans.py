"""The plan library (`docs/rebuild.md` S6): game plans that hold when greedy numbers are flat, kept as data so they do not drift between sessions.

  python -m agent.plans list [CHARACTER]                    # plans with their measured tables
  python -m agent.plans measure PLAN_ID [--attempts 96]     # fill a plan's table with the search against each threat

A plan (`data/plans.json`): id, character, archetype, the threats it answers (encounter ids), the core cards (what makes it work), support cards
(what it wants next), the basics it drops, status (proposed -> measured -> demoted) and measurements. A measurement plays the plan's deck (the
character's starter deck, minus the dropped basics, plus core, plus support) and its partial versions (core only; core minus each card) against each
threat at a stated act and HP with the search (`Solver`, common random numbers across versions), and records the win rates with their se, the
network that played, and the date. A measurement is a LOWER BOUND set by this solver and this exact deck: the solver may play the plan badly and
the deck is one construction of the archetype. It never demotes a plan by itself, and a plan changes only through a measurement.
"""
import argparse
import datetime
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
PATH = os.path.join(ROOT, "data", "plans.json")
STARTERS = {  # A10 starter decks and relics (`docs/spec/05-characters-and-inputs.md` section 4); Ascender's Bane from A5
    "IRONCLAD": (["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH"], "BURNING_BLOOD", 80),
    "SILENT": (["STRIKE_SILENT"] * 5 + ["DEFEND_SILENT"] * 5 + ["NEUTRALIZE", "SURVIVOR"], "RING_OF_THE_SNAKE", 70),
    "DEFECT": (["STRIKE_DEFECT"] * 4 + ["DEFEND_DEFECT"] * 4 + ["ZAP", "DUALCAST"], "CRACKED_CORE", 75),
    "NECROBINDER": (["STRIKE_NECROBINDER"] * 4 + ["DEFEND_NECROBINDER"] * 4 + ["BODYGUARD", "UNLEASH"], "BOUND_PHYLACTERY", 66),
    "REGENT": (["STRIKE_REGENT"] * 4 + ["DEFEND_REGENT"] * 4 + ["FALLING_STAR", "VENERATE"], "DIVINE_RIGHT", 75),
}


def load():
    return json.load(open(PATH)) if os.path.exists(PATH) else []


def save(plans):
    with open(PATH, "w", encoding="utf-8") as f:
        json.dump(plans, f, indent=1)


def deck(plan, version="full"):
    """The plan's deck for a version: full, core (no support), or `-CARD` (full minus one core card)."""
    starter, _relic, _hp = STARTERS[plan["character"]]
    d = list(starter) + ["ASCENDERS_BANE"]
    for c in plan.get("drop", []):
        if c in d:
            d.remove(c)
    cards = list(plan["core"]) + ([] if version == "core" else list(plan.get("support", [])))
    if version.startswith("-") and version[1:] in cards:
        cards.remove(version[1:])
    return d + cards


def scenario(plan, threat, act, hp, version="full"):
    starter, relic, max_hp = STARTERS[plan["character"]]
    return dict(name=f"{plan['id']}:{version}@{threat}", ascension=10, encounter=threat, character=plan["character"], hp=hp, max_hp=max_hp,
                max_energy=3, base_orb_slots=3 if plan["character"] == "DEFECT" else 0, max_potion_slots=2, gold=0, seed=f"plan-{threat}", total_floor=17 * act,
                act=act, deck=deck(plan, version), relics=[relic] + plan.get("relics", []), potions=[])


def measure(plan, attempts=96, engine=None):
    """Plays every version of the plan against each threat (search, common random numbers) and appends the measurement."""
    sys.path.insert(0, os.path.join(ROOT, "rl"))
    from solver import DEFAULT_CKPT, DEFAULT_VALUE_CKPTS, Solver
    if engine is None:
        engine = Solver(DEFAULT_CKPT, M=5, K=32, value_ckpts=DEFAULT_VALUE_CKPTS)
    versions = ["full", "core"] + [f"-{c}" for c in plan["core"]]
    rows = []
    for t in plan["threats"]:
        act, hp = t.get("act", 1), t.get("hp") or STARTERS[plan["character"]][2]
        scen = [scenario(plan, t["id"], act, hp, v) for v in versions]
        res = engine.solve(scen, attempts=attempts, groups=[0] * len(scen))
        rows.append(dict(threat=t["id"], act=act, hp=hp, wins={v: round(r["win"], 3) for v, r in zip(versions, res)},
                         se={v: round(r["win_se"], 3) for v, r in zip(versions, res)}))
    tested = dict(network=os.path.basename(DEFAULT_CKPT), values=[os.path.basename(v) for v in DEFAULT_VALUE_CKPTS], search="5x32", potions="none",
                  decks={v: deck(plan, v) for v in versions}, relics=[STARTERS[plan["character"]][1]] + plan.get("relics", []))
    plan.setdefault("measurements", []).append(dict(date=datetime.date.today().isoformat(), attempts=attempts, tested=tested, rows=rows))
    plan["status"] = "measured" if plan.get("status") in (None, "proposed", "measured") else plan["status"]
    return rows


def text(plan):
    out = [f"{plan['id']} [{plan.get('status', 'proposed')}] {plan['character']} {plan['archetype']}: core {', '.join(plan['core'])}"
           + (f"; support {', '.join(plan.get('support', []))}" if plan.get("support") else "") + (f"; drop {', '.join(plan['drop'])}" if plan.get("drop") else "")]
    if plan.get("why"):
        out.append(f"  why: {plan['why']}")
    for m in plan.get("measurements", [])[-1:]:
        for r in m["rows"]:
            out.append(f"  {r['threat']} (act {r['act'] + 1}, {r['hp']} HP, {m['attempts']} att, {m['date']}): " + "  ".join(f"{v} {w:.2f}" for v, w in r["wins"].items()))
        t = m.get("tested", {})
        out.append(f"  (solver lower bound: {t.get('network', '?')} search {t.get('search', '?')}, no potions, deck = starter - drop + core + support, {len(t.get('decks', {}).get('full', []))} cards; "
                   "says how THIS solver does with THIS deck, not whether the plan works)")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    ls = sub.add_parser("list"); ls.add_argument("character", nargs="?")
    me = sub.add_parser("measure"); me.add_argument("plan"); me.add_argument("--attempts", type=int, default=96)
    a = ap.parse_args()
    plans = load()
    if a.cmd == "list":
        for p in plans:
            if not a.character or p["character"] == a.character.upper():
                print(text(p) + "\n")
    else:
        p = next(p for p in plans if p["id"] == a.plan)
        measure(p, a.attempts)
        save(plans)
        print(text(p))


if __name__ == "__main__":
    main()
