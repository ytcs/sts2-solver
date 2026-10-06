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
