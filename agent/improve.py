#!/usr/bin/env python3
"""The outer loop: turn experience into improvements, and adopt an improvement only when it measurably helps.

  python -m agent.improve review [run-id]       report on a run: fights (predicted vs actual), search-vs-policy disagreement, simulator fidelity, macro decisions
  python -m agent.improve lessons               the strategy book's open hypotheses (`[hyp]` lines) = the experiment backlog
  python -m agent.improve corpus                collect the fights of all runs into data/corpus (train / held-out split by run)
  python -m agent.improve finetune [--iters N]  fine-tune the current network on corpus + base distribution (PPO, `rl/ppo.py`), in the background
  python -m agent.improve gate CKPT [--vs CKPT] compare a candidate with the current network on fixed held-out sets; writes evals/ledger.jsonl
  python -m agent.improve adopt CKPT --as NAME  make a checkpoint the default policy (models/current.json) after it passed the gate
  python -m agent.improve ledger                what was tried and decided

Two kinds of things get improved, each with its own gate:
  strategy (the skills in .claude/skills): a lesson moves from `[hyp]` to `[sim]` / `[played]` only with a test behind it (a macro `eval`, a solver A/B, a run record).
  model / search: a checkpoint or a search setting is adopted only if it beats the current one on the held-out fights (corpus holdout and the fixed eval set) with
  paired seeds; the result is in evals/ledger.jsonl.
Triggers for a fine-tune (from `review`): fights lost or surprising against the solver's own prediction, concentrated in some encounters / card kinds; a high
rate of search overriding the policy there (the policy is poor in that region); simulator fidelity is fine (otherwise fix the simulator first).
"""
import argparse
import collections
import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

from agent import runlog

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EVALS = os.path.join(ROOT, "evals")
CORPUS = os.path.join(ROOT, "data", "corpus")
MODELS = os.path.join(ROOT, "models")


def _runs():
    return sorted(glob.glob(os.path.join(runlog.ROOT, "*", "events.jsonl")))


def _append(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(obj, default=str) + "\n")


# ---------------------------------------------------------------------------------------------------------------- review

def review(run_id=None):
    paths = _runs()
    if not paths:
        return "no runs recorded"
    def has_fights(x):
        return any(e["kind"] == "fight_start" for e in runlog.read(os.path.dirname(x)))
    p = next((x for x in paths if run_id and run_id in x), next((x for x in reversed(paths) if has_fights(x)), paths[-1]))
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
            continue  # the end of the fight was not observed on the screen right after it
        hp_end = en["hp"][0]
        lost = hp_end is not None and hp_end <= 0
        maxhp = s["max_hp"]
        hp_lost = (s["hp"] / maxhp) if lost else ((s["hp"] - (hp_end or s["hp"])) / maxhp)
        pw = s["predicted"].get("win")
        rows.append((s["encounter"], pw, 0 if lost else 1, s["predicted"].get("hp_lost"), hp_lost))
        if pw is not None and abs(pw - (0 if lost else 1)) >= 0.6:
            surprises.append(dict(kind="surprise", run=os.path.basename(run_dir), fight=fid_, encounter=s["encounter"], predicted_win=pw, won=not lost, hp_lost=round(hp_lost, 3)))
        for k, v in (en.get("replay") or {}).items():
            if k.startswith(("diff", "residual", "action failed", "intent unmatched", "start unmatched", "missing")):
                fid[k] += v
        for a in f["acts"]:
            if a.get("searched") and a.get("options"):
                cands = [o for o in a["options"] if o.get("q") is not None] or a["options"]  # a held potion has p but was never searched: not the policy's choice
                top_p = max(cands, key=lambda o: o["p"])
                act_total += 1
                dis_by_enc[s["encounter"]][1] += 1
                if top_p["text"] != a["text"]:
                    dis_total += 1
                    dis_by_enc[s["encounter"]][0] += 1
    if rows:
        n = len(rows)
        brier = sum((w - a) ** 2 for _, w, a, _, _ in rows if w is not None) / n
        lines.append(f"fights: {sum(r[2] for r in rows)}/{n} won; predicted win rate {sum(r[1] for r in rows if r[1] is not None) / n:.2f}, Brier {brier:.3f}; "
                     f"HP lost {100 * sum(r[4] for r in rows) / n:.1f}% of max vs predicted {100 * sum((r[3] or 0) for r in rows) / n:.1f}%")
        for enc, pw, won, ph, ah in rows:
            lines.append(f"  {enc:30s} pred win {pw if pw is not None else '-':>5} actual {'W' if won else 'L'}   HP lost {100 * ah:4.0f}% (pred {100 * (ph or 0):3.0f}%)")
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
        follow.append(f"{len(overrides)} override(s) of the numbers: judge each against what happened (held / failed) and edit the blind-spot list in `sts2-deckbuilding` (tallied in evals/overrides.jsonl)")
    if unpriced:
        follow.append(f"{unpriced} card pick(s) were not priced by `reward` / `eval` first: the rule is numbers first, then judgment")
    if fid or div:
        follow.append("simulator fidelity first: reproduce the divergence (agent.validate, verify/regress) and fix the simulator before trusting any model comparison")
    if surprises:
        follow.append(f"{len(surprises)} fight(s) far from the prediction: add them to the corpus and look for a pattern (encounter, card type, relic); a pattern justifies a fine-tune; "
                      "write what the fight asked into `sts2-acts/encounters.md` (and `sts2-mechanics` for a new power) with `[played]`")
    if act_total and dis_total / act_total > 0.35:
        follow.append("the search overrides the policy often: the policy is poor on this distribution; a fine-tune on the corpus should raise greedy strength")
    if not follow:
        follow.append("no gap stands out in this run")
    lines.append("follow-ups:")
    lines += [f"  - {t}" for t in follow]
    for s_ in surprises:
        _append(os.path.join(EVALS, "gaps.jsonl"), s_)
    for k, v in fid.items():
        _append(os.path.join(EVALS, "gaps.jsonl"), dict(kind="fidelity", run=os.path.basename(run_dir), what=k, count=v))
    text = "\n".join(lines)
    open(os.path.join(run_dir, "review.md"), "w", encoding="utf-8").write(text + "\n")
    return text


def _run_outcome(ev):
    """How the run ended and what ended it (the `run_end` event the harness writes on the game-over / victory screen)."""
    ends = [e for e in ev if e["kind"] == "run_end"]
    if not ends:
        return ["outcome: run not finished (no game-over screen seen)"]
    e = ends[-1]
    return [f"outcome: {e.get('screen')} | {e.get('header')} | last encounter {e.get('last_encounter')}"]


def _macro_decisions(ev, run):
    """Non-combat decisions against the numbers: for every card reward priced by `reward`, did the pick follow the best smooth boss score (skip = option 0)? Every
    `-- why` containing `override` is listed and appended to evals/overrides.jsonl (the tally the deck-building blind-spot list is judged by). Returns
    (lines, overrides, number of card picks nobody priced)."""
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
            if "override" in why.lower():
                overrides.append(dict(run=run, screen=screen, choice=e.get("choice"), why=why[:300]))
            if screen == "CARD_REWARD":
                pick = (e.get("choice") or "").split()[0] if e.get("choice") else ""
                if pending is None:
                    unpriced += 1
                elif pending["kind"] == "reward_eval" and pick.isdigit():
                    res = (pending.get("result") or {}).get("boss") or {}
                    names = pending.get("options", [])
                    vi = int(pick) + 1 if int(pick) < len(names) else 0  # variants: 0 = skip, 1.. = the cards in screen order
                    wins = {int(k): v["win"] for k, v in res.items()}
                    if wins:
                        best = max(wins, key=wins.get)
                        priced += 1
                        if wins.get(vi, 0) >= wins[best] - 0.02:
                            followed += 1
                        else:
                            label = lambda i: "skip" if i == 0 else (names[i - 1] if i - 1 < len(names) else str(i))  # noqa: E731
                            off.append(f"  picked {label(vi)} ({wins.get(vi, 0):.3f}) over {label(best)} ({wins[best]:.3f}) vs boss, why: {why[:160]}")
                pending = None
    if priced or unpriced:
        lines.append(f"card picks: {priced} priced by `reward`, followed the best smooth boss score (within 0.02) in {followed}; {unpriced} not priced")
    lines += off
    for o in overrides:
        lines.append(f"  override [{o['screen']}] choice {o['choice']}: {o['why'][:200]}")
        _append(os.path.join(EVALS, "overrides.jsonl"), o)
    return lines, overrides, unpriced


# ---------------------------------------------------------------------------------------------------------------- strategy backlog

def lessons():
    out = []
    for p in sorted(glob.glob(os.path.join(ROOT, ".claude", "skills", "*", "*.md"))):
        if os.path.basename(p) == "runs.md":
            continue  # run logs hold history, not claims
        name = os.path.basename(os.path.dirname(p)) + "/" + os.path.basename(p)
        for i, l in enumerate(open(p, encoding="utf-8"), 1):
            if "[hyp]" in l:
                out.append(f"{name}:{i}: {l.strip()[:200]}")
    return f"{len(out)} open hypotheses\n" + "\n".join(out)


# ---------------------------------------------------------------------------------------------------------------- corpus / fine-tune / gate

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
            # the split is by run, so held-out fights come from runs the network was never fine-tuned on
            (hold if int(hashlib.sha1(run.encode()).hexdigest(), 16) % 5 == 0 else train).append(sc)
    json.dump(train, open(os.path.join(CORPUS, "fights_train.json"), "w"))
    json.dump(hold, open(os.path.join(CORPUS, "fights_holdout.json"), "w"))
    return f"corpus: {len(train)} train fights, {len(hold)} held-out fights in {CORPUS}"


def finetune(iters=200, name=None):
    train_json = os.path.join(CORPUS, "fights_train.json")
    if not os.path.exists(train_json) or not json.load(open(train_json)):
        return "no corpus yet: play runs, then `corpus`"
    base_train = _train_set("train", 30000, 1, 0.15)
    base = json.load(open(base_train))
    mine = json.load(open(train_json))
    reps = max(1, len(base) // (4 * max(len(mine), 1)))  # the corpus makes up about a fifth of the mix
    mix = os.path.join(ROOT, "target", "train", "mix.json")
    json.dump(base + mine * reps, open(mix, "w"))
    name = name or time.strftime("ft%Y%m%d-%H%M")
    out = os.path.join(ROOT, "target", "runs", name)
    cur = _current()
    cmd = [sys.executable, os.path.join(ROOT, "rl", "ppo.py"), "--train", mix, "--eval", _train_set("eval", 1500, 22, 0.0), "--out", out, "--resume", cur["policy"],
           "--warm", "--iters", str(iters), "--d", "128", "--hold-prob", "0.15"]
    os.makedirs(out, exist_ok=True)
    log = open(os.path.join(out, "train.log"), "w")
    subprocess.Popen(cmd, cwd=ROOT, stdout=log, stderr=log, env=dict(os.environ, STS2_DEVICE=os.environ.get("STS2_DEVICE", "cuda")))
    _append(os.path.join(EVALS, "ledger.jsonl"), dict(t=time.time(), kind="finetune_started", name=name, base=cur["policy"], corpus=len(mine), reps=reps, iters=iters))
    return f"fine-tune started: {out} (log: train.log); when it has produced a checkpoint run `gate {out}/ckpt.pt`"


def _train_set(name, n, seed, energy_prob):
    """A scenario set from `tools/gen_train.py` (realistic A10 fights; `energy_prob` = share with 4-7 energy), generated once into target/train/. Different seeds are
    disjoint, so the held-out sets (seeds 22, 23) never overlap the training set (seed 1)."""
    path = os.path.join(ROOT, "target", "train", f"{name}.json")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        subprocess.check_call([sys.executable, os.path.join(ROOT, "tools", "gen_train.py"), "--n", str(n), "--seed", str(seed), "--energy-prob", str(energy_prob), "--out", path],
                              cwd=ROOT, stdout=subprocess.DEVNULL)
    return path


def _current():
    p = os.path.join(MODELS, "current.json")
    if os.path.exists(p):
        return json.load(open(p))
    return dict(policy=os.path.join(MODELS, "solver_b128.pt"), values=[os.path.join(MODELS, "solver_c128.pt"), os.path.join(MODELS, "solver_d128.pt")])


def gate(candidate, vs=None, attempts=2, n_eval=600):
    sys.path.insert(0, os.path.join(ROOT, "rl"))
    from solver import Solver
    cur = _current()
    vs = vs or cur["policy"]
    sets = {}
    hold = os.path.join(CORPUS, "fights_holdout.json")
    if os.path.exists(hold):
        sets["corpus_holdout"] = json.load(open(hold))
    sets["eval"] = json.load(open(_train_set("eval", 1500, 22, 0.0)))[:n_eval]
    sets["eval_energy"] = json.load(open(_train_set("eval_energy", 600, 23, 1.0)))[:n_eval]  # every scenario at 4-7 energy: the old mix had almost none
    res = {}
    for label, ck, vals in (("candidate", candidate, None), ("current", vs, cur["values"] if vs == cur["policy"] else None)):
        S = Solver(ckpt=ck, value_ckpts=vals if vals else None)
        for sname, scen in sets.items():
            r = S.solve(scen, attempts=attempts, seed=7)
            w = sum(x["win"] for x in r) / len(r)
            h = sum((x["hp_lost"] or 0) for x in r) / len(r)
            se = (sum(x["win_se"] ** 2 for x in r) ** 0.5) / len(r)
            res[(label, sname)] = (w, h, se)
    lines, ok = [], True
    for sname in sets:
        (cw, ch, cse), (bw, bh, bse) = res[("candidate", sname)], res[("current", sname)]
        d, sd = cw - bw, (cse ** 2 + bse ** 2) ** 0.5
        lines.append(f"{sname:16s} candidate win {cw:.3f} HP lost {100 * ch:.1f}%   current win {bw:.3f} HP lost {100 * bh:.1f}%   diff {d:+.3f} (±{sd:.3f})")
        ok &= (d >= 0.01 if sname == "corpus_holdout" else d >= -0.01)
    verdict = "PASS" if ok else "FAIL"
    lines.append(f"gate: {verdict} (candidate must gain >= 1 point on the corpus holdout and lose <= 1 point on the fixed eval set)")
    _append(os.path.join(EVALS, "ledger.jsonl"), dict(t=time.time(), kind="gate", candidate=candidate, vs=vs, verdict=verdict, results={f"{a}/{b}": v for (a, b), v in res.items()}))
    return "\n".join(lines)


def adopt(ckpt, as_name):
    dst = os.path.join(MODELS, as_name)
    shutil.copy(ckpt, dst)
    cur = _current()
    json.dump(dict(policy=dst, values=cur["values"]), open(os.path.join(MODELS, "current.json"), "w"), indent=1)
    _append(os.path.join(EVALS, "ledger.jsonl"), dict(t=time.time(), kind="adopt", ckpt=dst))
    return f"adopted {dst} as the default policy (restart the harness daemon to load it)"


def ledger():
    p = os.path.join(EVALS, "ledger.jsonl")
    return "".join(l for l in open(p, encoding="utf-8")) if os.path.exists(p) else "empty"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["review", "lessons", "corpus", "finetune", "gate", "adopt", "ledger"])
    ap.add_argument("arg", nargs="?")
    ap.add_argument("--vs")
    ap.add_argument("--iters", type=int, default=200)
    ap.add_argument("--as", dest="as_name")
    a = ap.parse_args()
    out = dict(review=lambda: review(a.arg), lessons=lessons, corpus=corpus, finetune=lambda: finetune(a.iters), gate=lambda: gate(a.arg, a.vs),
               adopt=lambda: adopt(a.arg, a.as_name), ledger=ledger)[a.cmd]()
    print(out)


if __name__ == "__main__":
    main()
