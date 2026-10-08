import json
import os
import re

from agent import macro, pools


class NoRun(Exception):
    pass


def bosses_from_map(map_text):
    m = re.search(r"^boss: \d+ (\w+)(?: \+ (\w+))?", map_text or "", re.M)
    return [b for b in (m.groups() if m else ()) if b]


def act_of_bosses(bosses):
    for b in bosses:
        for name, d in pools.ACTS.items():
            if b in pools.pool(name, "boss"):
                return d["act"]
    return None


def encounters_met(events_path, act):
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
    names = pools.act_names(act)
    if len(names) > 1 and bosses:
        names = [n for n in names if any(b in pools.pool(n, "boss") for b in bosses)] or names
    return names


class RunContext:
    def __init__(self, act, bosses, seen, map_text):
        self.act, self.bosses, self.seen, self.map_text = act, list(bosses), list(seen), map_text
        self.names = act_names(act, self.bosses)

    @property
    def ctx(self):
        return dict(seen=list(self.seen), bosses=list(self.bosses))

    def horizon(self):
        ctx = self.ctx
        boss = [e for n in self.names for e in macro.narrow(pools.pool(n, "boss"), "boss", ctx)]
        elites = [e for n in self.names for e in macro.narrow(pools.pool(n, "elite"), "elite", ctx)]
        nxt = [e for n in (pools.act_names(self.act + 1) if self.act < 2 else []) for k in ("elite", "boss") for e in pools.pool(n, k)]
        return dict(boss=boss, elites=elites, next=nxt, ctx=ctx)

    def future(self):
        ctx, out = self.ctx, []
        for ai in range(self.act, 3):
            for n in (self.names if ai == self.act else pools.act_names(ai)):
                for kind in ("elite", "boss"):
                    out += macro.narrow(pools.pool(n, kind), kind, ctx)
        return out
