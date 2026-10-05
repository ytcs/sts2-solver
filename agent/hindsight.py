"""Review of a costly fight (the outer loop's check for gaps in the solver): was the loss bad luck, or did the solver (or I) play a clearly worse line?

    python -m agent.hindsight runs/<run>/fights/<id>_<ENCOUNTER>.json [--budget 20] [--replays 40] [--min-gap 2] [--max-decisions 60]

The harness keeps the whole export of every fight that lost >= 30% of max HP (or was lost) under `runs/<run>/fights/`. Two questions per fight:

1. **Luck or systematic?** The fight is replayed from its start in the simulator `--replays` times with the live decision procedure (same deck, relics, HP, no potion
   policy tricks) at a normal budget: where does the real HP loss fall in that distribution? Above the 90th percentile = unlucky draws / enemy rolls, in the middle or
   below = this deck simply loses that much there (a deck / route problem, not a solver problem).
2. **Play by play.** At every decision of the real fight the simulator is rebuilt from what was observed (as in play), `Engine.decide` runs at a large budget
   (`--budget` seconds) and the line the solver prefers is compared with the action that was played, in HP (the solver's own value of both). Decisions where the
   best option beats the played one by `--min-gap` HP or more are listed: a pattern there (the same card, the same kind of turn) is a solver gap to fix; scattered
   small gaps are search noise. This conditions on what a player knew; the realized draws are not used (a "lucky line" would not be a gap).

A fight in the log may end one action before the last (the final sync precedes the killing blow); the review stops there.
"""
import argparse
import json
import os
import sys

import numpy as np
import sts2

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from agent.fight import Replayer  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def percentile_of_real(eng, scenario, real_lost, replays, budget):
    from agent.budget_replay import play_fight
    lost = []
    for k in range(replays):
        o, hp_lost, _ = play_fight(eng, scenario, 9000 + k, budget, keep_potions=False)
        lost.append(hp_lost if o == 1 else scenario["hp"])
    lost = np.array(lost, dtype=float)
    return lost, float((lost < real_lost).mean() + 0.5 * (lost == real_lost).mean())


def decisions(fight):
    """Yield (index, truncated fight export) for every action of the log: the state before action `index` is what the replayer holds after the truncated log."""
    log, states = fight["log"], fight.get("states")
    for i in range(len(log)):
        cut = dict(fight, log=log[:i], states=(states[: i + 1] if states else None), state=(states[i] if states and states[i] else fight["state"]))
        yield i, cut


def review(eng, export, budget, min_gap, max_decisions):
    fight, sc = export["fight"], export["scenario"]
    max_hp = max(sc.get("max_hp", 80), 1)
    rows = []
    for i, cut in decisions(fight):
        if len(rows) >= max_decisions:
            break
        rp = Replayer(cut)
        try:
            rp.advance(cut)
        except Exception:  # noqa: BLE001
            continue
        sim = rp.sim
        if sim.stage() == "over":
            break
        act = fight["log"][i] if isinstance(fight["log"][i], str) else json.dumps(fight["log"][i])
        legal = sim.legal()
        if len(legal) < 2:
            continue  # nothing to decide
        d = eng.decide(sc, sim, budget, tol_hp=0.0)
        if not d.get("searched"):
            continue
        played = None
        for o in d["options"]:
            if o.get("q") is not None and _same(sim, o["action"], act):
                played = o
        best = max((o for o in d["options"] if o.get("q") is not None), key=lambda o: o["q"], default=None)
        if played is None or best is None:
            rows.append(dict(index=i, text=_text(sim, act), note="played action outside the solver's top options"))
            continue
        gap_hp = (best["q"] - played["q"]) * max_hp / 0.5
        if gap_hp >= min_gap:
            rows.append(dict(index=i, played=played["text"], best=best["text"], gap_hp=round(gap_hp, 1), turn=_turn(rp)))
    return rows


def _same(sim, action_idx, act_json):
    try:
        return json.loads(sim.action_json(action_idx)) == json.loads(act_json)
    except Exception:  # noqa: BLE001
        return False


def _text(sim, act_json):
    for idx, t in sim.legal():
        if _same(sim, idx, act_json):
            return t
    return act_json


def _turn(rp):
    try:
        return json.loads(rp.sim.snapshot()).get("round")
    except Exception:  # noqa: BLE001
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("fight")
    ap.add_argument("--budget", type=float, default=20.0)
    ap.add_argument("--replays", type=int, default=40)
    ap.add_argument("--replay-budget", type=float, default=1.0)
    ap.add_argument("--min-gap", type=float, default=2.0, help="HP the best option must beat the played one by to be listed")
    ap.add_argument("--max-decisions", type=int, default=80)
    ap.add_argument("--no-luck", action="store_true")
    ap.add_argument("--log", action="store_true", help="append the verdict to evals/gaps.jsonl (read by `agent.improve`)")
    a = ap.parse_args()
    export = json.load(open(a.fight))
    from agent.engine import Engine
    eng = Engine()
    sc = export["scenario"]
    real_lost = export["hp_start"][0] - export["hp_end"][0]
    print(f"{export['encounter']}: HP {export['hp_start'][0]} -> {export['hp_end'][0]} (lost {real_lost}), {len(export['fight']['log'])} actions")
    if not a.no_luck:
        lost, pct = percentile_of_real(eng, sc, real_lost, a.replays, a.replay_budget)
        print(f"simulator replays from the start ({a.replays}): mean HP lost {lost.mean():.1f}, median {np.median(lost):.0f}, range {lost.min():.0f}-{lost.max():.0f};"
              f" the real loss is at the {100 * pct:.0f}th percentile -> {'unlucky draws / rolls' if pct >= 0.9 else 'about what this deck loses here'}")
    rows = review(eng, export, a.budget, a.min_gap, a.max_decisions)
    print(f"\ndecisions where a {a.budget:.0f}s search prefers another line by >= {a.min_gap} HP: {len([r for r in rows if 'gap_hp' in r])}")
    if a.log:
        gaps = [r for r in rows if "gap_hp" in r]
        rec = dict(kind="hindsight", fight=a.fight, encounter=export["encounter"], hp_lost=real_lost, luck_percentile=None if a.no_luck else round(pct, 2),
                   gaps=len(gaps), worst_gap_hp=max([r["gap_hp"] for r in gaps], default=0), decisions=[(r["index"], r["played"], r["best"], r["gap_hp"]) for r in gaps[:10]])
        with open(os.path.join(ROOT, "evals", "gaps.jsonl"), "a", encoding="utf-8") as f:
            f.write(json.dumps(rec) + chr(10))
    for r in rows:
        if "gap_hp" in r:
            print(f"  action {r['index']:3d} turn {r['turn']}: played `{r['played']}`  vs  `{r['best']}`  (+{r['gap_hp']} HP)")
        else:
            print(f"  action {r['index']:3d}: {r['text']} ({r['note']})")


if __name__ == "__main__":
    main()
