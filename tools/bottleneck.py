#!/usr/bin/env python3
"""Deck bottleneck tracker (plan item 4b): per card, plays vs turns it was left in hand at end of turn (from the run log's `action` events:
`play X` texts and `end turn` events with hand_left / energy_left). Diagnosis, an input not a verdict:
- left with no energy to spare (energy_left < its cost) -> energy bottleneck;
- left while energy remained -> the deck chose not to play it: removal candidate, or a combo piece waiting for its partner (draw / retain);
Usage: tools/bottleneck.py <events.jsonl> [--act N]"""
import argparse, collections, json, re


def analyze(path, act=None):
    played, left, left_poor, left_rich, cost = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter(), {}
    turns = 0
    cur_act = None
    for l in open(path, encoding="utf-8"):
        e = json.loads(l)
        if e["kind"] == "fight_start":
            cur_act = (e.get("scenario") or {}).get("act")
            for c in (e.get("scenario") or {}).get("deck", []):
                cost.setdefault(c["id"], None)
        if e["kind"] != "action" or (act is not None and cur_act != act):
            continue
        t = str(e.get("text", ""))
        m = re.match(r"play (\S+)", t)
        if m:
            played[m.group(1).split("#")[0]] += 1
        elif t.startswith("end turn") and "hand_left" in e:
            turns += 1
            en = e.get("energy_left") or 0
            for cid in e["hand_left"]:
                base = cid.rstrip("+")
                left[base] += 1
                (left_rich if en >= 1 else left_poor)[base] += 1
    rows = []
    for cid in sorted(set(played) | set(left)):
        p, lf = played[cid], left[cid]
        rows.append(dict(card=cid, played=p, left=lf, left_no_energy=left_poor[cid], left_with_energy=left_rich[cid], ignore_rate=round(lf / max(1, p + lf), 2)))
    rows.sort(key=lambda r: -r["ignore_rate"])
    energy_bound = sum(left_poor.values()) / max(1, sum(left.values()))
    return dict(turns=turns, energy_bound_share=round(energy_bound, 2), cards=rows)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--act", type=int)
    a = ap.parse_args()
    r = analyze(a.events, a.act)
    print(f"{r['turns']} turns; cards left in hand with no energy to spare: {r['energy_bound_share']:.0%} (high -> energy bottleneck)")
    print(f"{'card':28s} played  left  left@0E  left@>=1E  ignore")
    for x in r["cards"]:
        if x["left"]:
            print(f"{x['card']:28s} {x['played']:6d} {x['left']:5d} {x['left_no_energy']:8d} {x['left_with_energy']:10d}  {x['ignore_rate']:.2f}")


if __name__ == "__main__":
    main()
