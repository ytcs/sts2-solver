"""Loose synergy candidates (plan item 4, first pass): tags, pairs, bundles in data/synergy_candidates.json.

    synergy.py ID                  partners by kind
    synergy.py --deck ID,ID,...    bundle commitment, missing core, anti-synergy present (cards and relics, '+' ignored)
    synergy.py --build             re-derive auto tags, src, pairs, bundles from tags + rules (needs decomp/)
"""
import collections
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PATH = os.path.join(ROOT, "data", "synergy_candidates.json")
PAT = {
    "strength_gain": r"Apply<StrengthPower>|PowerVar<StrengthPower> || StrengthPower", "dexterity_gain": r"Apply<DexterityPower> || DexterityPower",
    "multi_hit": r"WithHitCount|HitCount", "vulnerable_apply": r"Apply<VulnerablePower> || VulnerablePower", "weak_apply": r"Apply<WeakPower> || WeakPower",
    "exhaust_enabler": r"CardCmd\.Exhaust || PileType\.Exhaust|Exhaust", "exhaust_self": r"CardKeyword\.Exhaust", "ethereal": r"Ethereal",
    "hp_loss_self": r"CreatureCmd\.Damage\([^,]*, base\.Owner|HpLossVar", "hp_loss_reduce": r"ModifyHpLost",
    "block_gain": r"GainBlock|GainsBlock|BlockVar|PlatingPower || Block", "block_retain": r"ShouldClearBlock|ShouldFlush || Block",
    "power_card": r"CardType\.Power", "draw": r"CardPileCmd\.Draw|ModifyHandDraw|CardsVar || Draw|ToHand", "energy_gain": r"GainEnergy|ModifyMaxEnergy|EnergyVar || Energy",
    "x_cost": r"HasEnergyCostX", "status_generator": r"AddCurseToDeck|Dazed|Wound|Burn|Soot|Injury|Enthralled|Guilty|Folly|Greed || Curse",
    "attack_volume": r"base\(0, CardType\.Attack|CardType\.Attack || Attack", "skill_volume": r"TryModifyEnergyCost|CardType\.Skill",
    "card_volume": r"AutoPlay|CardCmd\.Play|AddGeneratedCard|CreateCard || AfterCardPlayed", "autoplay": r"CardCmd\.AutoPlay|AutoPlayFromDrawPile|ModifyCardPlayCount|Replay || AutoPlay",
    "card_generation": r"CreateCard|AddGeneratedCard|CardFactory|Transform", "upgrade_gain": r"CardCmd\.Upgrade|ForUpgrade|Upgrade\(|Upgraded|Apotheosis",
    "strike": r"CardTag\.Strike || Strike", "defend": r"CardTag\.Defend || Defend", "retain": r"ShouldFlush|Retain",
    "potion_gain": r"PotionCmd|Procure|PotionSlot || Potion", "gold_gain": r"GainGold|Gold", "enchant": r"Enchant",
}
CPAT = {
    "strength_gain": r"WithHitCount|HitCount|StrengthPower|Strength || TotalDamage", "x_cost": r"ModifyXValue|CostsX", "hp_loss_self": r"AfterDamageReceived|UnblockedDamage|HpLost|LostHp|CurrentHp|DamageReceived",
    "exhaust_enabler": r"AfterCardExhausted|PileType\.Exhaust || Exhaust", "exhaust_self": r"AfterCardExhausted|PileType\.Exhaust || Exhaust",
    "ethereal": r"AfterCardExhausted || Exhaust", "status_generator": r"CardSelectCmd\.FromHand|CardCmd\.Exhaust || CardType\.Curse", "vulnerable_apply": r"VulnerablePower || Debuff",
    "weak_apply": r"WeakPower || Debuff", "multi_hit": r"DamageMinimum|DamageThreshold|BlockBroken|Block", "retain": r"Hand|Retain", "energy_gain": r"HasEnergyCostX",
    "block_gain": r"AfterBlockGained|ModifyBlock|GainsBlock|Block", "power_card": r"CardType\.Power", "attack_volume": r"CardType\.Attack || Attack",
    "card_volume": r"AfterCardPlayed|CardPlay", "upgrade_gain": r"IsUpgraded|Upgraded", "high_cost": r"EnergyValue|EnergyCost|ConfusedPower",
    "draw": r"AfterCardDrawn|Draw", "gold_gain": r"Gold",
}
SKIP = re.compile(r"^\s*(using |namespace |public sealed class|protected override void OnUpgrade|\{|\})")
SUB = {"Cards": "MegaCrit.Sts2.Core.Models.Cards", "Relics": "MegaCrit.Sts2.Core.Models.Relics", "Powers": "MegaCrit.Sts2.Core.Models.Powers"}


def load(path=PATH):
    return json.load(open(path, encoding="utf-8"))


def _decomp():
    d = ROOT
    while d != os.path.dirname(d):
        if os.path.isdir(os.path.join(d, "decomp", SUB["Cards"])):
            return os.path.join(d, "decomp")
        d = os.path.dirname(d)
    sys.exit("decomp/ not found above " + ROOT)


def _files(dec, kind, cid):
    main = ("Cards" if kind == "card" else "Relics") + "/" + "".join(w.capitalize() for w in cid.lower().split("_")) + ".cs"
    out = [main]
    for p in re.findall(r"<(\w+Power)>", _read(dec, main)):
        if "Powers/" + p + ".cs" not in out and os.path.exists(os.path.join(dec, SUB["Powers"], p + ".cs")):
            out.append("Powers/" + p + ".cs")
    return out


def _read(dec, rel):
    d, f = rel.split("/")
    return open(os.path.join(dec, SUB[d], f), encoding="utf-8").read()


def _find(dec, files, pat):
    for tier in pat.split(" || "):
        for rel in files:
            for i, line in enumerate(_read(dec, rel).splitlines()):
                if not SKIP.match(line) and re.search(tier, line):
                    return f"{rel}:{i + 1}"
    return "text"


def _sel(tags, s):
    if isinstance(s, list):
        return set(s)
    if ":" not in s:
        return {s} if s != "cost0" else {i for i, t in tags.items() if t.get("cost") == 0 and "x_cost" not in t["provides"]}
    field, m = s.split(":")
    if field == "converts_to":
        return {i for i, t in tags.items() if any(c.endswith(">" + m) for c in t.get("converts", []))}
    return {i for i, t in tags.items() if m in t[field]}


def build():
    dec, db = _decomp(), load()
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json"), encoding="utf-8"))
    scope = [(c, "card") for p in ("IRONCLAD", "COLORLESS") for c in cat["cards"][p]] + [(r, "relic") for p in ("IRONCLAD", "SHARED", "EVENT") for r in cat["relics"][p]]
    tags = {}
    for c, kind in scope:
        cid = c["id"]
        t = db["tags"].get(cid, {})
        t = dict(kind=kind, provides=list(t.get("provides", [])), consumes=list(t.get("consumes", [])), **{k: t[k] for k in ("converts",) if t.get(k)})
        files, src = _files(dec, kind, cid), {}
        text = _read(dec, files[0])
        if kind == "card":
            t["cost"] = c["cost"]
            kw = re.search(r"CanonicalKeywords =>(.*?);", text, re.S)
            tg = re.search(r"CanonicalTags =>(.*?);", text, re.S)
            auto = {"exhaust_self": kw and "Exhaust" in kw.group(1), "ethereal": kw and "Ethereal" in kw.group(1), "retain": kw and "Retain" in kw.group(1),
                    "strike": tg and "Strike" in tg.group(1), "defend": tg and "Defend" in tg.group(1), "x_cost": c["x"], "power_card": c["type"] == "Power",
                    "high_cost": not c["x"] and c["cost"] >= 2}
            t["provides"] += [m for m, on in auto.items() if on and m not in t["provides"]]
            if c["x"] and "energy_gain" not in t["consumes"]:
                t["consumes"].append("energy_gain")
            if "MultiplayerOnly" in text:
                t["mp_only"] = True
                src["mp_only"] = _find(dec, files[:1], "MultiplayerOnly")
        for m in dict.fromkeys(t["provides"] + t["consumes"]):
            pat = PAT.get(m, re.escape(m)) if m in t["provides"] else CPAT.get(m, PAT.get(m, re.escape(m)))
            src[m] = "catalog" if m in ("power_card", "high_cost") else _find(dec, files, pat)
        t["src"] = src
        tags[cid] = t
    live = {i: t for i, t in tags.items() if not t.get("mp_only")}
    pairs, seen = [], set()

    def add(a, b, via, kind, **kw):
        if a != b and a in live and b in live and (a, b, via, kind) not in seen:
            seen.add((a, b, via, kind))
            pairs.append(dict(a=a, b=b, via=via, kind=kind, **kw))
    for r in db["rules"]["anti"]:
        for a in _sel(live, r["a"]):
            for b in _sel(live, r["b"]) - set(r.get("except", [])):
                if r.get("only") is None or live.get(b, {}).get("kind") == r["only"]:
                    add(a, b, r["via"], "anti", note=r["note"], src=_find(dec, _files(dec, live[a]["kind"], a), r["pat"]))
    anti = {frozenset((p["a"], p["b"])) for p in pairs}
    for r in db["rules"]["extra"]:
        for a in _sel(live, r["a"]):
            for b in _sel(live, r["b"]):
                if frozenset((a, b)) not in anti:
                    add(a, b, r["via"], r["kind"], note=r["note"], src=_find(dec, _files(dec, live[a]["kind"], a), r["pat"]))
    prov, cons = collections.defaultdict(set), collections.defaultdict(set)
    for i, t in live.items():
        for m in t["provides"]:
            prov[m].add(i)
        for m in t["consumes"]:
            cons[m].add(i)
    for m in sorted(prov):
        for a in sorted(prov[m]):
            for b in sorted(cons[m]):
                if frozenset((a, b)) not in anti:
                    add(a, b, m, "direct")
    ind = collections.defaultdict(set)
    for c, t in live.items():
        for conv in t.get("converts", []):
            m1, m2 = conv.split(">")
            for a in prov[m1] - {c}:
                for b in cons[m2] - {c, a}:
                    if frozenset((a, b)) not in anti:
                        ind[(a, b, conv)].add(c)
    for (a, b, conv), needs in sorted(ind.items()):
        add(a, b, conv, "indirect", needs=sorted(needs))
    plans = {p["id"]: p for p in json.load(open(os.path.join(ROOT, "data", "plans.json"), encoding="utf-8"))}
    bundles = []
    for spec in db["rules"]["bundles"]:
        ms = set(spec["mechanics"])
        touch = lambda t: {c.split(">")[k] for c in t.get("converts", []) for k in (0, 1)}  # noqa: E731
        members = {i for i, t in live.items() if ms & (set(t["provides"]) | set(t["consumes"]) | touch(t))}
        key = {i for i in members if ms & set(live[i]["consumes"]) or ms & touch(live[i]) or "power_card" in live[i]["provides"] and ms & set(live[i]["provides"]) - {"power_card"}}
        core = {i for i in key if live[i]["kind"] == "card"}
        core = core if len(core) >= 3 else key
        p = plans.get(spec.get("plan"), {})
        core |= {i for k in ("core", "enablers", "payoffs") for i in p.get(k, []) if i in live}
        rel = ms | {c.split(">")[1] for i in core for c in live[i].get("converts", []) if c.split(">")[0] in ms}
        bad = {q["a"] for q in pairs if q["kind"] == "anti" and q["b"] in core and q["via"] in rel} - core
        bundles.append(dict(name=spec["name"], plan=spec.get("plan"), mechanics=spec["mechanics"], core=sorted(core), support=sorted(members - core - bad), anti=sorted(bad)))
    db.update(tags=tags, pairs=pairs, bundles=bundles)
    with open(PATH, "w", encoding="utf-8") as f:
        f.write(dumps(db))
    k = collections.Counter(p["kind"] for p in pairs)
    unv = sorted(f"{i}:{m}" for i, t in tags.items() for m, s in t["src"].items() if s == "text")
    print(f"tags {len(tags)} (cards {sum(t['kind'] == 'card' for t in tags.values())}), pairs {dict(k)}, bundles {len(bundles)}, unverified {len(unv)}: {' '.join(unv)}")


def dumps(db):
    head = {k: db[k] for k in ("_doc", "rules") if k in db}
    s = json.dumps(head, indent=1)[:-2]
    s += ',\n "tags": {\n' + ",\n".join(f"  {json.dumps(i)}: {json.dumps(t)}" for i, t in db["tags"].items()) + "\n }"
    s += ',\n "pairs": [\n' + ",\n".join("  " + json.dumps(p) for p in db["pairs"]) + "\n ]"
    s += ',\n "bundles": [\n' + ",\n".join("  " + json.dumps(b) for b in db["bundles"]) + "\n ]\n}\n"
    return s


def partners(db, cid):
    out = collections.defaultdict(list)
    for p in db["pairs"]:
        if cid in (p["a"], p["b"]):
            o = p["b"] if p["a"] == cid else p["a"]
            out[p["kind"]].append((o, p["via"], p.get("needs") or p.get("note", "")))
    return out


def deck_report(db, ids):
    deck = collections.Counter(i.rstrip("+").upper() for i in ids if i)
    rows = []
    for b in db["bundles"]:
        core = sum(deck[i] for i in b["core"])
        sup = sum(deck[i] for i in b["support"])
        links = collections.Counter(p["a"] if p["b"] in deck else p["b"] for p in db["pairs"] if p["kind"] == "direct" and (p["a"] in deck) != (p["b"] in deck))
        missing = sorted((i for i in b["core"] if i not in deck), key=lambda i: -links[i])
        rows.append(dict(name=b["name"], core=core, support=sup, missing=missing, anti=[i for i in b["anti"] if i in deck]))
    rows.sort(key=lambda r: -(2 * r["core"] + r["support"]))
    anti = [(p["a"], p["b"], p["note"]) for p in db["pairs"] if p["kind"] == "anti" and p["a"] in deck and p["b"] in deck]
    return rows, anti


def main(argv):
    if argv[:1] == ["--build"]:
        return build()
    db = load()
    if argv[:1] == ["--deck"]:
        rows, anti = deck_report(db, argv[1].split(","))
        for r in rows:
            if r["core"] or r["support"]:
                print(f"{r['name']:<22} core {r['core']:>2} support {r['support']:>2}  missing core: {' '.join(r['missing'][:8])}" + (f"  anti: {' '.join(r['anti'])}" if r["anti"] else ""))
        for a, b, note in anti:
            print(f"ANTI {a} x {b}: {note}")
        return
    cid = argv[0].rstrip("+").upper()
    t = db["tags"][cid]
    print(cid, {k: v for k, v in t.items() if k != "src"})
    for kind, rows in sorted(partners(db, cid).items()):
        print(f"{kind} ({len(rows)}):")
        for o, via, extra in sorted(rows, key=lambda r: (r[1], r[0])):
            print(f"  {o:<24} {via:<32} {' '.join(extra) if isinstance(extra, list) else extra}")


if __name__ == "__main__":
    main(sys.argv[1:])
