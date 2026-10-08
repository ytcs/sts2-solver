import collections
import json
import os
import re

import sts2

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "data", "card_buckets_ironclad.json")
DUMMY = "WATERFALL_GIANT_BOSS"

OVERRIDE = {
    "INFERNO": ["SD"], "RUPTURE": ["SD"], "JUGGERNAUT": ["SD", "SB"], "FEED": ["SD", "FD"], "DEMON_FORM": ["SD"], "COMBUST": ["SD"], "BRUTALITY": ["ACC"],
    "BARRICADE": ["SB", "ACC"], "FEEL_NO_PAIN": ["SB"], "METALLICIZE": ["SB"], "DARK_EMBRACE": ["ACC", "SB"], "CORRUPTION": ["ACC", "SB"], "STONE_ARMOR": ["SB"],
    "FLAME_BARRIER": ["FB", "SB"], "CRIMSON_MANTLE": ["SB"], "UNMOVABLE": ["SB"], "FIGHT_ME": ["SD", "FD"], "HELLRAISER": ["SD"], "STAMPEDE": ["SD"],
    "BODY_SLAM": ["FD", "FB"], "COLOSSUS": ["FB"], "PERFECTED_STRIKE": ["FD", "SD"], "ASHEN_STRIKE": ["FD", "SD"], "TEAR_ASUNDER": ["FD", "SD"], "RAMPAGE": ["FD", "SD"],
    "MAUL": ["FD", "SD"], "BLAZE": ["FD"], "BRAND": ["SD", "ACC"], "BURNING_PACT": ["ACC"], "CASCADE": ["ACC", "FD"], "DEMONIC_SHIELD": ["FB"], "HAVOC": ["ACC"], "NOT_YET": ["FB"], "STOKE": ["ACC"], "TREMBLE": ["FD"], "BLOODLETTING": ["ACC"], "OFFERING": ["ACC"], "BATTLE_TRANCE": ["ACC"], "PRIMAL_FORCE": ["FD", "SD"], "PYRE": ["ACC"],
}


def _snap(sim):
    return json.loads(sim.snapshot())


def _power(powers, name):
    for p in powers or []:
        if p.get("id") == name:
            return p.get("amount", 0)
    return 0


def _scenario(card_id, upgrade):
    return dict(ascension=10, encounter=DUMMY, character="IRONCLAD", hp=9999, max_hp=9999, max_energy=10, gold=0, max_potion_slots=2, base_orb_slots=0,
                seed="tags", total_floor=1, act=0, deck=[{"id": card_id, "upgrade": upgrade}] * 12, relics=[], potions=[], name="tag")


def measure(card_id, upgrade=0, seeds=(1, 2, 3)):
    best = None
    for seed in seeds:
        try:
            sim = sts2.Sim(json.dumps(_scenario(card_id, upgrade)), seed)
        except Exception:  # noqa: BLE001
            return None
        before = _snap(sim)
        for idx, _t in sim.legal():
            a = json.loads(sim.action_json(idx))
            if "play" not in a:
                continue
            card = before["hand"][a["play"]["hand_pos"]]
            c = sim.copy()
            try:
                c.apply(sim.action_json(idx))
            except Exception:  # noqa: BLE001
                continue
            after = _snap(c)
            hpb = sum(e["hp"] + e["block"] for e in before["enemies"] if e["alive"])
            hpa = sum(e["hp"] + e["block"] for e in after["enemies"] if e["alive"])
            pb, pa = before["player"], after["player"]
            m = dict(cost=card.get("cost", 0), damage=max(0, hpb - hpa), block=max(0, pa["block"] - pb["block"]),
                     energy=after["energy"] - (before["energy"] - card.get("cost", 0)), draw=len(after["hand"]) - (len(before["hand"]) - 1),
                     strength=_power(pa["powers"], "STRENGTH_POWER") - _power(pb["powers"], "STRENGTH_POWER"),
                     dexterity=_power(pa["powers"], "DEXTERITY_POWER") - _power(pb["powers"], "DEXTERITY_POWER"),
                     exhaust="Exhaust" in card.get("keywords", []),
                     powers=sorted({p["id"] for p in pa["powers"]} - {p["id"] for p in pb["powers"]}),
                     debuff=any(p["id"] in ("VULNERABLE_POWER", "WEAK_POWER") for e in after["enemies"] for p in e.get("powers", [])))
            if m["powers"]:
                per = []
                d = c.copy()
                for _ in range(3):
                    s0 = _snap(d)
                    d.end_turn()
                    s1 = _snap(d)
                    per.append(dict(strength=_power(s1["player"]["powers"], "STRENGTH_POWER") - _power(s0["player"]["powers"], "STRENGTH_POWER"),
                                    energy=s1["energy"] - 10, hand=len(s1["hand"]) - 5,
                                    enemy_hp_lost=max(0, sum(e["hp"] for e in s0["enemies"]) - sum(e["hp"] for e in s1["enemies"]))))
                m["per_turn"] = per
            if best is None or (m["damage"], m["block"], len(m["powers"])) > (best["damage"], best["block"], len(best["powers"])):
                best = m
        if best and (best["damage"] or best["block"] or best["powers"] or best["energy"] or best["draw"]):
            break
    return best


def tags_from(m, card_id):
    t = set()
    if m is None:
        return sorted(OVERRIDE.get(card_id, []))
    if m["damage"] > 0:
        t.add("FD")
    if m["block"] > 0:
        t.add("FB")
    if m["strength"] > 0 or any(p.get("strength", 0) > 0 for p in m.get("per_turn", [])):
        t.add("SD")
    if m["dexterity"] > 0:
        t.add("SB")
    if m["energy"] > 0 or m["draw"] > 0 or any(p.get("energy", 0) > 0 or p.get("hand", 0) > 0 for p in m.get("per_turn", [])):
        t.add("ACC")
    if m["powers"] and not t:
        t.add("SD")
    t |= set(OVERRIDE.get(card_id, []))
    return sorted(t)


def build(character="ironclad"):
    ids = re.findall(r"ids::card::([A-Z0-9_]+)", open(os.path.join(ROOT, "crates", "sts2sim", "src", "content", "gen_pools.rs"), encoding="utf-8").read().split("pub static IRONCLAD")[1].split("];")[0])
    out = {}
    for cid in ids:
        if cid in ("STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "ASCENDERS_BANE"):
            m = measure(cid)
        else:
            m = measure(cid)
        t = tags_from(m, cid)
        out[cid] = dict(buckets=t, **({k: m[k] for k in ("cost", "damage", "block", "energy", "draw", "strength", "dexterity", "exhaust")} if m else {}))
    return out


def load(path=OUT):
    return json.load(open(path, encoding="utf-8"))


def deck_line(deck, tags):
    c = collections.Counter()
    for card in deck:
        cid = card["id"] if isinstance(card, dict) else card
        for b in tags.get(cid, {}).get("buckets", []):
            c[b] += 1
    return {b: c.get(b, 0) for b in ("FD", "SD", "FB", "SB", "ACC")}


def deficiencies(line, targets=None):
    targets = targets or dict(FD=5, SD=2, FB=4, SB=1, ACC=3)
    return {b: targets[b] - line[b] for b in targets if line[b] < targets[b]}


def candidates(tags, bucket, exclude=()):
    cs = [(cid, v) for cid, v in tags.items() if bucket in v["buckets"] and cid not in exclude]
    key = {"FD": lambda v: -(v.get("damage", 0) / max(1, v.get("cost", 1))), "FB": lambda v: -(v.get("block", 0) / max(1, v.get("cost", 1)))}.get(bucket, lambda v: v.get("cost", 1))
    return [cid for cid, v in sorted(cs, key=lambda x: key(x[1]))]


if __name__ == "__main__":
    tags = build()
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    json.dump(tags, open(OUT, "w", encoding="utf-8"), indent=1, sort_keys=True)
    cnt = collections.Counter(b for v in tags.values() for b in v["buckets"])
    print(len(tags), "cards;", dict(cnt), "; untagged:", [c for c, v in tags.items() if not v["buckets"]])
