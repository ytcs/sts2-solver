#!/usr/bin/env python3
"""Community runs (spire-codex.com bulk export, /api/exports/runs) as a jump-start prior, never a target: mixed skill, correlational; the export
carries no player identity (single-player id is 1), so the comparison is won vs lost runs.
table: one streaming pass over a version dump -> compact single-player standard A10 runs. decks: deck entering the first final boss, won vs lost.
usage: tools/codex.py table target/codex/runs-v0.111.0.jsonl.gz --out target/codex/a10.jsonl.gz
       tools/codex.py decks target/codex/a10.jsonl.gz [--character ironclad]"""
import argparse, collections, gzip, json, re

STARTERS = {"STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "STRIKE_SILENT", "DEFEND_SILENT", "STRIKE_DEFECT", "DEFEND_DEFECT", "STRIKE_NECROBINDER",
            "DEFEND_NECROBINDER", "STRIKE_REGENT", "DEFEND_REGENT"}


def _id(x):
    return re.sub(r"^[A-Z]+\.", "", x or "")


def _card(c):
    return [_id(c["id"]), int(c.get("current_upgrade_level", c.get("upgrade_level", 0)) or 0), _id((c.get("enchantment") or {}).get("id"))]


def _floor(pt):
    ps = (pt.get("player_stats") or [{}])[0]
    rooms = pt.get("rooms") or [{}]
    return dict(t=pt.get("map_point_type"), enc=[_id(r.get("model_id")) for r in rooms if r.get("model_id")], hp=ps.get("current_hp"), mx=ps.get("max_hp"),
                gold=ps.get("current_gold"), dmg=ps.get("damage_taken"), turns=sum(r.get("turns_taken") or 0 for r in rooms),
                offer=[[_id(c["card"]["id"]), int(c["card"].get("current_upgrade_level", 0) or 0), int(bool(c.get("was_picked")))] for c in ps.get("card_choices") or []],
                gained=[_id(c["id"]) for c in ps.get("cards_gained") or []], removed=[_id(c["id"] if isinstance(c, dict) else c) for c in ps.get("cards_removed") or []],
                upgraded=[_id(c) for c in ps.get("upgraded_cards") or []], rest=ps.get("rest_site_choices") or [],
                relics=[[_id(r.get("choice")), int(bool(r.get("was_picked")))] for r in ps.get("relic_choices") or []],
                ancient=[[a.get("TextKey"), int(bool(a.get("was_chosen")))] for a in ps.get("ancient_choice") or []],
                pot_used=[_id(p) for p in ps.get("potion_used") or []], bought=[_id(c) for c in (ps.get("bought_colorless") or []) + (ps.get("bought_potions") or [])])


def table(src, out):
    n, kept, why = 0, 0, collections.Counter()
    from agent.macro import card_id_set
    known = card_id_set()
    with gzip.open(src, "rt", encoding="utf-8") as f, gzip.open(out, "wt", encoding="utf-8") as g:
        for line in f:
            n += 1
            try:
                d = json.loads(line)
            except ValueError:
                why["bad json"] += 1
                continue
            ps = d.get("players") or []
            cards = {_id(c.get("id")) for c in (ps[0].get("deck") or [])} if ps else set()
            for test, k in ((len(ps) == 1, "multiplayer"), (cards <= known, "modded cards"), (d.get("game_mode") == "standard", "mode"), (not d.get("modifiers"), "modifiers"),
                            (d.get("ascension") == 10, "ascension"), (not d.get("was_abandoned"), "abandoned")):
                if not test:
                    why[k] += 1
                    break
            else:
                p = ps[0]
                floors = [[_floor(pt) for pt in act] for act in d.get("map_point_history") or []]
                g.write(json.dumps(dict(hash=d.get("run_hash"), char=_id(p.get("character")).lower(), win=bool(d.get("win")),
                                        seed=d.get("seed"), beta=bool(d.get("is_beta")), acts=[_id(a) for a in d.get("acts") or []],
                                        killed_by=_id(d.get("killed_by_encounter")), run_time=d.get("run_time"), deck=[_card(c) + [c.get("floor_added_to_deck")] for c in p.get("deck") or []],
                                        relics=[[_id(r["id"]), r.get("floor_added_to_deck")] for r in p.get("relics") or []], floors=floors)) + "\n")
                kept += 1
            if n % 50000 == 0:
                print(f"  {n} read, {kept} kept", flush=True)
    print(f"{n} runs read, {kept} kept (single-player standard A10, not abandoned); dropped: {dict(why)}")


def rows(path, character=None):
    with gzip.open(path, "rt", encoding="utf-8") as f:
        for l in f:
            r = json.loads(l)
            if character is None or r["char"] == character:
                yield r


def deck_at_pair(r):
    """the deck entering the first final boss: final deck minus cards added from that floor on (removals after it are not undone)"""
    act3 = r["floors"][2] if len(r["floors"]) >= 3 else []
    start = sum(len(a) for a in r["floors"][:2])
    boss = next((start + i + 1 for i, pt in enumerate(act3) if pt["t"] == "boss"), None)
    if boss is None:
        return None, None
    return [c for c in r["deck"] if (c[3] or 0) < boss], boss


def decks(path, character):
    agg = collections.defaultdict(list)
    for r in rows(path, character):
        d, _ = deck_at_pair(r)
        if d is not None:
            agg[r["win"]].append((len(d), sum(c[0] in STARTERS for c in d), sum(c[1] > 0 for c in d)))
    print("deck entering the first final boss (runs that reached it): n, mean size, starters left, upgraded cards")
    for w, v in sorted(agg.items()):
        n = len(v)
        print(f"  {'won ' if w else 'lost'} n {n:6d}  size {sum(x[0] for x in v) / n:5.1f}  starters {sum(x[1] for x in v) / n:4.1f}  upgraded {sum(x[2] for x in v) / n:4.1f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["table", "decks"])
    ap.add_argument("src")
    ap.add_argument("--out")
    ap.add_argument("--character")
    a = ap.parse_args()
    if a.cmd == "table":
        table(a.src, a.out)
    else:
        decks(a.src, a.character)


if __name__ == "__main__":
    main()
