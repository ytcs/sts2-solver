#!/usr/bin/env python3
import argparse
import collections
import glob
import hashlib
import json
import os
import re

from agent import runlog
from agent import screen as scr

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EVALS = os.path.join(ROOT, "evals")
CORPUS = os.path.join(ROOT, "data", "corpus")


def _runs():
    return sorted(glob.glob(os.path.join(runlog.ROOT, "*", "events.jsonl")), key=os.path.getmtime)


def _append(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(obj, default=str) + "\n")


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
        follow.append(f"{len(overrides)} decision(s) off the instrument or citing new: apply the `sts2-improve` meta-rules (ledger: `python -m agent.improve rules`)")
    if unpriced:
        follow.append(f"{unpriced} card pick(s) without a logged instrument")
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


RULE_ID = re.compile(r"\b(?:A[1-3]-\d+|IC\d+|[PDRFMI]\d+)\b")
LEDGER = os.path.join(EVALS, "rules", "ledger.jsonl")
LEDGER_KEY = ("run", "i")
SAFE_WIN = 0.9


def _field(why, name):
    m = re.search(rf"(?i){name}:\s*(.*?)(?:;\s*(?:rules|numbers|judgment):|$)", why or "")
    return m.group(1).strip() if m else ""


def _label(state, choice):
    num = (choice or "").split()[0] if choice else ""
    if num.startswith("~"):
        return num[1:]
    line = scr.option_line(state or "", num).partition(" ")[2] if num.isdigit() else ""
    return line.split("(")[0].strip()


def _score(inst, e):
    """the instrument's best option vs the operator's choice: (instrument, best, chosen, agree, margin in the instrument's units)"""
    kind, x = inst
    state, choice = e.get("state", ""), e.get("choice")
    if kind == "price" and x.get("result"):
        res, by = x["result"], x.get("ranked_by")
        chosen = _label(state, choice).lower()
        hit = next((k for k in res if chosen and (k.lower().startswith(chosen) or chosen.startswith(k.lower()))), None)
        if chosen.startswith("skip"):
            hit = next((k for k in res if k.lower().startswith("skip")), hit)
        if hit is None:
            return None
        best = x.get("best")
        margin = res[best][by] - res[hit][by] if best in res and by in res[best] else None
        return f"price:{by}", best, hit, hit.lower() == str(best).lower(), margin
    if kind == "routes":
        rows = {}
        for l in x.get("text", "").split(NL):
            m = re.match(r"^[A-Za-z?$]+(\d+)c(\d+)\s+([\d.]+)", l)
            if m and (m.group(1), m.group(2)) not in rows:
                rows[(m.group(1), m.group(2))] = float(m.group(3))
        m = re.search(r"r(\d+)c(\d+)", scr.option_line(state, (choice or " ").split()[0]))
        if not rows or not m or m.groups() not in rows:
            return None
        best, got = max(rows, key=rows.get), rows[m.groups()]
        return "routes", f"r{best[0]}c{best[1]}", f"r{m.group(1)}c{m.group(2)}", got >= min(rows[best], SAFE_WIN) - 1e-9, rows[best] - got
    if kind == "rest":
        label = _label(state, choice).lower()
        if not label.startswith(("rest", "smith")):
            return None
        r0, r1 = x["result"].get("0") or x["result"].get(0), x["result"].get("1") or x["result"].get(1)
        gain, dse = r1["win"] - r0["win"], r1.get("dse") or r1.get("se") or 0
        rest, chose_rest = gain > 2 * dse, label.startswith("rest")
        return "restcalc", "rest" if rest else "smith", "rest" if chose_rest else "smith", rest == chose_rest, 0.0 if rest == chose_rest else (gain if rest else -gain)
    return None


def _macro_decisions(ev, run):
    """every macro decision with its cited rules, scored against the instrument logged on that screen; rows go to the rule ledger"""
    outcome = next((f"{e.get('screen')} {e.get('header')}" for e in reversed(ev) if e["kind"] == "run_end"), "unfinished")
    inst, rows = None, []
    for e in ev:
        k = e["kind"]
        if k == "fight_start":
            inst = None
        elif k == "price":
            inst = ("price", e)
        elif k == "routes":
            inst = ("routes", e)
        elif k == "eval" and any(v.get("name") == "rest" for v in (e.get("spec") or {}).get("variants", [])):
            inst = ("rest", e)
        elif k == "macro":
            why = e.get("why") or ""
            cited = _field(why, "rules")
            sc = _score(inst, e) if inst else None
            if sc or "rules:" in why.lower():
                rows.append(dict(run=run, i=len(rows), screen=(e.get("screen") or "").split(" ")[0], header=e.get("screen"), choice=e.get("choice"),
                             rules=RULE_ID.findall(cited), new=cited.partition("new:")[2].strip() or None, judgment=_field(why, "judgment")[:300],
                             instrument=sc[0] if sc else None, best=sc[1] if sc else None, chosen=sc[2] if sc else None,
                             agree=sc[3] if sc else None, margin=round(sc[4], 4) if sc and sc[4] is not None else None, outcome=outcome))
            if sc or (e.get("screen") or "").startswith("MAP"):
                inst = None
    scored = [r for r in rows if r["agree"] is not None]
    lines = [f"macro decisions: {len(rows)}, citing a rule {sum(1 for r in rows if r['rules'])}, new: {sum(1 for r in rows if r['new'])}, "
             f"scored {len(scored)}, agreeing with the instrument {sum(1 for r in scored if r['agree'])}"]
    for r in scored:
        if not r["agree"]:
            lines.append(f"  [{r['screen']}] chose {r['chosen']} over {r['instrument']} best {r['best']} (margin {r['margin']}); rules {','.join(r['rules']) or '-'}; judgment: {r['judgment'][:160]}")
    for r in rows:
        if r["new"]:
            lines.append(f"  new: [{r['screen']}] {r['new'][:200]}")
    _append_new(LEDGER, [dict(run=run, i=-1, outcome=outcome, decisions=len(rows))] + rows, LEDGER_KEY)
    return lines, [r for r in rows if r["new"] or r["agree"] is False], sum(1 for r in rows if r["screen"] == "CARD_REWARD" and r["instrument"] is None)


def rules_table():
    if not os.path.exists(LEDGER):
        return "no ledger yet"
    rows = [r for r in map(json.loads, filter(str.strip, open(LEDGER, encoding="utf-8"))) if r["i"] >= 0]
    by = collections.defaultdict(list)
    for r in rows:
        for rid in r["rules"] or ["(none)"]:
            by[rid].append(r)
    out = [f"{len(rows)} decisions in {len({r['run'] for r in rows})} runs", f"{'rule':8s} {'cited':>5s} {'runs':>4s} {'scored':>6s} {'agree':>5s} {'mean margin off':>15s}"]
    for rid, rs in sorted(by.items(), key=lambda x: -len(x[1])):
        sc = [r for r in rs if r["agree"] is not None]
        off = [r["margin"] for r in sc if not r["agree"] and r["margin"] is not None]
        out.append(f"{rid:8s} {len(rs):5d} {len({r['run'] for r in rs}):4d} {len(sc):6d} {sum(1 for r in sc if r['agree']):5d} {(sum(off) / len(off)) if off else 0:15.3f}")
    return NL.join(out)


GAP_KEY = ("kind", "run", "fight", "what")


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
    ap.add_argument("cmd", choices=["review", "rules", "lessons", "gaps", "corpus"])
    ap.add_argument("arg", nargs="?")
    a = ap.parse_args()
    out = dict(review=lambda: review(a.arg), rules=rules_table, lessons=lessons, gaps=gaps, corpus=corpus)[a.cmd]()
    print(out)


if __name__ == "__main__":
    main()
