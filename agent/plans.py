import argparse
import datetime
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
PATH = os.path.join(ROOT, "data", "plans.json")
STARTERS = {
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
    starter, _relic, _hp = STARTERS[plan["character"]]
    d = list(starter) + ["ASCENDERS_BANE"]
    for c in plan.get("drop", []):
        if c in d:
            d.remove(c)
    cards = list(plan["core"]) + ([] if version.startswith("core") else list(plan.get("support", [])))
    minus = version.partition("-")[2]
    if minus in cards:
        cards.remove(minus)
    return d + cards


def _check_single_player(plan):
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
    mp = {c["id"] for pool in cat["cards"].values() for c in pool if c.get("multiplayer_only")}
    bad = [c for c in plan["core"] + plan.get("support", []) if c in mp]
    if bad:
        raise ValueError(f"{plan['id']}: multiplayer-only cards are never offered in single player: {bad}")


def check(plan):
    """problems with a plan entry: deck cards must be offerable (own pool or colorless, not basic/ancient/token, single player),
    substitutes offerable, drop from the starter, enablers/payoffs from core, threat and struggle ids real encounters"""
    import sts2
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json")))["cards"]
    pool = {c["id"]: c for k in (plan["character"], "COLORLESS") for c in cat[k] if not c.get("multiplayer_only")}
    offer = {i for i, c in pool.items() if c["rarity"] in ("Common", "Uncommon", "Rare")}
    encs = set(sts2.names()["encounter"])
    starter = STARTERS[plan["character"]][0]
    out = [f"deck card not offerable: {c}" for c in dict.fromkeys(plan["core"] + plan.get("support", [])) if c not in offer]
    out += [f"substitute not in pool: {c}" for k, v in plan.get("substitutes", {}).items() for c in [k] + v if c not in pool]
    out += [f"drop not in starter: {c}" for c in dict.fromkeys(plan.get("drop", [])) if plan["drop"].count(c) > starter.count(c)]
    out += [f"{k} not in core: {c}" for k in ("enablers", "payoffs") for c in plan.get(k, []) if c not in plan["core"]]
    out += [f"unknown encounter: {t['id']}" for t in plan["threats"] + plan.get("struggles", []) if t["id"] not in encs]
    out += [f"threat without act: {t['id']}" for t in plan["threats"] if "act" not in t]
    for b in ("full", "core"):
        n = len(deck(plan, b))
        out += [f"version {b}-{c} does not remove one card" for c in dict.fromkeys(plan["core"]) if len(deck(plan, f"{'' if b == 'full' else b}-{c}")) != n - 1]
    return out


def scenario(plan, threat, act, hp, version="full"):
    starter, relic, max_hp = STARTERS[plan["character"]]
    return dict(name=f"{plan['id']}:{version}@{threat}", ascension=10, encounter=threat, character=plan["character"], hp=hp, max_hp=max_hp,
                max_energy=3, base_orb_slots=3 if plan["character"] == "DEFECT" else 0, max_potion_slots=2, gold=0, seed=f"plan-{threat}", total_floor=17 * act,
                act=act, deck=deck(plan, version), relics=[relic] + plan.get("relics", []), potions=[])


def measure(plan, attempts=96, engine=None):
    sys.path.insert(0, os.path.join(ROOT, "rl"))
    from solver import DEFAULT_CKPT, Solver
    if engine is None:
        engine = Solver(DEFAULT_CKPT, M=5, K=32)
    _check_single_player(plan)
    versions = ["full", "core"] + [f"-{c}" for c in plan["core"]]
    rows = []
    for t in plan["threats"]:
        act, hp = t.get("act", 1), t.get("hp") or STARTERS[plan["character"]][2]
        scen = [scenario(plan, t["id"], act, hp, v) for v in versions]
        res = engine.solve(scen, attempts=attempts, groups=[0] * len(scen))
        rows.append(dict(threat=t["id"], act=act, hp=hp, wins={v: round(r["win"], 3) for v, r in zip(versions, res)},
                         se={v: round(r["win_se"], 3) for v, r in zip(versions, res)}))
    tested = dict(network=os.path.basename(DEFAULT_CKPT), search="5x32", potions="none",
                  decks={v: deck(plan, v) for v in versions}, relics=[STARTERS[plan["character"]][1]] + plan.get("relics", []))
    plan.setdefault("measurements", []).append(dict(date=datetime.date.today().isoformat(), attempts=attempts, tested=tested, rows=rows))
    plan["status"] = "measured" if plan.get("status") in (None, "proposed", "measured") else plan["status"]
    return rows


def text(plan):
    out = [f"{plan['id']} [{plan.get('status', 'proposed')}] {plan['character']} {plan['archetype']}: core {', '.join(plan['core'])}"
           + (f"; support {', '.join(plan.get('support', []))}" if plan.get("support") else "") + (f"; drop {', '.join(plan['drop'])}" if plan.get("drop") else "")]
    if plan.get("why"):
        out.append(f"  why: {plan['why']}")
    if plan.get("enablers") or plan.get("payoffs"):
        out.append(f"  enablers {', '.join(plan.get('enablers', []))}; payoffs {', '.join(plan.get('payoffs', []))}")
    if plan.get("threats"):
        out.append(f"  answers (act {plan['threats'][0].get('act', 1) + 1}): {', '.join(t['id'] for t in plan['threats'])}"
                   + "".join(f"; struggles {t['id']} ({t['why']})" for t in plan.get("struggles", [])))
    if plan.get("source"):
        out.append(f"  source: {'; '.join(plan['source'])}")
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
    sub.add_parser("check")
    a = ap.parse_args()
    plans = load()
    if a.cmd == "list":
        for p in plans:
            if not a.character or p["character"] == a.character.upper():
                print(text(p) + "\n")
    elif a.cmd == "check":
        bad = {p["id"]: check(p) for p in plans}
        for i, b in bad.items():
            print(f"{i}: {'ok' if not b else '; '.join(b)}")
        sys.exit(1 if any(bad.values()) else 0)
    else:
        p = next(p for p in plans if p["id"] == a.plan)
        measure(p, a.attempts)
        save(plans)
        print(text(p))


if __name__ == "__main__":
    main()
