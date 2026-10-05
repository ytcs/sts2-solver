"""Macro evaluation: what does a choice do to the fights ahead?

A choice (add / remove / upgrade a card, take or skip a relic, buy a potion, rest vs smith ...) is a *variant* of the current run state. Every variant is
played by the solver against the same set of encounters; the difference in win rate and HP lost is the choice's value for those fights. This prices the
combat side of a decision only: gold, future upgrades and route value are not in it, they stay my judgment (see `.claude/skills/sts2-strategy`).

  spec = dict(encounters=["NIBBITS_WEAK", ...] | dict(act="Hive", kind="regular", n=4),
              variants=[dict(name="skip"), dict(name="+Anger", add=["ANGER"]), dict(name="smith pommel", upgrade=["POMMEL_STRIKE"])],
              attempts=64, hp="current" | "full" | int)
  evaluate(engine, deck_json, spec) -> text table

Card names: `ID` or `ID+` (upgraded). Variant keys: add, remove, upgrade, relics_add, relics_remove, potions, hp (override).
"""
import copy

import numpy as np

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
            exact = isinstance(c, dict) or c.endswith("+")  # "ID" removes the first copy whatever its upgrade; "ID+" or a dict only that upgrade
            if d["id"] == want["id"] and (not exact or d.get("upgrade", 0) == want.get("upgrade", 0)):
                deck.pop(i)
                break
    for c in v.get("upgrade", []):
        for d in deck:
            if d["id"] == _card(c)["id"] and d.get("upgrade", 0) == 0:
                d["upgrade"] = 1
                break
    for e in v.get("enchant", []):  # ["OFFERING:IMBUED", ...] or [("OFFERING", "IMBUED")]: the first copy of the card without an enchantment gets it (ancient / event enchants)
        cid, ench = e.split(":") if isinstance(e, str) else e
        for d in deck:
            if d["id"] == cid and not d.get("enchantment"):
                d["enchantment"] = {"id": ench, "amount": 1}
                break
    deck.extend(_card(c) for c in v.get("add", []))
    for r in v.get("relics_remove", []):
        s["relics"] = [x for x in s["relics"] if x["id"] != r]
    for r in v.get("relics_add", []):
        s["relics"].append({"id": r} if isinstance(r, str) else r)
    if "potions" in v:
        s["potions"] = [{"id": p, "slot": i} for i, p in enumerate(v["potions"])]
        s["max_potion_slots"] = max(s.get("max_potion_slots", 2), len(v["potions"]))  # a variant with more potions than slots (Alchemical Coffer) widens the belt
    if "hp" in v:
        s["hp"] = int(v["hp"])
    return s


def narrow(ids, kind, ctx):
    """The encounters of `kind` that can still appear next, following the game's own draw (ActModel.GenerateRooms, AddWithoutRepeatingTags `[code]`).
    ctx = dict(seen=encounters met this act in order, bosses=[boss, second boss] in fight order, or empty when unknown).
    Weak, regular and elite encounters come from a bag per kind that is refilled with the whole pool whenever it is empty: after n met of a pool of P, the
    current bag has had n mod P of them removed, so the next one is among the rest. When a new bag starts (n mod P == 0, e.g. the 4th elite of 3) the whole pool is
    possible again except the encounter just met (a draw avoids repeating the previous entry unless nothing else is left). A boss pool shrinks to the act's
    known boss(es) not yet fought, in order (a second boss at A10 comes after the first)."""
    if not ctx:
        return ids
    seen = [x for x in ctx.get("seen", []) if x in ids]
    if kind == "boss":
        known = [b for b in ctx.get("bosses", []) if b in ids]
        fought = len(seen)
        return known[fought:] or known or ids
    n, P = len(seen), len(ids)
    if P <= 1:
        return ids
    k = n % P
    if k:
        consumed = set(seen[n - k:])
        return [i for i in ids if i not in consumed] or ids
    if seen:  # a fresh bag: anything but the entry just met
        return [i for i in ids if i != seen[-1]] or ids
    return ids


def resolve_encounters(spec):
    e = spec["encounters"]
    if isinstance(e, list):
        return e
    names = [e["act"]] if "act" in e and e["act"] in pools.ACTS else pools.act_names(e.get("act_index", 0))
    out = []
    for n in names:
        out += narrow(pools.pool(n, e.get("kind", "regular")), e.get("kind", "regular"), spec.get("_ctx"))
    return out[: e["n"]] if e.get("n") else out


SMOOTH_MULTS = (1.0, 1.5, 2.0, 3.0)


def evaluate_smooth(engine, deck_json, spec):
    """The graded objective for deck choices (`sts2-deckbuilding`, study `agent.deckstudy`): the win rate averaged over start HP x1 / 1.5 / 2 / 3. A deck far
    from beating the fight still wins with enough HP, so the average does not go flat when every option loses; what picks reduce is the HP a fight needs."""
    hp = spec.get("hp", "current")
    h0 = deck_json["max_hp"] if hp == "full" else (deck_json["hp"] if hp == "current" else hp)
    sub = dict(spec, smooth=False)
    parts = []
    for m in SMOOTH_MULTS:
        d = dict(deck_json, hp=int(round(h0 * m)), max_hp=max(deck_json["max_hp"], int(round(h0 * m))))
        parts.append(evaluate(engine, d, dict(sub, hp="current")))
    variants = spec["variants"]
    summary, lines = {}, [f"smooth objective: win rate averaged over start HP x{'/'.join(str(m) for m in SMOOTH_MULTS)} of {h0}  ({len(variants)} variants)"]
    for vi, v in enumerate(variants):
        wins = [p[1][vi]["win"] for p in parts]
        se = (sum(p[1][vi]["se"] ** 2 for p in parts) ** 0.5) / len(parts)
        per = {e: sum(p[1][vi]["per"][e] for p in parts) / len(parts) for e in parts[0][1][vi]["per"]}
        summary[vi] = dict(win=sum(wins) / len(wins), se=se, hp_lost=sum(p[1][vi]["hp_lost"] for p in parts) / len(parts), by_hp=wins, per=per)
        lines.append(f"{v.get('name', vi):24s} smooth {summary[vi]['win']:.3f} ±{se:.3f}  | " + " ".join(f"x{m}:{w:.2f}" for m, w in zip(SMOOTH_MULTS, wins)))
    for vi in range(1, len(variants)):
        d = summary[vi]["win"] - summary[0]["win"]
        sd = (summary[vi]["se"] ** 2 + summary[0]["se"] ** 2) ** 0.5
        lines.append(f"  {variants[vi].get('name', vi)} vs {variants[0].get('name', 0)}: smooth {d:+.3f} (±{sd:.3f})")
    return "\n".join(lines), summary


def evaluate(engine, deck_json, spec):
    if spec.get("smooth"):
        return evaluate_smooth(engine, deck_json, spec)
    base = dict(deck_json)
    hp = spec.get("hp", "current")
    if hp == "full":
        base["hp"] = base["max_hp"]
    elif isinstance(hp, int):
        base["hp"] = hp
    encs = resolve_encounters(spec)
    variants = spec["variants"]
    hold = spec.get("hold", ())
    drop_all = hold == "all"
    hold = set() if drop_all else set(hold)
    scen, index = [], []
    for vi, v in enumerate(variants):
        sv = apply_variant(base, v)
        for e in encs:
            sc = dict(sv, name=f"{v.get('name', vi)}@{e}", encounter=e, seed=f"macro{vi}")
            if (hold or drop_all) and not e.endswith("_BOSS"):  # potions are spent only when worth it and kept for the boss, so every other fight is priced without them (a lower bound)
                sc["potions"] = [] if drop_all else [p for p in sc.get("potions", []) if p["id"] not in hold]
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
        lost = np.concatenate([base["hp"] - np.array(r["ends"]) for _, r in rows if r.get("ends")]) if any(r.get("ends") for _, r in rows) else np.zeros(1)
        lq = [float(x) for x in np.percentile(lost, [10, 50, 90, 97.5])]  # the distribution of HP lost (a loss counts as the whole start HP), pooled over the encounters
        summary[vi] = dict(win=win, se=se, hp_lost=hpl, lost_q=lq, per={e: r["win"] for e, r in rows})  # per-encounter win: the weakest-fight views need it
        lines.append(f"{v.get('name', vi):24s} win {win:.3f} ±{se:.3f}  HP lost {100 * hpl:4.1f}% (q10/50/90/97.5: {lq[0]:.0f}/{lq[1]:.0f}/{lq[2]:.0f}/{lq[3]:.0f} HP)  | " + " ".join(f"{e.split('_')[0][:8]}:{r['win']:.2f}" for e, r in rows))
    if len(variants) > 1:
        b = summary[0]
        for vi in range(1, len(variants)):
            d = summary[vi]["win"] - b["win"]
            sd = (summary[vi]["se"] ** 2 + b["se"] ** 2) ** 0.5
            lines.append(f"  {variants[vi].get('name', vi)} vs {variants[0].get('name', 0)}: win {d:+.3f} (±{sd:.3f}), HP lost {100 * (summary[vi]['hp_lost'] - b['hp_lost']):+.1f} pts")
    return "\n".join(lines), summary


# ----------------------------------------------------------------------------------------------------------------------------- route HP budget

def route_budget(engine, deck_json, nodes, hp, act="Overgrowth", exclude=(), attempts=48, ctx=None):
    """Walk a planned route and chain the solver's results: every fight node is played at the HP I would arrive with, a rest heals 30% of max HP.

    nodes: tokens M (regular monster), W (weak monster), E (elite), B (boss), R (rest), S (smith instead of rest), ? $ T (no fight assumed).
    Pools: the act's pool for the token's kind minus `exclude` (encounters already seen: the bag does not repeat them until it empties).
    Prints win probability per node, expected HP after the node (conditional on winning, Burning Blood included) and the route's win probability.
    Potions: only a B node is solved with the belt (a potion is thrown once); every other node without potions, a lower bound.
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
        encs = narrow([e for e in pools.pool(act, kind[t]) if e not in set(exclude)], kind[t], ctx)
        pots = base.get("potions", []) if t == "B" else []  # a potion is thrown once: only the boss node gets the belt (every node used to get all of it: the same potion counted in every fight of the route)
        scen = [dict(base, hp=max(1, int(round(cur))), potions=pots, name=f"{t}@{e}", encounter=e, seed=f"route{i}") for e in encs]
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


# ----------------------------------------------------------------------------------------------------------------------------- one-call reports for the agent

_CARD_IDS = None


def card_id_set():
    """Every card id of the simulator (parsed from its card table once)."""
    global _CARD_IDS
    if _CARD_IDS is None:
        import os
        import re
        src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "sts2sim", "src", "content", "gen_cards.rs"), encoding="utf-8").read()
        _CARD_IDS = set(re.findall(r"CardDef::new\(ids::card::([A-Z0-9_]+)", src))
    return _CARD_IDS


def card_from_name(name):
    """'Setup Strike' / 'Ashen Strike+' -> ('SETUP_STRIKE', 0|1), or (None, 0) when the display name does not map to a simulator id."""
    import re
    up = 1 if name.strip().endswith("+") else 0
    cid = re.sub(r"[^A-Z0-9]+", "_", name.strip().rstrip("+").upper().replace("'", "")).strip("_")
    return (cid if cid in card_id_set() else None), up


def parse_card_options(state):
    """The options of a CARD_REWARD screen: [(index, display name, card id or None, upgrade)] and the index of Skip (or None)."""
    import re
    opts, skip = [], None
    for line in state.split("\n"):
        m = re.match(r"^(\d+) (.+?)\((\d+|X|-)\) ", line)
        if m:
            cid, up = card_from_name(m.group(2))
            opts.append((int(m.group(1)), m.group(2).strip(), cid, up))
            continue
        m = re.match(r"^(\d+) Skip", line)
        if m:
            skip = int(m.group(1))
    return opts, skip


def need_view(res, nvar, names=None):
    """The weakest-link view of a pick table. `res` = {set name: {variant index: summary with per-encounter wins}}; variant 0 is the baseline (skip / keep).
    For every variant: the weakest upcoming fight (the lowest win over the boss and the elites still to come) and the need-weighted gain: the mean change in win over all
    listed fights, each weighted by how unsolved it is for the baseline (1 - baseline win), so a fight the baseline already wins counts ~0 and an unsolved one counts in full.
    Returns (rows, solved) with rows[vi] = (weakest id, weakest win, need gain) and solved True when every fight is solved by the baseline (the gain is then n/a)."""
    fights = []
    for key in ("boss", "elites", "next act"):
        for e in res.get(key, {}).get(0, {}).get("per", {}):
            fights.append((key, e))
    if not fights:
        return {}, True
    w = {f: max(0.0, 1.0 - res[f[0]][0]["per"][f[1]]) for f in fights}
    tot = sum(w.values())
    rows = {}
    for vi in range(nvar):
        cur = [(e, res[k][vi]["per"][e]) for k, e in fights if k in ("boss", "elites")] or [(e, res[k][vi]["per"][e]) for k, e in fights]
        worst = min(cur, key=lambda t: t[1])
        gain = sum(w[f] * (res[f[0]][vi]["per"][f[1]] - res[f[0]][0]["per"][f[1]]) for f in fights) / tot if tot > 0.05 else None
        rows[vi] = (worst[0], worst[1], gain)
    return rows, tot <= 0.05


def reward_report(engine, deck_json, opts, hz, attempts=96, hp="full", hold=()):
    """One table for a card reward: every option (and skip) against the known boss (smooth objective), the elites still to come and the next act's elites and
    bosses (plain win rate / HP lost at `hp`). Prices only the combat side; gold, route and the plan stay my judgment."""
    from agent import card_tags
    variants = [dict(name="skip")] + [dict(name=n, add=[(cid + "+") if u else cid]) for _, n, cid, u in opts if cid]
    sets = [("boss", hz["boss"], True, attempts), ("elites", hz["elites"], False, max(48, attempts * 2 // 3)), ("next act", hz["next"], False, max(48, attempts * 2 // 3))]
    res = {}
    for key, encs, smooth, att in sets:
        if encs:
            _, res[key] = evaluate(engine, deck_json, dict(encounters=encs, variants=variants, attempts=att, hp=hp, smooth=smooth, hold=hold))
    lines = [f"card reward vs boss {','.join(hz['boss']) or '?'} (smooth = win averaged over start HP x{'/'.join(str(m) for m in SMOOTH_MULTS)} of full HP), "
             f"{len(hz['elites'])} elites left, {len(hz['next'])} next-act elite/boss fights; {attempts} attempts"]
    lines.append(f"{'option':22s} {'boss smooth':>16s} {'boss@full':>9s} {'elites win/HP':>14s} {'next act win/HP':>16s}  fills")
    try:
        tags = card_tags.load()
    except Exception:  # noqa: BLE001
        tags = {}
    base = res.get("boss", {}).get(0)
    for vi, v in enumerate(variants):
        b = res.get("boss", {}).get(vi)
        cells = [(f"{b['win']:.3f}" + (f" ({b['win'] - base['win']:+.3f})" if vi and base else "")) if b else "-", f"{b['by_hp'][0]:.2f}" if b else "-"]
        for key in ("elites", "next act"):
            r = res.get(key, {}).get(vi)
            cells.append(f"{r['win']:.2f}/{100 * r['hp_lost']:.0f}%" if r else "-")
        cid = v.get("add", [None])[0]
        fills = "/".join(tags.get(cid.rstrip("+"), {}).get("buckets", [])) if cid else ""
        lines.append(f"{v['name']:22s} {cells[0]:>16s} {cells[1]:>9s} {cells[2]:>14s} {cells[3]:>16s}  {fills}")
    nv, solved = need_view(res, len(variants))
    if nv:
        lines.append("weakest link (the lowest win over the boss and the elites to come) and need-weighted gain (each fight weighted by 1 - its skip win: a solved fight counts ~0)" + ("; every fight is solved by the baseline, so the gain is n/a" if solved else ""))
        for vi, v in enumerate(variants):
            e, wv, g = nv[vi]
            lines.append(f"  {v['name']:22s} weakest {e.split('_')[0][:14]:14s} {wv:.2f}   need-weighted gain " + (f"{g:+.3f}" if g is not None else "n/a"))
    unmapped = [n for _, n, cid, _ in opts if not cid]
    if unmapped:
        lines.append("not evaluated (no simulator id for the display name): " + ", ".join(unmapped))
    try:
        line = card_tags.deck_line(deck_json["deck"], tags)
        lines.append("deck buckets " + " ".join(f"{k} {n}" for k, n in line.items()) + "  gaps " + str(card_tags.deficiencies(line)))
    except Exception:  # noqa: BLE001
        pass
    return "\n".join(lines), res


ETERNAL = {"ASCENDERS_BANE", "GREED"}  # cannot be removed


def removal_report(engine, deck_json, hz, attempts=64, hp="full", hold=()):
    """One table for a card removal (shop service, event, Peace Pipe ...): every distinct removable card of the deck against the known boss (smooth objective), the
    elites still to come and the next act's elites and bosses, sorted by the boss smooth score. Prices the combat side only: what the removal costs (shop price rises per
    use), the card's role in a plan the simulator cannot see (enablers whose partner is not yet in the deck, relic synergies it does model) stay my judgment."""
    seen, variants = set(), [dict(name="keep all")]
    for d in deck_json["deck"]:
        key = (d["id"], d.get("upgrade", 0))
        if key in seen or d["id"] in ETERNAL:
            continue
        seen.add(key)
        variants.append(dict(name="-" + d["id"] + ("+" if key[1] else ""), remove=[dict(id=key[0], upgrade=key[1])]))
    n = sum(1 for d in deck_json["deck"] if d["id"] not in ETERNAL)
    sets = [("boss", hz["boss"], True, attempts), ("elites", hz["elites"], False, max(32, attempts * 2 // 3)), ("next act", hz["next"], False, max(32, attempts * 2 // 3))]
    res = {}
    for key, encs, smooth, att in sets:
        if encs:
            _, res[key] = evaluate(engine, deck_json, dict(encounters=encs, variants=variants, attempts=att, hp=hp, smooth=smooth, hold=hold))
    base = res.get("boss", {}).get(0)
    rows = []
    for vi, v in enumerate(variants):
        b = res.get("boss", {}).get(vi)
        e = res.get("elites", {}).get(vi)
        x = res.get("next act", {}).get(vi)
        rows.append((vi, v["name"], b, e, x))
    # the boss smooth score first (a 0.02 band is a tie), then the next act's win rate, then the HP the elites cost: a saturated boss must not leave the order arbitrary
    order = [rows[0]] + sorted(rows[1:], key=lambda r: (-round((r[2]["win"] if r[2] else 0) / 0.02), -(r[4]["win"] if r[4] else 0), (r[3]["hp_lost"] if r[3] else 1)))
    lines = [f"card removal vs boss {','.join(hz['boss']) or '?'} (smooth), {len(hz['elites'])} elites left, {len(hz['next'])} next-act fights; deck {n} removable cards; {attempts} attempts"]
    lines.append(f"{'remove':24s} {'boss smooth':>16s} {'boss@full':>9s} {'elites win/HP':>14s} {'next act win/HP':>16s}")
    for vi, name, b, e, x in order:
        c0 = (f"{b['win']:.3f}" + (f" ({b['win'] - base['win']:+.3f})" if vi and base else "")) if b else "-"
        c1 = f"{b['by_hp'][0]:.2f}" if b else "-"
        c2 = f"{e['win']:.2f}/{100 * e['hp_lost']:.0f}%" if e else "-"
        c3 = f"{x['win']:.2f}/{100 * x['hp_lost']:.0f}%" if x else "-"
        lines.append(f"{name:24s} {c0:>16s} {c1:>9s} {c2:>14s} {c3:>16s}")
    if base:
        se = base["se"]
        top = [r for r in order[1:] if r[2] and r[2]["win"] >= order[1][2]["win"] - 2 * se]
        lines.append(f"sorted by boss smooth (0.02 ties), then next-act win, then elite HP; best: {order[1][1]} ({order[1][2]['win'] - base['win']:+.3f}); boss within 2 se of it: {', '.join(r[1] for r in top)}")
    return "\n".join(lines), res


def brief_text(state, deck_json, hz):
    """The run at a glance in one call: header, deck by card, buckets, relics, potions, what the pools can still throw at me."""
    import collections
    import re
    from agent import card_tags
    head = next((l for l in state.split("\n") if re.search(r"A\d+ F\d+", l)), state.split("\n")[0])
    c = collections.Counter(d["id"] + ("+" if d.get("upgrade") else "") for d in deck_json["deck"])
    deck = ", ".join(f"{k}x{n}" if n > 1 else k for k, n in sorted(c.items()))
    lines = [head, f"deck ({len(deck_json['deck'])}): {deck}", "relics: " + ", ".join(r["id"] for r in deck_json["relics"]),
             "potions: " + (", ".join(p["id"] for p in deck_json["potions"]) or "-")]
    try:
        tags = card_tags.load()
        line = card_tags.deck_line(deck_json["deck"], tags)
        lines.append("buckets " + " ".join(f"{k} {n}" for k, n in line.items()) + "  gaps " + str(card_tags.deficiencies(line)))
    except Exception:  # noqa: BLE001
        pass
    lines.append(f"boss: {', '.join(hz['boss']) or 'unknown'} | elites that can still appear: {', '.join(hz['elites']) or '-'} | met this act: {', '.join(hz['ctx']['seen']) or '-'}")
    return "\n".join(lines)
