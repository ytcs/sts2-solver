"""The self-play harness: one long-lived object that holds the game connection, the solver and the aligned simulator of the current fight.

Commands (`Harness.handle(line)`, reachable from the shell as `python -m agent <command>`):
  s                     the game state (numbered options), as the bridge renders it
  adv [secs]            the state plus the solver's advice for the next combat action and the enemies' expected damage over the next turns (nothing is played)
  turn [secs]           play the current combat turn with the solver; combat [secs] plays the whole fight. secs = search budget per decision (default 1 s,
                        `budget <s>` changes the default): a fraction of a second is enough for obvious turns, give high-stakes turns 5-20 s
  a <i> [target] [-- why]   take option i of the screen (macro and everything else); `-- why` is stored with the decision. A map click onto an elite or boss
                        below 60% HP is refused unless confirmed with `a <i> !`
  eval {json} | eval --enc IDS --v "name|add=A,B|upgrade=C|remove=D" ...   combat value of variants of the current deck against encounter pools
  route <M E R S B ...> [--hp N] [--act Hive] [--exclude IDS]   HP budget along a planned route (fights played at the HP I would arrive with, rests heal 30%)
  relics                relic counters in combat (Pen Nib, Book of Five Rings ...)
  note <text>           a free-text note in the run record
  status                what the harness is holding (run id, fight, replay fidelity, engine)
  d | p draw | m | draw r1c6 r2c6 ... | x ... | f ...   straight to the bridge (deck, piles, map, draw a route on the map, dev console, fast mode)

Micro = `turn` / `combat`: the solver searches every action from a state rebuilt out of observations only (`agent.fight`), then the action is sent to the game.
Macro = me, with `eval` for the combat side of a choice and the strategy book (`.claude/skills/sts2-*`) for everything else.
"""
import json
import re
import shlex
import time
import traceback

from agent import macro
from agent.bridge import call
from agent.fight import Replayer
from agent.runlog import RunLog


def _kind(text):
    return text.split("\n", 1)[0].split(" ")[0] if text else "?"


def _hp(text):
    m = re.search(r"HP (\d+)/(\d+)", text)
    return (int(m.group(1)), int(m.group(2))) if m else None


class Harness:
    def __init__(self):
        self.engine = None
        self.rp = None
        self.fight_id = None
        self.log = RunLog()
        self.last_state = ""
        self.fight_hp0 = None
        self.fight_actions = 0
        self._ended = set()
        self.budget = 1.0  # seconds of search per combat decision; `budget <s>` sets the default, `adv/turn/combat <s>` override once

    # ------------------------------------------------------------------ plumbing

    def eng(self):
        if self.engine is None:
            from agent.engine import Engine
            self.engine = Engine()
        return self.engine

    def state(self):
        self.last_state = call("s")
        return self.last_state

    def _fight_json(self):
        raw = call("fight").strip()
        return None if raw == "null" else json.loads(raw)

    def sync(self):
        """Bring the aligned simulator up to date with the live fight. Returns the fight export, or None outside combat."""
        f = self._fight_json()
        if f is None:
            if self.rp is not None:
                self._fight_end()
            self.rp = None
            return None
        if self.rp is None or f["id"] != self.fight_id or len(f["log"]) < self.rp.applied:
            if self.rp is not None and f["id"] != self.fight_id:
                self._fight_end()
            self.rp = Replayer(f)
            self.fight_id = f["id"]
            self.fight_actions = 0
            self._fight_start(f)
        self.rp.advance(f)
        return f

    def _fight_start(self, f):
        sc = f["scenario"]
        pred = {}
        try:
            r = self.eng().solve([dict(sc, name="start")], attempts=48)[0]
            pred = dict(win=round(r["win"], 3), hp_lost=round(r["hp_lost"] or 0, 3))
        except Exception as e:  # noqa: BLE001
            pred = dict(error=str(e)[:80])
        self.fight_hp0 = (sc["hp"], sc["max_hp"])
        self.log.event("fight_start", id=f["id"], encounter=sc["encounter"], hp=sc["hp"], max_hp=sc["max_hp"], deck=len(sc["deck"]), relics=[r["id"] for r in sc["relics"]],
                       potions=[p["id"] for p in sc["potions"]], scenario=sc, predicted=pred)

    def _fight_end(self, text=None):
        """Record the end of the current fight. `text` = the screen right after it (HP is read from it); without it the HP is unknown."""
        if self.rp is None or self.fight_id in self._ended:
            return
        self._ended.add(self.fight_id)
        hp = _hp(text) if text else None
        self.log.event("fight_end", id=self.fight_id, hp=hp, screen=_kind(text) if text else None, hp_start=self.fight_hp0, actions=self.fight_actions, replay=dict(self.rp.stats),
                       errors=self.rp.errors[:3])

    # ------------------------------------------------------------------ micro

    def _advice_text(self, d):
        opts = sorted(d["options"], key=lambda o: -(o["q"] if o["q"] is not None else -9))
        alts = "; ".join(f"{o['text']} q{o['q']}" for o in opts[:3] if o["action"] != d["action"])
        mine = next((o for o in d["options"] if o["action"] == d["action"]), None)
        q = f" q{mine['q']}" if mine and mine["q"] is not None else ""
        return f"{d['text']}{q}" + (f"   [alt: {alts}]" if alts else "")

    def _sync_problem(self, f):
        """None when the simulator matches the game, else a loud description. Advice from a desynced simulator is stale or wrong: never use it silently."""
        if self.rp.errors:
            return f"SIMULATOR DESYNC ({self.rp.errors[-1][:120]}). Do not trust advice; play by hand or restart the fight tracking with `status`."
        from agent.fight import RANDOM_PREFIXES
        bad = [l for l in self.rp.sim.diff(json.dumps(f["state"])) if not l.startswith(RANDOM_PREFIXES)]
        return f"SIMULATOR DIFFERS FROM THE GAME: {bad[0][:140]}" if bad else None

    def _outlook(self):
        """Expected enemy damage over the next turns after the shown intent (the move pattern humans know by heart)."""
        rows = self.rp.sim.lookahead()
        return "outlook (expected damage, next 3 turns): " + "  ".join(f"e{i} " + "/".join(f"{x:.0f}" for x in v) for i, v in rows) if rows else ""

    def advice(self, budget=None):
        f = self.sync()
        if f is None:
            return "not in combat"
        bad = self._sync_problem(f)
        if bad:
            return bad
        d = self.eng().decide(self.rp.scenario, self.rp.sim, self.budget if budget is None else budget)
        return self._advice_text(d) + f"   ({d['rounds']} rounds, {d['seconds']}s)\n" + self._outlook()

    def _answer_selection(self):
        """A card-selection prompt: the search picks card by card on a copy of the simulator; the whole answer goes to the game at once."""
        sim = self.rp.sim
        if sim.stage() != "choice":
            self.log.event("divergence", what="the game asks for a selection, the simulator does not")
            return call("a 0")
        s2, picks = sim.copy(), []
        for _ in range(40):
            d = self.eng().decide(self.rp.scenario, s2, min(self.budget, 0.3))
            aj = json.loads(d["json"])
            if "pick" in aj:
                picks.append(s2.pick_game_index(aj["pick"]))
                s2.step(d["action"])
            else:
                break
        self.log.event("action", kind_="choose", picks=picks, text=f"choose {picks}")
        return call("do " + json.dumps({"choose": picks}))

    def play(self, whole_fight=False, budget=None, max_actions=120):
        budget = self.budget if budget is None else budget
        out = []
        for _ in range(max_actions):
            txt = self.state()
            k = _kind(txt)
            if k not in ("COMBAT", "SELECT"):
                self._fight_end(txt)
                self.sync()
                out.append("-- combat over")
                out.append(txt)
                return "\n".join(out)
            if txt.split("\n")[0].endswith("(busy)"):
                time.sleep(0.5)
                continue
            f = self.sync()
            if f is None:
                continue
            bad = self._sync_problem(f)
            if bad:
                out.append(bad)
                out.append(txt)
                return "\n".join(out)
            if k == "SELECT":
                reply = self._answer_selection()
                out.append("  choose")
            else:
                d = self.eng().decide(self.rp.scenario, self.rp.sim, budget)
                self.fight_actions += 1
                self.log.event("action", fight=self.fight_id, text=d["text"], json=d["json"], searched=d["searched"], options=d["options"])
                out.append("  " + self._advice_text(d))
                reply = call("do " + d["json"])
                if reply.startswith("ERR"):
                    self.log.event("divergence", what="the game rejected a legal simulator action", action=d["json"], reply=reply.split("\n")[0])
                    out.append(reply)
                    return "\n".join(out)
                if '"end_turn"' in d["json"] and not whole_fight:
                    self.sync()
                    out.append(reply)
                    return "\n".join(out)
            if reply.startswith("ERR"):
                out.append(reply)
                return "\n".join(out)
            self.last_state = reply
        out.append("-- stopped after max_actions")
        out.append(self.state())
        return "\n".join(out)

    # ------------------------------------------------------------------ macro

    def act(self, argline):
        why = None
        if " -- " in argline:
            argline, why = argline.split(" -- ", 1)
        before = self.state()
        guard = self._map_guard(before, argline)
        if guard:
            return guard
        reply = call("a " + argline)
        self.log.event("macro", screen=before.split("\n")[0], state=before[:1500], choice=argline, why=why, result=reply.split("\n")[0])
        self.last_state = reply
        return reply

    def _map_guard(self, state, argline):
        """Entering an elite below 60% HP needs an explicit `!` after the option number (e.g. `a 0 !`). A shakedown run died by chaining a map click that was an elite
        at 37/80 HP when the rest site was the plan. Read the options before every map choice."""
        if _kind(state) != "MAP":
            return None
        toks = argline.split()
        if not toks or not toks[0].isdigit() or "!" in toks:
            return None
        hp = _hp(state)
        line = next((l for l in state.split("\n") if re.match(rf"^{toks[0]} ", l)), "")
        if hp and hp[0] < 0.6 * hp[1] and re.match(rf"^{toks[0]} (Elite|Boss)", line):
            return f"REFUSED: `{line}` at {hp[0]}/{hp[1]} HP. Heal first, or confirm with `a {toks[0]} !` if this is deliberate (check `route` first).\n" + state
        return None

    def route(self, argline):
        """route <tokens> [--hp N] [--act Overgrowth] [--exclude ID,ID] [--attempts N]: HP budget along a planned route (see agent.macro.route_budget)."""
        toks = shlex.split(argline)
        nodes, hp, act, excl, att = [], None, "Overgrowth", [], 48
        i = 0
        while i < len(toks):
            t = toks[i]
            if t == "--hp":
                hp = int(toks[i + 1]); i += 1
            elif t == "--act":
                act = toks[i + 1]; i += 1
            elif t == "--exclude":
                excl = toks[i + 1].split(","); i += 1
            elif t == "--attempts":
                att = int(toks[i + 1]); i += 1
            else:
                nodes.append(t)
            i += 1
        raw = call("deck.json").strip()
        if raw == "null":
            return "no run in progress"
        deck = json.loads(raw)
        text = macro.route_budget(self.eng(), deck, nodes, hp if hp is not None else deck["hp"], act, excl, att)
        self.log.event("route", nodes=nodes, hp=hp, text=text)
        return text

    def evaluate(self, argline):
        argline = argline.strip()
        if argline.startswith("{"):
            spec = json.loads(argline)
        else:
            toks = shlex.split(argline)
            spec = dict(variants=[dict(name="baseline")], attempts=64)
            i = 0
            while i < len(toks):
                t = toks[i]
                if t == "--enc":
                    spec["encounters"] = toks[i + 1].split(",")
                    i += 1
                elif t == "--pool":
                    a, kind, *n = toks[i + 1].split(":")
                    spec["encounters"] = dict(act=a, kind=kind, n=int(n[0]) if n else 0)
                    i += 1
                elif t == "--attempts":
                    spec["attempts"] = int(toks[i + 1])
                    i += 1
                elif t == "--hp":
                    spec["hp"] = toks[i + 1] if toks[i + 1] in ("full", "current") else int(toks[i + 1])
                    i += 1
                elif t == "--v":
                    parts = toks[i + 1].split("|")
                    v = dict(name=parts[0])
                    for p in parts[1:]:
                        k, val = p.split("=", 1)
                        v[k] = int(val) if k == "hp" else val.split(",")
                    spec["variants"].append(v)
                    i += 1
                i += 1
        if "encounters" not in spec:
            return "need --enc IDS or --pool Act:kind[:n]"
        raw = call("deck.json").strip()
        if raw == "null":
            return "no run in progress"
        text, summary = macro.evaluate(self.eng(), json.loads(raw), spec)
        self.log.event("eval", spec=spec, result=summary)
        return text

    # ------------------------------------------------------------------ dispatch

    def status(self):
        st = dict(self.rp.stats) if self.rp else {}
        bad = {k: v for k, v in st.items() if k.startswith(("diff", "residual", "action failed", "intent unmatched", "start unmatched", "missing"))}
        return (f"run {self.log.run_id}  engine {'loaded' if self.engine else 'not loaded'}  fight {self.fight_id}  actions {self.fight_actions}\n"
                f"replay: {st.get('end_turn_matched', 0)} enemy turns matched, {st.get('end_turn_unmatched', 0)} unmatched; divergences: {bad or 'none'}")

    def handle(self, line):
        line = line.strip()
        cmd, _, rest = line.partition(" ")
        try:
            if cmd in ("", "s"):
                return self.state()
            secs = float(rest) if cmd in ("adv", "turn", "combat", "budget") and rest.replace(".", "", 1).isdigit() else None
            if cmd == "budget":
                if secs is not None:
                    self.budget = secs
                return f"search budget {self.budget}s per combat decision\n"
            if cmd == "adv":
                return self.state() + "advice: " + self.advice(secs) + "\n"
            if cmd == "turn":
                return self.play(False, secs)
            if cmd == "combat":
                return self.play(True, secs)
            if cmd == "a":
                return self.act(rest)
            if cmd == "eval":
                return self.evaluate(rest)
            if cmd == "relics":   # relic counters and saved state of the live fight (e.g. Pen Nib: attacks played so far, Book of Five Rings ...)
                raw = call("snap").strip()
                if raw == "null":
                    return "not in combat\n"
                st = json.loads(raw)
                return "\n".join(f"{r['id']}" + (f" counter {r['counter']}" if "counter" in r else "") + (f" {r['props']}" if "props" in r else "") for r in st["relics"]) + "\n"
            if cmd == "route":
                return self.route(rest)
            if cmd == "note":
                self.log.event("note", text=rest)
                return "noted\n"
            if cmd == "status":
                return self.status() + "\n"
            if cmd == "newrun":
                self.log.new_run()
                return f"run {self.log.run_id}\n"
            return call(line)
        except Exception:  # noqa: BLE001
            return "ERR harness: " + traceback.format_exc()
