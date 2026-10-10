import re

import numpy as np

from agent import potions

HEAD_BIN, HEAD_NB = 2, 75
HEAD_NC = HEAD_NB + 1
WIN_ONLY_TIE = 0.01
ATTEMPTS = 32
PICK_ATTEMPTS = 8
NOISE = 2.0
LAST_ACT = 2
ARMS = ("now", "keep", "save")


def win_only_worth(max_hp, tie=WIN_ONLY_TIE):
    mx = max(int(max_hp or 1), 1)
    u = [0.0] + [1.0 + tie * min((b * HEAD_BIN - (HEAD_BIN - 1) / 2.0) / mx, 1.0) for b in range(1, HEAD_NB + 1)]
    return dict(u=u, price_u=[0.0] + [1.0] * HEAD_NB, kind="win only")


def fight_objective(scenario, bosses=(), seen=()):
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
    elif int(scenario.get("ascension") or 0) >= 10:
        final = any(str(e).endswith("_BOSS") and e != enc for e in seen)
    else:
        final = True
    if final:
        return win_only_worth(scenario.get("max_hp")), f"win only (the run's final boss; {tie})"
    return None, "linear (the first of the two final bosses: HP carries to the second)"


def end_class(won, hp):
    return int(min(max(int(np.ceil(max(hp, 0) / HEAD_BIN)), 1), HEAD_NB)) if won else 0


def score(row, max_hp, worth):
    won = row[0] == 1
    hp = row[2] if len(row) > 2 else row[1] * max_hp
    if worth is None:
        return 1.0 + 0.5 * row[1] if won else -1.0
    return float(worth.get("price_u", worth["u"])[end_class(won, hp)])


def _futures(sim, n, s0):
    out = []
    for r in range(n):
        c = sim.copy()
        c.determinize(1_000_003 * (r + 1) + s0)
        out.append(c)
    return out, [7_919 * (r + 1) + s0 for r in range(n)]


def _keep_start(base, actions, others):
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
    every_pot = potions.live_slots(sim)
    pots = [(i, pid) for i, pid in every_pot if i not in skip]
    if not pots:
        return []
    mx = scenario.get("max_hp", 80)
    every = [n for n, _ in every_pot]
    bases, seeds = _futures(sim, attempts, seed)
    picks, pseeds = _futures(sim, PICK_ATTEMPTS, seed + 500_009)

    def only(c, i):
        left = [n for n in every if n != i]
        return c.without_potions(left) if left else c.copy()

    def thrown(c, a, i):
        c = only(c, i)
        c.step(a)
        return c

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
        keep = [got.get((i, "keep", r), save_rows[r]) for r in range(attempts)]
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
    def free(r):
        return boss or r["id"] not in aside
    now = [r for r in rows if r["beats"] and free(r)]
    if now:
        return max(now, key=lambda r: r["d"]["now_save"][0]), "using it now beats keep and save beyond noise"
    stake = [r for r in rows if r["stake"]]
    if stake:
        return max(stake, key=lambda r: r["dwin"]["best_save"][0]), "the win is at stake: a potion changes this fight's win beyond noise"
    return None, ""


def table(rows, turn, why, aside=(), boss=False):
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
