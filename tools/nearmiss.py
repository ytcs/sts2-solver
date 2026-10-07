#!/usr/bin/env python3
"""Near-miss losses of collected fights, and restart states inside them (the user's idea: spend search compute where a different line could flip a loss).

  tools/nearmiss.py target/exit/r3_*.npz --out target/exit/nm_r3.json [--turns 3] [--frac 0.2]

Replays every lost fight of the `rl/exit.py collect` parts (`sts2.Sim` from the scenario and the job seed, then the recorded actions; exact) and keeps a
loss as a near-miss when the enemies' HP + block left at the end is within one turn of the damage I was dealing (`rem <= mean HP the enemies lost per
turn over the last 3 turns`) or at most `--frac` of their max HP. For each near-miss it writes restart states at the start of each of the last `--turns`
player turns (the turn's hand drawn, `Play` phase): the scenario, the original job seed and the action prefix that reaches the state.

`rl/exit.py collect --restarts` searches from these TRUE states (the real RNG continues: replayable from seed + prefix + new actions). The real future
of a restart is the one that produced a loss (the states were selected on it), so their outcome labels are pessimistic: collect marks such parts
`policy_only` and `train` leaves them out of the outcome loss; the policy target (search estimates over determinized futures) is unaffected.
"""
import argparse, glob, json, os, sys, time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl"))
import sts2  # noqa: E402


def enemy_hp(sn):
    """HP + block of the living enemies, and their max HP (all enemies, summons included)."""
    return sum(e["hp"] + e["block"] for e in sn["enemies"] if e["alive"]), sum(e["max_hp"] for e in sn["enemies"])


def scan_fight(sj, seed, acts):
    """Replays one fight: (enemy HP left at the end, the largest enemy max HP seen, mean enemy HP lost per turn over the last 3 turns,
    [(prefix length, turn)] at every player turn start, outcome)."""
    s = sts2.Sim(sj, int(seed))
    sn = json.loads(s.snapshot())
    h, m = enemy_hp(sn)
    turn, hist, starts, mx = sn["turn"], [h], [(0, sn["turn"])], m
    for k, a in enumerate(acts):
        s.step(int(a))
        sn = json.loads(s.snapshot())
        h, m = enemy_hp(sn)
        mx = max(mx, m)
        if sn.get("turn", turn) != turn:
            turn = sn["turn"]
            hist.append(h)
            if sn.get("phase") == "Play" and not sn.get("combat_over"):
                starts.append((k + 1, turn))
    hist.append(h)
    dpt = [hist[i] - hist[i + 1] for i in range(max(0, len(hist) - 4), len(hist) - 1)]
    return h, mx, float(np.mean(dpt)) if dpt else 0.0, starts, s.outcome()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("parts", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--turns", type=int, default=3, help="restart states at the start of each of the last N player turns")
    ap.add_argument("--frac", type=float, default=0.2, help="also a near-miss: enemy HP left <= this share of their max HP")
    ap.add_argument("--limit", type=int, default=0, help="at most this many near-misses (0: all)")
    a = ap.parse_args()
    files = sorted(f for p in a.parts for f in glob.glob(p))
    t0, n_loss, bad, near, restarts = time.time(), 0, 0, [], []
    for fn in files:
        z = np.load(fn, allow_pickle=True)
        scen = json.loads(str(z["scenarios"]))
        sj = [json.dumps(s) for s in scen]
        for i in np.nonzero(z["f_cls"] == 0)[0]:
            n_loss += 1
            acts = z["acts"][z["f_off"][i]:z["f_off"][i + 1]]
            si, seed = int(z["f_scen"][i]), int(z["f_seed"][i])
            rem, mx, dpt, starts, oc = scan_fight(sj[si], seed, acts)
            if oc != -1:
                bad += 1
                continue
            one_turn = rem <= max(dpt, 0.0)
            if not (one_turn or rem <= a.frac * max(mx, 1)):
                continue
            near.append(dict(part=fn, fight=int(i), encounter=scen[si].get("encounter"), rem=rem, max_hp=mx, dpt=dpt, one_turn=bool(one_turn), turns=starts[-1][1]))
            for plen, turn in starts[-a.turns:]:
                restarts.append(dict(scenario=scen[si], seed=seed, prefix=[int(x) for x in acts[:plen]], turn=turn, near=len(near) - 1))
            if a.limit and len(near) >= a.limit:
                break
        if a.limit and len(near) >= a.limit:
            break
    one = sum(r["one_turn"] for r in near)
    print(f"{n_loss} losses in {len(files)} parts ({time.time() - t0:.0f}s): {len(near)} near-misses ({len(near) / max(n_loss, 1):.3f}; {one} within one turn), "
          f"{len(restarts)} restart states; replay mismatches {bad}")
    json.dump(dict(near=near, restarts=restarts), open(a.out, "w"))
    print(f"-> {a.out}")


if __name__ == "__main__":
    main()
