"""The run as the calculators see it: which act, which of its pool variants, the known boss(es), the encounters met this act, and the encounter sets a
choice is priced against. `Harness._context()` builds one per command (the map, the header and the run record are read once) and drops it when an action
runs; this module holds the rules.
"""
import json
import os
import re

from agent import macro, pools


class NoRun(Exception):
    """`deck.json` is null: no run in progress (each command prints its own text for it)."""


def bosses_from_map(map_text):
    """The act's boss(es) in fight order when the map shows them (`boss: <row> ID [+ ID]`), else []."""
    m = re.search(r"^boss: \d+ (\w+)(?: \+ (\w+))?", map_text or "", re.M)
    return [b for b in (m.groups() if m else ()) if b]


def act_of_bosses(bosses):
    """0-based act the boss belongs to, None when no boss is known."""
    for b in bosses:
        for name, d in pools.ACTS.items():
            if b in pools.pool(name, "boss"):
                return d["act"]
    return None


def encounters_met(events_path, act):
    """The encounters met in act `act`, in order, one per fight id (from the run record's `fight_start` lines, so a daemon restart loses nothing)."""
    if not os.path.exists(events_path):
        return
    ids = set()
    with open(events_path, encoding="utf-8") as f:
        for line in f:
            if '"fight_start"' not in line:
                continue
            e = json.loads(line)
            if e.get("scenario", {}).get("act") == act and e["id"] not in ids:
                ids.add(e["id"])
                yield e["encounter"]


def act_names(act, bosses):
    """The pool names of act `act`; an act with two variants (Overgrowth / Underdocks) is the one the known boss belongs to (both while it is unknown)."""
    names = pools.act_names(act)
    if len(names) > 1 and bosses:
        names = [n for n in names if any(b in pools.pool(n, "boss") for b in bosses)] or names
    return names


class RunContext:
    """act (0-based), names (its pool variants), bosses, seen (met this act); `ctx` is what `macro.narrow` reads (and what `eval` logs)."""

    def __init__(self, act, bosses, seen, map_text):
        self.act, self.bosses, self.seen, self.map_text = act, list(bosses), list(seen), map_text
        self.names = act_names(act, self.bosses)

    @property
    def ctx(self):
        return dict(seen=list(self.seen), bosses=list(self.bosses))

    def horizon(self):
        """The three encounter sets a pick is judged against (`sts2-deckbuilding` section 4): `boss` = the act's known boss (or its pool when the map has not
        shown it), `elites` = the elites of this act that can still appear, `next` = every elite and boss of the next act (empty in the last act)."""
        ctx = self.ctx
        boss = [e for n in self.names for e in macro.narrow(pools.pool(n, "boss"), "boss", ctx)]
        elites = [e for n in self.names for e in macro.narrow(pools.pool(n, "elite"), "elite", ctx)]
        nxt = [e for n in (pools.act_names(self.act + 1) if self.act < 2 else []) for k in ("elite", "boss") for e in pools.pool(n, k)]
        return dict(boss=boss, elites=elites, next=nxt, ctx=ctx)

    def future(self):
        """Elite and boss encounters of the current act and every later act (`eval --future`)."""
        ctx, out = self.ctx, []
        for ai in range(self.act, 3):
            for n in (self.names if ai == self.act else pools.act_names(ai)):
                for kind in ("elite", "boss"):
                    out += macro.narrow(pools.pool(n, kind), kind, ctx)
        return out
