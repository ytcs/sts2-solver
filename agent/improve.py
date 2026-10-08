#!/usr/bin/env python3
import argparse
import collections
import glob
import hashlib
import json
import os

from agent import runlog

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EVALS = os.path.join(ROOT, "evals")
CORPUS = os.path.join(ROOT, "data", "corpus")


def _runs():
    return sorted(glob.glob(os.path.join(runlog.ROOT, "*", "events.jsonl")), key=os.path.getmtime)


def _append(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(obj, default=str) + "\n")


def _load(path):
    with open(path) as f:
        return json.load(f)


def _dump(obj, path, **kw):
    with open(path, "w") as f:
        json.dump(obj, f, **kw)


def review(run_id=None):
    paths = _runs()
    if not paths:
        return "no runs recorded"
    def has_fights(x):
        return any(e["kind"] == "fight_start" for e in runlog.read(os.path.dirname(x)))
    want = os.path.basename(os.path.normpath(run_id)) if run_id else None
    p = next((x for x in paths if want and os.path.basename(os.path.dirname(x)) == want), None) if want else None
    if want and p is None:
        return f"no run named {want}"
    p = p or next((x for x in reversed(paths) if has_fights(x)), paths[-1])
    run_dir = os.path.dirname(p)
    ev = runlog.read(run_dir)
    fights = collections.OrderedDict()
    for e in ev:
        if e["kind"] == "fight_start":
            fights[e["id"]] = dict(start=e, end=None, acts=[])
        elif e["kind"] == "fight_end" and e["id"] in fights:
            fights[e["id"]]["end"] = e
        elif e["kind"] == "action" and e.get("fight") in fights:
            fights[e["fight"]]["acts"].append(e)
    lines = [f"run {os.path.basename(run_dir)}: {len(fights)} fights, {sum(1 for e in ev if e['kind'] == 'macro')} macro decisions, {sum(1 for e in ev if e['kind'] == 'eval')} evaluations"]
    rows, surprises, fid = [], [], collections.Counter()
    dis_total = act_total = 0
    dis_by_enc = collections.defaultdict(lambda: [0, 0])
    for fid_, f in fights.items():
        s, en = f["start"], f["end"]
        if en is None or not en.get("hp"):
            continue
        hp_end = en["hp"][0]
        lost = hp_end is not None and hp_end <= 0
        maxhp = s["max_hp"]
        hp_lost = (s["hp"] / maxhp) if lost else ((s["hp"] - (hp_end or s["hp"])) / maxhp)
        pw = s["predicted"].get("win")
        rows.append((s["encounter"], pw, 0 if lost else 1, s["predicted"].get("hp_lost"), hp_lost, en.get("pit"), (s["predicted"].get("lost_q") or [None] * 21)[10], (s["predicted"].get("lost_q") or [None] * 21)[18]))
        if pw is not None and abs(pw - (0 if lost else 1)) >= 0.6:
            surprises.append(dict(kind="surprise", run=os.path.basename(run_dir), fight=fid_, encounter=s["encounter"], predicted_win=pw, won=not lost, hp_lost=round(hp_lost, 3)))
        for k, v in (en.get("replay") or {}).items():
            if k.startswith(("diff", "residual", "action failed", "intent unmatched", "start unmatched", "missing")):
                fid[k] += v
        for a in f["acts"]:
            if a.get("searched") and a.get("options"):
                cands = [o for o in a["options"] if o.get("q") is not None] or a["options"]
                top_p = max(cands, key=lambda o: o["p"])
                act_total += 1
                dis_by_enc[s["encounter"]][1] += 1
                if top_p["text"] != a["text"]:
                    dis_total += 1
                    dis_by_enc[s["encounter"]][0] += 1
    if rows:
        n = len(rows)
        brier = sum((r[1] - r[2]) ** 2 for r in rows if r[1] is not None) / n
        lines.append(f"fights: {sum(r[2] for r in rows)}/{n} won; predicted win rate {sum(r[1] for r in rows if r[1] is not None) / n:.2f}, Brier {brier:.3f}; "
                     f"HP lost {100 * sum(r[4] for r in rows) / n:.1f}% of max vs predicted {100 * sum((r[3] or 0) for r in rows) / n:.1f}%")
        for enc, pw, won, ph, ah, pit, q50, q90 in rows:
            dist = f"  loss pct {pit:.2f} (pred median {q50:.0f} HP, q90 {q90:.0f} HP)" if pit is not None and q50 is not None else ""
            lines.append(f"  {enc:30s} pred win {pw if pw is not None else '-':>5} actual {'W' if won else 'L'}   HP lost {100 * ah:4.0f}% (pred {100 * (ph or 0):3.0f}%){dist}")
        pits = [r[5] for r in rows if r[5] is not None]
        if pits:
            lines.append(f"calibration of the predicted HP-loss distribution over {len(pits)} fights: mean percentile {sum(pits) / len(pits):.2f} (0.50 if calibrated), {sum(1 for x in pits if x > 0.9)} in the worst 10% (expected {0.1 * len(pits):.1f}), {sum(1 for x in pits if x > 0.97)} beyond the 97th")
    if act_total:
        lines.append(f"search overrode the policy's top action in {dis_total}/{act_total} searched decisions ({100 * dis_total / act_total:.0f}%)")
        worst = sorted(((a / max(b, 1), e, a, b) for e, (a, b) in dis_by_enc.items() if b >= 5), reverse=True)[:3]
        if worst:
            lines.append("  most overridden: " + ", ".join(f"{e} {a}/{b}" for _, e, a, b in worst))
    div = [e for e in ev if e["kind"] == "divergence"]
    lines.append(f"fidelity: replay divergences {dict(fid) or 'none'}; game rejected {len(div)} simulator action(s)")
    lines += _run_outcome(ev)
    dec_lines, overrides, unpriced = _macro_decisions(ev, os.path.basename(run_dir))
    lines += dec_lines
    costly = sorted(glob.glob(os.path.join(run_dir, "fights", "*.json")))
    if costly:
        lines.append(f"costly fights kept for hindsight review ({len(costly)}): " + ", ".join(os.path.basename(c) for c in costly))
    follow = []
    if costly:
        follow.append("hindsight review of each costly fight: `python -m agent.hindsight <file> --log` (luck or a solver gap?); a gap pattern goes to the corpus / fine-tune, an encounter pattern to `sts2-acts/encounters.md`")
    if overrides:
        follow.append(f"{len(overrides)} recorded judgment(s): judge each against what happened (held / failed) and edit the rules in `sts2-deckbuilding` they relied on (tallied in evals/judgments.jsonl)")
    if unpriced:
        follow.append(f"{unpriced} card pick(s) were not priced by `reward` / `eval` first: the rule is numbers first, then judgment")
    if fid or div:
        follow.append("simulator fidelity first: reproduce the divergence (agent.fidelity_sweep) and fix the simulator before trusting any model comparison")
    if surprises:
        follow.append(f"{len(surprises)} fight(s) far from the prediction: add them to the corpus and look for a pattern (encounter, card type, relic); a pattern justifies a fine-tune; "
                      "write what the fight asked into `sts2-acts/encounters.md` (and `sts2-mechanics` for a new power) as a rule tagged `[code]` / `[sim]` or `[hyp]` with its test, no run history")
    if act_total and dis_total / act_total > 0.35:
        follow.append("the search overrides the policy often: the policy is poor on this distribution; a fine-tune on the corpus should raise greedy strength")
    if not follow:
        follow.append("no gap stands out in this run")
    lines.append("follow-ups:")
    lines += [f"  - {t}" for t in follow]
    _append_new(os.path.join(EVALS, "gaps.jsonl"), surprises + [dict(kind="fidelity", run=os.path.basename(run_dir), what=k, count=v) for k, v in fid.items()], GAP_KEY)
    text = "\n".join(lines)
    with open(os.path.join(run_dir, "review.md"), "w", encoding="utf-8") as f:
        f.write(text + "\n")
    return text


def _run_outcome(ev):
    ends = [e for e in ev if e["kind"] == "run_end"]
    if not ends:
        return ["outcome: run not finished (no game-over screen seen)"]
    e = ends[-1]
    return [f"outcome: {e.get('screen')} | {e.get('header')} | last encounter {e.get('last_encounter')}"]


def _macro_decisions(ev, run):
    lines, overrides = [], []
    pending, priced, followed, unpriced, off = None, 0, 0, 0, []
    for e in ev:
        if e["kind"] == "reward_eval":
            pending = e
        elif e["kind"] == "eval":
            pending = pending or e
        elif e["kind"] == "macro":
            screen = (e.get("screen") or "").split(" ")[0]
            why = e.get("why") or ""
            lw = why.lower()
            if "judgment:" in lw or "override" in lw:
                j = why[lw.index("judgment:"):] if "judgment:" in lw else why
                overrides.append(dict(run=run, screen=screen, choice=e.get("choice"), judgment=j[:300], why=why[:300]))
            if screen == "CARD_REWARD":
                pick = (e.get("choice") or "").split()[0] if e.get("choice") else ""
                if pending is None:
                    unpriced += 1
                elif pending["kind"] == "reward_eval" and pick.isdigit():
                    res = (pending.get("result") or {}).get("boss") or {}
                    names = pending.get("options", [])
                    cards = _variant_options(names)
                    vi = _pick_variant(int(pick), names, cards)
                    wins = {int(k): v["win"] for k, v in res.items()}
                    if wins and vi is not None:
                        best = max(wins, key=wins.get)
                        priced += 1
                        if wins.get(vi, 0) >= wins[best] - 0.02:
                            followed += 1
                        else:
                            label = lambda i: "skip" if i == 0 else (names[cards[i - 1]] if i - 1 < len(cards) else str(i))  # noqa: E731
                            off.append(f"  picked {label(vi)} ({wins.get(vi, 0):.3f}) over {label(best)} ({wins[best]:.3f}) vs boss, why: {why[:160]}")
                pending = None
    if priced or unpriced:
        lines.append(f"card picks: {priced} priced by `reward`, followed the best smooth boss score (within 0.02) in {followed}; {unpriced} not priced")
    lines += off
    for o in overrides:
        lines.append(f"  judgment [{o['screen']}] choice {o['choice']}: {o['judgment'][:200]}")
    _append_new(os.path.join(EVALS, "judgments.jsonl"), overrides, JUDGMENT_KEY)
    return lines, overrides, unpriced


def _variant_options(names):
    from agent.macro import card_from_name
    return [i for i, n in enumerate(names) if card_from_name(n)[0]]


def _pick_variant(pick, names, cards):
    if pick >= len(names):
        return 0
    return cards.index(pick) + 1 if pick in cards else None


GAP_KEY = ("kind", "run", "fight", "what")
JUDGMENT_KEY = ("run", "screen", "choice", "why")


def _append_new(path, objs, key):
    def ident(o):
        return tuple(json.dumps(o.get(k), sort_keys=True, default=str) for k in key)
    have = collections.Counter()
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            for l in f:
                try:
                    have[ident(json.loads(l))] += 1
                except ValueError:
                    continue
    seen = collections.Counter()
    for o in objs:
        k = ident(o)
        seen[k] += 1
        if seen[k] > have[k]:
            _append(path, o)


NL = chr(10)


def lessons():
    out = []
    for p in sorted(glob.glob(os.path.join(ROOT, ".claude", "skills", "*", "*.md"))):
        name = os.path.basename(os.path.dirname(p)) + "/" + os.path.basename(p)
        under = False
        for i, l in enumerate(open(p, encoding="utf-8"), 1):
            if l.startswith("#"):
                under = "[hyp]" in l
                continue
            if l.strip() and ("[hyp]" in l or (under and l.lstrip().startswith(("-", "*", "1", "2", "3", "4", "5", "6", "7", "8", "9")))):
                out.append(f"{name}:{i}: {l.strip()[:200]}")
    return f"{len(out)} open hypotheses" + NL + NL.join(out)


def gaps():
    p = os.path.join(EVALS, "gaps.jsonl")
    if not os.path.exists(p):
        return "no gaps recorded"
    rows = [json.loads(l) for l in open(p, encoding="utf-8") if l.strip()]
    by = collections.Counter((r.get("kind"), r.get("encounter") or r.get("what", "")) for r in rows)
    return f"{len(rows)} records" + NL + NL.join(f"  {k[0]:10s} {k[1]:40s} {n}" for k, n in by.most_common(25))


def corpus():
    os.makedirs(CORPUS, exist_ok=True)
    seen, train, hold = set(), [], []
    for p in _runs():
        run = os.path.basename(os.path.dirname(p))
        for e in runlog.read(os.path.dirname(p)):
            if e["kind"] != "fight_start":
                continue
            sc = dict(e["scenario"])
            key = hashlib.sha1(json.dumps(sc, sort_keys=True).encode()).hexdigest()[:12]
            if key in seen:
                continue
            seen.add(key)
            sc["name"] = f"corpus_{run}_{e['id']}"
            sc["seed"] = key
            (hold if int(hashlib.sha1(run.encode()).hexdigest(), 16) % 5 == 0 else train).append(sc)
    _dump(train, os.path.join(CORPUS, "fights_train.json"))
    _dump(hold, os.path.join(CORPUS, "fights_holdout.json"))
    return f"corpus: {len(train)} train fights, {len(hold)} held-out fights in {CORPUS}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["review", "lessons", "gaps", "corpus"])
    ap.add_argument("arg", nargs="?")
    a = ap.parse_args()
    out = dict(review=lambda: review(a.arg), lessons=lessons, gaps=gaps, corpus=corpus)[a.cmd]()
    print(out)


if __name__ == "__main__":
    main()
