#!/usr/bin/env python3
import argparse, json, os, random, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fuzz_gen_mix as fg  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=1000)
    ap.add_argument("--seed", default="1")
    ap.add_argument("--out", required=True)
    ap.add_argument("--catalog", default=next((q for q in (os.path.join(ROOT, "target/train/catalog.json"), os.path.join(ROOT, "data/catalog.json")) if os.path.exists(q)), os.path.join(ROOT, "data/catalog.json")))
    ap.add_argument("--character", help="comma list; default all five (weights as in the fuzz generator)")
    ap.add_argument("--act", help="comma list of acts 0,1,2 (default all)")
    ap.add_argument("--room", help="comma list of monster,elite,boss")
    ap.add_argument("--encounter")
    ap.add_argument("--relics", default="2-7")
    ap.add_argument("--focus", default="mix", choices=["mix", "colorless", "junk", "gen", "turn"])
    ap.add_argument("--enchant", type=float, default=0.02)
    ap.add_argument("--no-stratagem", action="store_true")
    ap.add_argument("--energy-prob", type=float, default=0.0, help="fraction of scenarios with 4-7 energy a turn (the base set has 4+ in 3%% and 5+ in 0.03%%: decks with Pyre / Lantern / Seal of Gold are outside it)")
    ap.add_argument("--drop-unwinnable", action="store_true", help="drop fights `sts2.provably_unwinnable` proves lost (see docs/solver.md)")
    a = ap.parse_args()
    g = fg.Gen(a.catalog)
    chars = a.character.split(",") if a.character else None
    acts = [int(x) for x in a.act.split(",")] if a.act else [0, 1, 2]
    out = []
    i = 0
    while len(out) < a.n:
        ns = argparse.Namespace(character=None, encounter=a.encounter, act=None, room=a.room, mode="uniform", focus=a.focus,
                                enchant=a.enchant, force_cards=None, force_potions=None, force_relics=None, relics=a.relics)
        r = random.Random(f"pick/{a.seed}/{i}")
        if chars:
            ns.character = r.choice(chars)
        ns.act = r.choice(acts)
        try:
            s = g.scenario(i, a.seed, ns)
        except IndexError:
            i += 1
            continue
        i += 1
        if a.no_stratagem and any((c if isinstance(c, str) else c["id"]) == "STRATAGEM" for c in s["deck"]):
            continue
        s.pop("policy", None)
        if a.energy_prob and random.Random(f"energy/{a.seed}/{i}").random() < a.energy_prob:
            s["max_energy"] = random.Random(f"energy2/{a.seed}/{i}").choices([4, 5, 6, 7], [50, 25, 15, 10])[0]
        try:
            import sts2
            why = sts2.provably_unwinnable(s)
        except Exception:
            why = None
        s.setdefault("meta", {})["proved_unwinnable"] = why or False
        if why and a.drop_unwinnable:
            continue
        s["name"] = f"tr{a.seed}_{s['name']}"
        out.append(s)
    json.dump(out, open(a.out, "w"))
    from collections import Counter
    n_lost = sum(1 for s in out if s['meta'].get('proved_unwinnable'))
    print(f"{len(out)} scenarios ({n_lost} provably unwinnable) -> {a.out}; characters {dict(Counter(s['character'] for s in out))}; "
          f"rooms/enc sample {dict(Counter(s['encounter'] for s in out).most_common(6))}")


if __name__ == "__main__":
    main()
