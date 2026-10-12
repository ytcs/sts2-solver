"""Plan-aware pick terms from data/synergy_candidates_<char>.json (character inferred from the owned ids). A bundle is live when the deck + relics hold
>= 1 provider and >= 1 consumer of its mechanics; an addition's plan weight is the count of other core pieces owned (copies counted) in its live bundles;
anti = an anti pair between the addition and an owned item; indirect = A + B need one of C (bridge, source or payoff); other-pool cards carry pool,
cross pairs and unfueled mechanics."""
import collections
import functools
import json
import os

_DATA = os.path.join(os.path.dirname(__file__), "..", "data")
CHARS = ("ironclad", "silent", "defect", "necrobinder", "regent")


@functools.lru_cache(maxsize=None)
def _db(char):
    d = json.load(open(os.path.join(_DATA, f"synergy_candidates_{char}.json"), encoding="utf-8"))
    tags = {k: (frozenset(v.get("provides", ())), frozenset(v.get("consumes", ()))) for k, v in d["tags"].items() if "mp_only" not in v.get("provides", ())}
    cost = {k: v["cost"] for k, v in d["tags"].items() if isinstance(v.get("cost"), int)}
    anti, ind, direct = {}, collections.defaultdict(list), collections.defaultdict(set)
    for p in d["pairs"]:
        if p["kind"] == "direct":
            direct[p["a"]].add(p["b"]), direct[p["b"]].add(p["a"])
        elif p["kind"] == "anti":
            anti.setdefault(p["a"], set()).add(p["b"])
            anti.setdefault(p["b"], set()).add(p["a"])
        elif p["kind"] == "indirect":
            row = (p["a"], p["b"], p["via"], p.get("role", "bridge"), frozenset(p["needs"]))
            for x in {p["a"], p["b"]} | row[4]:
                ind[x].append(row)
    bundles = [(b["name"], frozenset(b["mechanics"])) for b in d["bundles"]]
    core = {b["name"]: frozenset(b["core"]) for b in d["bundles"]}
    support = {b["name"]: frozenset(b["support"]) for b in d["bundles"]}
    cross = {k: (v["pool"], v.get("unfueled", {})) for k, v in d["tags"].items() if v.get("pool")}
    return tags, anti, bundles, core, support, cost, ind, direct, cross


@functools.lru_cache(maxsize=1)
def _owner():
    cat = json.load(open(os.path.join(_DATA, "catalog.json"), encoding="utf-8"))
    return {x["id"]: c.lower() for k in ("cards", "relics") for c, xs in cat[k].items() if c.lower() in CHARS for x in xs}


def _base(i):
    return str(i).rstrip("+").upper()


def character(ids):
    """the character whose cards / relics dominate the owned ids (ironclad when none)"""
    n = collections.Counter(_owner().get(_base(i)) for i in ids)
    n.pop(None, None)
    return n.most_common(1)[0][0] if n else "ironclad"


def live(ids, char=None):
    """{bundle name: (providers, consumers)} with both sides present"""
    tags, _, bundles = _db(char or character(ids))[:3]
    ids = {_base(i) for i in ids}
    out = {}
    for name, mech in bundles:
        pro = {i for i in ids if i in tags and tags[i][0] & mech}
        con = {i for i in ids if i in tags and tags[i][1] & mech}
        if pro and con and pro != con:
            out[name] = (pro, con)
    return out


def overlap(item, ids, char=None):
    """[(bundle, role of the item, other core pieces owned, support pieces owned, live)] for the bundles the item belongs to, most owned first"""
    char = char or character(ids)
    core, support = _db(char)[3:5]
    item = _base(item)
    own = collections.Counter(_base(i) for i in ids)
    own.pop(item, None)
    lv = live(set(own) | {item}, char)
    rows = [(n, "core" if item in core[n] else "support", sum(own[i] for i in core[n]), sum(own[i] for i in support[n]), n in lv)
            for n in core if item in core[n] or item in support[n]]
    return sorted(rows, key=lambda r: (-r[2], -r[3]))


def in_plan(item, ids):
    """other core pieces owned in the item's live bundles where it is core (max over bundles)"""
    return max((r[2] for r in overlap(item, ids) if r[1] == "core" and r[4]), default=0)


def indirect(item, ids, char=None, k=3):
    """item's indirect triples with owned pieces: ('completes', A, B, via) when it is a missing piece of an owned pair, ('needs', A, via, [C]) when it pairs
    with owned A and no C is owned; completes first, at most k of each"""
    item, own = _base(item), {_base(i) for i in ids}
    done, need = [], []
    for a, b, via, role, needs in _db(char or character(ids))[6].get(item, ()):
        if item in needs and a in own and b in own:
            done.append(("completes", a, b, via))
        elif item in (a, b) and (b if item == a else a) in own and not needs & own:
            need.append(("needs", b if item == a else a, via, sorted(needs)))
    return list(dict.fromkeys(done))[:k] + sorted(set((r[0], r[1], r[2], tuple(r[3])) for r in need), key=lambda r: len(r[3]))[:k]


def cross(item, ids, char=None):
    """other-pool item: (pool, direct partners owned in the host table's cross pairs and its own pool's table, {unfueled mechanic: same-pool fuel}) or None"""
    char, item = char or character(ids), _base(item)
    c = _db(char)[8].get(item)
    if not c:
        return None
    own = {_base(i) for i in ids}
    return c[0], sorted((_db(char)[7].get(item, set()) | _db(c[0].lower())[7].get(item, set())) & own), c[1]


def lines(items, ids):
    """operator view, one line per offered item: bundles with owned core/support counts, indirect completions / missing pieces, other-pool partners and
    unfueled mechanics, anti partners owned"""
    char, out = character(ids), []
    for it in items:
        ov = [f"{n} {r} ({c} core, {sp} support owned{'' if lv else ', not live'})" for n, r, c, sp, lv in overlap(it, ids, char) if c or sp]
        ov += [f"completes {r[1]} + {r[2]} ({r[3]})" if r[0] == "completes" else
               f"with {r[1]} needs one of {', '.join(r[3][:4])}{' ...' if len(r[3]) > 4 else ''} ({r[2]})" for r in indirect(it, ids, char)]
        cx = cross(it, ids, char)
        if cx:
            ov.append(f"other pool {cx[0]}" + (f", direct with {', '.join(cx[1][:5])}" if cx[1] else "") + "".join(
                f", UNFUELED {m} (no {char} source; {('fuel: ' + ', '.join(f[:3])) if f else 'no fuel in its pool'})" for m, f in cx[2].items()))
        an = anti(it, ids, char)
        if ov or an:
            out.append(f"  {_base(it)}: " + "; ".join(ov) + (f"{'; ' if ov else ''}ANTI with {', '.join(an)}" if an else ""))
    return out


def anti(item, ids, char=None):
    a = _db(char or character(ids))[1]
    item = _base(item)
    return sorted(a.get(item, set()) & {_base(i) for i in ids})


def item_tags(item, char=None):
    """(provides, consumes, energy cost or -1) of a card / relic id; the character's table, else the first table that has it"""
    item = _base(item)
    for c in [char] if char else [_owner().get(item, "ironclad")] + list(CHARS):
        tags, cost = _db(c)[0], _db(c)[5]
        if item in tags or item in cost:
            p, cs = tags.get(item, (frozenset(), frozenset()))
            return p, cs, cost.get(item, -1)
    return frozenset(), frozenset(), -1
