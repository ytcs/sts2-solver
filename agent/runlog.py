import json
import os
import re
import time

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "runs")
SCREEN_LINE = re.compile(r"^[A-Z][A-Z_]{2,}( \d+| \(busy\))?\s*$")


def _set_current(run_id):
    os.makedirs(ROOT, exist_ok=True)
    with open(os.path.join(ROOT, "CURRENT"), "w") as f:
        f.write(run_id)


class RunLog:
    def __init__(self, run_id=None):
        cur = os.path.join(ROOT, "CURRENT")
        if run_id is None and os.path.exists(cur):
            with open(cur) as f:
                run_id = f.read().strip() or None
        self.run_id = run_id or time.strftime("%Y%m%d-%H%M%S")
        self.dir = os.path.join(ROOT, self.run_id)
        self._f = None
        self._live = {}
        _set_current(self.run_id)

    def _open(self):
        if self._f is None:
            os.makedirs(self.dir, exist_ok=True)
            self._f = open(os.path.join(self.dir, "events.jsonl"), "a", encoding="utf-8")
        return self._f

    def event(self, kind, **kw):
        f = self._open()
        f.write(json.dumps(dict(t=round(time.time(), 2), kind=kind, **kw), default=str) + "\n")
        f.flush()

    def live(self, screen=None, reply=None, fight=..., **kw):
        try:
            if reply is not None and SCREEN_LINE.match(reply.split("\n", 1)[0]):
                screen = reply
            live = self._live
            if live.get("run") != self.run_id:
                live.clear()
            live.update(kw, run=self.run_id, t=round(time.time(), 2))
            if screen is not None:
                live.update(screen=screen[:20000], t_screen=live["t"])
            if reply is not None:
                live["reply"] = reply[:20000]
            if fight is not ...:
                live["fight"] = {k: v for k, v in fight.items() if k != "states"} if isinstance(fight, dict) else None
            os.makedirs(self.dir, exist_ok=True)
            p = os.path.join(self.dir, "live.json")
            with open(p + ".tmp", "w", encoding="utf-8") as f:
                json.dump(live, f, default=str)
            for i in range(5):
                try:
                    os.replace(p + ".tmp", p)
                    break
                except PermissionError:
                    time.sleep(0.01 * (i + 1))
        except Exception:  # noqa: BLE001
            pass

    def new_run(self):
        if self._f:
            self._f.close()
        self._f = None
        self.run_id = time.strftime("%Y%m%d-%H%M%S")
        self.dir = os.path.join(ROOT, self.run_id)
        _set_current(self.run_id)


def read(run_dir):
    p = os.path.join(run_dir, "events.jsonl")
    if not os.path.exists(p):
        return []
    with open(p, encoding="utf-8") as f:
        return [json.loads(l) for l in f]
