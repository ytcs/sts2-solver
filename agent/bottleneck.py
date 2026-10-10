#!/usr/bin/env python3
"""Deck bottleneck tracker (plan item 4b): per card, plays vs turns it was left in hand at end of turn (from the run log's `action` events:
`play X` texts and `end turn` events with hand_left / energy_left). Diagnosis, an input not a verdict:
- left with energy_left < its cost (< 1 without a logged cost) -> energy bottleneck; unplayable (cost < 0) not counted;
- stranded = non-starter cards of cost >= 2 left with energy_left < cost, per turn (control arm: r -0.61 with death floor in act 1, n 20);
- left while energy remained -> the deck chose not to play it: removal candidate, or a combo piece waiting for its partner (draw / retain);
Usage: python -m agent.bottleneck <events.jsonl> [--act N]"""
import argparse, collections, json, re

STARTER = {"STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "BASH", "ASCENDERS_BANE"}


def analyze(path, act=None):
    from agent import synergy
    played, left, left_poor, left_rich = collections.Counter(), collections.Counter(), collections.Counter(), collections.Counter()
    turns = stranded = 0
    cur_act = None
    for l in open(path, encoding="utf-8"):
        e = json.loads(l)
        if e["kind"] == "fight_start":
            cur_act = (e.get("scenario") or {}).get("act")
        if e["kind"] != "action" or (act is not None and cur_act != act):
            continue
        t = str(e.get("text", ""))
        m = re.match(r"play (\S+)", t)
        if m:
            played[m.group(1).split("#")[0].rstrip("+")] += 1
        elif t.startswith("end turn") and "hand_left" in e:
            turns += 1
            en = e.get("energy_left") or 0
            for cid, c in zip(e["hand_left"], e.get("hand_cost") or [None] * len(e["hand_left"])):
                if c is not None and c < 0:
                    continue
                base = cid.rstrip("+")
                left[base] += 1
                (left_rich if en >= (1 if c is None else c) else left_poor)[base] += 1
                k = synergy.item_tags(base)[2] if c is None else c
                stranded += base not in STARTER and k >= 2 and en < k
    rows = []
    for cid in sorted(set(played) | set(left)):
        p, lf = played[cid], left[cid]
        rows.append(dict(card=cid, played=p, left=lf, left_no_energy=left_poor[cid], left_with_energy=left_rich[cid], ignore_rate=round(lf / max(1, p + lf), 2)))
    rows.sort(key=lambda r: -r["ignore_rate"])
    energy_bound = sum(left_poor.values()) / max(1, sum(left.values()))
    return dict(turns=turns, energy_bound_share=round(energy_bound, 2), ignored_share=round(sum(left_rich.values()) / max(1, sum(left.values())), 2),
                stranded=round(stranded / max(1, turns), 2), cards=rows)


_CACHE = {}


def term_vars(path, act):
    """macro terms DSL variables (agent/terms.py) for this act, cached by file size"""
    import os
    try:
        key = (path, os.path.getsize(path), act)
    except OSError:
        return None
    if key not in _CACHE:
        r = analyze(path, act)
        _CACHE.clear()
        _CACHE[key] = dict(bn_turns=r["turns"], energy_bound=r["energy_bound_share"], ignored_share=r["ignored_share"], stranded=r["stranded"])
    return _CACHE[key]


def line(path, act=None, top=4):
    """one line for `brief`: energy-bound share and the most ignored cards (left at end of turn, no energy / with energy / played)"""
    r = analyze(path, act)
    if not r["turns"]:
        return ""
    ign = [f"{c['card']} {c['left_no_energy']}/{c['left_with_energy']}/{c['played']}" for c in r["cards"] if c["left"] >= 3 and c["ignore_rate"] >= 0.5][:top]
    return (f"bottleneck ({r['turns']} turns{'' if act is None else f', act {act + 1}'}): cards left with no energy {r['energy_bound_share']:.0%}"
            + (f"; ignored (left@0E/left@>=1E/played): {', '.join(ign)}" if ign else ""))


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
