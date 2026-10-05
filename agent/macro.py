"""Macro evaluation: what does a choice do to the fights ahead?

A choice (add / remove / upgrade a card, take or skip a relic, buy a potion, rest vs smith ...) is a *variant* of the current run state. Every variant is
played by the solver against the same set of encounters; the difference in win rate and HP lost is the choice's value for those fights. This prices the
combat side of a decision only: gold, future upgrades and route value are not in it, they stay my judgment (see `.claude/skills/sts2-core`).

  spec = dict(encounters=["NIBBITS_WEAK", ...] | dict(act="Hive", kind="regular", n=4),
              variants=[dict(name="skip"), dict(name="+Anger", add=["ANGER"]), dict(name="smith pommel", upgrade=["POMMEL_STRIKE"])],
              attempts=64, hp="current" | "full" | int)
  evaluate(engine, deck_json, spec) -> text table

Card names: `ID` or `ID+` (upgraded). Variant keys: add, remove, upgrade, relics_add, relics_remove, potions, hp (override).
"""
import copy

from agent import pools


def _card(c):
    if isinstance(c, dict):
        return dict(c)
    up = 1 if c.endswith("+") else 0
    return {"id": c.rstrip("+"), "upgrade": up}


def apply_variant(base, v):
    s = copy.deepcopy(base)
    deck = s["deck"]
    for c in v.get("remove", []):
        want = _card(c)
        for i, d in enumerate(deck):
            if d["id"] == want["id"] and (not c.endswith("+") and not isinstance(c, dict) or d.get("upgrade", 0) == want.get("upgrade", 0)):
                deck.pop(i)
                break
    for c in v.get("upgrade", []):
        for d in deck:
            if d["id"] == _card(c)["id"] and d.get("upgrade", 0) == 0:
                d["upgrade"] = 1
                break
    deck.extend(_card(c) for c in v.get("add", []))
    for r in v.get("relics_remove", []):
        s["relics"] = [x for x in s["relics"] if x["id"] != r]
    for r in v.get("relics_add", []):
        s["relics"].append({"id": r} if isinstance(r, str) else r)
    if "potions" in v:
        s["potions"] = [{"id": p, "slot": i} for i, p in enumerate(v["potions"])]
    if "hp" in v:
        s["hp"] = int(v["hp"])
    return s


def resolve_encounters(spec):
    e = spec["encounters"]
    if isinstance(e, list):
        return e
    names = [e["act"]] if "act" in e and e["act"] in pools.ACTS else pools.act_names(e.get("act_index", 0))
    out = []
    for n in names:
        out += pools.pool(n, e.get("kind", "regular"))
    return out[: e["n"]] if e.get("n") else out


def evaluate(engine, deck_json, spec):
    base = dict(deck_json)
    hp = spec.get("hp", "current")
    if hp == "full":
        base["hp"] = base["max_hp"]
    elif isinstance(hp, int):
        base["hp"] = hp
    encs = resolve_encounters(spec)
    variants = spec["variants"]
    scen, index = [], []
    for vi, v in enumerate(variants):
        sv = apply_variant(base, v)
        for e in encs:
            sc = dict(sv, name=f"{v.get('name', vi)}@{e}", encounter=e, seed=f"macro{vi}")
            scen.append(sc)
            index.append((vi, e))
    res = engine.solve(scen, attempts=spec.get("attempts", 64))
    by = {}
    for (vi, e), r in zip(index, res):
        by.setdefault(vi, []).append((e, r))
    lines = [f"{len(variants)} variants x {len(encs)} encounters x {spec.get('attempts', 64)} attempts, start HP {base['hp']}/{base['max_hp']}"]
    summary = {}
    for vi, v in enumerate(variants):
        rows = by[vi]
        win = sum(r["win"] for _, r in rows) / len(rows)
        se = (sum(r["win_se"] ** 2 for _, r in rows) ** 0.5) / len(rows)
        hpl = sum((r["hp_lost"] or 0) for _, r in rows) / len(rows)
        summary[vi] = dict(win=win, se=se, hp_lost=hpl)
        lines.append(f"{v.get('name', vi):24s} win {win:.3f} ±{se:.3f}  HP lost {100 * hpl:4.1f}%  | " + " ".join(f"{e.split('_')[0][:8]}:{r['win']:.2f}" for e, r in rows))
    if len(variants) > 1:
        b = summary[0]
        for vi in range(1, len(variants)):
            d = summary[vi]["win"] - b["win"]
            sd = (summary[vi]["se"] ** 2 + b["se"] ** 2) ** 0.5
            lines.append(f"  {variants[vi].get('name', vi)} vs {variants[0].get('name', 0)}: win {d:+.3f} (±{sd:.3f}), HP lost {100 * (summary[vi]['hp_lost'] - b['hp_lost']):+.1f} pts")
    return "\n".join(lines), summary


# ----------------------------------------------------------------------------------------------------------------------------- route HP budget

def route_budget(engine, deck_json, nodes, hp, act="Overgrowth", exclude=(), attempts=48, smith_rests=()):
    """Walk a planned route and chain the solver's results: every fight node is played at the HP I would arrive with, a rest heals 30% of max HP.

    nodes: tokens M (regular monster), W (weak monster), E (elite), B (boss), R (rest), S (smith instead of rest), ? $ T (no fight assumed).
    Pools: the act's pool for the token's kind minus `exclude` (encounters already seen: the bag does not repeat them until it empties).
    Prints win probability per node, expected HP after the node (conditional on winning, Burning Blood included) and the route's win probability.
    """
    base = dict(deck_json)
    maxhp = base["max_hp"]
    kind = dict(M="regular", W="weak", E="elite", B="boss")
    lines = [f"route from {hp}/{maxhp} HP, act {act}, {attempts} attempts per encounter"]
    p_route = 1.0
    cur = float(hp)
    for i, tok in enumerate(nodes):
        t = tok.upper()
        if t in ("R",):
            cur = min(maxhp, cur + 0.3 * maxhp)
            lines.append(f"{i + 1:2d} R  rest: HP {cur:5.1f}")
            continue
        if t in ("S", "?", "$", "T"):
            lines.append(f"{i + 1:2d} {t}  (no fight assumed)  HP {cur:5.1f}")
            continue
        encs = [e for e in pools.pool(act, kind[t]) if e not in set(exclude)]
        scen = [dict(base, hp=max(1, int(round(cur))), name=f"{t}@{e}", encounter=e, seed=f"route{i}") for e in encs]
        res = engine.solve(scen, attempts=attempts)
        w = sum(r["win"] for r in res) / len(res)
        left = sum(r["win"] * r["hp_left_on_win"] for r in res) / max(sum(r["win"] for r in res), 1e-9)
        worst = min(res, key=lambda r: r["win"])
        wname = encs[res.index(worst)]
        p_route *= w
        lines.append(f"{i + 1:2d} {t}  win {w:.3f} (worst {wname.split('_')[0]} {worst['win']:.2f})  HP in {cur:5.1f} -> after win {left:5.1f}   route win so far {p_route:.3f}")
        cur = max(1.0, left)
    lines.append(f"route win probability (product of node win rates): {p_route:.3f}")
    return "\n".join(lines)
