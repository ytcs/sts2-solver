"""Run records: `runs/<run-id>/events.jsonl`, one JSON object per line `{"t": unix time, "kind": ..., ...}`.

kinds: `fight_start` (scenario, the solver's prediction), `action` (a micro decision: chosen action, options with p / q), `fight_end` (outcome, HP, replay fidelity),
`macro` (a non-combat decision: screen, choice, `why`), `eval` / `reward_eval` / `route` (the numbers behind a macro decision), `run_end` (game-over or victory screen),
`note`, `divergence` (the game rejected an action the simulator allowed). Costly fights are kept whole under `runs/<run-id>/fights/`.
`agent.improve review` reads these; `agent.improve corpus` collects the fight scenarios for fine-tuning.
"""
import json
import os
import re
import time

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "runs")
SCREEN_LINE = re.compile(r"^[A-Z][A-Z_]{2,}( \d+| \(busy\))?\s*$")  # a bridge screen's first line (`COMBAT`, `SELECT 1`); not `ERR ...`, `REFUSED: ...`


def _set_current(run_id):
    """runs/CURRENT names the record a daemon restart continues (the seen-encounter bags and the review depend on it)."""
    os.makedirs(ROOT, exist_ok=True)
    with open(os.path.join(ROOT, "CURRENT"), "w") as f:
        f.write(run_id)


class RunLog:
    def __init__(self, run_id=None):
        cur = os.path.join(ROOT, "CURRENT")
        if run_id is None and os.path.exists(cur):
            with open(cur) as f:
                run_id = f.read().strip() or None  # a daemon restart continues the same run record
        self.run_id = run_id or time.strftime("%Y%m%d-%H%M%S")
        self.dir = os.path.join(ROOT, self.run_id)
        self._f = None
        self._live = {}  # what `live()` last wrote to live.json
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
        """`runs/<id>/live.json`: the latest screen, harness reply and fight export the harness ALREADY fetched (no bridge call here), for the
        read-only dashboard (`tools/dashboard`). `reply` replaces `screen` when it is a screen itself (first line `COMBAT`, `MAP`, `SELECT 1` ...);
        the fight export is written without its growing `states` list. Atomic (temp file + rename); never raises."""
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
            if fight is not ...:  # None = not in a fight (the harness keeps the last export after a fight ends)
                live["fight"] = {k: v for k, v in fight.items() if k != "states"} if isinstance(fight, dict) else None
            os.makedirs(self.dir, exist_ok=True)
            p = os.path.join(self.dir, "live.json")
            with open(p + ".tmp", "w", encoding="utf-8") as f:
                json.dump(live, f, default=str)
            for i in range(5):  # Windows: the rename fails while the dashboard holds the file open (a read takes well under a millisecond)
                try:
                    os.replace(p + ".tmp", p)
                    break
                except PermissionError:
                    time.sleep(0.01 * (i + 1))
        except Exception:  # noqa: BLE001  the dashboard feed must never break a command
            pass

    def new_run(self):
        """Start a new record (a new run began)."""
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
