"""Fast protocol-level re-enactment of an expert record on a headless `OracleCombat serve` (no solver, no daemon).

usage: python tools/serve_replay.py RECORD.compact.jsonl --port P [--log OUT.jsonl] [--live LIVE_REPLAY.jsonl] [--start]
Uses agent/reenact.py's Reenactor (the code behind the harness's `replay`) with a minimal harness: `fight` JSON instead of the
simulator replayer. --start launches a fresh server on P; --live diffs every decision (screen, deck, combat state) with a live log.
"""
import json, os, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "tools"))


class _Log:
    run_id = "serve-replay"
    dir = os.path.join(ROOT, "target")

    def event(self, *a, **k):
        pass


class ThinHarness:
    gate = False

    def __init__(self):
        from agent.bridge import call
        self.call = call
        self.log = _Log()
        self._last_f = None

    def _send(self, line):
        return self.call(line)

    def _new_run(self):
        pass

    def _skill_refusal(self, screen):
        return None

    def sync(self):
        raw = self.call("fight").strip()
        f = None if raw in ("null", "") or raw.startswith("ERR") else json.loads(raw)
        self._last_f = f
        return f

    def _fight_end(self, reply):
        pass


def replay(rec, log_path, live=None):
    from agent.reenact import Reenactor
    r = Reenactor(ThinHarness(), os.path.abspath(rec))
    r.log_path = log_path
    r.fight_dir = os.path.splitext(log_path)[0] + "_fights"
    if os.path.exists(log_path):
        os.remove(log_path)
    t0 = time.time()
    out = r.run()
    secs = time.time() - t0
    rows = [json.loads(l) for l in open(log_path, encoding="utf-8")]
    res = dict(secs=round(secs, 2), done=sum(1 for x in rows if x.get("event") == "done"), steps=len(r.flat),
               stops=[x.get("why") for x in rows if x.get("event") == "stop"], end=[l for l in out.split("\n") if l.startswith(("record replayed", "STOP", "cursor"))])
    if live:
        from serve_replay_check import compare
        keys, bad, M, L = compare(rows, [json.loads(l) for l in open(live, encoding="utf-8")])
        res.update(compared=len(keys), live_decisions=len(L), differing={k: v[:10] for k, v in bad.items() if v},
                   identical={k: f"{len(keys) - len(v)}/{len(keys)}" for k, v in bad.items()})
    return res


def main():
    a = sys.argv[1:]
    get = lambda k, d=None: a[a.index(k) + 1] if k in a else d  # noqa: E731
    port = int(get("--port", 15740))
    os.environ["STS2_BRIDGE"] = f"127.0.0.1:{port}"
    srv = None
    if "--start" in a:
        from serve_replay_check import start_server
        srv = start_server(port, open(os.path.join(ROOT, "target", f"serve_{port}.log"), "w"))
    try:
        res = replay(a[0], get("--log", os.path.join(ROOT, "target", f"serve_replay_{port}.jsonl")), get("--live"))
    finally:
        if srv:
            srv.kill()
    print(json.dumps(res, indent=1))


if __name__ == "__main__":
    main()
