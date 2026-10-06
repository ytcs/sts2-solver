"""Offline fakes for the harness tests: a scripted bridge, a deterministic engine, a pytest-like monkeypatch, golden files.

Nothing here may reach the live game (bridge :15555) or the harness daemon (:15556): the fake bridge raises on any command it has no reply for, the bridge
socket is patched to raise, and every file the harness writes (run records, evals, skill state) is redirected under a temporary directory.
"""
import hashlib
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
FIX = os.path.join(HERE, "fixtures")
GOLD = os.path.join(HERE, "golden")
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)


def screen(name):
    with open(os.path.join(FIX, "screens", name + ".txt"), encoding="utf-8") as f:  # universal newlines: a CRLF checkout (core.autocrlf) reads the same
        return f.read()


def fixture_json(name):
    with open(os.path.join(FIX, name), encoding="utf-8") as f:
        return json.load(f)


def deck():
    return fixture_json("deck.json")


# a synthesized Act 2 (Hive) map in the bridge's `m` format (mods/AgentBridge/src/Decisions.cs FullMap): I stand on r1c1
MAP_A2 = """rows bottom->top; point = <type>c<col>><child cols>; * = visited
r0: *Ac3>1,3,5
r1: *Mc1>0,2 Mc3>3 Mc5>6
r2: Mc0>1 Mc2>1,2 $c3>4 Mc6>5
r3: ?c1>1 Ec2>2 Mc4>4 Mc5>5
r4: Rc1>2 Ec2>2,3 Mc4>3 ?c5>4
r5: Mc2>2 Tc3>3 Mc4>3
r6: Ec2>2 Rc3>2,3
r7: Rc2> Rc3>
boss: 8 KNOWLEDGE_DEMON_BOSS
"""

MAP_SCREEN_A2 = """MAP
A2 F16 IRONCLAD A10 HP 52/80 G180 pots[Power Potion, -]
full map: m
0 Monster r2c0 -> ?c1
1 Monster r2c2 -> ?c1,Ec2
"""


# ---------------------------------------------------------------------------------------------------------------- monkeypatch

class MonkeyPatch:
    """The part of pytest's `monkeypatch` the tests use (setattr, setitem, setenv, delenv, undo)."""

    _MISSING = object()

    def __init__(self):
        self._undo = []

    def setattr(self, obj, name, value):
        old = getattr(obj, name, self._MISSING)
        self._undo.append(lambda: delattr(obj, name) if old is self._MISSING else setattr(obj, name, old))
        setattr(obj, name, value)

    def setitem(self, d, key, value):
        old = d.get(key, self._MISSING)
        self._undo.append(lambda: d.pop(key, None) if old is self._MISSING else d.__setitem__(key, old))
        d[key] = value

    def setenv(self, name, value):
        self.setitem(os.environ, name, str(value))

    def delenv(self, name, raising=False):
        if name in os.environ:
            self.setitem(os.environ, name, os.environ[name])
            del os.environ[name]

    def undo(self):
        while self._undo:
            self._undo.pop()()


# ---------------------------------------------------------------------------------------------------------------- the bridge

class FakeBridge:
    """Canned replies by command. `screen` is what `s` / `peek` show; `a <...>` replies come from `on_action` (a function of the command, or a list
    consumed in order); `m`, `deck.json`, `fight`, `p draw` and `snap` from attributes. Any other command fails the test."""

    def __init__(self, screen_text="", deck_json=None, map_text="no map\n", fight=None, on_action=None, draw="draw (0):\n"):
        self.screen = screen_text
        self.deck = deck_json
        self.map = map_text
        self.fight = fight
        self.on_action = on_action
        self.draw = draw
        self.calls = []
        self.extra = {}

    def __call__(self, line, *a, **kw):
        self.calls.append(line)
        cmd = line.split(" ", 1)[0]
        if line in self.extra:
            r = self.extra[line]
            return r(line) if callable(r) else r
        if cmd in ("s", "peek"):
            return self.screen
        if cmd == "m":
            return self.map
        if cmd == "deck.json":
            return (json.dumps(self.deck) if self.deck is not None else "null") + "\n"
        if cmd == "fight":
            f = self.fight() if callable(self.fight) else self.fight
            return (json.dumps(f) if f is not None else "null") + "\n"
        if line == "p draw":
            return self.draw
        if cmd in ("a", "do") and self.on_action is not None:
            if isinstance(self.on_action, list):
                if not self.on_action:
                    raise AssertionError(f"fake bridge: no scripted reply left for {line!r}")
                r = self.on_action.pop(0)
            else:
                r = self.on_action(line)
            if not r.startswith("ERR"):
                self.screen = r
            return r
        raise AssertionError(f"fake bridge: unexpected command {line!r}")

    def actions(self):
        return [c for c in self.calls if c.split(" ", 1)[0] in ("a", "do")]


def patch_bridge(mp, fake):
    """Replace `call` everywhere it was imported by name (agent.harness and any module split out of it later), and make the socket path raise."""
    import agent.bridge as bridge
    import agent.harness  # noqa: F401  imported first, so its modules bind the real `call` that is replaced below (a cold import after patching would keep a fake)
    real = bridge.call
    for name, mod in list(sys.modules.items()):
        c = getattr(mod, "call", None) if mod is not None and (name == "agent" or name.startswith("agent.")) else None
        if c is not None and (c is real or isinstance(c, FakeBridge)):  # a second harness in the same test replaces the first one's fake
            mp.setattr(mod, "call", fake)
    for name in ("agent.bridge", "agent.harness", "agent.live"):  # (agent.live: after the refactor)
        assert name not in sys.modules or sys.modules[name].call is fake, name

    def no_socket(*a, **kw):
        raise AssertionError("a test tried to reach the real bridge")
    mp.setattr(bridge, "_once", no_socket)
    return fake


# ---------------------------------------------------------------------------------------------------------------- the engine

def _h(*parts):
    return int(hashlib.sha1(json.dumps(parts, sort_keys=True, default=str).encode()).hexdigest()[:12], 16)


class FakeEngine:
    """Deterministic stand-in for `agent.engine.Engine`: every result is a function of the scenario's content (never of its position in the batch),
    so the same pricing request gives the same numbers before and after a refactor. `log` records every solve call (attempts + scenario keys)."""

    util_trained = False

    def __init__(self, decide=None):
        self.log = []
        self._decide = decide
        self.decide_calls = []

    @staticmethod
    def _key(sc):
        return (sc.get("name"), sc.get("encounter"), str(sc.get("seed")), sc.get("hp"), sc.get("max_hp"),
                sorted((c["id"], c.get("upgrade", 0), json.dumps(c.get("enchantment"), sort_keys=True)) for c in sc.get("deck", [])),
                sorted(r["id"] for r in sc.get("relics", [])), [p["id"] for p in sc.get("potions", [])])

    def solve(self, scenarios, attempts=64, seed=0, util=None, groups=None):
        self.log.append(dict(attempts=attempts, util=util is not None, scen=[[sc.get("name"), sc.get("encounter"), sc.get("hp"), [p["id"] for p in sc.get("potions", [])]] for sc in scenarios]))
        out = []
        for sc in scenarios:
            rng = random.Random(_h(self._key(sc), attempts))
            hp = int(sc.get("hp", 1))
            p = 0.15 + 0.8 * rng.random()
            ends = [round(hp * (0.2 + 0.8 * rng.random())) if rng.random() < p else 0 for _ in range(attempts)]
            wins = [e for e in ends if e > 0]
            w = len(wins) / attempts
            out.append(dict(win=w, win_se=(w * (1 - w) / attempts) ** 0.5, hp_lost=sum(hp - e for e in ends) / attempts / max(sc.get("max_hp", 80), 1),
                            hp_lost_se=0.01, hp_left_on_win=(sum(wins) / len(wins)) if wins else 0.0, attempts=attempts, aborted=0, ends=ends))
        return out

    def decide(self, scenario, sim, budget=1.0, seed=None, tol_hp=1.0, keep_potions=False, util=None):
        self.decide_calls.append(dict(budget=budget, tol_hp=tol_hp, keep_potions=keep_potions))
        if self._decide is not None:
            return self._decide(scenario, sim, budget, keep_potions)
        raise AssertionError("FakeEngine.decide called without a scripted decision")


# ---------------------------------------------------------------------------------------------------------------- the harness

def isolate(mp, tmp):
    """Every file a test could write goes under `tmp`; the per-process caches start empty."""
    from agent import improve, pickplan, routes, runlog, skillgate
    mp.setattr(runlog, "ROOT", os.path.join(str(tmp), "runs"))
    mp.setattr(improve, "EVALS", os.path.join(str(tmp), "evals"))
    mp.setattr(skillgate, "STATE", os.path.join(str(tmp), "skill_state"))
    routes._CACHE.clear()
    pickplan._GAINS.clear()


def make_harness(mp, tmp, fake, events=(), run_id="testrun", engine=None):
    """A bare Harness (skill gate off) on the fake bridge, its run record under tmp/runs/<run_id> pre-filled with `events` (dicts)."""
    isolate(mp, tmp)
    from agent import runlog
    d = os.path.join(runlog.ROOT, run_id)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(runlog.ROOT, "CURRENT"), "w") as f:
        f.write(run_id)
    if events:
        with open(os.path.join(d, "events.jsonl"), "w", encoding="utf-8") as f:
            for e in events:
                f.write(json.dumps(e) + "\n")
    patch_bridge(mp, fake)
    from agent.harness import Harness
    h = Harness()
    h.engine = engine or FakeEngine()
    return h


def fight_starts(upto=None):
    """The fight_start events of the recorded run (id, encounter, scenario.act), up to fight id `upto`."""
    out = []
    with open(os.path.join(FIX, "fight_starts.jsonl"), encoding="utf-8") as f:
        for l in f:
            e = json.loads(l)
            if upto is None or int(e["id"]) <= upto:
                out.append(e)
    return out


def events(h):
    p = os.path.join(h.log.dir, "events.jsonl")
    if not os.path.exists(p):
        return []
    with open(p, encoding="utf-8") as f:
        return [json.loads(l) for l in f]


def golden(name, text):
    """Compare with tests/agent/golden/<name>; STS2_UPDATE_GOLDEN=1 rewrites it."""
    p = os.path.join(GOLD, name)
    if os.environ.get("STS2_UPDATE_GOLDEN") == "1" or not os.path.exists(p):
        os.makedirs(GOLD, exist_ok=True)
        with open(p, "w", encoding="utf-8", newline="") as f:
            f.write(text)
        return
    with open(p, encoding="utf-8") as f:
        want = f.read()
    if want != text:
        import difflib
        diff = "".join(list(difflib.unified_diff(want.splitlines(True), text.splitlines(True), "golden", "now"))[:60])
        raise AssertionError(f"golden {name} differs:\n{diff}")


class Skip(Exception):
    """A skipped test (tests/agent/run.py counts it); under pytest, pytest.skip."""


def skip(why):
    try:
        import pytest
    except ImportError:
        raise Skip(why) from None
    pytest.skip(why)


def ok(out):
    """A command's output that must not be the harness's exception text."""
    assert not out.startswith("ERR harness"), out
    return out
