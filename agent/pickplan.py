"""Pick planner: how picky to be at a card reward when more offers are still coming (optimal stopping on a stream of offers with a limited number of deck slots).

`python -m agent pickplan [--screens K] [--elites E] [--shops S] [--slots N] [--rho R]` (read-only).

Why: a pick that is positive now can still be a mistake when the next K reward screens and the shops will offer cards worth more for the same deck slot. The pools and the
odds are known (`[code]` CardRarityOdds.cs: A10 Scarcity: regular 61.5% common / 37% uncommon / 1.49% + offset rare, offset starts at -5%, +0.5% per card rolled, reset by a
rare; elite 54.9 / 40 / 5% + offset; shop 58.5 / 37 / 4.5%, the offset is not changed by shops; MerchantInventory.cs: 5 class cards (types Attack, Attack, Skill, Skill, Power) +
2 colorless (Uncommon, Rare); a reward screen = 3 distinct cards of the class pool).

Method:
1. Gain of every pool card for the CURRENT deck: the need-weighted gain over the boss, the elites to come and the next act (`macro.need_view`: each fight weighted by how
   unsolved it is), one simulator evaluation per card.
2. Monte Carlo of the offer stream: K regular reward screens (+ E elite screens, + S shops), each card's gain drawn from the table by its rarity / type.
3. A threshold policy: take an offered card when its gain >= tau and a slot is free; the m-th card taken counts rho**m of its gain (a card added to a bigger deck is worth less:
   density, `sts2-deckbuilding` blind spot 7). Search tau for the best expected total. The current screen: take the options with gain >= tau (best first), skip the rest.
`[hyp]`: rho and the slot count are judgments (defaults rho 0.85, slots 6); the gains of different cards are treated as additive.
"""
import os
import re

import numpy as np

from agent import macro

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
RUST = os.path.join(ROOT, "crates", "sts2sim", "src", "content")

REGULAR = dict(common=0.615, uncommon=0.37, rare=0.0149)  # A10 (Scarcity)
ELITE = dict(common=0.549, uncommon=0.40, rare=0.05)
SHOP = dict(common=0.585, uncommon=0.37, rare=0.045)
GROWTH, OFFSET0, OFFSET_MAX = 0.005, -0.05, 0.4


def _read(name):
    with open(os.path.join(RUST, name), encoding="utf-8") as f:
        return f.read()


def _pool(name):
    src = _read("gen_pools.rs")
    m = re.search(r"pub static %s: \[u16; \d+\] = \[(.*?)\];" % name, src, re.S)
    return re.findall(r"ids::card::(\w+)", m.group(1))


def card_table():
    """{ID: (rarity, type)} for the Ironclad and colorless pools, parsed from the generated card definitions."""
    src = _read("gen_cards.rs")
    info = {}
    for m in re.finditer(r"CardDef::new\(ids::card::(\w+), -?\d+, CardType::(\w+), CardRarity::(\w+),", src):
        info[m.group(1)] = (m.group(3).lower(), m.group(2).lower())
    return info


def pools():
    info = card_table()
    cls = {c: info[c] for c in _pool("IRONCLAD") if c in info and info[c][0] in ("common", "uncommon", "rare")}
    col = {c: info[c] for c in _pool("COLORLESS") if c in info and info[c][0] in ("uncommon", "rare")}
    return cls, col


_GAINS = {}  # (deck, relics, fights, attempts) -> gains: repeated calls at the same deck are free


def _gains(engine, deck_json, hz, batch, attempts, smooth, hold):
    """{ID: need-weighted gain over the current deck} for the cards in `batch` (one evaluation; a card the simulator cannot run is dropped by bisection)."""
    out = {}

    def run(b):
        variants = [dict(name="keep")] + [dict(name=c, add=[c]) for c in b]
        res = macro.price_horizon(engine, deck_json, variants, hz, "full", hold, attempts, attempts, max(8, attempts // 2), smooth_boss=smooth)
        nv, solved = macro.need_view(res, len(variants))
        for vi, c in enumerate(b, start=1):
            g = nv[vi][2] if nv and nv[vi][2] is not None else 0.0
            out[c] = g

    def safe(b):
        if not b:
            return
        try:
            run(b)
        except Exception:  # noqa: BLE001  unported content: split until the culprit is alone
            if len(b) == 1:
                return
            mid = len(b) // 2
            safe(b[:mid])
            safe(b[mid:])

    safe(list(batch))
    return out


def card_gains(engine, deck_json, hz, ids, attempts=32, hold=(), top=40):
    """Need-weighted gain of every pool card for the current deck, in two stages: a cheap screen of all cards (a quarter of the attempts, the boss without the HP-smooth
    average), then the `top` best re-priced properly (smooth boss, full attempts). Cards that did not make the second stage keep half their (clipped) screening gain."""
    key = (tuple(sorted((c["id"], c.get("upgrade", 0)) for c in deck_json["deck"])), tuple(r["id"] for r in deck_json.get("relics", [])), tuple(p["id"] for p in deck_json.get("potions", [])),
           tuple(hz["boss"]), tuple(hz["elites"]), tuple(hz["next"]), attempts, top)
    if key in _GAINS:
        return dict(_GAINS[key])
    coarse = _gains(engine, deck_json, hz, ids, max(8, attempts // 4), False, hold)
    best = [c for c, _ in sorted(coarse.items(), key=lambda kv: -kv[1])[:top]]
    fine = _gains(engine, deck_json, hz, best, attempts, True, hold)
    out = {c: 0.5 * max(0.0, g) for c, g in coarse.items()}
    out.update(fine)
    if len(_GAINS) >= 4:
        _GAINS.pop(next(iter(_GAINS)))
    _GAINS[key] = dict(out)
    return out


def _rarity(rng, odds, offset):
    x = rng.random()
    rare = max(0.0, odds["rare"] + offset)
    if x < rare:
        return "rare"
    if x < rare + odds["uncommon"]:
        return "uncommon"
    return "common"


def simulate(gains, cls, col, screens, elites, shops, offset, slots, rho, taus, trials=4000, seed=1):
    """Expected total value of a threshold policy for each tau in `taus` (+ the greedy policy 'take any positive gain' = tau just above 0)."""
    rng = np.random.default_rng(seed)
    import random
    r = random.Random(seed)
    by_r = {k: [c for c in cls if cls[c][0] == k and c in gains] for k in ("common", "uncommon", "rare")}
    by_t = {(rar, t): [c for c in cls if cls[c][0] == rar and cls[c][1] == t and c in gains] for rar in ("common", "uncommon", "rare") for t in ("attack", "skill", "power")}
    colr = {k: [c for c in col if col[c][0] == k and c in gains] for k in ("uncommon", "rare")}
    totals = np.zeros(len(taus))
    for _ in range(trials):
        stream = []  # list of lists of gains (one list per screen / shop)
        off = offset
        order = ["reg"] * screens + ["elite"] * elites
        r.shuffle(order)
        for kind in order:
            odds = REGULAR if kind == "reg" else ELITE
            cards = []
            for _c in range(3):
                for _try in range(20):
                    rar = _rarity(r, odds, off)
                    pool = by_r[rar]
                    if not pool:
                        continue
                    c = r.choice(pool)
                    if c in cards:
                        continue
                    cards.append(c)
                    off = OFFSET0 if rar == "rare" else min(off + GROWTH, OFFSET_MAX)
                    break
            stream.append([gains[c] for c in cards])
        for _s in range(shops):
            cards = []
            for t in ("attack", "attack", "skill", "skill", "power"):
                rar = _rarity(r, SHOP, off)
                pool = by_t.get((rar, t)) or by_t.get(("common", t)) or []
                pool = [c for c in pool if c not in cards]
                if pool:
                    cards.append(r.choice(pool))
            for rar in ("uncommon", "rare"):
                if colr[rar]:
                    cards.append(r.choice(colr[rar]))
            stream.append([gains[c] for c in cards])
        for ti, tau in enumerate(taus):
            taken, tot = 0, 0.0
            for offer in stream:
                for g in sorted(offer, reverse=True):
                    if g >= tau and g > 0 and taken < slots:
                        tot += g * rho ** taken
                        taken += 1
            totals[ti] += tot
    return totals / trials


def analyse(engine, deck_json, hz, offer=None, screens=3, elites=0, shops=0, slots=6, rho=0.85, offset=OFFSET0, attempts=32, hold=()):
    """The planner's report; `offer` = the card reward on screen ({display name: card id}): each option is judged against tau* (a card outside the
    priced pools has no gain: `nan`, skip)."""
    cls, col = pools()
    ids = [c for c in list(cls) + list(col)]
    gains = card_gains(engine, deck_json, hz, ids, attempts, hold)
    taus = [0.0] + [round(x, 3) for x in np.arange(0.005, 0.25, 0.005)]
    ev = simulate(gains, cls, col, screens, elites, shops, offset, slots, rho, taus)
    best = int(np.argmax(ev))
    greedy = ev[0]
    ranked = sorted(gains.items(), key=lambda kv: -kv[1])
    lines = [f"pick planner: {screens} regular + {elites} elite reward screens and {shops} shops ahead, {slots} deck slots, rho {rho}, rarity offset {offset:+.3f}; {len(gains)} pool cards priced "
             f"(need-weighted gain over the current deck: boss {','.join(hz['boss'])[:30]}, elites to come, next act)",
             f"expected total gain: take anything positive {greedy:.3f}; threshold policy tau* = {taus[best]:.3f} -> {ev[best]:.3f}",
             "best cards for this deck: " + ", ".join(f"{c} {g:+.3f}" for c, g in ranked[:12]),
             f"share of pool cards with gain >= tau*: {sum(1 for g in gains.values() if g >= taus[best]) / max(len(gains), 1):.0%}"]
    curve = []
    for k in (0, 1, 2, 3, 4, 6, 8):
        evk = simulate(gains, cls, col, k, 0, 0, offset, slots, rho, taus, trials=1500, seed=7)
        curve.append((k, taus[int(np.argmax(evk))]))
    lines.append("runway curve (regular reward screens left before the dangerous fight, no shop): tau* = " + ", ".join(f"{k}: {t:.3f}" for k, t in curve) + "  (the closer the fight, the less picky)")
    if offer:
        tau = taus[best]
        lines.append(f"this screen (take iff gain >= tau* = {tau:.3f}): " + "; ".join(f"{n} {gains.get(c, float('nan')):+.3f} {'TAKE' if gains.get(c, -1) >= tau and gains.get(c, -1) > 0 else 'skip'}"
                                                                                for n, c in sorted(offer.items(), key=lambda kv: -gains.get(kv[1], -9))))
    return "\n".join(lines), dict(tau=taus[best], gains=gains, ev=dict(zip(taus, ev.tolist())))
