"""Run records: what the outer loop learns from.

`runs/<run-id>/events.jsonl` has one JSON object per line:
  {"t": unix time, "kind": ..., ...}
kinds: `fight_start` (scenario summary and the solver's prediction), `action` (one micro decision: chosen action, options with p / q, searched or forced),
`fight_end` (outcome, HP), `macro` (a non-combat decision: screen, options, choice, my reason, optional evaluation), `eval` (a macro evaluation and its
numbers), `note` (free text), `divergence` (the simulator disagreed with the game).
`agent.improve review` turns these into gap reports; `agent.improve corpus` collects the fight scenarios for fine-tuning.
"""
import json
import os
import time

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "runs")


class RunLog:
    def __init__(self, run_id=None):
        cur = os.path.join(ROOT, "CURRENT")
        if run_id is None and os.path.exists(cur):
            run_id = open(cur).read().strip() or None  # a daemon restart continues the same run record
        self.run_id = run_id or time.strftime("%Y%m%d-%H%M%S")
        self.dir = os.path.join(ROOT, self.run_id)
        self._f = None
        os.makedirs(ROOT, exist_ok=True)
        open(cur, "w").write(self.run_id)  # a daemon restart must continue this record (the seen-encounter bags and the review depend on it)

    def _open(self):
        if self._f is None:
            os.makedirs(self.dir, exist_ok=True)
            self._f = open(os.path.join(self.dir, "events.jsonl"), "a", encoding="utf-8")
        return self._f

    def event(self, kind, **kw):
        f = self._open()
        f.write(json.dumps(dict(t=round(time.time(), 2), kind=kind, **kw), default=str) + "\n")
        f.flush()

    def new_run(self):
        """Start a new record (a new run began)."""
        if self._f:
            self._f.close()
        self._f = None
        self.run_id = time.strftime("%Y%m%d-%H%M%S")
        self.dir = os.path.join(ROOT, self.run_id)
        os.makedirs(ROOT, exist_ok=True)
        open(os.path.join(ROOT, "CURRENT"), "w").write(self.run_id)


def read(run_dir):
    p = os.path.join(run_dir, "events.jsonl")
    return [json.loads(l) for l in open(p, encoding="utf-8")] if os.path.exists(p) else []
