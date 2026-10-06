"""What a potion is worth spent in this fight vs kept, in one unit: the probability of winning the act boss (run survival through the act).

    throw = E[ V_without_p(end HP) ]   over this fight played on from now with only p allowed
    hold  = E[ V_with_p(end HP) ]      over this fight played on from now with no potion

End HP comes from the batch solver playing the fight from the current state (both arms on the same determinized futures and seeds: common random
numbers; a loss ends at 0). V(hp) = P(win the act boss | leave this node with hp) is the route DP (`routes.continuation_values`): rests, the fights left,
and the other potions thrown where they help most, with p in the budget or spent. In the act boss itself the HP does not carry (the next act starts healed):
V is then the win of the next act's bosses with / without p (`macro.evaluate`), and the end HP only matters through winning; when today's deck wins that
boss under `REACH`, only this fight's win counts.

This is the decision layer of the RL redesign in miniature (`docs/rl_redesign.md`): the network predicts the fight's outcome distribution, the run-level
worth of HP is applied outside it.
"""
import json

import numpy as np


REACH = 0.05  # below this next-act boss win, today's deck says nothing about the potion's later worth (as `routes`: then the goal is to survive)
ALERT_WIN = 0.03  # the per-turn check alerts when throwing a potion now adds this much win over the rest of the fight ...
ALERT_HP = 0.10  # ... or saves this share of max HP (mean end HP, a loss counted as 0), in both cases beyond 2 paired se
NOW_ATTEMPTS = 32  # futures per arm of the per-turn check


def now_vs_hold(engine, scenario, sim, skip=(), attempts=NOW_ATTEMPTS, seed=0):
    """The per-turn question: does throwing a potion NOW save HP (or win) over the rest of this fight? Per potion in the simulator (packed index, not in
    `skip`): the fight played on from now after throwing it now (its best target) with no potion after, vs played on with no potion at all; both arms on the
    same determinized futures and job seeds. Returns [dict(i, id, action, win_now, win_hold, hp_now, hp_hold, d_win, d_hp, se_win, se_hp, alert)]."""
    from agent.potions import live_slots
    pots = live_slots(scenario, sim)
    if not pots:
        return []
    mx = scenario.get("max_hp", 80)
    every = [n for n, _ in pots]
    seeds = [7_919 * (r + 1) + seed for r in range(attempts)]
    bases = []
    for r in range(attempts):
        c = sim.copy()
        c.determinize(1_000_003 * (r + 1) + seed)
        bases.append(c)

    def ends(starts):
        res = engine.play_on(scenario, starts, seeds)
        return np.array([hp * mx if oc == 1 else 0.0 for oc, hp in res])
    hold = ends([b.without_potions(every) for b in bases])
    out = []
    for i, pid in pots:
        if i in skip:
            continue
        best = None
        for a, t in sim.legal():
            if not t.startswith(f"potion {i}") or t.startswith("discard"):
                continue
            starts = []
            for b in bases:
                c = b.copy()
                c.step(a)
                left = [n for n in every if n != i]
                starts.append(c.without_potions(left) if left else c)
            e = ends(starts)
            if best is None or e.mean() > best[2].mean():
                best = (a, t, e)
        if best is None:
            continue
        a, t, now = best
        dw, dh = (now > 0).astype(float) - (hold > 0), now - hold
        se = lambda x: float(x.std(ddof=1) / len(x) ** 0.5) if len(x) > 1 else 0.0  # noqa: E731
        r = dict(i=i, id=pid, action=t, win_now=float((now > 0).mean()), win_hold=float((hold > 0).mean()), hp_now=float(now.mean()), hp_hold=float(hold.mean()),
                 d_win=float(dw.mean()), d_hp=float(dh.mean()), se_win=se(dw), se_hp=se(dh), attempts=attempts)
        r["alert"] = bool((r["d_win"] >= ALERT_WIN and r["d_win"] > 2 * r["se_win"]) or (r["d_hp"] >= ALERT_HP * mx and r["d_hp"] > 2 * r["se_hp"]))
        out.append(r)
    return out


def now_text(r):
    """One line of the per-turn check."""
    return (f"  {r['id']}: throw NOW ({r['action']}) then none vs never this fight: win {r['win_now']:.2f} vs {r['win_hold']:.2f} ({r['d_win']:+.2f} ±{r['se_win']:.2f}), "
            f"mean end HP {r['hp_now']:.0f} vs {r['hp_hold']:.0f} ({r['d_hp']:+.1f} ±{r['se_hp']:.1f}; a loss counts 0; {r['attempts']} paired futures)")


def fight_ends(engine, scenario, sim, drop, attempts=64, seed=0):
    """End HP per attempt (0 = a loss or a stalled fight) of the fight continued from `sim` by the batch solver, the potions at simulator indices `drop`
    removed. Attempt r uses the same determinized future and job seed whatever `drop` is (paired arms)."""
    base = sim.without_potions(sorted(drop)) if drop else sim
    starts = []
    for r in range(attempts):
        c = base.copy()
        c.determinize(1_000_003 * (r + 1) + seed)
        starts.append(c)
    res = engine.play_on(scenario, starts, [7_919 * (r + 1) + seed for r in range(attempts)])
    mx = scenario.get("max_hp", 80)
    return np.array([hp * mx if oc == 1 else 0.0 for oc, hp in res])


def _at(V, ends):
    """V over integer HP evaluated at each end HP (0 = a loss: worth 0)."""
    idx = np.clip(np.round(ends).astype(int), 0, len(V) - 1)
    return np.where(ends > 0, np.asarray(V)[idx], 0.0)


def price(engine, scenario, sim, i, deck_json, act_values=None, next_boss=None, attempts=128, seed=0):
    """Throw vs hold for the potion at simulator index i. `act_values(spend)` -> ([V per entry of spend], goal) or (None, why) (the route DP) for a fight inside an act;
    `next_boss(drop_ids)` -> win of the next act's bosses for the act boss. Returns a dict (throw, hold, diff, se, win_throw, win_hold, ...)."""
    from agent.potions import live_slots
    pots = dict(live_slots(scenario, sim))  # slot -> id of the potions usable now (slots stay fixed when one is thrown)
    pid = pots[i]
    every = set(pots)
    only = fight_ends(engine, scenario, sim, every - {i}, attempts, seed)  # every other slot dropped, duplicates of p included: exactly one p allowed
    none = fight_ends(engine, scenario, sim, every, attempts, seed)
    out = dict(potion=pid, attempts=attempts, win_throw=float((only > 0).mean()), win_hold=float((none > 0).mean()),
               hp_throw=float(only[only > 0].mean()) if (only > 0).any() else 0.0, hp_hold=float(none[none > 0].mean()) if (none > 0).any() else 0.0)
    if act_values is not None:
        Vs, why = act_values(((pid,), ()))  # p spent / p kept
        if Vs is None:
            return dict(out, error=f"no route value: {why}")
        a, b = _at(Vs[0], only), _at(Vs[1], none)
        out.update(kind="act", V_full=float(Vs[1][-1]))
    elif next_boss is not None and next_boss([]) >= REACH:
        w_without, w_with = next_boss([pid]), next_boss([])
        a, b = (only > 0) * w_without, (none > 0) * w_with
        out.update(kind="next act", next_win_with=w_with, next_win_without=w_without)
    else:  # no later fight to price against, or the deck is far from the next boss (the deck will change before it): this fight's win alone
        a, b = (only > 0).astype(float), (none > 0).astype(float)
        out.update(kind="this fight")
    d = a - b
    out.update(throw=float(a.mean()), hold=float(b.mean()), diff=float(d.mean()), se=float(d.std(ddof=1) / len(d) ** 0.5) if len(d) > 1 else 0.0)
    return out


def text(p):
    """The gate's lines for one price."""
    if "error" in p:
        return [f"  {p['potion']}: no price ({p['error']})"]
    unit = {"act": "P(win the act boss)", "next act": "P(win here) x P(win the next act's boss)", "this fight": "P(win this fight)"}[p["kind"]]
    if p["kind"] == "this fight":  # nothing later is priced: keeping it is worth nothing measurable
        verdict = "SPEND it here (no later fight priced)" if p["diff"] > 0 else "no gain here"
    else:
        verdict = "SPEND it in this fight" if p["diff"] > 2 * p["se"] else ("KEEP it" if p["diff"] < -2 * p["se"] else "a tie within 2 se")
    lines = [f"  {p['potion']}: spend in this fight {p['throw']:.3f} vs keep {p['hold']:.3f} in {unit}: {p['diff']:+.3f} ±{p['se']:.3f} (paired, {p['attempts']} futures) -> {verdict}",
             f"    this fight with it: win {p['win_throw']:.2f}, end HP {p['hp_throw']:.0f} on a win; without any potion: win {p['win_hold']:.2f}, end HP {p['hp_hold']:.0f}"]
    if p["kind"] == "next act":
        lines.append(f"    next act's boss win with it {p['next_win_with']:.2f}, without {p['next_win_without']:.2f}")
    lines.append("    whether to spend it here, not when: timing within the fight is my call (a depth-2 tie on 'now' says nothing about timing)")
    return lines
