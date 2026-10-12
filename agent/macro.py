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
            exact = isinstance(c, dict) or c.endswith("+")
            if d["id"] == want["id"] and (not exact or d.get("upgrade", 0) == want.get("upgrade", 0)):
                deck.pop(i)
                break
    for c in v.get("upgrade", []):
        for d in deck:
            if d["id"] == _card(c)["id"] and d.get("upgrade", 0) == 0:
                d["upgrade"] = 1
                break
    for e in v.get("enchant", []):
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
        s["max_potion_slots"] = max(s.get("max_potion_slots", 2), len(v["potions"]))
    if "hp" in v:
        s["hp"] = int(v["hp"])
    return s


PANTOGRAPH_HEAL = 25


def start_hp(sc):
    """HP the fight starts at after start-of-combat relic heals: the sim applies them itself, so this is for reporting losses, never a scenario's hp"""
    hp = int(sc["hp"])
    if str(sc.get("encounter", "")).endswith("_BOSS") and hp > 0 and any((r if isinstance(r, str) else r["id"]) == "PANTOGRAPH" for r in sc.get("relics", [])):
        hp = min(int(sc["max_hp"]), hp + PANTOGRAPH_HEAL)
    return hp


def narrow(ids, kind, ctx):
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
    if seen:
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


def _pooled_se(ses):
    ses = list(ses)
    return (sum(s ** 2 for s in ses) ** 0.5) / len(ses)


def _vs(summary, vi, key="win"):
    d = summary[vi][key] - summary[0][key]
    if key == "win" and summary[vi].get("dse") is not None:
        return d, summary[vi]["dse"]
    return d, (summary[vi]["se"] ** 2 + summary[0]["se"] ** 2) ** 0.5


def loggable(summary):
    return {vi: {k: v for k, v in s.items() if k != "diffs"} for vi, s in summary.items()}


def _paired_se(diffs):
    ses = []
    for x in diffs.values():
        x = np.asarray(x, float)
        x = x[~np.isnan(x)]
        if len(x) < 2:
            return None
        ses.append(x.std(ddof=1) / len(x) ** 0.5)
    return _pooled_se(ses) if ses else None


def _diffs(rows, rows0):
    out = {}
    for (e, r), (e0, r0) in zip(rows, rows0):
        if e != e0 or r.get("wins") is None or r0.get("wins") is None or len(r["wins"]) != len(r0["wins"]):
            return None
        out[e] = [np.nan if a is None or b is None else a - b for a, b in zip(r["wins"], r0["wins"])]
    return out


def _smooth_parts(deck_json, spec):
    hp = spec.get("hp", "current")
    h0 = deck_json["max_hp"] if hp == "full" else (deck_json["hp"] if hp == "current" else hp)
    sub = dict(spec, smooth=False, hp="current")
    return h0, [(dict(deck_json, hp=int(round(h0 * m)), max_hp=max(deck_json["max_hp"], int(round(h0 * m)))), sub) for m in SMOOTH_MULTS]


def _smooth_summary(spec, h0, parts):
    variants = spec["variants"]
    summary, lines = {}, [f"smooth objective: win rate averaged over start HP x{'/'.join(str(m) for m in SMOOTH_MULTS)} of {h0}  ({len(variants)} variants)"]
    for vi, v in enumerate(variants):
        wins = [p[1][vi]["win"] for p in parts]
        se = _pooled_se(p[1][vi]["se"] for p in parts)
        per = {e: sum(p[1][vi]["per"][e] for p in parts) / len(parts) for e in parts[0][1][vi]["per"]}
        dl = [p[1][vi].get("diffs") for p in parts]
        diffs = {e: np.mean([d[e] for d in dl], axis=0) for e in dl[0]} if all(d is not None for d in dl) else None
        summary[vi] = dict(win=sum(wins) / len(wins), se=se, diffs=diffs, dse=_paired_se(diffs) if vi and diffs is not None else None, hp_lost=sum(p[1][vi]["hp_lost"] for p in parts) / len(parts), by_hp=wins, per=per)
        lines.append(f"{v.get('name', vi):24s} smooth {summary[vi]['win']:.3f} ±{se:.3f}  | " + " ".join(f"x{m}:{w:.2f}" for m, w in zip(SMOOTH_MULTS, wins)))
    for vi in range(1, len(variants)):
        d, sd = _vs(summary, vi)
        lines.append(f"  {variants[vi].get('name', vi)} vs {variants[0].get('name', 0)}: smooth {d:+.3f} (±{sd:.3f}{' paired' if summary[vi]['dse'] is not None else ''})")
    return "\n".join(lines), summary


def _build(deck_json, spec):
    base = dict(deck_json)
    hp = spec.get("hp", "current")
    if hp == "full":
        base["hp"] = base["max_hp"]
    elif isinstance(hp, int):
        base["hp"] = hp
    encs = resolve_encounters(spec)
    hold = spec.get("hold", ())
    drop_all = hold == "all"
    hold = set() if drop_all else set(hold)
    scen, index = [], []
    for vi, v in enumerate(spec["variants"]):
        sv = apply_variant(base, v)
        for e in encs:
            sc = dict(sv, name=f"{v.get('name', vi)}@{e}", encounter=e, seed=f"macro{encs.index(e)}")
            if (hold or drop_all) and not e.endswith("_BOSS"):
                sc["potions"] = [] if drop_all else [p for p in sc.get("potions", []) if p["id"] not in hold]
            scen.append(sc)
            index.append((vi, e))
    return base, encs, scen, index


def _summary(spec, base, encs, index, res):
    variants = spec["variants"]
    by = {}
    for (vi, e), r in zip(index, res):
        by.setdefault(vi, []).append((e, r))
    lines = [f"{len(variants)} variants x {len(encs)} encounters x {spec.get('attempts', 64)} attempts, start HP {base['hp']}/{base['max_hp']}"]
    summary = {}
    for vi, v in enumerate(variants):
        rows = by[vi]
        win = sum(r["win"] for _, r in rows) / len(rows)
        se = _pooled_se(r["win_se"] for _, r in rows)
        hpl = sum((r["hp_lost"] or 0) for _, r in rows) / len(rows)
        sv = apply_variant(base, v)
        lost = np.concatenate([start_hp(dict(sv, encounter=e)) - np.array(r["ends"]) for e, r in rows if r.get("ends")]) if any(r.get("ends") for _, r in rows) else np.zeros(1)
        lq = [float(x) for x in np.percentile(lost, [10, 50, 90, 97.5])]
        diffs = _diffs(rows, by[0])
        summary[vi] = dict(win=win, se=se, hp_lost=hpl, lost_q=lq, per={e: r["win"] for e, r in rows},
                           diffs=diffs, dse=_paired_se(diffs) if vi and diffs is not None else None)
        lines.append(f"{v.get('name', vi):24s} win {win:.3f} ±{se:.3f}  HP lost {100 * hpl:4.1f}% (q10/50/90/97.5: {lq[0]:.0f}/{lq[1]:.0f}/{lq[2]:.0f}/{lq[3]:.0f} HP)  | " + " ".join(f"{e.split('_')[0][:8]}:{r['win']:.2f}" for e, r in rows))
    for vi in range(1, len(variants)):
        d, sd = _vs(summary, vi)
        lines.append(f"  {variants[vi].get('name', vi)} vs {variants[0].get('name', 0)}: win {d:+.3f} (±{sd:.3f}{' paired' if summary[vi]['dse'] is not None else ''}), HP lost {100 * (summary[vi]['hp_lost'] - summary[0]['hp_lost']):+.1f} pts")
    return "\n".join(lines), summary


def evaluate_many(engine, items):
    """[(deck_json, spec)] -> [(text, summary)], all in ONE batch search (smooth specs expand to their HP multiples); each scenario keeps the
    attempts and the common-random-number group it has in a call of its own"""
    flat = []
    for deck_json, spec in items:
        if spec.get("smooth"):
            h0, parts = _smooth_parts(deck_json, spec)
            flat.append(("smooth", spec, h0, [_build(d, s) + (s,) for d, s in parts]))
        else:
            flat.append(("plain", spec, None, [_build(deck_json, spec) + (spec,)]))
    scen, att, grp, spans = [], [], [], []
    for _kind, _spec, _h0, builds in flat:
        for base, encs, sc, index, s in builds:
            spans.append((len(scen), len(sc)))
            scen += sc
            att += [s.get("attempts", 64)] * len(sc)
            grp += [encs.index(e) for _, e in index]

    def summarize(res):
        out, k = [], 0
        for kind, spec, h0, builds in flat:
            parts = []
            for base, encs, sc, index, s in builds:
                a, n = spans[k]
                k += 1
                parts.append(_summary(s, base, encs, index, res[a:a + n]))
            out.append(_smooth_summary(spec, h0, parts) if kind == "smooth" else parts[0])
        return out
    if not scen:
        return summarize([])
    return summarize(engine.solve(scen, attempts=att, groups=grp))


def evaluate(engine, deck_json, spec):
    return evaluate_many(engine, [(deck_json, spec)])[0]


_CARD_IDS = None


def card_id_set():
    global _CARD_IDS
    if _CARD_IDS is None:
        import os
        import re
        with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "sts2sim", "src", "content", "gen_cards.rs"), encoding="utf-8") as f:
            src = f.read()
        _CARD_IDS = set(re.findall(r"CardDef::new\(ids::card::([A-Z0-9_]+)", src))
    return _CARD_IDS


def card_from_name(name):
    import re
    up = 1 if name.strip().endswith("+") else 0
    cid = re.sub(r"[^A-Z0-9]+", "_", name.strip().rstrip("+").upper().replace("'", "")).strip("_")
    return (cid if cid in card_id_set() else None), up


def parse_card_options(state):
    import re
    from agent import screen
    opts, skip = [], None
    for num, label in screen.options(state):
        m = re.match(r"^(.+?)\(([^()]*)\) ", label)
        if m:
            cid, up = card_from_name(m.group(1))
            opts.append((int(num), m.group(1).strip(), cid, up))
        elif label.startswith("Skip"):
            skip = int(num)
    return opts, skip


def brief_text(state, deck_json, hz):
    import collections
    from agent import card_tags, screen
    head = screen.header_line(state, state.split("\n")[0])
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
