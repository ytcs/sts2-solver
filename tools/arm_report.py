#!/usr/bin/env python3
"""Pre-registered read-out for a macro arm vs a control (review section 10): paired ab metrics plus, per arm, potions held at the
final pair, elites per act, deck size at the pair, act-2 boss pass, and policy events by component.
usage: tools/arm_report.py <arm rows .jsonl> <arm events dir> <control rows .jsonl> <control events dir>"""
import collections, glob, json, os, subprocess, sys

import numpy as np


def stats(rows_path, events_dir):
    rows = {json.loads(l)["seed"]: json.loads(l) for l in open(rows_path)}
    out = {}
    for f in sorted(glob.glob(os.path.join(events_dir, "**", "events.jsonl"), recursive=True)):
        seed = os.path.basename(os.path.dirname(f))
        if seed not in rows or seed in out:
            continue
        el = collections.Counter(); pots_pair = None; deck_pair = None; a2_boss = None; pol = collections.Counter()
        for l in open(f, encoding="utf-8"):
            e = json.loads(l)
            if e["kind"] == "fight_start":
                if "ELITE" in e["encounter"]:
                    el[e.get("act")] += 1
                if e["encounter"].endswith("_BOSS") and e.get("act") == 2 and pots_pair is None:
                    pots_pair = len(e["scenario"].get("potions", [])); deck_pair = len(e["scenario"]["deck"])
                if e["encounter"].endswith("_BOSS") and e.get("act") == 1:
                    a2_boss = 0
            elif e["kind"] == "fight_end" and a2_boss == 0:
                hp = e.get("hp"); hp = json.loads(hp) if isinstance(hp, str) else hp
                a2_boss = 1 if hp and hp[0] > 0 else 0
                a2_boss = a2_boss if a2_boss else -1
            elif e["kind"] == "policy":
                pol[e.get("component")] += 1
        out[seed] = dict(e1=el.get(0, 0), e2=el.get(1, 0), e3=el.get(2, 0), pots_pair=pots_pair, deck_pair=deck_pair,
                         a2_boss=None if a2_boss is None else int(a2_boss == 1), pol=pol, floor=rows[seed]["floor"], won=rows[seed]["result"] == "win")
    return out


def summary(name, s, seeds):
    v = lambda k: [s[x][k] for x in seeds if x in s and s[x][k] is not None]  # noqa: E731
    pol = sum((s[x]["pol"] for x in seeds if x in s), collections.Counter())
    print(f"{name}: n {len([x for x in seeds if x in s])} | wins {sum(v('won'))} | final pair reached {sum(1 for x in v('floor') if x >= 48)} | "
          f"elites A1 {np.mean(v('e1')):.2f} A2 {np.mean(v('e2')):.2f} A3 {np.mean(v('e3')):.2f} | potions at pair {np.mean(v('pots_pair')) if v('pots_pair') else float('nan'):.2f} (n {len(v('pots_pair'))}) | "
          f"deck at pair {np.mean(v('deck_pair')) if v('deck_pair') else float('nan'):.1f} | act-2 boss passed {sum(v('a2_boss'))}/{len(v('a2_boss'))}" + (f" | policy {dict(pol)}" if pol else ""))


def main():
    arm_rows, arm_ev, ctl_rows, ctl_ev = sys.argv[1:5]
    a, c = stats(arm_rows, arm_ev), stats(ctl_rows, ctl_ev)
    seeds = sorted(set(a) & set(c))
    summary("arm", a, seeds)
    summary("control", c, seeds)
    tmp = os.path.join(os.path.dirname(arm_rows), "_paired_ctl.jsonl")
    with open(tmp, "w") as fh:
        for l in open(ctl_rows):
            if json.loads(l)["seed"] in seeds:
                fh.write(l)
    subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "ab.py"), tmp, arm_rows])
    os.remove(tmp)


if __name__ == "__main__":
    main()
