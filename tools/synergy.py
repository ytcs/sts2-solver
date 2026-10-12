"""Loose synergy candidates: tags, pairs, bundles in data/synergy_candidates.json.

    synergy.py ID                  partners by kind
    synergy.py --deck ID,ID,...    bundle commitment, missing core, anti-synergy present (cards and relics, '+' ignored)
    synergy.py --build             re-derive auto tags, src, pairs, bundles from tags + rules (needs decomp/)
    synergy.py codex SRC [CHAR]    measured pick-given-offer synergy from community runs (tools/codex.py table), cross-referenced with the table
"""
import collections
import json
import math
import os
import re
import sys

import numpy as np

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


COMBAT = {"monster", "elite", "boss", "unknown"}
GROUPS = {"silent": {"shiv": ("BLADE_DANCE BLADE_OF_INK BLADE_SYMPHONY CLOAK_AND_DAGGER FAN_OF_KNIVES HIDDEN_DAGGERS LEADING_STRIKE STORM_OF_STEEL UP_MY_SLEEVE INFINITE_BLADES", "ACCURACY PHANTOM_BLADES KNIFE_TRAP"),
                     "poison": ("BOUNCING_FLASK BUBBLE_BUBBLE DEADLY_POISON HAZE OUTBREAK POISONED_STAB SNAKEBITE NOXIOUS_FUMES ENVENOM CORROSIVE_WAVE CONCOCT", "ACCELERANT BUBBLE_BUBBLE OUTBREAK"),
                     "sly": ("ACROBATICS CALCULATED_GAMBLE DAGGER_THROW HIDDEN_DAGGERS PREPARED SHADOW_STEP STORM_OF_STEEL SURVIVOR", "ABRASIVE FLICK_FLACK HAND_TRICK MASTER_PLANNER REFLEX RICOCHET SNEAKY TACTICIAN UNTOUCHABLE")}}
CONTROLS = {"ironclad": [("BARRICADE", "BODY_SLAM"), ("CORRUPTION", "FEEL_NO_PAIN"), ("CORRUPTION", "DARK_EMBRACE"), ("FEED", "FEEL_NO_PAIN"), ("INFLAME", "TWIN_STRIKE"), ("r:TUNGSTEN_ROD", "RUPTURE")],
            "silent": [("ACCURACY", "BLADE_DANCE"), ("ACCURACY", "CLOAK_AND_DAGGER"), ("NOXIOUS_FUMES", "ACCELERANT"), ("DEADLY_POISON", "ACCELERANT"), ("PREPARED", "TACTICIAN"), ("PREPARED", "REFLEX")]}
OUTS = (("win", 1), ("dmg", -1), ("turns", -1))


def _logit(X, y, ridge=1e-3):
    b = np.zeros(X.shape[1])
    for _ in range(25):
        mu = 1 / (1 + np.exp(-np.clip(X @ b, -30, 30)))
        H = (X * (mu * (1 - mu))[:, None]).T @ X + ridge * np.eye(X.shape[1])
        st = np.linalg.solve(H, X.T @ (y - mu) - ridge * b)
        b += st
        if np.abs(st).max() < 1e-7:
            break
    return b[-1], math.sqrt(np.linalg.inv(H)[-1, -1])


def _ols(X, y):
    Hi = np.linalg.inv(X.T @ X + 1e-6 * np.eye(X.shape[1]))
    b = Hi @ X.T @ y
    e = y - X @ b
    return b[-1], math.sqrt(Hi[-1, -1] * (e @ e) / max(len(y) - X.shape[1], 1))


def _fit(X, Y, half):
    """{outcome: (interaction, se, half-0 estimate, half-1 estimate)}; win by logit, dmg / turns z by OLS on rows with a later fight"""
    out = {}
    for k, (name, _) in enumerate(OUTS):
        s = np.isfinite(Y[:, k])
        f = _logit if name == "win" else _ols
        out[name] = f(X[s], Y[s, k]) + tuple(f(X[s & (half == h)], Y[s & (half == h), k])[0] for h in (0, 1))
    return out


def _bh(p):
    o, m = np.argsort(p), len(p)
    q = np.empty(m)
    q[o] = np.minimum.accumulate((p[o] * m / np.arange(1, m + 1))[::-1])[::-1]
    return q


def _p(e, s):
    return math.erfc(abs(e / s) / math.sqrt(2))


def offer_rows(src, char, K=6):
    """pick-given-offer rows (half, room, act, depth, deck size, hp frac, offered, picked, owned, [win, dmg z, turns z over the next K fights]): combat card rewards; shop and act-2/3 ancient relics"""
    import codex
    st = collections.defaultdict(list)
    for r in codex.rows(src, char):
        for f in (f for a in r["floors"] for f in a if f["enc"] and f.get("turns")):
            st[f["enc"][0]].append((f.get("dmg") or 0, f["turns"]))
    es = {k: (np.mean(v, 0), np.std(v, 0) + 1e-6) for k, v in st.items() if len(v) >= 20}
    rows = []
    for r in codex.rows(src, char):
        seq = [f for a in r["floors"] for f in a]
        z = [(np.array([f.get("dmg") or 0, f["turns"]]) - es[f["enc"][0]][0]) / es[f["enc"][0]][1] if f["enc"] and f.get("turns") and f["enc"][0] in es else None for f in seq]
        deck = collections.Counter(c[0] for c in r["deck"] if (c[3] or 0) <= 1) + collections.Counter(c for f in seq for c in f["removed"] if c in codex.STARTERS or c == "BASH")
        half, g = int(r["hash"] or "0", 16) & 1, 0
        for ai, act in enumerate(r["floors"]):
            for i, f in enumerate(act):
                g += 1
                own = frozenset({c for c, n in deck.items() if n > 0} | {"r:" + x for x, fx in r["relics"] if (fx or 0) < g})
                nz = [v for v in z[g:] if v is not None][:K]
                base = (half, f["t"], ai, i / len(act), sum(deck.values()), (f["hp"] or 0) / (f["mx"] or 1))
                y = (float(r["win"]),) + (tuple(np.mean(nz, 0)) if nz else (np.nan, np.nan))
                offered = collections.defaultdict(int)
                if f["enc"] and f["t"] in COMBAT and 3 <= len(f["offer"]) <= 5:
                    for c, _, pk in f["offer"]:
                        offered[c] |= pk
                if g > 1 and f["t"] in ("shop", "ancient"):
                    for x, pk in f["relics"] if f["t"] == "shop" else f["ancient"]:
                        if x:
                            offered["r:" + x] |= pk
                rows += [base + (b, pk, own, y) for b, pk in offered.items()]
                for c in f["gained"] if g > 1 else []:
                    deck[c] += 1
                for c in f["removed"]:
                    deck[c] -= 1
    return rows


def _controls(R, p):
    """stage, deck size, HP, owns B, room; each also times picked (the pick's value moves over the run)"""
    act, ds = np.array([r[2] for r in R]), np.array([r[4] for r in R], float)
    C = np.column_stack([act == 1, act == 2, [r[3] for r in R], (ds - ds.mean()) / (ds.std() + 1e-9), [r[5] for r in R], [r[6] in r[8] for r in R],
                         [r[1] == "elite" for r in R], [r[1] == "boss" for r in R], [r[1] == "shop" for r in R]]).astype(float)
    C = C[:, C.std(0) > 0]
    return np.column_stack([C, C * p[:, None]])


def pair_fits(rows, min_n=30, min_wl=5):
    """per (owned A, offered B): logit(win) / OLS(z) ~ 1 + picked + controls (+ x picked) + owns A + picked x owns A; gate: n(picked, owns A), n(not picked, owns A) >= min_n, >= min_wl wins and losses in all 4 picked x owns cells"""
    byB = collections.defaultdict(list)
    for r in rows:
        byB[r[6]].append(r)
    voc = collections.Counter(a for r in rows for a in r[8])
    items = [a for a, n in voc.items() if n >= 200]
    ix = {a: i for i, a in enumerate(items)}
    res = []
    for B, R in byB.items():
        p = np.array([r[7] for r in R], float)
        if min(p.sum(), len(R) - p.sum()) < min_n:
            continue
        Y, half = np.array([r[9] for r in R]), np.array([r[0] for r in R])
        X0 = np.column_stack([np.ones(len(R)), p, _controls(R, p)])
        M = np.zeros((len(R), len(items)), bool)
        for j, r in enumerate(R):
            M[j, [ix[a] for a in r[8] if a in ix]] = True
        ok = (M[p == 1].sum(0) >= min_n) & (M[p == 0].sum(0) >= min_n)
        for pv in (0, 1):
            for yv in (0, 1):
                s = (p == pv) & (Y[:, 0] == yv)
                ok &= (M[s].sum(0) >= min_wl) & ((~M[s]).sum(0) >= min_wl)
        for j in np.nonzero(ok)[0]:
            if items[j] != B:
                h = M[:, j].astype(float)
                res.append(dict(a=items[j], b=B, n11=int(h @ p), nown=int(h.sum()), fit=_fit(np.column_stack([X0, h, p * h]), Y, half)))
    return res


def pooled(rows, partners):
    """one model over offers of every B in partners: B and picked x B fixed effects + controls + owns any partner (owned in < 90% of rows) + picked x owns; None when < 30 picks while owning"""
    R = [r for r in rows if r[6] in partners]
    common = {a for a, n in collections.Counter(a for r in R for a in r[8]).items() if n > 0.9 * len(R)}
    partners = {b: v - common for b, v in partners.items()}
    bi = {b: i for i, b in enumerate(sorted({r[6] for r in R}))}
    p = np.array([r[7] for r in R], float)
    h = np.array([bool(partners[r[6]] & r[8]) for r in R], float)
    if not R or min(h @ p, h.sum() - h @ p) < 30:
        return None, int(h @ p) if R else 0
    FE = np.zeros((len(R), 2 * len(bi)))
    for j, r in enumerate(R):
        FE[j, bi[r[6]]], FE[j, len(bi) + bi[r[6]]] = 1, p[j]
    return _fit(np.column_stack([FE[:, FE.sum(0) > 0], _controls(R, p), h, p * h]), np.array([r[9] for r in R]), np.array([r[0] for r in R])), int(h @ p)


def deck_pairs(src, char, kind):
    """naive design: deck + relics entering the first final boss, logit(win) ~ deck size + A + B + A x B; (runs, tests, p<.05, BH q<.1, {kind: interactions})"""
    import codex
    D = []
    for r in codex.rows(src, char):
        dk, boss = codex.deck_at_pair(r)
        if dk is not None:
            D.append((r["win"], len(dk), {c[0] for c in dk} | {"r:" + x for x, f in r["relics"] if (f or 0) < boss}))
    y, ds = np.array([x[0] for x in D], float), np.array([x[1] for x in D], float)
    voc = collections.Counter(a for x in D for a in x[2])
    items = [a for a, n in voc.items() if 100 <= n <= len(D) - 100]
    ix = {a: i for i, a in enumerate(items)}
    M = np.zeros((len(D), len(items)))
    for j, x in enumerate(D):
        M[j, [ix[a] for a in x[2] if a in ix]] = 1
    N, W, res, pv = M.T @ M, M.T @ (M * y[:, None]), collections.defaultdict(list), []
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            if N[i, j] >= 30 and min(W[i, j], N[i, j] - W[i, j]) >= 5:
                e, s = _logit(np.column_stack([np.ones(len(y)), (ds - ds.mean()) / ds.std(), M[:, i], M[:, j], M[:, i] * M[:, j]]), y)
                pv.append(_p(e, s))
                res[kind.get(frozenset((items[i], items[j])))].append(e)
    pv = np.array(pv)
    return len(D), len(pv), int((pv < 0.05).sum()), int((_bh(pv) < 0.1).sum()), res


def _kinds(char):
    if char != "ironclad":
        return {}, {}
    db = load()
    rel = {k for k, v in db["tags"].items() if v["kind"] != "card"}
    nm = lambda x: "r:" + x if x in rel else x
    kind = {frozenset((nm(p["a"]), nm(p["b"]))): p["kind"] for p in db["pairs"]}
    groups = [(f"bundle {b['name']}", {nm(i): {nm(c) for c in b["core"]} - {nm(i)} for i in b["core"] + b["support"]}) for b in db["bundles"]]
    for kd in ("direct", "indirect", "anti"):
        pt = collections.defaultdict(set)
        for p in db["pairs"]:
            if p["kind"] == kd:
                pt[nm(p["a"])].add(nm(p["b"])), pt[nm(p["b"])].add(nm(p["a"]))
        groups.append((f"table {kd} pairs", pt))
    tags = {nm(k): v for k, v in db["tags"].items()}
    for m in sorted({m for v in tags.values() for m in v.get("consumes", ())}):
        pro = {k for k, v in tags.items() if m in v.get("provides", ())}
        groups.append((f"mechanic {m}", {k: pro - {k} for k, v in tags.items() if m in v.get("consumes", ())}))
    return kind, groups


def _cell(f, name):
    e, s = f[name][:2]
    return f"{name} {e:+.3f} [{e - 1.96 * s:+.3f},{e + 1.96 * s:+.3f}]"


def _row(x, n, q=None):
    return f"{x['a']} -> {x['b']}: {_cell(x['fit'], n)} n(pick,own) {x['n11']} n(own) {x['nown']}" + (f" q {q:.3f}" if q is not None else "")


def codex_report(src, char):
    """measured synergy from community runs (pick-given-offer interaction), cross-referenced with the candidate table"""
    rows, (kind, groups), sg = offer_rows(src, char), _kinds(char), dict(OUTS)
    res = pair_fits(rows)
    print(f"{char}: {len(rows)} offer rows, {len({r[6] for r in rows})} offered items, {len(res)} gated pairs (A owned -> B offered); interaction picked x owns A: "
          "win log-odds, dmg / turns = per-fight z (by encounter) averaged over the next 6 fights; synergy sign = win +, dmg -, turns -")
    for name, s1 in OUTS:
        e, s, h0, h1 = (np.array([x["fit"][name][k] for x in res]) for k in range(4))
        p = np.array([_p(a, b) for a, b in zip(e, s)])
        q = _bh(p)
        rep = (q < 0.1) & (np.sign(h0) == np.sign(e)) & (np.sign(h1) == np.sign(e))
        for x, qq, rr in zip(res, q, rep):
            x.setdefault("q", {})[name], x.setdefault("rep", {})[name] = qq, rr
        print(f"  {name}: median SE {np.median(s):.3f}, effect sd {e.std():.3f}, p<.05 {int((p < .05).sum())} (null {0.05 * len(p):.0f}), BH q<.1 {int((q < .1).sum())}, "
              f"replicated in both halves {int(rep.sum())} (synergy sign {int((rep & (s1 * e > 0)).sum())})")
        if kind:
            k = np.array([kind.get(frozenset((x["a"], x["b"])), "") for x in res])
            base = s1 * e[k == ""]
            print(f"    signed effect, table kind minus untabled ({base.mean():+.4f}, n {len(base)}): " + ", ".join(
                f"{kd} {np.mean(s1 * e[k == kd]) - base.mean():+.4f} +-{1.96 * np.std(s1 * e[k == kd]) / math.sqrt((k == kd).sum()):.4f} (n {(k == kd).sum()})" for kd in ("direct", "indirect", "anti") if (k == kd).any()))
    print("controls (mechanism certain from code):")
    by = {(x["a"], x["b"]): x for x in res}
    for a, b in CONTROLS.get(char, []):
        n11 = sum(1 for r in rows if r[6] == b and r[7] and a in r[8])
        print("  " + ("  ".join([_row(by[a, b], "win")] + [_cell(by[a, b]["fit"], n) for n, _ in OUTS[1:]]) if (a, b) in by else f"{a} -> {b}: not gated, n(pick,own) {n11}"))
    if kind:
        fam = [(x, n) for x in res if frozenset((x["a"], x["b"])) in kind for n, _ in OUTS]
        q = _bh(np.array([_p(*x["fit"][n][:2]) for x, n in fam]))
        hit = collections.defaultdict(list)
        for (x, n), qq in zip(fam, q):
            e, s, h0, h1 = x["fit"][n]
            if qq < 0.1 and np.sign(h0) == np.sign(h1) == np.sign(e):
                hit["confirmed" if (sg[n] * e > 0) == (kind[frozenset((x["a"], x["b"]))] != "anti") else "contradicted"].append((x, n, qq))
        seen = {a for r in rows for a in r[8]} | {r[6] for r in rows}
        tested = {frozenset((x["a"], x["b"])) for x in res} & set(kind)
        print(f"table cross-reference: {len(kind)} pairs {dict(sorted(collections.Counter(kind.values()).items()))}; both items in the data {sum(p <= seen for p in kind)}; gated {len(tested)} "
              f"{dict(sorted(collections.Counter(kind[p] for p in tested).items()))}; table-family BH over {len(fam)} (direction x outcome) tests, q<.1 and both halves agree: "
              f"confirmed {len(hit['confirmed'])}, contradicted {len(hit['contradicted'])}")
        for lab in ("confirmed", "contradicted"):
            for x, n, qq in hit[lab]:
                print(f"  {lab} {kind[frozenset((x['a'], x['b']))]}: {_row(x, n, qq)}")
    miss = sorted(((x, n) for x in res for n, _ in OUTS if x["rep"][n] and frozenset((x["a"], x["b"])) not in kind), key=lambda t: t[0]["q"][t[1]])
    print(f"{'untabled' if kind else 'all'} pairs, all-pairs BH q<.1 replicated: {len(miss)} (synergy sign {sum(sg[n] * x['fit'][n][0] > 0 for x, n in miss)})")
    for x, n in miss:
        print(f"  {'syn ' if sg[n] * x['fit'][n][0] > 0 else 'anti'} {_row(x, n, x['q'][n])}")
    print("pooled (B and picked x B fixed effects; owns any partner): " + ("bundles: offered core/support piece, partner = other core piece; table pairs: partner = any table partner" if kind else "code groups: offered consumer, partner = provider"))
    for lab, parts in groups or [(f"group {g}", {c: set(pro.split()) - {c} for c in con.split()}) for g, (pro, con) in GROUPS.get(char, {}).items()]:
        f, n = pooled(rows, parts)
        print(f"  {lab:<32} n(pick,own) {n:6d}  " + ("  ".join(_cell(f, nn) for nn, _ in OUTS) if f else "too few"))
    n, t, p05, bh, ks = deck_pairs(src, char, kind)
    print(f"naive deck design (deck entering the first final boss, {n} runs, win | reached): {t} pairs, p<.05 {p05} (null {0.05 * t:.0f}), BH q<.1 {bh}" + (
        "; mean interaction " + ", ".join(f"{k or 'untabled'} {np.mean(v):+.3f} (n {len(v)})" for k, v in ks.items()) if kind else ""))


def main(argv):
    if argv[:1] == ["--build"]:
        return build()
    if argv[:1] == ["codex"]:
        return codex_report(argv[1], argv[2] if len(argv) > 2 else "ironclad")
    db = load()
    if argv[:1] == ["--deck"]:
        rows, anti = deck_report(db, argv[1].split(","))
        for r in rows:
            if r["core"] or r["support"]:
                print(f"{r['name']:<22} core {r['core']:>2} support {r['support']:>2}  missing core: {' '.join(r['missing'][:8])}" + (f"  anti: {' '.join(r['anti'])}" if r["anti"] else ""))
        for a, b, note in anti:
            print(f"ANTI {a} x {b}: {note}")
        return
    if not argv or argv[0].startswith("-") or argv[0].rstrip("+").upper() not in db["tags"]:
        return print(__doc__)
    cid = argv[0].rstrip("+").upper()
    t = db["tags"][cid]
    print(cid, {k: v for k, v in t.items() if k != "src"})
    for kind, rows in sorted(partners(db, cid).items()):
        print(f"{kind} ({len(rows)}):")
        for o, via, extra in sorted(rows, key=lambda r: (r[1], r[0])):
            print(f"  {o:<24} {via:<32} {' '.join(extra) if isinstance(extra, list) else extra}")


if __name__ == "__main__":
    main(sys.argv[1:])
