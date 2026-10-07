"""`price`: every option of the current screen priced by paired rollouts of the run model (`agent/runmodel.py`, `docs/rebuild.md` S5).

One currency at three horizons, longest first: P(win the run), P(clear this act), floors reached. A weak deck has P(win run) ~ 0 for every option;
the act horizon then still separates them, and the floors after it. Options per screen:
  MAP          each node on offer
  CARD_REWARD  each card and skip
  RESTSITE     rest, and smith of each upgradable card
  SHOP         nothing, each affordable card / relic / potion, the removal of each distinct card
  EVENT        each option of a catalogued event (`data/events.json`) or an ancient (`data/ancient_relics.json`: the relic plus its pickup effects); an unmodelled effect is a no-op
Each option meets the same draws (common random numbers); the table prints each option's mean with its se and the paired difference to the best.
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
    elif kind == "SHOP":
        out.append(("nothing", None))
        for _, label in scr.options(state_text):
            m = re.match(r"^(\d+)g (card|relic|potion) (.+?)(?:\(|:)", label)
            if m and "can't afford" not in label:
                price, what, name = int(m.group(1)), m.group(2), m.group(3).strip()
                ident = re.sub(r"[^A-Z0-9]+", "_", name.upper()).strip("_")
                if what == "card":
                    cid, up = _card_id(name)
                    if cid:
                        out.append((f"{name} ({price}g)", lambda s, _d, c=cid, u=up, p=price: (s.deck.append({"id": c, "upgrade": u}), setattr(s, "gold", s.gold - p))))
                elif what == "relic":
                    out.append((f"{name} ({price}g)", lambda s, _d, r=ident, p=price: (s.relics.append(r), setattr(s, "gold", s.gold - p))))
                elif len(st.potions) < st.slots:
                    out.append((f"{name} ({price}g)", lambda s, _d, q=ident, p=price: (s.potions.append(q), setattr(s, "gold", s.gold - p))))
            m = re.match(r"^(\d+)g remove a card", label)
            if m and "can't afford" not in label:
                price = int(m.group(1))
                for cid in sorted({c["id"] for c in st.deck if c["id"] != "ASCENDERS_BANE"}):
                    def rm(s, _d, cid=cid, p=price):
                        s.deck.remove(next(c for c in s.deck if c["id"] == cid))
                        s.gold -= p
                        s.removals += 1
                    out.append((f"remove {cid} ({price}g)", rm))
    return out


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
    lines = [f"{'option':34s} " + "  ".join(f"{t:>18s}" for _, t in keys) + "   vs best (paired)"]
    for lb in sorted(labels, key=lambda lb: tuple(-res[lb][k].mean() for k, _ in keys)):
        cells = []
        for k, _ in keys:
            x = res[lb][k]
            cells.append(f"{x.mean():8.3f} ±{x.std(ddof=1) / len(x) ** 0.5:.3f}".rjust(18))
        d = res[lb]["act"] - res[best]["act"]
        dfl = res[lb]["floors"] - res[best]["floors"]
        vs = "" if lb == best else f"   act {d.mean():+.3f} ±{d.std(ddof=1) / len(d) ** 0.5:.3f}, floors {dfl.mean():+.1f} ±{dfl.std(ddof=1) / len(dfl) ** 0.5:.1f}"
        lines.append(f"{lb[:34]:34s} " + "  ".join(cells) + vs)
    return "\n".join(lines)
