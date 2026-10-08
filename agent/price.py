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
    return R.RunState(deck, ctx.act, act_name, deck["hp"], deck["max_hp"], deck.get("gold", 0), deck["deck"], relics, pots,
                      deck.get("max_potion_slots", 2), (t.potion, t.offset, dict(t.unknown), t.removals), seen, ctx.bosses, frontier, nodes or None,
                      len(seen.get("weak", [])) + len(seen.get("regular", [])))


def _card_id(name):
    from agent import macro
    return macro.card_from_name(name)


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
            for label, o in zip(labels, EV.match(title, labels)):
                if o is not None and not o["key"].endswith("_LOCKED"):
                    idx = entry["options"].index(o)
                    out.append((label.split(":", 1)[0][:34], lambda s, d, i=idx, eid=entry["id"]: EV.play_option(s, eid, i, d)))
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


def shop_items(st, state_text, unpriced=None):
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


def price(st, opts, predictor, n=128, seed=0, shuffles=4, cont=None):
    ro = R.Rollouts(predictor, shuffles)
    pol = R.BasePolicy()
    pol.gates = cont is not None
    states, seeds, firsts, owner = [], [], [], []
    for oi, (_label, first) in enumerate(opts):
        for j in range(n):
            states.append(st.copy())
            seeds.append(seed * 100_003 + j)
            firsts.append(first)
            owner.append(oi)
    won = ro.run(states, seeds, pol=pol, firsts=firsts)
    owner = np.array(owner)
    res = {}
    for oi, (label, _f) in enumerate(opts):
        sel = owner == oi
        ss = [s for s, o in zip(states, owner) if o == oi]
        cleared = np.array([1.0 if (s.end is None or s.end[0] > st.act or s.end[1] == "won") else 0.0 for s in ss])
        r = dict(win=won[sel], act=cleared)
        if st.act < 2:
            r["ready"] = np.array([0.0 if s.ready is None else s.ready for s in ss])
            r["ready_worth"] = np.array([-1.0 if s.ready_worth is None else s.ready_worth for s in ss])
        r["floors"] = np.array([s.floors for s in ss], float)
        if cont is not None:
            r["cont"] = np.array([arrival(s.gates, cont) if getattr(s, "gates", None) else 0.0 for s in ss])
        res[label] = r
    return res


def arrival(g, rule):
    """V at the current act's boss: its boss gate times every later gate (the current act's elites are behind)"""
    return R.combine([(g[0][0], 1.0, g[0][2])] + list(g[1:]), rule)


def closed_gates(st, opts, predictor, seeds=range(1000, 1008)):
    """gates() of each option's state right after it (no rollout; an event's fights are not played), one value per shuffle seed for paired se"""
    import random
    sts = []
    for _label, first in opts:
        s = st.copy()
        if first is not None:
            first(s, R.Draws(random.Random(0), st.base["character"], st.act))
        sts.append(s)
    pol = R.BasePolicy()
    return [R.Rollouts(predictor, 1, sd).drive([R.gates(s, pol) for s in sts]) for sd in seeds]


def gates_text(opts, G, rule):
    labels = [lb for lb, _ in opts]
    acts = [j for j, _, _ in G[0][0]]
    E = np.array([[[x[1] for x in g] for g in Gs] for Gs in G])
    B = np.array([[[x[2] for x in g] for g in Gs] for Gs in G])
    V = np.array([[R.combine(g, rule) for g in Gs] for Gs in G])
    best = int(V.mean(0).argmax())
    W = min(40, max(16, *(len(lb) for lb in labels)))
    head = "".join(f"{f'A{j + 1} elite':>9s}{f'A{j + 1} boss' + ('es' if j == R.LAST_ACT else ''):>11s}" for j in acts)
    lines = [f"gates on each option's deck right after it (closed form, no rollout, {len(G)} shuffle seeds; scores the current deck: "
             f"enablers that pay off after later picks are undervalued); V = {rule} combination",
             f"{'option':{W}s}{head}{'V':>10s}   vs best (paired over seeds)"]
    for i in np.argsort(-V.mean(0)):
        cells = "".join(f"{E[:, i, k].mean():9.3f}{B[:, i, k].mean():11.4f}" for k in range(len(acts)))
        d = V[:, i] - V[:, best]
        lines.append(f"{labels[i][:W]:{W}s}{cells}{V[:, i].mean():10.5f}" + ("" if i == best else f"   {d.mean():+.5f} ±{_se(d):.5f}"))
    return "\n".join(lines)


HORIZONS = (("cont", "V gates"), ("win", "P(win run)"), ("act", "P(clear act)"), ("ready", "next act ready"), ("floors", "floors"))
ACT_SATURATED = 0.9


def _se(x):
    return float(x.std(ddof=1) / len(x) ** 0.5) if len(x) > 1 else float("nan")


def separates(res, k, z=2.0):
    labels = list(res)
    best = max(labels, key=lambda lb: res[lb][k].mean())
    for lb in labels:
        d = res[best][k] - res[lb][k]
        if lb != best and d.mean() > 0 and d.mean() > z * _se(d):
            return True
    return False


def ladder(res, saturated=ACT_SATURATED):
    if all("cont" in r for r in res.values()):
        return "cont", "rollouts to this act's boss, then its boss gate x every later act's elite and boss gates on the deck at arrival (S5 surrogate)"
    if separates(res, "win"):
        return "win", "P(win run) separates the options (> 2 paired se)"
    best = max(r["act"].mean() for r in res.values())
    if best < saturated:
        return "act", f"P(win run) flat; P(clear act) not saturated (best {best:.3f} < {saturated:g})"
    if separates(res, "act"):
        return "act", f"P(win run) flat; P(clear act) separates the options (> 2 paired se) at best {best:.3f}"
    why = f"P(win run) flat; P(clear act) saturated (best {best:.3f} >= {saturated:g}, within 2 paired se)"
    if all("ready" in r for r in res.values()):
        return "ready", why
    return "floors", why + "; next-act readiness not estimable in the last act"


def table(res, saturated=ACT_SATURATED):
    labels = list(res)
    keys = [(k, t) for k, t in HORIZONS if all(k in r for r in res.values())]
    main, why = ladder(res, saturated)
    order = [main] + [k for k, _ in keys if k != main]
    best = max(labels, key=lambda lb: tuple(res[lb][k].mean() for k in order))
    W = min(64, max(34, *(len(lb) for lb in labels)))
    lines = [f"ranked by: {dict(HORIZONS)[main]} ({why})",
             f"{'option':{W}s} " + "  ".join(f"{('*' if k == main else '') + t:>16s}" for k, t in keys) + "   vs best (paired)"]
    for lb in sorted(labels, key=lambda lb: tuple(-res[lb][k].mean() for k in order)):
        cells = []
        for k, _ in keys:
            x = res[lb][k]
            nd = {"floors": 1, "cont": 5}.get(k, 3)
            cells.append(f"{x.mean():.{nd}f} ±{_se(x):.{nd}f}".rjust(16))
        vs = ""
        if lb != best:
            parts = []
            for k, _ in keys:
                d = res[lb][k] - res[best][k]
                nd = {"floors": 1, "cont": 5}.get(k, 3)
                parts.append(f"{'*' if k == main else ''}{k} {d.mean():+.{nd}f} ±{_se(d):.{nd}f}")
            vs = "   " + ", ".join(parts)
        lines.append(f"{lb[:W]:{W}s} " + "  ".join(cells) + vs)
    return "\n".join(lines)


HEADER = re.compile(r"A(\d+) F(\d+) \w+ A\d+ HP (\d+)/(\d+) G(\d+)")


def recorded_screens(events_path, kinds=("CARD_REWARD", "RESTSITE")):
    """(floor, state text, RunState, recorded choice label, old calculator's best label or None) per decision screen of a recorded run;
    deck = the last fight's deck plus the card picks since, the rest of the act from the template (no map)"""
    import json
    sc, picks, seen, bosses, old, act = None, [], [], [], None, None
    for raw in open(events_path, encoding="utf-8"):
        e = json.loads(raw)
        k = e.get("kind")
        if k == "fight_start":
            sc, picks = e.get("scenario") or sc, []
            if sc and sc.get("act") != act:
                act, seen, bosses = sc.get("act"), [], []
            seen.append(e["encounter"])
        elif k == "reward_eval":
            old = e
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
            st = R.RunState(sc, act, name, int(m.group(3)), int(m.group(4)), int(m.group(5)), deck, relics, [p for p in belt if p in _ids("potions")],
                            sc.get("max_potion_slots", 2), (tracker.POTION_START, tracker.OFFSET_START, dict(tracker.UNKNOWN_BASE), 0), seen_kind,
                            bosses, None, None, len(seen_kind.get("weak", [])) + len(seen_kind.get("regular", [])))
            choice = labels.get(e.get("choice", "").split(" ")[0], "")
            best_old = None
            if e["screen"] == "CARD_REWARD" and old is not None:
                boss = old["result"].get("boss", {})
                if boss:
                    i = max(boss, key=lambda vi: boss[vi]["win"])
                    best_old = "skip" if i == "0" else old["options"][int(i) - 1]
            yield int(m.group(2)), state, st, choice, best_old
            if e["screen"] == "CARD_REWARD":
                cm = re.match(r"^(.+?)\(", choice)
                cid, up = _card_id(cm.group(1)) if cm else (None, 0)
                if cid:
                    picks.append({"id": cid, "upgrade": up})
                old = None


def best(res, saturated=ACT_SATURATED):
    main, _ = ladder(res, saturated)
    order = [main] + [k for k, _ in HORIZONS if k != main and all(k in r for r in res.values())]
    return max(res, key=lambda lb: tuple(res[lb][k].mean() for k in order)), main


def _played(state, choice, opts):
    if scr.kind(state) == "RESTSITE":
        return "rest" if choice.lower().startswith("rest") else "smith"
    name = choice.split("(")[0].strip().lower()
    return next((lb for lb, _ in opts if lb.lower() == name), "skip" if name.startswith("skip") else name)


def _family(label):
    return "smith" if label.startswith("smith") else label


def replay(run, n=128, cont="late", kinds=("CARD_REWARD", "RESTSITE")):
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    pred = Predictor(PREDICTOR_CKPT, batch=1024)
    path = run if os.path.isfile(run) else os.path.join(ROOT, "runs", run, "events.jsonl")
    rows = []
    for floor, state, st, choice, old in recorded_screens(path, kinds):
        opts = options(st, state)
        if len(opts) < 2:
            continue
        res = price(st, opts, pred, n=n, seed=floor, cont=cont)
        now, ranked = best({lb: {k: v for k, v in r.items() if k != "cont"} for lb, r in res.items()})
        surr, _ = best(res)
        V = np.array([[R.combine(g, cont) for g in Gs] for Gs in closed_gates(st, opts, pred)])
        closed = opts[int(V.mean(0).argmax())][0]
        row = dict(floor=floor, screen=scr.kind(state), played=_played(state, choice, opts), old=old, now=now, ranked=ranked, surrogate=surr,
                   sep=separates(res, "cont"), closed=closed, closed_sep=separates({lb: {"v": V[:, i]} for i, (lb, _) in enumerate(opts)}, "v"))
        rows.append(row)
        print(f"F{floor:<3d} {row['screen']:11s} {st.hp:3d}/{st.max_hp} | played {row['played'][:16]:16s} | old {str(old)[:16]:16s} | "
              f"price[{ranked}] {now[:16]:16s} | S5 rollout {surr[:16]:16s}{'' if row['sep'] else ' (flat)':7s} | S5 closed {closed[:16]}{'' if row['closed_sep'] else ' (flat)'}", flush=True)
    return rows


def main():
    import argparse
    ap = argparse.ArgumentParser(description="replay a recorded run's decision screens: current price ladder vs the S5 gate surrogate")
    ap.add_argument("run", help="runs/<run> name or an events.jsonl path")
    ap.add_argument("--n", type=int, default=128)
    ap.add_argument("--cont", default="late", choices=R.RULES)
    ap.add_argument("--screens", default="CARD_REWARD,RESTSITE")
    a = ap.parse_args()
    rows = replay(a.run, a.n, a.cont, tuple(a.screens.split(",")))
    if not rows:
        return print("no decision screens with two or more priced options")

    def rate(x, y):
        pairs = [(_family(r[x]), _family(r[y])) for r in rows if r[x] is not None and r[y] is not None]
        return f"{np.mean([p == q for p, q in pairs]):.2f} of {len(pairs)}" if pairs else "n/a"
    print(f"{len(rows)} screens; S5 rollout+gates vs price {rate('surrogate', 'now')}, vs S5 closed {rate('surrogate', 'closed')}, "
          f"vs played {rate('surrogate', 'played')}, vs old calc {rate('surrogate', 'old')}; price vs played {rate('now', 'played')}, "
          f"vs old calc {rate('now', 'old')}; S5 closed vs played {rate('closed', 'played')}; separated by > 2 paired se: S5 rollout "
          f"{np.mean([r['sep'] for r in rows]):.2f}, S5 closed {np.mean([r['closed_sep'] for r in rows]):.2f}")


if __name__ == "__main__":
    main()
