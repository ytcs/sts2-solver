"""Plan-aware pick terms from data/synergy_candidates.json (plan item 4). A bundle is live when the deck + relics hold >= 1 provider and
>= 1 consumer of its mechanics; an addition's plan weight is the count of other core pieces owned (copies counted) in its live bundles;
anti = an anti pair between the addition and an owned item."""
import collections
import functools
import json
import os

_PATH = os.path.join(os.path.dirname(__file__), "..", "data", "synergy_candidates.json")


@functools.lru_cache(maxsize=1)
def _db():
    d = json.load(open(_PATH, encoding="utf-8"))
    tags = {k: (frozenset(v.get("provides", ())), frozenset(v.get("consumes", ()))) for k, v in d["tags"].items() if "mp_only" not in v.get("provides", ())}
    anti = {}
    for p in d["pairs"]:
        if p["kind"] == "anti":
            anti.setdefault(p["a"], set()).add(p["b"])
            anti.setdefault(p["b"], set()).add(p["a"])
    bundles = [(b["name"], frozenset(b["mechanics"])) for b in d["bundles"]]
    core = {b["name"]: frozenset(b["core"]) for b in d["bundles"]}
    support = {b["name"]: frozenset(b["support"]) for b in d["bundles"]}
    return tags, anti, bundles, core, support


def _base(i):
    return str(i).rstrip("+").upper()


def live(ids):
    """{bundle name: (providers, consumers)} with both sides present"""
    tags, _, bundles, _, _ = _db()
    ids = {_base(i) for i in ids}
    out = {}
    for name, mech in bundles:
        pro = {i for i in ids if i in tags and tags[i][0] & mech}
        con = {i for i in ids if i in tags and tags[i][1] & mech}
        if pro and con and pro != con:
            out[name] = (pro, con)
    return out


def overlap(item, ids):
    """[(bundle, role of the item, other core pieces owned, support pieces owned, live)] for the bundles the item belongs to, most owned first"""
    _, _, _, core, support = _db()
    item = _base(item)
    own = collections.Counter(_base(i) for i in ids)
    own.pop(item, None)
    lv = live(set(own) | {item})
    rows = [(n, "core" if item in core[n] else "support", sum(own[i] for i in core[n]), sum(own[i] for i in support[n]), n in lv)
            for n in core if item in core[n] or item in support[n]]
    return sorted(rows, key=lambda r: (-r[2], -r[3]))


def in_plan(item, ids):
    """other core pieces owned in the item's live bundles where it is core (max over bundles)"""
    return max((r[2] for r in overlap(item, ids) if r[1] == "core" and r[4]), default=0)


def lines(items, ids):
    """operator view, one line per offered item: bundles with owned core/support counts, anti partners owned"""
    out = []
    for it in items:
        ov = [f"{n} {r} ({c} core, {sp} support owned{'' if lv else ', not live'})" for n, r, c, sp, lv in overlap(it, ids) if c or sp]
        an = anti(it, ids)
        if ov or an:
            out.append(f"  {_base(it)}: " + "; ".join(ov) + (f"{'; ' if ov else ''}ANTI with {', '.join(an)}" if an else ""))
    return out


def anti(item, ids):
    _, a, _, _, _ = _db()
    item = _base(item)
    return sorted(a.get(item, set()) & {_base(i) for i in ids})
