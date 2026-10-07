"""Potion proposals and the per-fight objective (`docs/rebuild.md` S4). Stdlib + numpy; the engine is passed in.

The solver sees every potion and PROPOSES; only the operator commits one, one potion per commit, and the next turn's proposal re-prices whether another
is needed. The live card-play search plans without potions (a line that counts on a potion nobody committed is the wrong line; Living Fog, 99th
percentile), never plays one, and every turn this module prices each potion three ways.

Per potion p (simulator slot), at the start of each player turn, three arms on the same N determinized futures and job seeds (common random numbers),
each played to the fight's end by the batch solver under the fight's objective. The other potions are out of every arm: each potion is priced on its own,
and after a commit the next turn re-prices the rest.
  now   p thrown now at its best target (the target is chosen on other futures, so the reported gain is not the max of noisy estimates), then no potion
  keep  this turn played without potions, p usable from the next turn on (the save arm's actions of this turn are replayed on a copy that still holds p,
        then the batch solver plays on with p in the belt)
  save  p not used this fight: the belt without it (`Sim.without_potions`)
Each arm reports P(win) and the end-HP distribution over all futures (a loss counts 0 HP), and its score in the fight's objective.

Stop (`turn` / `combat` print the proposal and return):
  - using p now beats keep AND save beyond noise: both paired score differences above 2 paired standard errors;
  - or the win is at stake: the better of now / keep wins more often than save, beyond 2 paired se on P(win).
A potion set aside for the boss (`potion aside`) stops a non-boss fight only when the win is at stake. No other thresholds.

Objective (`fight_objective`): an act boss followed by the ancient's heal (the acts 1-2 bosses; in the last act the final boss: at A10 the second of the
two, since HP carries from the first) is searched and priced on P(win) only: the win-only table over the outcome head's classes (loss 0, a win 1 plus a
tiny HP tiebreak), through the search's per-job worth (`rl/fastsearch.py` `worth_row`, `crates/sts2env/src/search.rs` `Worth`). Every other fight keeps
the linear return (win +1 + 0.5 x end HP / max HP, loss -1) until a continuation value supplies per-fight tables.
"""
import re

import numpy as np

from agent import potions

# the outcome head's classes (`rl/heads.py`; `agent.engine` asserts they agree): class 0 a loss, class b a win ending at HP in ((b - 1) * BIN, b * BIN]
HEAD_BIN, HEAD_NB = 2, 75
HEAD_NC = HEAD_NB + 1
WIN_ONLY_TIE = 0.01  # win-only table: ending at full HP adds this much to a win (1% of a win): a tiebreak between equal win chances, nothing more
ATTEMPTS = 32  # paired futures per arm
PICK_ATTEMPTS = 8  # futures (other seeds) that choose a potion's target before the reported arms
NOISE = 2.0  # "beyond noise" = more than this many paired standard errors
LAST_ACT = 2  # 0-based: the act whose final boss ends the run
ARMS = ("now", "keep", "save")


# ------------------------------------------------------------------ the per-fight objective

def win_only_worth(max_hp, tie=WIN_ONLY_TIE):
    """The win-only table: loss 0, a win 1 + tie x (end HP / max HP) at each win class's centre."""
    mx = max(int(max_hp or 1), 1)
    u = [0.0] + [1.0 + tie * min((b * HEAD_BIN - (HEAD_BIN - 1) / 2.0) / mx, 1.0) for b in range(1, HEAD_NB + 1)]
    return dict(u=u, kind="win only")


def fight_objective(scenario, bosses=(), seen=()):
    """(worth table or None for the linear return, why). `bosses`: the act's boss(es) in fight order (the map); `seen`: encounters met this act before this
    fight (finds the second of a double boss when the map is unread)."""
    enc = str(scenario.get("encounter", ""))
    if not enc.endswith("_BOSS"):
        return None, "linear (win +1 + 0.5 x end HP / max HP, loss -1): HP carries to the next fights"
    act = scenario.get("act")
    tie = f"end HP only a {WIN_ONLY_TIE:.0%} tiebreak"
    if act is None:
        return None, "linear (a boss of an unknown act)"
    if act < LAST_ACT:
        return win_only_worth(scenario.get("max_hp")), f"win only (act {act + 1} boss, the ancient's heal follows; {tie})"
    bosses = [b for b in bosses if b]
    if len(bosses) >= 2:
        final = enc == bosses[-1] and enc != bosses[0]
    elif int(scenario.get("ascension") or 0) >= 10:  # double boss with the map unread: the second when another boss was met this act
        final = any(str(e).endswith("_BOSS") and e != enc for e in seen)
    else:
        final = True
    if final:
        return win_only_worth(scenario.get("max_hp")), f"win only (the run's final boss; {tie})"
    return None, "linear (the first of the two final bosses: HP carries to the second)"


def end_class(won, hp):
    return int(min(max(int(np.ceil(max(hp, 0) / HEAD_BIN)), 1), HEAD_NB)) if won else 0


def score(row, max_hp, worth):
    """One play-out (outcome, end HP fraction[, end HP]) in the objective's units."""
    won = row[0] == 1
    hp = row[2] if len(row) > 2 else row[1] * max_hp
    if worth is None:
        return 1.0 + 0.5 * row[1] if won else -1.0
    return float(worth["u"][end_class(won, hp)])


# ------------------------------------------------------------------ pricing

def _futures(sim, n, s0):
    out = []
    for r in range(n):
        c = sim.copy()
        c.determinize(1_000_003 * (r + 1) + s0)
        out.append(c)
    return out, [7_919 * (r + 1) + s0 for r in range(n)]


def _keep_start(base, actions, others):
    """The keep arm's start for one future: replay the save arm's actions of this turn on `base` (holding p, the other potions removed) up to and including
    its first end turn and the selections that end of turn asks for. Returns (sim, how): how = "play" (play on from it), "over" (the fight ended this turn:
    the save outcome stands) or "replay failed"."""
    c = base.without_potions(others) if others else base.copy()
    ended = False
    for a in actions:
        if c.outcome() != 0 or c.stage() == "over":
            return c, "over"
        if ended and c.stage() != "choice":
            return c, "play"
        text = dict(c.legal()).get(a)
        if text is None:
            return c, "replay failed"
        c.step(a)
        ended = ended or text == "end turn"
    if c.outcome() != 0 or c.stage() == "over":
        return c, "over"
    return c, ("play" if ended and c.stage() != "choice" else "replay failed")


def _summary(rows, mx, worth):
    won = np.array([r[0] == 1 for r in rows], float)
    hp = np.array([(r[2] if len(r) > 2 else r[1] * mx) if r[0] == 1 else 0.0 for r in rows], float)
    sc = np.array([score(r, mx, worth) for r in rows], float)
    q = np.percentile(hp, [10, 50, 90]) if len(hp) else [0, 0, 0]
    return dict(win=float(won.mean()), hp=float(hp.mean()), q10=float(q[0]), q50=float(q[1]), q90=float(q[2]), score=float(sc.mean()), _won=won, _sc=sc)


def _paired(a, b):
    d = a - b
    return float(d.mean()), float(d.std(ddof=1) / len(d) ** 0.5) if len(d) > 1 else 0.0


def price(engine, scenario, sim, worth=None, attempts=ATTEMPTS, seed=0, skip=()):
    """The three arms of every potion usable now (simulator slots not in `skip`). Returns [row]: dict(i, id, action, text, arms={now, keep, save: summary},
    d={now_keep, now_save, keep_save: (mean, se)} in score units, dwin={...} in P(win), beats, stake, verdict, attempts, replay_failed)."""
    every_pot = potions.live_slots(scenario, sim)
    pots = [(i, pid) for i, pid in every_pot if i not in skip]
    if not pots:
        return []
    mx = scenario.get("max_hp", 80)
    every = [n for n, _ in every_pot]
    bases, seeds = _futures(sim, attempts, seed)
    picks, pseeds = _futures(sim, PICK_ATTEMPTS, seed + 500_009)

    def only(c, i):  # p alone in the belt
        left = [n for n in every if n != i]
        return c.without_potions(left) if left else c.copy()

    def thrown(c, a, i):
        c = only(c, i)
        c.step(a)
        return c

    # call 1: the save arm (recorded: the keep arm replays its turn) and the target picks
    targets = {}
    jobs, jseeds, tags = [b.without_potions(every) for b in bases], list(seeds), []
    for i, _ in pots:
        acts = [(a, t) for a, t in sim.legal() if re.match(rf"potion {i}( |$)", t)]
        targets[i] = acts
        if len(acts) > 1:
            for a, t in acts:
                for c, sd in zip(picks, pseeds):
                    jobs.append(thrown(c, a, i))
                    jseeds.append(sd)
                    tags.append((i, a))
    res = engine.play_on(scenario, jobs, jseeds, worth=worth, record=True)
    save_rows = [r[:3] for r in res[:attempts]]
    save_acts = [r[3] if len(r) > 3 else [] for r in res[:attempts]]
    pick_score = {}
    for (i, a), r in zip(tags, res[attempts:]):
        pick_score.setdefault((i, a), []).append(score(r, mx, worth))
    chosen = {}
    for i, acts in targets.items():
        if acts:
            chosen[i] = max(acts, key=lambda at: np.mean(pick_score.get((i, at[0]), [0.0]))) if len(acts) > 1 else acts[0]

    # call 2: now and keep arms of every potion
    jobs, jseeds, slots = [], [], []
    keep_fixed = {}
    for i, _ in pots:
        if i not in chosen:
            continue
        a, _t = chosen[i]
        for r, (b, sd) in enumerate(zip(bases, seeds)):
            jobs.append(thrown(b, a, i))
            jseeds.append(sd)
            slots.append((i, "now", r))
        others = [n for n in every if n != i]
        for r, (b, sd) in enumerate(zip(bases, seeds)):
            c, how = _keep_start(b, save_acts[r], others)
            if how == "play":
                jobs.append(c)
                jseeds.append(sd)
                slots.append((i, "keep", r))
            else:
                keep_fixed[(i, r)] = how
    res = engine.play_on(scenario, jobs, jseeds, worth=worth) if jobs else []
    got = {}
    for (i, arm, r), row in zip(slots, res):
        got[(i, arm, r)] = row[:3]

    out = []
    for i, pid in pots:
        if i not in chosen:
            continue
        a, t = chosen[i]
        now = [got[(i, "now", r)] for r in range(attempts)]
        keep = [got.get((i, "keep", r), save_rows[r]) for r in range(attempts)]  # the fight over this turn (or a failed replay): the save outcome stands
        arms = dict(now=_summary(now, mx, worth), keep=_summary(keep, mx, worth), save=_summary(save_rows, mx, worth))
        d = dict(now_keep=_paired(arms["now"]["_sc"], arms["keep"]["_sc"]), now_save=_paired(arms["now"]["_sc"], arms["save"]["_sc"]),
                 keep_save=_paired(arms["keep"]["_sc"], arms["save"]["_sc"]))
        best_win = arms["now"]["_won"] if arms["now"]["win"] >= arms["keep"]["win"] else arms["keep"]["_won"]
        dwin = dict(now_keep=_paired(arms["now"]["_won"], arms["keep"]["_won"]), now_save=_paired(arms["now"]["_won"], arms["save"]["_won"]),
                    keep_save=_paired(arms["keep"]["_won"], arms["save"]["_won"]), best_save=_paired(best_win, arms["save"]["_won"]))
        row = dict(i=i, id=pid, action=int(a), text=t, arms={k: {x: y for x, y in v.items() if not x.startswith("_")} for k, v in arms.items()}, d=d, dwin=dwin,
                   attempts=attempts, replay_failed=sum(1 for (j, _), how in keep_fixed.items() if j == i and how == "replay failed"))
        row.update(judge(row))
        out.append(row)
    return out


def _beyond(m_se):
    m, se = m_se
    return m > 0 and m > NOISE * se


def judge(row):
    """beats (now beats keep and save beyond noise), stake (a potion changes this fight's win beyond noise), verdict (use now / keep / save)."""
    beats = _beyond(row["d"]["now_keep"]) and _beyond(row["d"]["now_save"])
    stake = _beyond(row["dwin"]["best_save"])
    if beats:
        verdict = "use now"
    elif _beyond(row["d"]["keep_save"]):
        verdict = "keep (worth using later this fight)"
    else:
        verdict = "save (no gain in this fight beyond noise)"
    return dict(beats=bool(beats), stake=bool(stake), verdict=verdict)


def proposal(rows, aside=(), boss=False):
    """(row to propose or None, why): the potion whose 'now' beats keep and save by the most (a potion set aside for the boss only in a boss fight), else
    the one that changes this fight's win the most when the win is at stake."""
    def free(r):
        return boss or r["id"] not in aside
    now = [r for r in rows if r["beats"] and free(r)]
    if now:
        return max(now, key=lambda r: r["d"]["now_save"][0]), "using it now beats keep and save beyond noise"
    stake = [r for r in rows if r["stake"]]
    if stake:
        return max(stake, key=lambda r: r["dwin"]["best_save"][0]), "the win is at stake: a potion changes this fight's win beyond noise"
    return None, ""


# ------------------------------------------------------------------ the table

def table(rows, turn, why, aside=(), boss=False):
    """The proposal table: one block of three rows (now / keep / save) per potion, all in the same units."""
    n = rows[0]["attempts"] if rows else ATTEMPTS
    lines = [f"POTIONS (turn {turn}; objective: {why}; {n} paired futures per arm, each potion priced alone; end HP over all futures, a loss counts 0)",
             f"  {'potion':<22}{'arm':<6}{'P(win)':>7}{'HP mean':>9}{'p10':>5}{'p50':>5}{'p90':>5}{'score':>9}   vs save (paired)"]
    for r in rows:
        for k, arm in enumerate(ARMS):
            s = r["arms"][arm]
            label = r["id"] if k == 0 else (f"({r['text']})" if k == 1 else ("set aside" if r["id"] in aside and not boss else ""))
            vs = "" if arm == "save" else (lambda m_se: f"{m_se[0]:+.3f} ±{m_se[1]:.3f}")(r["d"][f"{arm}_save"])
            lines.append(f"  {label:<22}{arm:<6}{s['win']:>7.2f}{s['hp']:>9.1f}{s['q10']:>5.0f}{s['q50']:>5.0f}{s['q90']:>5.0f}{s['score']:>+9.3f}   {vs}")
        nk = r["d"]["now_keep"]
        extra = f"; {r['replay_failed']} keep replays failed (save outcome used)" if r.get("replay_failed") else ""
        lines.append(f"  -> {r['verdict'].upper()}: now - keep {nk[0]:+.3f} ±{nk[1]:.3f}; win: best of now/keep - save "
                     f"{r['dwin']['best_save'][0]:+.2f} ±{r['dwin']['best_save'][1]:.2f}{extra}")
    return lines
