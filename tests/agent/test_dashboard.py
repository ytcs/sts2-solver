"""The live dashboard (tools/dashboard): the harness's live.json hook, the read-only server and the page's read of the record (model.js),
offline on a fixture run directory (no game, no daemon)."""
import json
import os
import shutil
import subprocess
import sys
import threading
import types
import urllib.error
import urllib.request

from support import FIX, FakeBridge, make_harness, ok, screen

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "dashboard"))
RUN = "20261007-000000"


# ---------------------------------------------------------------------------------------------------------------- the hook

def test_handle_writes_live_json(monkeypatch, tmp_path):
    h = make_harness(monkeypatch, tmp_path, FakeBridge(screen("rewards")))
    out = ok(h.handle("s"))
    with open(os.path.join(h.log.dir, "live.json"), encoding="utf-8") as f:
        live = json.load(f)
    assert live["run"] == h.log.run_id and live["cmd"] == "s"
    assert live["screen"].startswith("REWARDS") and live["reply"] == out
    assert live["fight"] is None
    assert not os.path.exists(os.path.join(h.log.dir, "live.json.tmp"))


def test_live_keeps_last_screen_and_drops_fight_states(monkeypatch, tmp_path):
    h = make_harness(monkeypatch, tmp_path, FakeBridge(screen("rewards")))
    h.log.live(screen="COMBAT\nA1 F2 ...\n", fight=dict(id=7, state={"turn": 1}, states=[{}] * 50, log=[]))
    h.log.live(cmd="eval --boss", reply="win 0.9 +- 0.02\n", screen="COMBAT\nA1 F2 ...\n", fight=dict(id=7, state={"turn": 2}, states=[{}] * 51, log=[]))
    with open(os.path.join(h.log.dir, "live.json"), encoding="utf-8") as f:
        live = json.load(f)
    assert live["screen"].startswith("COMBAT")  # a calculator's reply is not a screen: the last screen stays
    assert live["reply"].startswith("win") and live["fight"]["state"]["turn"] == 2 and "states" not in live["fight"]
    h.log.live(screen="COMBAT\nA1 F2 ...\ne0 plan: +1 Slice\n")
    h.log.live(screen="COMBAT\nA1 F2 ...\n", reply="WARNING: harness code changed\nREWARDS\nA1 F2 ...\n")
    with open(os.path.join(h.log.dir, "live.json"), encoding="utf-8") as f:
        assert json.load(f)["screen"].startswith("REWARDS")  # the warning line is not the screen's first line
    h.log.live(screen="COMBAT\nA1 F3 ...\ne0 plan: +1 Slice\n")
    h.log.live(screen="COMBAT\nA1 F3 ...\n")
    with open(os.path.join(h.log.dir, "live.json"), encoding="utf-8") as f:
        assert "e0 plan" in json.load(f)["screen"]  # the same screen without its combat info does not replace it


def _live(h):
    with open(os.path.join(h.log.dir, "live.json"), encoding="utf-8") as f:
        return json.load(f)


def _record_live(monkeypatch, h):
    seen, real = [], h.log.live
    def rec(*a, **kw):
        seen.append(dict(kw))
        return real(*a, **kw)
    monkeypatch.setattr(h.log, "live", rec)
    return seen


def test_a_chain_writes_live_after_each_step(monkeypatch, tmp_path):
    rewards = screen("rewards")
    after_gold = rewards.replace("0 9 Gold\n1 card", "0 card").replace("2 proceed", "1 proceed")
    map_screen = "MAP\nA1 F2 IRONCLAD A10 HP 68/80 G108 pots[-, -]\nfull map: m\n0 Monster r2c1\n"
    h = make_harness(monkeypatch, tmp_path, FakeBridge(rewards, on_action=[after_gold, map_screen]))
    seen = _record_live(monkeypatch, h)
    ok(h.handle("a 0; ~proceed"))
    assert seen[0].get("running") == "a 0; ~proceed"  # the command is on the page before it runs
    assert [kw.get("screen") for kw in seen[1:3]] == [after_gold, map_screen]
    live = _live(h)
    assert live["running"] is None and live["cmd"] == "a 0; ~proceed" and live["screen"].startswith(map_screen) and live["potion"] is None

    h2 = make_harness(monkeypatch, tmp_path, FakeBridge(rewards, on_action=["ERR the game refused"]), run_id="err")
    h2.handle("s")
    h2.handle("a 0")
    assert _live(h2)["screen"] == rewards and h2.last_state == rewards  # an ERR reply is not a screen


def test_turn_leaves_the_screen_after_end_turn(monkeypatch, tmp_path):
    t1 = "COMBAT\nA1 F2 DEFECT A10 HP 60/75 G99 pots[-, -]\nT1 E0/3 draw1 disc0 exh0\nplay: a <i> [e<target>]\n0 end turn\n"
    t2 = t1.replace("HP 60/75", "HP 52/75").replace("T1 E0/3", "T2 E3/3")
    fake = FakeBridge(t1, on_action=[t2])
    h = make_harness(monkeypatch, tmp_path, fake)
    def state():
        h.last_state = fake.screen
        return fake.screen
    snap = json.dumps(dict(hand=[], energy=0, round=1, turn=1))
    monkeypatch.setattr(h, "state", state)
    monkeypatch.setattr(h, "sync", lambda: dict(id=1, state={}))
    monkeypatch.setattr(h, "_sync_problem", lambda f: None)
    monkeypatch.setattr(h, "_proposal_check", lambda: None)
    monkeypatch.setattr(h, "_decide", lambda *a, **kw: dict(action=0, text="end turn", json='{"end_turn": true}', searched=True, options=[dict(action=0, text="end turn", q=0.5)]))
    h.rp = types.SimpleNamespace(scenario={}, sim=types.SimpleNamespace(stage=lambda: "play", snapshot=lambda: snap))
    h.fight_id = 1
    out = h.handle("turn")
    assert out.rstrip().endswith("0 end turn") and "T2 E3/3" in out, out
    assert h.last_state == t2 and "HP 52/75" in _live(h)["screen"]  # not the screen from before the enemy turn


def test_live_falls_back_to_an_in_place_write(monkeypatch, tmp_path):
    from agent import runlog
    h = make_harness(monkeypatch, tmp_path, FakeBridge(screen("rewards")))
    def locked(*a, **kw):
        raise PermissionError("a reader holds live.json")
    monkeypatch.setattr(runlog.os, "replace", locked)
    h.log.live(cmd="s", reply=screen("rewards"))
    assert _live(h)["cmd"] == "s" and not os.path.exists(os.path.join(h.log.dir, "live.json.tmp"))


def test_explicit_run_id_leaves_current_alone(monkeypatch, tmp_path):
    from agent import runlog
    h = make_harness(monkeypatch, tmp_path, FakeBridge(screen("rewards")))
    runlog.RunLog("a-script-run")
    with open(os.path.join(runlog.ROOT, "CURRENT"), encoding="utf-8") as f:
        assert f.read().strip() == h.log.run_id


# ---------------------------------------------------------------------------------------------------------------- the server

def _serve(tmp_path):
    import serve
    runs = os.path.join(str(tmp_path), "records")
    shutil.copytree(os.path.join(FIX, "dashboard"), runs)
    srv = serve.make_server(port=0, runs=runs, assets=os.path.join(str(tmp_path), "no_assets"))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, runs, f"http://127.0.0.1:{srv.server_address[1]}"


def _get(url, headers=None):
    try:
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers or {}), timeout=10) as r:
            return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def test_server_serves_page_and_json(tmp_path):
    srv, runs, base = _serve(tmp_path)
    try:
        code, _, body = _get(base + "/")
        assert code == 200 and b"<title>STS2 Live</title>" in body and b'<script src="/model.js">' in body
        code, hdr, _ = _get(base + "/model.js")
        assert code == 200 and hdr["Content-Type"].startswith("text/javascript")

        code, _, body = _get(base + "/api/runs")
        j = json.loads(body)
        assert code == 200 and j["current"] == RUN and RUN in j["runs"]

        code, hdr, body = _get(base + "/api/live")  # runs/CURRENT when no run is named
        live = json.loads(body)
        assert code == 200 and live["run"] == RUN and live["screen"].startswith("COMBAT") and live["fight"]["state"]["enemies"][0]["id"] == "NIBBIT"
        code, _, _ = _get(base + "/api/live?run=" + RUN, {"If-None-Match": hdr["ETag"]})
        assert code == 304

        code, _, body = _get(base + "/api/events?from=0")
        j = json.loads(body)
        kinds = [e["kind"] for e in j["events"]]
        assert code == 200 and j["next"] == j["size"] and kinds[:3] == ["price", "macro", "map"] and "fight_end" in kinds

        code, _, body = _get(base + f"/api/events?run={RUN}&from={j['next']}")  # nothing new: an empty, cheap reply
        assert json.loads(body)["events"] == []

        code, _, body = _get(base + f"/api/fights?run={RUN}")
        assert json.loads(body)["fights"] == ["1_NIBBITS_WEAK.json"]
        code, _, body = _get(base + f"/api/fight?run={RUN}&name=1_NIBBITS_WEAK.json")
        f = json.loads(body)
        assert code == 200 and f["encounter"] == "NIBBITS_WEAK" and "states" not in f["fight"]

        assert _get(base + "/api/live?run=nope")[0] == 404
        assert _get(base + "/api/fight?run=" + RUN + "&name=../CURRENT")[0] == 400
        assert _get(base + "/assets/..%2F..%2FCURRENT")[0] in (403, 404)
        assert _get(base + "/assets/manifest.json")[0] == 404  # no art cache: the page degrades to text
    finally:
        srv.shutdown()
        srv.server_close()


def test_events_tail_only_complete_lines(tmp_path):
    srv, runs, base = _serve(tmp_path)
    try:
        p = os.path.join(runs, RUN, "events.jsonl")
        j = json.loads(_get(base + "/api/events")[2])
        n, nxt = len(j["events"]), j["next"]
        with open(p, "a", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(dict(t=1.0, kind="note", text="one")) + "\n" + '{"t": 2.0, "kind": "no')  # the second line is still being written
        j = json.loads(_get(base + f"/api/events?from={nxt}")[2])
        assert [e["kind"] for e in j["events"]] == ["note"] and j["next"] < j["size"]
        with open(p, "a", encoding="utf-8", newline="\n") as f:
            f.write('te", "text": "two"}\n')
        j2 = json.loads(_get(base + f"/api/events?from={j['next']}")[2])
        assert [e.get("text") for e in j2["events"]] == ["two"] and j2["next"] == j2["size"]
        assert n == 16
    finally:
        srv.shutdown()
        srv.server_close()


# ---------------------------------------------------------------------------------------------------------------- the page's read (model.js)

NODE_READ = r"""
const m = require(process.argv[1]), fs = require("fs"), path = require("path"), d = process.argv[2];
const live = JSON.parse(fs.readFileSync(path.join(d, "live.json"), "utf8"));
const events = fs.readFileSync(path.join(d, "events.jsonl"), "utf8").split(/\r?\n/).filter(Boolean).map((l) => JSON.parse(l));
const card = events.find((e) => e.kind === "price" && e.screen === "CARD_REWARD"), map = events.find((e) => e.kind === "price" && e.screen === "MAP");
process.stdout.write(JSON.stringify({
  model: m.model(live, events), noLive: m.model(null, events), running: m.model(Object.assign({}, live, { running: "price", t_running: 1, potion: null }), events),
  why: m.parseWhy(events.find((e) => e.kind === "macro" && e.screen === "CARD_REWARD").why), freeWhy: m.parseWhy("rules: P6,F4 ; adv's best non-potion action"),
  card: m.priceRows(card), cardBest: m.bestOption(card), map: m.priceRows(map), one: m.priceRows({ options: ["x"], result: null }),
}));
"""


def test_model_reads_the_fixture_into_panels():
    node = shutil.which("node")
    if not node:
        print("skipped: no node")
        return
    d = os.path.join(FIX, "dashboard", RUN)
    p = subprocess.run([node, "-e", NODE_READ, os.path.join(ROOT, "tools", "dashboard", "model.js"), d], capture_output=True, text=True, encoding="utf-8", timeout=60)
    assert p.returncode == 0, p.stderr
    j = json.loads(p.stdout)
    m = j["model"]
    assert m["warning"].startswith("WARNING: harness code changed") and m["reply"].startswith("POTION: ")
    assert m["potion"]["kind"] == "confirm" and m["potion"]["text"] == "potion 0 -> e0" and m["potion"]["line"].startswith("POTION: the search would throw")
    assert m["running"] is None and m["cmd"] == "turn"
    assert m["screen"]["kind"] == "COMBAT" and m["screen"]["live"] and m["screen"]["header"] == dict(act=1, floor=2, char="DEFECT", asc=10, hp=55, max=75, gold=110, pots=["Fire Potion", "-"])
    assert m["fight"]["id"] == 2 and m["fight"]["start"]["drive"][0] == "manual" and m["fight"]["state"]["enemies"][0]["id"] == "NIBBIT"
    assert [r.get("counter") for r in m["relics"]] == [None, 10] and m["character"] == "DEFECT" and len(m["deck"]) == 3
    assert m["decision"]["label"] == "Unknown r2c2" and m["decision"]["why"]["rules"] == [] and m["decision"]["why"]["rest"] == "only option"
    assert m["plan"]["text"].startswith("plan: frost") and m["target"]["variant"]["add"] == ["BALL_LIGHTNING", "BALL_LIGHTNING"]
    assert m["price"] is None  # no price since the last decision

    assert j["noLive"]["screen"]["live"] is False and j["noLive"]["screen"]["kind"] == "MAP" and j["noLive"]["potion"] is None
    assert [r["id"] for r in j["noLive"]["relics"]] == ["CRACKED_CORE"]  # the fight start after the relics event
    assert j["running"]["running"] == dict(cmd="price", since=1) and j["running"]["potion"] is None

    assert j["why"] == dict(text=j["why"]["text"], rules=["D2"], numbers="price search check Ball Lightning +0.42+-0.07 vs skip", judgment="front damage for the Kin", rest=None)
    assert j["freeWhy"]["rules"] == ["P6", "F4"] and j["freeWhy"]["rest"] == "adv's best non-potion action"

    ball = next(r for r in j["card"] if r["option"] == "Ball Lightning")
    assert j["cardBest"] == "Ball Lightning" and (ball["check"], ball["se"], ball["act"]) == (0.42, 0.07, None)
    a2 = next(f for f in ball["factors"] if f["name"] == "A2 boss")
    kin = next(f for f in ball["factors"] if f["name"] == "near THE_KIN_BOSS")
    assert (a2["win"], a2["scored"], a2["mult"], a2["floored"]) == (0, 0.22, 3, True) and (kin["win"], kin["floored"]) == (0.55, False)
    assert [(r["option"], r["act"], r["floors"], r["check"]) for r in j["map"]] == [("M r1c0", 0.65, 19.7, 0.0), ("M r1c2", 0.84, 20.8, 0.12)]
    assert j["one"] == []
