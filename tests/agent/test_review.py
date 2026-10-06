"""`improve.review`: which run it reads, what it reports for fights and card picks, what it appends to evals/."""
import json
import os
import time

from support import isolate

from agent import improve, runlog


def write_run(root, run_id, evs, mtime=None):
    d = os.path.join(root, run_id)
    os.makedirs(d, exist_ok=True)
    p = os.path.join(d, "events.jsonl")
    with open(p, "w", encoding="utf-8") as f:
        for e in evs:
            f.write(json.dumps(e) + "\n")
    if mtime is not None:
        os.utime(p, (mtime, mtime))
    return d


def fight(fid, enc, hp, max_hp, end_hp, win, lost_q=None, replay=None):
    q = lost_q or [float(i) for i in range(21)]
    return [dict(t=1, kind="fight_start", id=fid, encounter=enc, hp=hp, max_hp=max_hp, scenario=dict(act=0), predicted=dict(win=win, hp_lost=0.1, lost_q=q)),
            dict(t=2, kind="action", fight=fid, text="play STRIKE #0 -> e0", searched=True, options=[dict(text="play STRIKE #0 -> e0", p=0.2, q=0.5), dict(text="end turn", p=0.8, q=0.1)]),
            dict(t=3, kind="fight_end", id=fid, hp=[end_hp, max_hp], pit=0.5, replay=replay or {"end_turn_matched": 2})]


def test_run_selection(monkeypatch, tmp_path):
    isolate(monkeypatch, tmp_path)
    assert improve.review() == "no runs recorded"
    now = time.time()
    write_run(runlog.ROOT, "20261001-000000", fight("1", "NIBBITS_WEAK", 70, 80, 60, 0.9), mtime=now - 300)
    write_run(runlog.ROOT, "shakedown-1", fight("1", "CULTISTS_NORMAL", 70, 80, 65, 0.95), mtime=now - 100)  # newest with fights (ids are not all timestamps)
    write_run(runlog.ROOT, "20261009-000000", [dict(t=1, kind="note", text="x")], mtime=now)  # newest, no fight
    assert improve.review().startswith("run shakedown-1: 1 fights")
    assert improve.review("20261001-000000").startswith("run 20261001-000000: 1 fights")
    assert improve.review("runs/20261001-000000").startswith("run 20261001-000000:")
    assert improve.review("20261009-000000").startswith("run 20261009-000000: 0 fights")
    assert improve.review("2026100") == "no run named 2026100"  # exact name only
    with open(os.path.join(runlog.ROOT, "shakedown-1", "review.md"), encoding="utf-8") as f:
        assert f.read().startswith("run shakedown-1: 1 fights")


def test_review_report(monkeypatch, tmp_path):
    isolate(monkeypatch, tmp_path)
    evs = fight("4", "NIBBITS_WEAK", 70, 80, 60, 0.9) + fight("5", "PHROG_PARASITE_ELITE", 60, 80, 0, 0.95, replay={"diff .enemies[0].hp": 3, "end_turn_matched": 1})
    evs += [dict(t=4, kind="reward_eval", options=["Armaments", "Headbutt", "Perfected Strike"],
                 result=dict(boss={"0": dict(win=0.10), "1": dict(win=0.12), "2": dict(win=0.30), "3": dict(win=0.20)})),
            dict(t=5, kind="macro", screen="CARD_REWARD A1 F2", choice="0", why="buckets: x; judgment: Armaments is the plan", result="REWARDS"),
            dict(t=6, kind="macro", screen="CARD_REWARD A1 F5", choice="1", why="no numbers", result="REWARDS"),
            dict(t=7, kind="reward_eval", options=["A", "B", "C"], result=dict(boss={"0": dict(win=0.5), "1": dict(win=0.2), "2": dict(win=0.2), "3": dict(win=0.2)})),
            dict(t=8, kind="macro", screen="CARD_REWARD A1 F8", choice="3", why="skip", result="REWARDS"),
            dict(t=9, kind="run_end", screen="GAME_OVER", header="A1 F9 IRONCLAD", last_encounter="PHROG_PARASITE_ELITE")]
    write_run(runlog.ROOT, "r1", evs)
    text = improve.review("r1")
    lines = text.split("\n")
    assert lines[0] == "run r1: 2 fights, 3 macro decisions, 0 evaluations"
    assert lines[1] == "fights: 1/2 won; predicted win rate 0.93, Brier 0.456; HP lost 43.8% of max vs predicted 10.0%"
    assert "  picked Armaments (0.120) over Headbutt (0.300) vs boss, why: buckets: x; judgment: Armaments is the plan" in lines
    assert "card picks: 2 priced by `reward`, followed the best smooth boss score (within 0.02) in 1; 1 not priced" in lines
    assert "  judgment [CARD_REWARD] choice 0: judgment: Armaments is the plan" in lines
    assert "outcome: GAME_OVER | A1 F9 IRONCLAD | last encounter PHROG_PARASITE_ELITE" in lines
    assert "search overrode the policy's top action in 2/2 searched decisions (100%)" in lines
    with open(os.path.join(improve.EVALS, "gaps.jsonl"), encoding="utf-8") as f:
        gaps = [json.loads(l) for l in f]
    assert dict(kind="surprise", run="r1", fight="5", encounter="PHROG_PARASITE_ELITE", predicted_win=0.95, won=False, hp_lost=0.75) in gaps
    assert dict(kind="fidelity", run="r1", what="diff .enemies[0].hp", count=3) in gaps
    with open(os.path.join(improve.EVALS, "judgments.jsonl"), encoding="utf-8") as f:
        js = [json.loads(l) for l in f]
    assert js == [dict(run="r1", screen="CARD_REWARD", choice="0", judgment="judgment: Armaments is the plan", why="buckets: x; judgment: Armaments is the plan")]


def read_lines(path):
    with open(path, encoding="utf-8") as f:
        return f.read().splitlines()


def test_review_appends_once(monkeypatch, tmp_path):
    """Bug fix: every `review` of a run appended its surprises, fidelity counts and judgments again (evals/ held up to 3 copies of a line)."""
    isolate(monkeypatch, tmp_path)
    evs = fight("5", "PHROG_PARASITE_ELITE", 60, 80, 0, 0.95, replay={"diff .enemies[0].hp": 3})
    evs += [dict(t=5, kind="macro", screen="SHOP A1 F3", choice="14", why="numbers: n; judgment: keep the gold", result="MAP"),
            dict(t=6, kind="macro", screen="SHOP A1 F9", choice="14", why="numbers: n; judgment: keep the gold", result="MAP")]  # the same record twice: both are kept
    write_run(runlog.ROOT, "r1", evs)
    gp, jp = os.path.join(improve.EVALS, "gaps.jsonl"), os.path.join(improve.EVALS, "judgments.jsonl")
    os.makedirs(improve.EVALS, exist_ok=True)
    with open(gp, "w", encoding="utf-8") as f:
        f.write(json.dumps(dict(kind="hindsight", fight="x", encounter="E")) + "\n")
    first = improve.review("r1")
    g1, j1 = read_lines(gp), read_lines(jp)
    assert len(g1) == 3 and len(j1) == 2
    assert improve.review("r1") == first
    assert read_lines(gp) == g1 and read_lines(jp) == j1
    write_run(runlog.ROOT, "r2", evs)
    improve.review("r2")
    assert len(read_lines(gp)) == 5 and len(read_lines(jp)) == 4  # another run's records are new


def test_pick_audit_maps_variants(monkeypatch, tmp_path):
    """Bug fix: `reward` prices only the cards with a simulator id, so an unmapped option shifts the variants; the audit took option i as variant i + 1."""
    isolate(monkeypatch, tmp_path)
    res = dict(boss={"0": dict(win=0.10), "1": dict(win=0.20), "2": dict(win=0.60)})  # 0 skip, 1 Armaments, 2 Headbutt (Mystery Card not evaluated)
    names = ["Armaments", "Mystery Card", "Headbutt"]
    evs = [dict(t=1, kind="reward_eval", options=names, result=res),
           dict(t=2, kind="macro", screen="CARD_REWARD A1 F2", choice="2", why="w1", result="REWARDS"),  # Headbutt = variant 2: the best
           dict(t=3, kind="reward_eval", options=names, result=res),
           dict(t=4, kind="macro", screen="CARD_REWARD A1 F5", choice="0", why="w2", result="REWARDS"),  # Armaments = variant 1
           dict(t=5, kind="reward_eval", options=names, result=res),
           dict(t=6, kind="macro", screen="CARD_REWARD A1 F8", choice="1", why="w3", result="REWARDS"),  # the unevaluated card: not in the tally
           dict(t=7, kind="reward_eval", options=names, result=res),
           dict(t=8, kind="macro", screen="CARD_REWARD A1 F9", choice="3", why="w4", result="REWARDS")]  # skip
    write_run(runlog.ROOT, "r1", evs)
    lines = improve.review("r1").split("\n")
    assert "card picks: 3 priced by `reward`, followed the best smooth boss score (within 0.02) in 1; 0 not priced" in lines
    assert "  picked Armaments (0.200) over Headbutt (0.600) vs boss, why: w2" in lines
    assert "  picked skip (0.100) over Headbutt (0.600) vs boss, why: w4" in lines
    assert not any("Mystery Card" in l or "w3" in l for l in lines)
