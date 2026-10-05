"""The self-play harness: one long-lived object that holds the game connection, the solver and the aligned simulator of the current fight.

Commands (`Harness.handle(line)`, reachable from the shell as `python -m agent <command>`):
  s                     the game state (numbered options), as the bridge renders it
  adv [secs]            the state plus the solver's advice for the next combat action and the enemies' expected damage over the next turns (nothing is played)
  turn [secs]           play the current combat turn with the solver; combat [secs] plays the whole fight. secs = search budget per decision (default auto: the cap is set at fight start from the
                        solver's predicted danger, 1-15 s; `budget <s>` fixes it, `budget auto` restores). Search stops early on a clear winner or a tie
  a <i> [target] [-- why]   (chain steps with `;`, `~text` picks the option containing text: `a ~gold; ~card; ~proceed`; stops on error / combat, map click last)
                        take option i of the screen (macro and everything else); `-- why` is stored with the decision. A map click onto an elite or boss
                        below 60% HP is refused unless confirmed with `a <i> !`
  reward [--attempts N] [--hp full]   on a card reward screen: every option and skip priced against the boss (smooth), the elites left and the next act, in one table
  brief                 the run at a glance: header, deck, buckets and gaps, relics, potions, the known boss and the elites that can still appear
  eval {json} | eval [--all] [--smooth] (--enc IDS | --pool Act:kind[:n] | --future | --boss | --elites | --next) --v "name|add=A,B|upgrade=C|remove=D" ...   combat value of variants of the current deck against encounter pools
  route <M E R S B ...> [--hp N] [--act Hive] [--exclude IDS]   HP budget (pools narrowed to what can still appear: not the encounters already met this act, only the known boss) along a planned route (fights played at the HP I would arrive with, rests heal 30%)
  relics                relic counters in combat (Pen Nib, Book of Five Rings ...)
  hold ID[,ID]          keep those potions out of the solver's choices (`hold none` releases)
  note <text>           a free-text note in the run record
  newrun                start a new run record
  status                what the harness is holding (run id, fight, replay fidelity, engine)
  d | p draw | m | draw r1c6 r2c6 ... | x ... | f ...   straight to the bridge (deck, piles, map, draw a route on the map, dev console, fast mode)

Micro = `turn` / `combat`: the solver searches every action from a state rebuilt out of observations only (`agent.fight`), then the action is sent to the game.
Macro = me, with `eval` for the combat side of a choice and the strategy book (`.claude/skills/sts2-*`) for everything else.
"""
import json
import os
import re
import shlex
import threading
import time
import traceback

from agent import macro, skillgate
from agent.bridge import call
from agent.fight import Replayer
from agent.runlog import RunLog


def _kind(text):
    return text.split("\n", 1)[0].split(" ")[0] if text else "?"


def _act_index(text):
    """0-based act from a state header ("A2 F20 IRONCLAD ..."), None when the screen has no header."""
    m = re.search(r"A(\d+) F\d+", text or "")
    return int(m.group(1)) - 1 if m else None


def _hp(text):
    m = re.search(r"HP (\d+)/(\d+)", text)
    return (int(m.group(1)), int(m.group(2))) if m else None


PRED_ATTEMPTS = 320  # fights played from the start for the prediction (~1.2 s, same as 48): win rate +-0.03 at worst, HP cost +-0.5% of max HP


class Harness:
    def __init__(self):
        self.engine = None
        self._eng_lock = threading.Lock()
        self.rp = None
        self.fight_id = None
        self.log = RunLog()
        self.last_state = ""
        self.gate = False  # the daemon turns the skill gate on (`agent.skillgate`): no game action before the governing skills are loaded; tests build a bare Harness
        self.fight_hp0 = None
        self.fight_actions = 0
        self._ended = set()
        self.budget = None  # fixed seconds of search per decision (`budget <s>`); None = auto from the fight's predicted danger (`budget auto`)
        self.fight_budget = 1.0
        self.keep_potions = False
        self.hold = set()  # potion ids the solver may not use (kept for the boss): `hold ID,ID`, `hold none`
        self.fight_tol = 1.0  # HP of expected regret the search may leave on the table per decision

    # ------------------------------------------------------------------ plumbing

    def eng(self):
        with self._eng_lock:
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
        self._last_f = f
        return f

    def _fight_start(self, f):
        sc = f["scenario"]
        pred = {}
        try:
            usable = [p for p in sc["potions"] if p["id"] not in self.hold]  # held potions are not part of the plan for this fight
            r = self.eng().solve([dict(sc, name="start", potions=usable)], attempts=PRED_ATTEMPTS)[0]
            pred = dict(win=round(r["win"], 3), win_se=round(r["win_se"], 3), hp_lost=round(r["hp_lost"] or 0, 3), hp_lost_se=round(r.get("hp_lost_se") or 0, 3), n=r["attempts"])
        except Exception as e:  # noqa: BLE001
            pred = dict(error=str(e)[:80])
        self.fight_hp0 = (sc["hp"], sc["max_hp"])
        self.last_enc = sc.get("encounter")
        self.fight_budget, self.fight_tol = self._auto_budget(pred, sc["hp"], sc["max_hp"])
        # potions are for fights the solver may lose or that cost a lot: a comfortable fight keeps them (a clear win leaves the strongest potion for the elite or boss)
        def comfortable(p):
            return "win" in p and p["win"] - p["win_se"] >= 0.95 and p["hp_lost"] * sc["max_hp"] <= 0.4 * sc["hp"]
        self.keep_potions = comfortable(pred)
        if self.keep_potions and sc["potions"]:  # the prediction above may have used potions: confirm the fight is comfortable without them (else the live play, which may not use them, loses HP the prediction did not expect)
            try:
                r = self.eng().solve([dict(sc, name="start", potions=[])], attempts=PRED_ATTEMPTS)[0]
                pred_np = dict(win=round(r["win"], 3), win_se=round(r["win_se"], 3), hp_lost=round(r["hp_lost"] or 0, 3))
                pred["without_potions"] = pred_np
                self.keep_potions = comfortable(pred_np)
                if not self.keep_potions:
                    pred.update(pred_np, hp_lost_se=0.0)
            except Exception:  # noqa: BLE001
                self.keep_potions = False
        self.log.event("fight_start", id=f["id"], encounter=sc["encounter"], hp=sc["hp"], max_hp=sc["max_hp"], deck=len(sc["deck"]), relics=[r["id"] for r in sc["relics"]],
                       potions=[p["id"] for p in sc["potions"]], scenario=sc, predicted=pred, budget=self.fight_budget, tol_hp=self.fight_tol, keep_potions=(True if self.keep_potions else self.hold))

    @staticmethod
    def _auto_budget(pred, hp, max_hp):
        """(time cap in s, tolerated expected regret in HP) per decision, from the solver's prediction for this fight at this HP. The search stops by itself when the
        expected regret of the leading action is under the tolerance (a tie or a clear winner), so the cap only bounds contested decisions: it grows with the danger
        (2 * P(loss) + expected HP cost / current HP) and the tolerance shrinks. A fight that costs the same HP whatever I do ends early: that loss belongs to the macro side."""
        if "win" not in pred:
            return 5.0, 0.5
        # one standard error on the pessimistic side: a small sample must not make a fight look safer than it is
        danger = (1 - pred["win"] + pred["win_se"]) * 2 + (pred["hp_lost"] + pred["hp_lost_se"]) * max_hp / max(hp, 1)
        return round(min(15.0, 0.5 + 12 * danger), 1), round(max(0.25, 1 / (1 + 3 * danger)), 2)

    def _budget(self, override=None):
        return override if override is not None else (self.budget if self.budget is not None else self.fight_budget)

    def _fight_end(self, text=None):
        """Record the end of the current fight. `text` = the screen right after it (HP is read from it); without it the HP is unknown."""
        if self.rp is None or self.fight_id in self._ended:
            return
        self._ended.add(self.fight_id)
        hp = _hp(text) if text else None
        self._save_costly_fight(hp)
        self.log.event("fight_end", id=self.fight_id, hp=hp, screen=_kind(text) if text else None, hp_start=self.fight_hp0, actions=self.fight_actions, replay=dict(self.rp.stats),
                       errors=self.rp.errors[:3], diff_examples={k: v for k, v in self.rp.examples.items() if not k.startswith("random")})

    COSTLY = 0.30  # a fight that loses this share of max HP (or is lost) is kept whole for the hindsight review (`python -m agent.hindsight`)

    def _save_costly_fight(self, hp):
        """Keep the full export (scenario, action log, observed state after every action) of a costly fight: `runs/<run>/fights/<id>_<encounter>.json`. The log may
        end one action before the last (the final sync happens before the killing blow)."""
        f = getattr(self, "_last_f", None)
        if not hp or f is None or f.get("id") != self.fight_id or not self.fight_hp0:
            return
        lost = self.fight_hp0[0] - hp[0]
        if lost < self.COSTLY * self.fight_hp0[1] and hp[0] > 0:
            return
        try:
            d = os.path.join(self.log.dir, "fights")
            os.makedirs(d, exist_ok=True)
            enc = self.rp.scenario.get("encounter", "?") if self.rp is not None else "?"
            json.dump(dict(id=self.fight_id, encounter=enc, hp_start=self.fight_hp0, hp_end=hp, scenario=self.rp.scenario, fight=f), open(os.path.join(d, f"{self.fight_id}_{enc}.json"), "w"))
        except Exception:  # noqa: BLE001  never let bookkeeping break a fight
            pass

    # ------------------------------------------------------------------ micro

    def _advice_text(self, d):
        opts = sorted(d["options"], key=lambda o: -(o["q"] if o["q"] is not None else -9))
        alts = "; ".join(f"{o['text']} q{o['q']}" for o in opts[:3] if o["action"] != d["action"])
        mine = next((o for o in d["options"] if o["action"] == d["action"]), None)
        q = f" q{mine['q']}" if mine and mine["q"] is not None else ""
        return f"{d['text']}{q}" + (f"   [alt: {alts}]" if alts else "")

    def _game_json(self, j):
        """The simulator numbers potions by position in its own list; the game by slot (an empty first slot makes them differ). Translate a `use_potion` action."""
        a = json.loads(j)
        if "use_potion" in a:
            slots = [p["slot"] for p in self.rp.scenario.get("potions", [])]
            i = a["use_potion"]["slot"]
            if i < len(slots):
                a["use_potion"]["slot"] = slots[i]
            return json.dumps(a)
        return j

    def _sync_problem(self, f):
        """None when the simulator matches the game, else a loud description. Advice from a desynced simulator is stale or wrong: never use it silently."""
        if self.rp.errors:
            return f"SIMULATOR DESYNC ({self.rp.errors[-1][:120]}). Do not trust advice; play by hand or restart the fight tracking with `status`."
        from agent.fight import RANDOM_PREFIXES
        bad = [l for l in self.rp.sim.diff(json.dumps(f["state"])) if not l.startswith(RANDOM_PREFIXES) and "props.Skin" not in l]  # a relic's random cosmetic skin (Pael's Legion) is not game state
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
        d = self.eng().decide(self.rp.scenario, self.rp.sim, self._budget(budget), tol_hp=self.fight_tol, keep_potions=(True if self.keep_potions else self.hold))
        return self._advice_text(d) + f"   ({d['rounds']} rounds, {d['seconds']}s)\n" + self._outlook()

    def _answer_selection(self):
        """A card-selection prompt: the search picks card by card on a copy of the simulator; the whole answer goes to the game at once."""
        sim = self.rp.sim
        if sim.stage() != "choice":
            self.log.event("divergence", what="the game asks for a selection, the simulator does not")
            return call("a 0")
        s2, picks = sim.copy(), []
        for _ in range(40):
            d = self.eng().decide(self.rp.scenario, s2, min(self._budget(), 0.3))
            aj = json.loads(d["json"])
            if "pick" in aj:
                picks.append(s2.pick_game_index(aj["pick"]))
                s2.step(d["action"])
            else:
                break
        self.log.event("action", kind_="choose", picks=picks, text=f"choose {picks}")
        return call("do " + json.dumps({"choose": picks}))

    def play(self, whole_fight=False, budget=None, max_actions=120):
        budget = self._budget(budget)
        out = []
        tm = dict(state=0.0, sync=0.0, decide=0.0, do=0.0)
        T = time.perf_counter
        for _ in range(max_actions):
            t0 = T()
            txt = self.state()
            tm["state"] += T() - t0
            k = _kind(txt)
            if k not in ("COMBAT", "SELECT"):
                self._fight_end(txt)
                self.sync()
                out.append("-- combat over  (secs: " + " ".join(f"{k} {v:.1f}" for k, v in tm.items()) + ")")
                out.append(txt)
                return "\n".join(out)
            if txt.split("\n")[0].endswith("(busy)"):
                time.sleep(0.5)
                continue
            t0 = T()
            f = self.sync()
            tm["sync"] += T() - t0
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
                t0 = T()
                d = self.eng().decide(self.rp.scenario, self.rp.sim, budget, tol_hp=self.fight_tol, keep_potions=(True if self.keep_potions else self.hold))
                tm["decide"] += T() - t0
                self.fight_actions += 1
                self.log.event("action", fight=self.fight_id, text=d["text"], json=d["json"], searched=d["searched"], options=d["options"])
                out.append("  " + self._advice_text(d))
                t0 = T()
                reply = call("do " + self._game_json(d["json"]))
                tm["do"] += T() - t0
                tm["do_end"] = tm.get("do_end", 0.0) + (T() - t0 if '"end_turn"' in d["json"] else 0.0)
                tm["n"] = tm.get("n", 0) + 1
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
        """`a <i> [target] [-- why]`, or a chain `a 0; ~gold; ~card 1; ~proceed -- why`: steps run one after the other, each against the screen the previous one left.
        A step is an option number (+ args) or `~text` = the first option whose label contains text. The chain stops at the first error or refusal and when a combat
        starts; a map choice must be the last step (never chain map clicks)."""
        why = None
        if " -- " in argline:
            argline, why = argline.split(" -- ", 1)
        steps = [t.strip() for t in argline.split(";") if t.strip()]
        reply, last_kind = "", None
        for i, step in enumerate(steps):
            before = self.last_state if i else call("peek")  # after a step the reply is already the settled state
            if not i and (before.startswith("ERR") or before.split("\n")[0].endswith("(busy)")):
                before = self.state()  # mid-transition: wait for it to settle
            kind = _kind(before)
            if kind == "MENU" and not i and step.split()[0] == "0" and len(step.split()) >= 2:
                self.log.new_run()  # a new run starts from the menu: its own record (the narrowing of the encounter pools reads it)
            if i and self.gate:
                refusal = self._skill_refusal(before)
                if refusal:
                    return reply + f"[chain stopped before `{step}`: {refusal}]" + chr(10)
            if i and ((kind == "SELECT" and not step.startswith("~")) or kind == "MENU" or (kind == "COMBAT" and last_kind != "COMBAT")):
                return reply + f"[chain stopped before `{step}`: {_kind(before)}]\n"
            if _kind(before) == "MAP" and i < len(steps) - 1:
                return "REFUSED: a map choice must be the last step of a chain.\n" + before
            step = self._resolve(before, step)
            if step.startswith("ERR"):
                return step + "\n" + before
            guard = self._map_guard(before, step)
            if guard:
                return guard
            last_kind = kind
            reply = call("a " + step)
            self.log.event("macro", screen=before.split("\n")[0], state=before[:1500], choice=step, why=why, result=reply.split("\n")[0])
            self.last_state = reply
            if reply.startswith("ERR"):
                return reply
        return reply

    @staticmethod
    def _resolve(state, step):
        """`~text [args]` -> `<i> [args]` for the first option line whose label contains text (case-insensitive); text may be several words (args are `eN` or `!`)."""
        if not step.startswith("~"):
            return step
        words = step[1:].split()
        args = []
        while words and (re.fullmatch(r"e\d+|!|\d+", words[-1]) and len(words) > 1):
            args.insert(0, words.pop())
        want = " ".join(words).lower()
        for l in state.split("\n"):
            m = re.match(r"^(\d+) (.*)", l)
            if m and want in m.group(2).lower():
                return " ".join([m.group(1)] + args)
        return f"ERR no option matching `{want}`"

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
        text = macro.route_budget(self.eng(), deck, nodes, hp if hp is not None else deck["hp"], act, excl, att, ctx=self._ctx())
        self.log.event("route", nodes=nodes, hp=hp, text=text)
        return text

    def _future_encounters(self):
        """Elite and boss encounters of the current act and every later act (an act with two variants: the one the boss belongs to, when known)."""
        from agent import pools
        ctx = self._ctx()
        cur = _act_index(call("peek")) or 0
        out = []
        for ai in range(cur, 3):
            names = pools.act_names(ai)
            if ai == cur and len(names) > 1 and ctx["bosses"]:
                names = [n for n in names if any(b in pools.pool(n, "boss") for b in ctx["bosses"])] or names
            for n in names:
                for kind in ("elite", "boss"):
                    ids = macro.narrow(pools.pool(n, kind), kind, ctx)
                    out += ids
        return out

    def _horizon(self):
        """The three encounter sets a pick is judged against (`sts2-deckbuilding` section 4): `boss` = the act's known boss (or its pool when the map has not shown it),
        `elites` = the elites of this act that can still appear, `next` = every elite and boss of the next act (empty in the last act)."""
        from agent import pools
        ctx = self._ctx()
        cur = _act_index(call("peek")) or 0
        names = pools.act_names(cur)
        if len(names) > 1 and ctx["bosses"]:
            names = [n for n in names if any(b in pools.pool(n, "boss") for b in ctx["bosses"])] or names
        boss = [e for n in names for e in macro.narrow(pools.pool(n, "boss"), "boss", ctx)]
        elites = [e for n in names for e in macro.narrow(pools.pool(n, "elite"), "elite", ctx)]
        nxt = [e for n in (pools.act_names(cur + 1) if cur < 2 else []) for k in ("elite", "boss") for e in pools.pool(n, k)]
        return dict(boss=boss, elites=elites, next=nxt, ctx=ctx)

    def _ctx(self):
        """What narrows the encounter pools (see `macro.narrow`): the encounters met in the current act, in order (from this run's record, so a daemon restart loses
        nothing), and the act's boss(es) in fight order when the map shows them (`boss: <row> ID [+ ID]`)."""
        bosses, act, seen, ids = [], None, [], set()
        try:
            m = re.search(r"^boss: \d+ (\w+)(?: \+ (\w+))?", call("m"), re.M)
            bosses = [b for b in (m.groups() if m else ()) if b]
            act = _act_index(call("peek"))
            path = os.path.join(self.log.dir, "events.jsonl")
            if act is not None and os.path.exists(path):
                for line in open(path, encoding="utf-8"):
                    if '"fight_start"' not in line:
                        continue
                    e = json.loads(line)
                    if e.get("scenario", {}).get("act") == act and e["id"] not in ids:
                        ids.add(e["id"])
                        seen.append(e["encounter"])
        except Exception:  # noqa: BLE001
            pass
        return dict(seen=seen, bosses=bosses)

    def reward(self, argline):
        """reward [--attempts N] [--hp full|current|N]: the card reward on screen, every option and skip priced in one call (boss smooth, elites, next act)."""
        state = call("peek")
        if _kind(state) != "CARD_REWARD":
            return f"reward: not a card reward screen ({_kind(state)}); use eval\n"
        toks = shlex.split(argline)
        att = int(toks[toks.index("--attempts") + 1]) if "--attempts" in toks else 96
        hp = toks[toks.index("--hp") + 1] if "--hp" in toks else "full"
        hp = hp if hp in ("full", "current") else int(hp)
        raw = call("deck.json").strip()
        if raw == "null":
            return "no run in progress\n"
        deck = json.loads(raw)
        opts, skip = macro.parse_card_options(state)
        hz = self._horizon()
        text, res = macro.reward_report(self.eng(), deck, opts, hz, att, hp)
        self.log.event("reward_eval", options=[o[1] for o in opts], result={k: {str(i): v for i, v in r.items()} for k, r in res.items()}, boss=hz["boss"])
        return text + f"\nskip is option {skip}; pick with `a <i> -- why`\n"

    def brief(self):
        state = call("peek")
        raw = call("deck.json").strip()
        if raw == "null":
            return state
        return macro.brief_text(state, json.loads(raw), self._horizon()) + "\n"

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
                elif t in ("--boss", "--elites", "--next"):  # the horizon sets (see _horizon): the known boss, the elites that can still appear, the next act's elites + bosses
                    spec["encounters"] = self._horizon()[t[2:]]
                elif t == "--all":  # do not narrow the pool to the fights that can still appear
                    spec["all"] = True
                elif t == "--future":  # eval: the boss and elite pools of this act and every later act (horizon check)
                    spec["encounters"] = self._future_encounters()
                elif t == "--smooth":  # the deck-choice objective: win rate averaged over start HP x1 / 1.5 / 2 / 3 (macro.evaluate_smooth)
                    spec["smooth"] = True
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
        if not spec.pop("all", False):
            spec["_ctx"] = self._ctx()
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
        out = self._handle(line)
        self._watch_run_end(out)
        return out

    def _watch_run_end(self, out):
        """Record how a run ended (once per run record): the screen after the last fight, the floor, the encounter that ended it. `improve review` reads it."""
        try:
            kind = _kind(out)
            first = out.splitlines()[0].lower() if out else ""
            if not (kind == "GAME_OVER" or "victory" in first):
                return
            if getattr(self, "_run_end_logged", None) == self.log.run_id:
                return
            self._run_end_logged = self.log.run_id
            head = next((l for l in out.splitlines() if re.search(r"A\d+ F\d+", l)), "")
            self.log.event("run_end", screen=kind, header=head, last_encounter=getattr(self, "last_enc", None), text=out[:400])
        except Exception:  # noqa: BLE001  never let bookkeeping break a command
            pass

    def _skill_refusal(self, state_text):
        """None, or why this game action may not happen yet (skills of the ACTIVE session not loaded). Off for a bare Harness (tests) and with STS2_SKILL_GATE=off."""
        if not self.gate:
            return None
        return skillgate.gate_message(skillgate.active(), state_text)

    def _handle(self, line):
        line = line.strip()
        cmd, _, rest = line.partition(" ")
        if self.gate and cmd not in skillgate.READ_ONLY:
            why = self._skill_refusal(call("peek"))
            if why:
                return "REFUSED: " + why + chr(10)
        try:
            if cmd in ("", "s"):
                return self.state()
            secs = float(rest) if cmd in ("adv", "turn", "combat", "budget") and rest.replace(".", "", 1).isdigit() else None
            if cmd == "hold":
                self.hold = set() if rest.strip() in ("", "none") else {x.strip().upper().replace(" ", "_") for x in rest.split(",")}
                return f"solver may not use: {sorted(self.hold) or 'nothing held'}\n"
            if cmd == "budget":
                if secs is not None:
                    self.budget = secs
                elif rest.strip() == "auto":
                    self.budget = None
                now = f"auto (this fight: {self.fight_budget}s)" if self.budget is None else f"{self.budget}s"
                return f"search budget {now} per combat decision\n"
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
            if cmd == "reward":
                return self.reward(rest)
            if cmd == "brief":
                return self.brief()
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
