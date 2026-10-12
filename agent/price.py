import json
import os
import re
import sys

import numpy as np

from agent import bottleneck, events as EV, macro, pools, routes, runmodel as R, screen as scr, terms as TM, tracker

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, os.path.join(ROOT, "rl"))


def _kind_of(act_name, enc):
    for k in ("weak", "regular", "elite", "boss"):
        if enc in pools.pool(act_name, k):
            return k
    return None


def run_state(deck, ctx, events_path, state_text):
    t = tracker.from_record(events_path)
    act_name = ctx.names[0]
    seen = {}
    for e in ctx.seen:
        k = _kind_of(act_name, e)
        if k:
            seen.setdefault(k, []).append(e)
    nodes, _ = routes.parse_map(ctx.map_text)
    if scr.kind(state_text) == "MAP":
        frontier = routes.offered(state_text)
    else:
        here = max((k for k, n in nodes.items() if n["visited"]), default=None)
        frontier = [k for k in nodes[here]["children"] if k in nodes] if here else None
    relics = [r["id"] if isinstance(r, dict) else r for r in deck.get("relics", [])]
    pots = [p["id"] if isinstance(p, dict) else p for p in deck.get("potions", [])]
    st = R.RunState(deck, ctx.act, act_name, deck["hp"], deck["max_hp"], deck.get("gold", 0), deck["deck"], relics, pots,
                    deck.get("max_potion_slots", 2), (t.potion, t.offset, dict(t.unknown), t.removals), seen,
                    [b for b in ctx.bosses if b not in set(ctx.seen)] or list(ctx.bosses), frontier, nodes or None,
                    len(seen.get("weak", [])) + len(seen.get("regular", [])))
    st.bottleneck = bottleneck.term_vars(events_path, ctx.act) if TM.USED & set(R.NO_BOTTLENECK) else None
    return st


def _card_id(name):
    return macro.card_from_name(name)


def _stated(o, label):
    """the page's own numbers over the catalog's: an escalating HP cost ("Lose 13 HP", "Take 7 damage") and a named removal ("Defend is removed")"""
    import copy
    hp = re.search(r"(?:Lose|Take) (\d+) (?:HP|damage)", label)
    rm = re.search(r":\s*(.+?) is removed from your Deck", label)
    if not hp and not rm:
        return o
    o = copy.deepcopy(o)
    eff = o["effects"]
    if hp:
        i = next((i for i, e in enumerate(eff) if "hp" in e), None)
        if i is None:
            eff.insert(0, {"hp": -int(hp.group(1))})
        else:
            eff[i] = {"hp": -int(hp.group(1))}
    cid = (_card_id(rm.group(1))[0] or _ident(rm.group(1).rstrip("+"))) if rm else None
    if cid:
        i = next((i for i, e in enumerate(eff) if "card_remove" in e), None)
        if i is not None:
            eff[i] = {"card_remove": {"n": 1, "match": cid}}
    return o


def options(st, state_text):
    kind = scr.kind(state_text)
    out = []
    if kind == "MAP":
        for k in st.frontier or []:
            out.append((f"{st.nodes[k]['type']} r{k[0]}c{k[1]}", lambda s, _d, k=k: setattr(s, "frontier", [k])))
    elif kind == "CARD_REWARD":
        out.append(("skip", None))
        for _, label in scr.options(state_text):
            m = re.match(r"^(.+?)\(", label)
            if m:
                cid, up = _card_id(m.group(1))
                if cid:
                    out.append((m.group(1).strip(), lambda s, _d, c=cid, u=up: s.deck.append({"id": c, "upgrade": u})))
    elif kind == "RESTSITE":
        out.append(("rest", lambda s, _d: setattr(s, "hp", min(s.max_hp, s.hp + int(R.HEAL_REST * s.max_hp)))))
        for cid in sorted({c["id"] for c in st.deck if not c.get("upgrade") and c["id"] != "ASCENDERS_BANE"}):
            def smith(s, _d, cid=cid):
                next(c for c in s.deck if c["id"] == cid and not c.get("upgrade"))["upgrade"] = 1
            out.append((f"smith {cid}", smith))
    elif kind == "EVENT":
        from agent import events as EV
        lines = state_text.split("\n")
        title = lines[2].split(":", 1)[0].strip() if len(lines) > 2 else ""
        entry = EV.get(title)
        if entry is None:
            for _, label in scr.options(state_text):
                rid, _eff = EV.ancient_option(label)
                if rid:
                    out.append((label.split(":", 1)[0][:34], lambda s, d, r=rid: EV.apply_ancient(s, r, d)))
        if entry:
            labels = [label for _, label in scr.options(state_text)]
            for label, o in zip(labels, EV.match_page(title, labels)):
                if o is not None and not o["key"].endswith("_LOCKED"):
                    o = _stated(o, label)
                    out.append((label.split(":", 1)[0][:34], lambda s, d, o=o, eid=entry["id"]: EV.play_option(s, eid, o, d)))
    elif kind == "SHOP":
        out.append(("nothing", None))
        out += [(f"{label} ({price}g)", first) for label, _what, price, first in shop_items(st, state_text)]
    elif kind == "REWARDS":
        for _, label in scr.options(state_text):
            m = re.match(r"^potion (.+?):", label)
            pid = _ident(m.group(1)) if m else None
            if pid in _ids("potions") and len(st.potions) >= st.slots:
                out.append((f"leave {m.group(1).strip()}", None))
                belt = [_ident(b) for b in scr.belt(state_text)]
                for held in dict.fromkeys(st.potions):
                    slot = f" (a dp {belt.index(held)})" if held in belt else ""
                    out.append((f"{m.group(1).strip()} for {held}{slot}", lambda s, _d, h=held, q=pid: s.potions.__setitem__(s.potions.index(h), q)))
                break
    return out


def _ident(name):
    return re.sub(r"[^A-Z0-9]+", "_", name.strip().upper().replace("'", "").replace("’", "")).strip("_")


def _ids(kind):
    return {r["id"] for rows in R.CAT[kind].values() for r in rows}


def _short(cid):
    return re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", cid)


def shop_items(st, state_text, unpriced=None, removals=None):
    out = []
    for _, label in scr.options(state_text):
        if "can't afford" in label:
            continue
        m = re.match(r"^(\d+)g (card|relic|potion) (.+?)(?:\(|:)", label)
        if m:
            price, what, name = int(m.group(1)), m.group(2), m.group(3).strip()
            cid, up = _card_id(name) if what == "card" else (("FAKE_" if name.endswith("???") else "") + _ident(name), 0)
            if what == "card" and cid:
                out.append((name, what, price, lambda s, _d, c=cid, u=up, p=price: (s.deck.append({"id": c, "upgrade": u}), setattr(s, "gold", s.gold - p))))
            elif what == "relic" and cid in _ids("relics"):
                out.append((name, what, price, lambda s, _d, r=cid, p=price: (s.relics.append(r), setattr(s, "gold", s.gold - p))))
            elif what == "potion" and cid in _ids("potions"):
                if len(st.potions) < st.slots:
                    out.append((name, what, price, lambda s, _d, q=cid, p=price: (s.potions.append(q), setattr(s, "gold", s.gold - p))))
            elif unpriced is not None:
                unpriced.append(name)
        m = re.match(r"^(\d+)g remove a card", label)
        if m:
            price = int(m.group(1))
            for cid in removal_candidates(st.deck) if removals is None else removals:
                def rm(s, _d, cid=cid, p=price):
                    s.deck.remove(next(c for c in s.deck if c["id"] == cid))
                    s.gold -= p
                    s.removals += 1
                out.append((f"remove {_short(cid)}", "remove", price, rm))
    return out


def card_type(cid):
    return R.CARD_ROWS.get(cid, {}).get("type")


def removal_candidates(deck):
    """distinct removable ids: while an unupgraded basic of a card type is in the deck, non-basic cards of that type are not candidates"""
    ids = sorted({c["id"] for c in deck if c["id"] not in EV.UNREMOVABLE})
    basic_types = {card_type(c["id"]) for c in deck if c["id"].startswith(R.BASICS[:2]) and not c.get("upgrade")}
    return [i for i in ids if i.startswith(R.BASICS[:2]) or card_type(i) not in basic_types]


def bundles(st, items, predictor, keep=6, removals=2, top=8, shuffles=4):
    import itertools
    import random
    refs = R.reference_fights(st, random.Random(0))

    def screen(groups):
        sts = []
        for g in groups:
            s = st.copy()
            for it in g:
                it[3](s, None)
            sts.append(s)
        P = predictor.fight_start([s.scenario(e, hp) for s in sts for e, hp in refs], shuffles)
        return R.worth(P, st.max_hp).reshape(len(groups), len(refs)).mean(1)

    free = st.slots - len(st.potions)

    def fits(c):
        return sum(items[i][2] for i in c) <= st.gold and sum(items[i][1] == "remove" for i in c) <= 1 and sum(items[i][1] == "potion" for i in c) <= free

    w = screen([()] + [(it,) for it in items])
    gain = {(i,): g for i, g in enumerate(w[1:] - w[0]) if fits((i,))}
    rank = sorted((c[0] for c in gain), key=lambda i: -gain[(i,)])
    cand = sorted([i for i in rank if items[i][1] != "remove"] + [i for i in rank if items[i][1] == "remove"][:removals])
    pairs = [c for c in itertools.combinations(cand, 2) if fits(c)]
    if pairs:
        gain.update(zip(pairs, screen([tuple(items[i] for i in c) for c in pairs]) - w[0]))
    best_with = {i: max(g for c, g in gain.items() if i in c) for i in cand}
    pool = sorted(sorted(cand, key=lambda i: -best_with[i])[:keep])
    triples = [c for c in itertools.combinations(pool, 3) if fits(c)]
    if triples:
        gain.update(zip(triples, screen([tuple(items[i] for i in c) for c in triples]) - w[0]))
    if not gain:
        return [("nothing", None)], "nothing affordable"
    best = sorted(gain, key=lambda c: -gain[c])[:top]
    opts = [("nothing", None)]
    for c in best:
        label = " + ".join(items[i][0] for i in c) + f" ({sum(items[i][2] for i in c)}g)"
        opts.append((label, lambda s, d, c=c: [items[i][3](s, d) for i in c]))
    note = f"the best {len(best)} of {len(gain)} affordable bundles by a screen against " + " and ".join(f"{e} at {hp} HP" for e, hp in refs)
    return opts, note


def price(st, opts, predictor, n=128, seed=0, pol=None):
    ro = R.Rollouts(predictor, 4)
    pol = pol or R.BasePolicy()

    def rollouts(stop):
        states, seeds, firsts, owner = [], [], [], []
        for oi, (_label, first) in enumerate(opts):
            for j in range(n):
                states.append(st.copy())
                seeds.append(seed * 100_003 + j)
                firsts.append(first)
                owner.append(oi)
        won = ro.run(states, seeds, pol=pol, firsts=firsts, stop_at_next=stop)
        return states, np.array([0.0 if w is None else float(w) for w in won]), np.array(owner)

    # P(win run) needs whole-run rollouts; before the last act they only pay once this act and the next act's readiness are saturated
    staged = st.act < R.LAST_ACT
    states, won, owner = rollouts(staged)
    res = {}
    for oi, (label, _f) in enumerate(opts):
        sel = owner == oi
        ss = [s for s, o in zip(states, owner) if o == oi]
        cleared = np.array([1.0 if (s.end is None or s.end[0] > st.act or s.end[1] == "won") else 0.0 for s in ss])
        r = dict(act=cleared) if staged else dict(win=won[sel], act=cleared)
        if st.act < 2:
            r["ready"] = np.array([0.0 if s.ready is None else s.ready for s in ss])
            r["ready_worth"] = np.array([-1.0 if s.ready_worth is None else s.ready_worth for s in ss])
        r["floors"] = np.array([s.floors for s in ss], float)
        res[label] = r
    if staged and max(r["act"].mean() for r in res.values()) >= ACT_SATURATED and max(r["ready"].mean() for r in res.values()) >= READY_SATURATED:
        _states, won, owner = rollouts(False)
        for oi, (label, _f) in enumerate(opts):
            res[label]["win"] = won[owner == oi]
    return res


HORIZONS = (("win", "P(win run)"), ("act", "P(clear act)"), ("ready", "next act ready"), ("floors", "floors"))
ACT_SATURATED = 0.9
READY_SATURATED = 0.9


def _se(x):
    return float(x.std(ddof=1) / len(x) ** 0.5) if len(x) > 1 else float("nan")


def separates(res, k, z=2.0):
    if not all(k in r for r in res.values()):
        return False
    labels = list(res)
    best = max(labels, key=lambda lb: res[lb][k].mean())
    for lb in labels:
        d = res[best][k] - res[lb][k]
        if lb != best and d.mean() > 0 and d.mean() > z * _se(d):
            return True
    return False


def ladder(res):
    if separates(res, "win"):
        return "win", "P(win run) separates the options (> 2 paired se)"
    best = max(r["act"].mean() for r in res.values())
    if best < ACT_SATURATED:
        return "act", f"P(win run) flat; P(clear act) not saturated (best {best:.3f} < {ACT_SATURATED:g})"
    if separates(res, "act"):
        return "act", f"P(win run) flat; P(clear act) separates the options (> 2 paired se) at best {best:.3f}"
    why = f"P(win run) flat; P(clear act) saturated (best {best:.3f} >= {ACT_SATURATED:g}, within 2 paired se)"
    if all("ready" in r for r in res.values()):
        return "ready", why
    return "floors", why + "; next-act readiness not estimable in the last act"


def table(res):
    labels = list(res)
    keys = [(k, t) for k, t in HORIZONS if all(k in r for r in res.values())]
    main, why = ladder(res)
    order = [main] + [k for k, _ in keys if k != main]
    best = max(labels, key=lambda lb: tuple(res[lb][k].mean() for k in order))
    W = min(64, max(34, *(len(lb) for lb in labels)))
    lines = [f"ranked by: {dict(HORIZONS)[main]} ({why})",
             f"{'option':{W}s} " + "  ".join(f"{('*' if k == main else '') + t:>16s}" for k, t in keys) + "   vs best (paired)"]
    for lb in sorted(labels, key=lambda lb: tuple(-res[lb][k].mean() for k in order)):
        cells = []
        for k, _ in keys:
            x = res[lb][k]
            nd = 1 if k == "floors" else 3
            cells.append(f"{x.mean():.{nd}f} ±{_se(x):.{nd}f}".rjust(16))
        vs = ""
        if lb != best:
            parts = []
            for k, _ in keys:
                d = res[lb][k] - res[best][k]
                nd = 1 if k == "floors" else 3
                parts.append(f"{'*' if k == main else ''}{k} {d.mean():+.{nd}f} ±{_se(d):.{nd}f}")
            vs = "   " + ", ".join(parts)
        lines.append(f"{lb[:W]:{W}s} " + "  ".join(cells) + vs)
    return "\n".join(lines)


HEADER = re.compile(r"A(\d+) F(\d+) \w+ A\d+ HP (\d+)/(\d+) G(\d+)")


def recorded_screens(events_path, kinds=("CARD_REWARD", "RESTSITE")):
    """(floor, state text, RunState, recorded choice label) per decision screen"""
    sc, picks, seen, bosses, act, map_text = None, [], [], [], None, None
    for raw in open(events_path, encoding="utf-8"):
        e = json.loads(raw)
        k = e.get("kind")
        if k == "price" and e.get("map"):
            map_text = e["map"]
        if k == "fight_start":
            sc, picks = e.get("scenario") or sc, []
            if sc and sc.get("act") != act:
                act, seen, bosses, map_text = sc.get("act"), [], [], None
            seen.append(e["encounter"])
        elif k == "reward_eval":
            bosses = [b for b in (e.get("result", {}).get("boss", {}).get("0", {}).get("per") or {}) if b.endswith("_BOSS")] or bosses
        elif k == "macro" and e.get("screen") in kinds and sc is not None:
            state = e["state"]
            m = HEADER.search(state)
            labels = dict(scr.options(state))
            if not m or any(v.startswith("proceed") for v in labels.values()):
                continue
            names = [n for n in pools.ACTS if pools.ACTS[n]["act"] == act]
            name = next((n for n in names if any(x in pools.pool(n, kk) for x in seen for kk in ("weak", "regular", "elite", "boss"))), names[0])
            seen_kind = {}
            for x in seen:
                kk = _kind_of(name, x)
                if kk:
                    seen_kind.setdefault(kk, []).append(x)
            belt = [_ident(b) for b in scr.belt(state) if b != "-"]
            deck = [dict(c) for c in sc["deck"]] + picks
            relics = [r["id"] if isinstance(r, dict) else r for r in sc.get("relics", [])]
            nodes, frontier = (routes.parse_map(map_text)[0] or None) if map_text else None, None
            if nodes:
                here = max((kk for kk, n in nodes.items() if n["visited"]), default=None)
                frontier = [kk for kk in nodes[here]["children"] if kk in nodes] if here else [kk for kk in nodes if kk[0] == min(n[0] for n in nodes)]
            st = R.RunState(sc, act, name, int(m.group(3)), int(m.group(4)), int(m.group(5)), deck, relics, [p for p in belt if p in _ids("potions")],
                            sc.get("max_potion_slots", 2), (tracker.POTION_START, tracker.OFFSET_START, dict(tracker.UNKNOWN_BASE), 0), seen_kind,
                            bosses, frontier, nodes, len(seen_kind.get("weak", [])) + len(seen_kind.get("regular", [])))
            choice = labels.get(e.get("choice", "").split(" ")[0], "")
            yield int(m.group(2)), state, st, choice
            if e["screen"] == "CARD_REWARD":
                cm = re.match(r"^(.+?)\(", choice)
                cid, up = _card_id(cm.group(1)) if cm else (None, 0)
                if cid:
                    picks.append({"id": cid, "upgrade": up})


def best(res, adj=None):
    """ladder best; adj = {label: macro-terms adjustment in P units} added on the main horizon"""
    main, _ = ladder(res)
    order = [main] + [k for k, _ in HORIZONS if k != main and all(k in r for r in res.values())]
    adj = adj or {}
    return max(res, key=lambda lb: (res[lb][main].mean() + adj.get(lb, 0.0),) + tuple(res[lb][k].mean() for k in order[1:])), main


def event_adjust(st, state_text):
    """macro terms on an event page's options ({label: adjustment}); only this page's own effects (max_hp_delta), not follow-up pages"""
    from agent import events as EV, terms as TM
    if scr.kind(state_text) != "EVENT":
        return {}
    lines = state_text.split("\n")
    title = lines[2].split(":", 1)[0].strip() if len(lines) > 2 else ""
    if EV.get(title) is None:
        return {}
    labels = [label for _, label in scr.options(state_text)]
    out = {}
    for label, o in zip(labels, EV.match_page(title, labels)):
        if o is None:
            continue
        mhp = sum(next(iter(e.values())) for e in o["effects"] if isinstance(e, dict) and "max_hp" in e and isinstance(e["max_hp"], (int, float)))
        a = TM.adjust("event", R.term_ctx(st, max_hp_delta=mhp))
        if a:
            out[label.split(":", 1)[0][:34]] = a
    return out


RUNGS = (3, 6, 10, 99)
# a card is taken when it beats skip by TAKE_SE paired se, or by > 0 when a fight unsafe for the skip deck (P < R.SAFE_WIN) lies within DEADLINE_RADIUS floors
TAKE_SE, DEADLINE_RADIUS = 2.0, 3
P_FLOOR = 0.05


def _reachable(st, radius):
    """node types reachable from the frontier within radius floors (the map the run model holds), and whether the act boss is in range"""
    nodes, frontier = st.nodes or {}, list(st.frontier or [])
    if not nodes or not frontier:
        return None, False
    seen, layer, types = set(frontier), list(frontier), []
    for _ in range(radius):
        types += [nodes[k]["type"] for k in layer if k in nodes]
        nxt = [c for k in layer if k in nodes for c in nodes[k]["children"] if c not in seen]
        if not nxt:
            return types, True
        seen.update(nxt)
        layer = nxt
    return types, False


def _rung_encounters(st, types, boss_in_range):
    ctx = dict(seen=[e for v in st.seen.values() for e in v], bosses=list(st.bosses or []))
    out = []
    if any(t in ("M", "?") for t in types):
        out += macro.narrow(pools.pool(st.act_name, "weak"), "weak", ctx) if st.monsters < R.WEAK_FIGHTS[st.act] else []
        out += macro.narrow(pools.pool(st.act_name, "regular"), "regular", ctx)
    if "E" in types:
        out += macro.narrow(pools.pool(st.act_name, "elite"), "elite", ctx)
    if boss_in_range:
        out += list(st.bosses or []) or pools.pool(st.act_name, "boss")
    return list(dict.fromkeys(out))


def binding_fights(st, engine, seed, k=2, attempts=32):
    """first RUNGS radius where the skip deck at its current HP has a reachable encounter under SAFE_WIN: its k most dangerous.
    -> (encounters, radius, {encounter: P}) or ([], None, {})"""
    for radius in RUNGS:
        types, end = _reachable(st, radius)
        if types is None:
            return [], None, {}
        encs = _rung_encounters(st, types, end)
        if not encs:
            continue
        rows = engine.solve([st.scenario(e) for e in encs], attempts=attempts, seed=seed)
        p = {e: float(r["win"]) for e, r in zip(encs, rows)}
        worst = sorted(encs, key=lambda e: p[e])[:k]
        if p[worst[0]] < R.SAFE_WIN:
            return worst, radius, p
        if end:
            break
    return [], None, {}


# mandatory future fights only: elites are optional (a weak deck routes around them), so they are no factor of P(win run)
FAR_RUNGS = ((1, "boss"), (2, "boss"))
INFORMATIVE = 0.1


def _at_hp(x, h):
    return dict(x, hp=int(h), max_hp=max(x["max_hp"], int(h)))


def operating_mults(st, engine, sets, seed, attempts=24):
    """{factor key: HP multiple} for the full-HP factors: the first informative multiple"""
    out = {}
    flat = [(key, e, m) for key, encs, hp in sets if hp == "full" for e in encs for m in macro.SMOOTH_MULTS]
    if not flat:
        return out
    rows = engine.solve([_at_hp(st.scenario(e), st.max_hp * m) for _k, e, m in flat], attempts=attempts, seed=seed)
    p = {}
    for (key, _e, m), r in zip(flat, rows):
        p.setdefault(key, {}).setdefault(m, []).append(r["win"])
    for key, d in p.items():
        out[key] = next((m for m in macro.SMOOTH_MULTS if np.mean(d[m]) >= INFORMATIVE), macro.SMOOTH_MULTS[-1])
    return out


def _horizon_sets(st, engine, seed):
    """[(factor key, encounters, hp)]: near binding fights at current HP, this act's known bosses not among them and future boss rungs at full HP"""
    out = []
    near, radius, _p = binding_fights(st, engine, seed)
    for e in near:
        out.append((f"near {e}", [e], "current"))
    for e in st.bosses or []:
        if e not in near:
            out.append((f"boss {e}", [e], "full"))
    for a, k in FAR_RUNGS:
        if a > st.act:
            out.append((f"A{a + 1} {k}", [e for n in pools.act_names(a) for e in pools.pool(n, k)], "full"))
    return out, radius


def search_check(st, opts, res, engine, seed, top=3, attempts=128, far_attempts=48, variants=4):
    """score = log ratio vs the reference summed over the factors of P(win run) ahead (progress instead of wins where P < INFORMATIVE), common seeds, paired se.
    -> ({label: (score, se, {factor: measure}, per-attempt scores, {factor: P(win)}, {factor: per-attempt contribution}, {factor: HP multiple})}, binding radius)"""
    import random
    if engine is None:
        return {}, None
    sets, radius = _horizon_sets(st, engine, seed)
    if not sets:
        return {}, radius
    main, _ = ladder(res)
    ranked = sorted(res, key=lambda lb: -float(res[lb][main].mean()))
    ref = next((lb for lb in ("skip", "nothing") if lb in res), ranked[0])
    others = [lb for lb in ranked if lb != ref]
    pick = list(dict.fromkeys([ref] + others[:top] + [lb for lb in others if "remove" in lb.lower()][:1]))
    first = dict(opts)
    probe = st.copy()
    if first.get(ref) is not None:
        first[ref](probe, R.Draws(random.Random(0), st.base["character"], st.act))
    mult = operating_mults(probe, engine, sets, seed)
    slots = [(key, e, hp) for key, encs, hp in sets for e in encs]
    scen, owner, att, grp = [], [], [], []
    for lb in pick:
        seen = set()
        for k in range(variants if first.get(lb) is not None else 1):
            s2 = st.copy()
            if first.get(lb) is not None:
                first[lb](s2, R.Draws(random.Random(k), st.base["character"], st.act))
            sc = [_at_hp(s2.scenario(e), s2.max_hp * mult[key]) if hp == "full" else s2.scenario(e, hp=s2.hp) for key, e, hp in slots]
            sig = json.dumps(sc, sort_keys=True)
            if sig in seen:
                continue
            seen.add(sig)
            for si, x in enumerate(sc):
                scen.append(x)
                owner.append((lb, si))
                att.append(attempts if slots[si][0].startswith("near") else far_attempts)
                grp.append(si)
    rows = engine.solve(scen, attempts=att, groups=grp, seed=seed)
    wins, prog = {}, {}
    for (lb, si), r in zip(owner, rows):
        wins.setdefault(lb, {}).setdefault(si, []).append(np.array([np.nan if x is None else x for x in r["wins"]], float))
        prog.setdefault(lb, {}).setdefault(si, []).append(np.array([np.nan if x is None else x for x in (r.get("progress") or r["wins"])], float))
    W = {lb: {si: np.nanmean(np.stack(v), 0) for si, v in d.items()} for lb, d in wins.items()}
    G = {lb: {si: np.nanmean(np.stack(v), 0) for si, v in d.items()} for lb, d in prog.items()}
    keys = list(dict.fromkeys(sl[0] for sl in slots))
    sis_of = {a: [si for si, sl in enumerate(slots) if sl[0] == a] for a in keys}
    pwin = {lb: {a: float(np.nanmean(np.concatenate([W[lb][si] for si in sis_of[a]]))) for a in keys} for lb in W}
    M = {a: (W if pwin[ref][a] >= INFORMATIVE else G) for a in keys}
    meas = {lb: {a: float(np.nanmean(np.concatenate([M[a][lb][si] for si in sis_of[a]]))) for a in keys} for lb in W}
    out = {}
    for lb in pick:
        if lb not in W:
            continue
        contrib = []
        for a in keys:
            n = min(len(M[a][lb][si]) for si in sis_of[a])
            d = np.nanmean(np.stack([M[a][lb][si][:n] - M[a][ref][si][:n] for si in sis_of[a]]), 0)
            contrib.append(d / max(meas[ref][a], P_FLOOR))
        n = min(len(c) for c in contrib)
        tot = np.sum(np.stack([c[:n] for c in contrib]), 0)
        se = float(np.nanstd(tot, ddof=1) / np.sqrt(max(1, np.sum(~np.isnan(tot))))) if lb != ref else 0.0
        point = sum(np.log(max(meas[lb][a], P_FLOOR) / max(meas[ref][a], P_FLOOR)) for a in keys)
        out[lb] = (float(point) if lb != ref else 0.0, se, meas[lb], tot if lb != ref else np.zeros_like(tot), pwin[lb], dict(zip(keys, contrib)), mult)
    return out, radius


def hybrid_best(res, check, adj=None):
    """the run model's ladder best (with macro-term adjustments), unless search finds an option better than it by > 2 se of the difference"""
    b, main = best(res, adj=adj)
    if b not in check:
        return b, main, None
    alt = max(check, key=lambda lb: check[lb][0])
    d = check[alt][0] - check[b][0]
    x, y = check[alt][3], check[b][3]
    n = min(len(x), len(y))
    diff = (x[:n] - y[:n])[~np.isnan(x[:n] - y[:n])]
    se = float(diff.std(ddof=1) / np.sqrt(len(diff))) if len(diff) > 1 else float("inf")
    return (alt, "search", (round(d, 3), round(se, 3))) if alt != b and d > 2 * se else (b, main, None)


def _played(state, choice, opts):
    if scr.kind(state) == "RESTSITE":
        return "rest" if choice.lower().startswith("rest") else "smith"
    name = choice.split("(")[0].strip().lower()
    return next((lb for lb, _ in opts if lb.lower() == name), "skip" if name.startswith("skip") else name)


def _family(label):
    return "smith" if label.startswith("smith") else label


def screen_seed(tag, state_text):
    """stable per screen across processes (Python's str hash is salted per process)"""
    import zlib
    return zlib.crc32(f"{tag}|{scr.floor_key(state_text)}".encode()) % 10_000


def decide(st, state_text, pred, engine, opts, n=128, seed=0, hybrid=True):
    """shared by the live `price` command and the scripted baseline. -> dict(res, adj, check, best, ranked_by, override, terms_skip, radius)"""
    kind = scr.kind(state_text)
    res = price(st, opts, pred, n=n, seed=seed)
    adj = event_adjust(st, state_text)
    top = len(opts) if POLICY_C and kind == "CARD_REWARD" else 3
    check, radius = search_check(st, opts, res, engine, seed, top=top) if hybrid and engine is not None and kind in ("CARD_REWARD", "SHOP", "EVENT") else ({}, None)
    best_, ranked, override = hybrid_best(res, check, adj)
    terms_skip, policy = False, None
    if POLICY_C and kind == "CARD_REWARD" and "skip" in check and st.act >= 1:
        best_, ranked, override, policy = late_take(st, check, radius)
        terms_skip = best_ == "skip"
    elif kind == "CARD_REWARD" and "skip" in check:
        cand = max((lb for lb in check if lb != "skip"), key=lambda lb: check[lb][0], default=None)
        urgent = radius is not None and radius <= DEADLINE_RADIUS
        if cand is not None and (check[cand][0] > TAKE_SE * check[cand][1] or (urgent and check[cand][0] > 0)):
            best_, ranked, override = cand, "search", (round(check[cand][0], 3), round(check[cand][1], 3))
        else:
            best_, terms_skip = "skip", True
    elif kind == "CARD_REWARD" and best_ != "skip" and "skip" in res and override is None:
        main, _ = ladder(res)
        d = res[best_][main] - res["skip"][main]
        cid = _card_id(best_)[0]
        if cid and R.pick_with_terms("card_reward", st, [0.0, float(d.mean())], [(None, None), ("card", cid)], se=_se(d)) == 0:
            best_, terms_skip = "skip", True
    return dict(res=res, adj=adj, check=check, best=best_, ranked_by=ranked, override=override, terms_skip=terms_skip, radius=radius, policy=policy)


# arm C (docs/plan.md section 3), on only under POLICY_C
POLICY_C = False
GOLD_RESERVE, ROUTE_SAFE, REST_MARGIN, BOSS_ATTEMPTS = 300, 0.9, 0.02, 48
NO_SMITH = ("STRIKE_", "DEFEND_", "BASH")


def boss_factor_gain(st, entry):
    """(gain, paired se) of a search_check entry on this act's boss factor(s): 'boss X' keys and 'near X' keys naming a boss"""
    bosses = set(st.bosses or [])
    arrs = [v for k, v in entry[5].items() if k.startswith("boss ") or (k.startswith("near ") and (k[5:] in bosses or k.endswith("_BOSS")))]
    if not arrs:
        return 0.0, float("inf")
    n = min(len(a) for a in arrs)
    tot = np.sum(np.stack([a[:n] for a in arrs]), 0)
    tot = tot[~np.isnan(tot)]
    return float(tot.mean()) if len(tot) else 0.0, float(tot.std(ddof=1) / np.sqrt(len(tot))) if len(tot) > 1 else float("inf")


def on_plan(st, label, entry):
    """a Power, or a gain on this act's boss factor > TAKE_SE paired se -> (bool, power, gain, se)"""
    cid = _card_id(label)[0]
    power = card_type(cid) == "Power"
    g, se = boss_factor_gain(st, entry)
    return power or g > TAKE_SE * se, power, g, se


def late_take(st, check, radius):
    """act 2 on: skip unless search's best card has score > 0 with an unsafe fight within DEADLINE_RADIUS, or is on-plan"""
    cand = max((lb for lb in check if lb != "skip"), key=lambda lb: check[lb][0], default=None)
    urgent = radius is not None and radius <= DEADLINE_RADIUS
    pol = dict(component="card_reward", act=st.act + 1, cand=cand, urgent=urgent, radius=radius)
    if cand is None:
        return "skip", "policy", None, dict(pol, took=None)
    plan, power, g, se = on_plan(st, cand, check[cand])
    pol.update(score=round(check[cand][0], 3), se=round(check[cand][1], 3), power=power, boss_gain=round(g, 3), boss_se=round(se, 3))
    why = "urgent" if urgent and check[cand][0] > 0 else "power" if power else "boss factor" if plan else None
    if why is None:
        return "skip", "policy", None, dict(pol, took=None, why="off-plan")
    return cand, "policy", (round(check[cand][0], 3), round(check[cand][1], 3)), dict(pol, took=cand, why=why)


def act1_picks(events_path):
    """card ids chosen on act-1 CARD_REWARD / SHOP screens (macro events), with multiplicity"""
    out = []
    if not events_path or not os.path.exists(events_path):
        return out
    for raw in open(events_path, encoding="utf-8"):
        e = json.loads(raw)
        if e.get("kind") != "macro" or e.get("screen") not in ("CARD_REWARD", "SHOP"):
            continue
        m = HEADER.search(e.get("state", ""))
        if not m or m.group(1) != "1":
            continue
        label = dict(scr.options(e["state"])).get(str(e.get("choice", "")).split(" ")[0], "")
        cm = re.match(r"^\d+g card (.+?)(?:\(|:)", label) if e["screen"] == "SHOP" else re.match(r"^(.+?)\(", label)
        cid = _card_id(cm.group(1).strip())[0] if cm else None
        if cid:
            out.append(cid)
    return out


def off_plan(st):
    """act-1 picks still in the deck that are not Powers (by id, multiplicity capped by the deck's count)"""
    picks = getattr(st, "act1_picks", None) or []
    ids = [c["id"] for c in st.deck]
    return [cid for cid in dict.fromkeys(picks) if card_type(cid) != "Power" for _ in range(min(picks.count(cid), ids.count(cid)))]


def act_bosses(st):
    return list(st.bosses or []) or pools.pool(st.act_name, "boss")


def boss_gain(st, variants, engine, attempts=BOSS_ATTEMPTS, seed=0):
    """{label: mean P(win) vs this act's boss(es) at full HP, no potions} for [(label, mutate)] plus None = the deck as is, one batched solve"""
    bosses = act_bosses(st)
    rows = [(None, None)] + list(variants)
    sts = []
    for _lb, f in rows:
        x = st.copy()
        if f is not None:
            f(x)
        sts.append(x)
    res = engine.solve([_at_hp(x.scenario(b, potions=[]), x.max_hp) for x in sts for b in bosses], attempts=attempts, seed=seed)
    win = np.array([float(r["win"]) for r in res]).reshape(len(rows), len(bosses)).mean(1)
    if win[0] < INFORMATIVE:
        prog = [np.nanmean(np.array([np.nan if v is None else v for v in (r.get("progress") or r["wins"])], float)) for r in res]
        win = np.array(prog).reshape(len(rows), len(bosses)).mean(1)
    return {lb: float(w) for (lb, _f), w in zip(rows, win)}


def removal_order(st, engine=None, seed=0):
    """curses, then the dominance rule's basics (Strike before Defend), then act-1 picks still in the deck that are not Powers, then the rest by P(this act's boss) after the removal"""
    deck = [c["id"] for c in st.deck if c["id"] not in EV.UNREMOVABLE]
    cands = removal_candidates(st.deck)
    first = sorted({c for c in deck if c in R.CURSES}) + [c for c in cands if c.startswith("STRIKE_")] + [c for c in cands if c.startswith("DEFEND_")]
    early = [c for c in dict.fromkeys(off_plan(st)) if c not in first]
    if first or early or engine is None:
        return first + early
    rest = sorted(set(deck))
    if len(rest) < 2:
        return rest
    g = boss_gain(st, [(c, lambda x, c=c: x.deck.remove(next(d for d in x.deck if d["id"] == c))) for c in rest], engine, seed=seed)
    return sorted(rest, key=lambda c: -g[c])


def smith_target(st, opts, engine, seed=0):
    """the 'smith X' option with the best P(this act's boss) at full HP among non-basics -> (label or None, {label: P})"""
    sm = [(lb, f) for lb, f in opts if lb.startswith("smith ") and not lb[6:].startswith(NO_SMITH) and lb[6:] not in R.CURSES]
    if not sm:
        return None, {}
    g = boss_gain(st, [(lb, lambda x, f=f: f(x, None)) for lb, f in sm], engine, seed=seed)
    return max((lb for lb, _ in sm), key=lambda lb: g[lb]), g
