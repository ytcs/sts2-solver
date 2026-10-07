"""The live dashboard (tools/dashboard): the harness's live.json hook and the read-only server, offline on a fixture run directory (no game, no daemon)."""
import json
import os
import shutil
import sys
import threading
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
        assert code == 200 and b"<title>STS2 Live</title>" in body

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
        assert n == 8
    finally:
        srv.shutdown()
        srv.server_close()
