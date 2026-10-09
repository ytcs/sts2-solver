import hashlib
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
FIX = os.path.join(HERE, "fixtures")
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)


def screen(name):
    with open(os.path.join(FIX, "screens", name + ".txt"), encoding="utf-8") as f:
        return f.read()


class MonkeyPatch:
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


class FakeBridge:
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
    import agent.bridge as bridge
    import agent.harness  # noqa: F401
    real = bridge.call
    for name, mod in list(sys.modules.items()):
        c = getattr(mod, "call", None) if mod is not None and (name == "agent" or name.startswith("agent.")) else None
        if c is not None and (c is real or isinstance(c, FakeBridge)):
            mp.setattr(mod, "call", fake)
    for name in ("agent.bridge", "agent.harness", "agent.live"):
        assert name not in sys.modules or sys.modules[name].call is fake, name

    def no_socket(*a, **kw):
        raise AssertionError("a test tried to reach the real bridge")
    mp.setattr(bridge, "_once", no_socket)
    return fake


def _h(*parts):
    return int(hashlib.sha1(json.dumps(parts, sort_keys=True, default=str).encode()).hexdigest()[:12], 16)


class FakeEngine:
    worth_ok = True

    def __init__(self, decide=None):
        self.log = []
        self._decide = decide
        self.decide_calls = []

    @staticmethod
    def _key(sc):
        return (sc.get("name"), sc.get("encounter"), str(sc.get("seed")), sc.get("hp"), sc.get("max_hp"),
                sorted((c["id"], c.get("upgrade", 0), json.dumps(c.get("enchantment"), sort_keys=True)) for c in sc.get("deck", [])),
                sorted(r["id"] for r in sc.get("relics", [])), [p["id"] for p in sc.get("potions", [])])

    def solve(self, scenarios, attempts=64, seed=None, groups=None, worth=None):
        self.log.append(dict(attempts=attempts, **({"worth": worth.get("kind", "table")} if worth is not None else {}), scen=[[sc.get("name"), sc.get("encounter"), sc.get("hp"), [p["id"] for p in sc.get("potions", [])]] for sc in scenarios]))
        out = []
        for sc in scenarios:
            rng = random.Random(_h(self._key(sc), attempts))
            hp = int(sc.get("hp", 1))
            p = 0.15 + 0.8 * rng.random()
            ends = [round(hp * (0.2 + 0.8 * rng.random())) if rng.random() < p else 0 for _ in range(attempts)]
            wins = [e for e in ends if e > 0]
            w = len(wins) / attempts
            out.append(dict(win=w, win_se=(w * (1 - w) / attempts) ** 0.5, hp_lost=sum(hp - e for e in ends) / attempts / max(sc.get("max_hp", 80), 1),
                            hp_lost_se=0.01, hp_left_on_win=(sum(wins) / len(wins)) if wins else 0.0, attempts=attempts, aborted=0, ends=ends,
                            wins=[1.0 if e > 0 else 0.0 for e in ends]))
        return out

    @staticmethod
    def arm_key(sim):
        sn = json.loads(sim.snapshot())
        st = sum(p.get("amount", 0) for p in sn.get("player", {}).get("powers", []) if p.get("id") == "STRENGTH_POWER")
        return len(sn.get("potions", [])), st, sn.get("turn")

    def outcome(self, key, sd):
        rng = random.Random(_h(str(sd)))
        return (1, 0.2 + 0.5 * rng.random()) if rng.random() < 0.6 else (-1, 0.0)

    def play_on(self, scenario, starts, seeds, worth=None, record=False):
        self.log.append(dict(play_on=len(starts), worth=(worth or {}).get("kind", "linear"), potions=[len(json.loads(s.snapshot()).get("potions", [])) for s in starts[:1]]))
        mx = scenario.get("max_hp", 80)
        out = []
        for s, sd in zip(starts, seeds):
            oc, fr = self.outcome(self.arm_key(s), sd)
            row = (oc, fr, round(fr * mx) if oc == 1 else 0.0)
            if record:
                c, acts = s.copy(), [a for a, t in s.legal() if t == "end turn"][:1]
                for a in acts[:1]:
                    c.step(a)
                while c.stage() == "choice" and len(acts) < 40:
                    acts.append(c.legal()[0][0])
                    c.step(acts[-1])
                row += (acts,)
            out.append(row)
        return out

    def decide(self, scenario, sim, budget=1.0, seed=None, tol_hp=1.0, keep_potions=False, worth=None):
        self.decide_calls.append(dict(budget=budget, tol_hp=tol_hp, keep_potions=keep_potions, worth=(worth or {}).get("kind", "linear")))
        if self._decide is not None:
            return self._decide(scenario, sim, budget, keep_potions)
        raise AssertionError("FakeEngine.decide called without a scripted decision")


def isolate(mp, tmp):
    from agent import improve, pickplan, routes, runlog, skillgate
    mp.setattr(runlog, "ROOT", os.path.join(str(tmp), "runs"))
    mp.setattr(improve, "EVALS", os.path.join(str(tmp), "evals"))
    mp.setattr(skillgate, "STATE", os.path.join(str(tmp), "skill_state"))
    routes._CACHE.clear()
    pickplan._GAINS.clear()


def make_harness(mp, tmp, fake, events=(), run_id="testrun", engine=None):
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


def ok(out):
    assert not out.startswith("ERR harness"), out
    return out
