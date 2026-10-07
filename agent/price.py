"""`price`: every option of the current screen priced by paired rollouts of the run model (`agent/runmodel.py`, `docs/rebuild.md` S5).

One currency at three horizons, longest first: P(win the run), P(clear this act), floors reached. A weak deck has P(win run) ~ 0 for every option;
the act horizon then still separates them, and the floors after it. Options per screen:
  MAP          each node on offer
  CARD_REWARD  each card and skip
  RESTSITE     rest, and smith of each upgradable card
  SHOP         nothing (save the gold), and the best affordable bundles of up to 3 purchases (cards, relics, potions, one removal), chosen by a quick screen (`bundles`)
  REWARDS      a potion offered to a full belt: leave it, or take it in place of each held potion
  EVENT        each option of a catalogued event (`data/events.json`) or an ancient (`data/ancient_relics.json`: the relic plus its pickup effects); an unmodelled effect is a no-op
Each option meets the same draws (common random numbers); the table prints each option's mean with its se and the paired difference to the best.
A screen with a single option is not priced.
"""
import os
import re
import sys

import numpy as np

from agent import pools, routes, runmodel as R, screen as scr, tracker

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, os.path.join(ROOT, "rl"))


def _kind_of(act_name, enc):
    for k in ("weak", "regular", "elite", "boss"):
        if enc in pools.pool(act_name, k):
            return k
    return None


def run_state(deck, ctx, events_path, state_text):
    """The live run as a `RunState`: deck.json, the act context (boss, encounters met, map), the public counters, the map position."""
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
    else:  # inside a room: the rest of the act starts from the children of the room I am in (the visited node of the highest row)
        here = max((k for k, n in nodes.items() if n["visited"]), default=None)
        frontier = [k for k in nodes[here]["children"] if k in nodes] if here else None
    relics = [r["id"] if isinstance(r, dict) else r for r in deck.get("relics", [])]
    pots = [p["id"] if isinstance(p, dict) else p for p in deck.get("potions", [])]
    return R.RunState(deck, ctx.act, act_name, deck["hp"], deck["max_hp"], deck.get("gold", 0), deck["deck"], relics, pots,
                      deck.get("max_potion_slots", 2), (t.potion, t.offset, dict(t.unknown), t.removals), seen, ctx.bosses, frontier, nodes or None,
                      len(seen.get("weak", [])) + len(seen.get("regular", [])))


def _card_id(name):
    from agent import macro
    return macro.card_from_name(name)


def options(st, state_text):
    """[(label, first)] for the screen: `first` applies the option to a rollout's copy of the state."""
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
        if entry is None:  # an ancient (Neow, Orobas, Darv ...): each option is a relic with its pickup effects (`data/ancient_relics.json`)
            for _, label in scr.options(state_text):
                rid, _eff = EV.ancient_option(label)
                if rid:
                    out.append((label.split(":", 1)[0][:34], lambda s, d, r=rid: EV.apply_ancient(s, r, d)))
        if entry:
            labels = [label for _, label in scr.options(state_text)]
            for label, o in zip(labels, EV.match(title, labels)):
                if o is not None and not o["key"].endswith("_LOCKED"):
                    idx = entry["options"].index(o)
                    out.append((label.split(":", 1)[0][:34], lambda s, d, i=idx, eid=entry["id"]: EV.play_option(s, eid, i, d)))
    elif kind == "SHOP":  # singles; `bundles` prices sets of purchases within the budget
        out.append(("nothing", None))
        out += [(f"{label} ({price}g)", first) for label, _what, price, first in shop_items(st, state_text)]
    elif kind == "REWARDS":  # a potion offered to a full belt: leave it, or take it in place of each held potion
        for _, label in scr.options(state_text):
            m = re.match(r"^potion (.+?):", label)
            pid = _ident(m.group(1)) if m else None
            if pid in _ids("potions") and len(st.potions) >= st.slots:
                out.append((f"leave {m.group(1).strip()}", None))
                belt = [_ident(b) for b in scr.belt(state_text)]
                for held in dict.fromkeys(st.potions):
                    slot = f" (a dp {belt.index(held)})" if held in belt else ""
                    out.append((f"{m.group(1).strip()} for {held}{slot}", lambda s, _d, h=held, q=pid: s.potions.__setitem__(s.potions.index(h), q)))
                break  # one potion reward at a time: price the next after taking or leaving this one
    return out


def _ident(name):
    """'Gambler's Brew' -> 'GAMBLERS_BREW' (the catalog's id)."""
    return re.sub(r"[^A-Z0-9]+", "_", name.strip().upper().replace("'", "").replace("’", "")).strip("_")


def _ids(kind):
    return {r["id"] for rows in R.CAT[kind].values() for r in rows}


def _short(cid):
    return re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", cid)


def shop_items(st, state_text, unpriced=None):
    """The affordable purchases of a SHOP screen: [(label, kind, price, first)], the removal once per distinct card (`first` applies the purchase and pays).
    A card, relic or potion without a simulator / catalog id is left out (it cannot be priced) and its name added to `unpriced` when given."""
    out = []
    for _, label in scr.options(state_text):
        if "can't afford" in label:
            continue
        m = re.match(r"^(\d+)g (card|relic|potion) (.+?)(?:\(|:)", label)
        if m:
            price, what, name = int(m.group(1)), m.group(2), m.group(3).strip()
            cid, up = _card_id(name) if what == "card" else (_ident(name), 0)
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
            for cid in sorted({c["id"] for c in st.deck if c["id"] != "ASCENDERS_BANE"}):
                def rm(s, _d, cid=cid, p=price):
                    s.deck.remove(next(c for c in s.deck if c["id"] == cid))
                    s.gold -= p
                    s.removals += 1
                out.append((f"remove {_short(cid)}", "remove", price, rm))
    return out


def bundles(st, items, predictor, keep=6, removals=2, top=8, shuffles=4):
    """The shop as a budget problem: "nothing" (the gold carries to the next shops) and the `top` affordable bundles of up to 3 purchases (total
    price <= gold, one removal per visit, potions within the free slots), chosen by a quick screen: the worth of the deck after the bundle against
    the act's reference fights (`runmodel.reference_fights`), each bundle screened as a whole (a set can be positive while each part alone is not).
    Every single and every pair is screened (removals: the best `removals` cards only); triples are built from the `keep` purchases that did best
    alone or in a pair. Returns ([(label, first)], note)."""
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


def price(st, opts, predictor, n=128, seed=0, shuffles=4):
    """Rollouts per option with shared seeds: dict label -> arrays (win run, cleared this act, floors)."""
    ro = R.Rollouts(predictor, shuffles)
    states, seeds, firsts, owner = [], [], [], []
    for oi, (_label, first) in enumerate(opts):
        for j in range(n):
            states.append(st.copy())
            seeds.append(seed * 100_003 + j)  # the option never enters the seed: paired draws
            firsts.append(first)
            owner.append(oi)
    won = ro.run(states, seeds, firsts=firsts)
    owner = np.array(owner)
    res = {}
    for oi, (label, _f) in enumerate(opts):
        sel = owner == oi
        ss = [s for s, o in zip(states, owner) if o == oi]
        cleared = np.array([1.0 if (s.end is None or s.end[0] > st.act or s.end[1] == "won") else 0.0 for s in ss])
        res[label] = dict(win=won[sel], act=cleared, floors=np.array([s.floors for s in ss], float))
    return res


def table(res):
    labels = list(res)
    keys = (("win", "P(win run)"), ("act", "P(clear act)"), ("floors", "floors"))
    best = max(labels, key=lambda lb: tuple(res[lb][k].mean() for k, _ in keys))
    W = min(64, max(34, *(len(lb) for lb in labels)))
    lines = [f"{'option':{W}s} " + "  ".join(f"{t:>18s}" for _, t in keys) + "   vs best (paired)"]
    for lb in sorted(labels, key=lambda lb: tuple(-res[lb][k].mean() for k, _ in keys)):
        cells = []
        for k, _ in keys:
            x = res[lb][k]
            cells.append(f"{x.mean():8.3f} ±{x.std(ddof=1) / len(x) ** 0.5:.3f}".rjust(18))
        d = res[lb]["act"] - res[best]["act"]
        dfl = res[lb]["floors"] - res[best]["floors"]
        vs = "" if lb == best else f"   act {d.mean():+.3f} ±{d.std(ddof=1) / len(d) ** 0.5:.3f}, floors {dfl.mean():+.1f} ±{dfl.std(ddof=1) / len(dfl) ** 0.5:.1f}"
        lines.append(f"{lb[:W]:{W}s} " + "  ".join(cells) + vs)
    return "\n".join(lines)
