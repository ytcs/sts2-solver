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
  routes [--attempts N] [--pf P]   survival of every route on the act map (exact DP over node x HP with the solver's fight outcomes): per option on offer, P(win boss) with at least k more elites, and the representative route per k (agent/routes.py)
  rmcalc [--attempts N] [--hp full|current|N]   every removable card priced as a removal (boss smooth, elites left, next act), ranked: use at a shop's removal, a removal event
  relics                relic counters in combat (Pen Nib, Book of Five Rings ...)
  hold ID[,ID]          keep those potions out of the per-turn check, the search and every table (`hold none` releases)
  potions               the per-turn potion check on demand: throw each potion now vs never this fight, and spend vs keep (agent.potion_price)
  potion allow|deny|keep <name|all>   allow/deny: the potions the live search may use in this fight (default none: potions are my call; `turn` / `combat` stop on
                        POTION ALERT); keep: no more alerts for it this fight unless this fight's win is at stake
  note <text>           a free-text note in the run record
  newrun                start a new run record
  status                what the harness is holding (run id, fight, replay fidelity, engine)
  d | p draw | m | draw r1c6 r2c6 ... | x ... | f ...   straight to the bridge (deck, piles, map, draw a route on the map, dev console, fast mode)

Micro = `turn` / `combat`: the solver searches every action from a state rebuilt out of observations only (`agent.fight`), then the action is sent to the game.
Macro = me, with `eval` for the combat side of a choice and the strategy book (`.claude/skills/sts2-*`) for everything else.

Layout: this file dispatches commands and owns the state; `agent.live` (mixin) is the fight loop and the potion junctures, `agent.guards` the decision guards,
`agent.screen` reads screen text, `agent.runctx` the run context the calculators price against, `agent.potions` the slot numbering and the potion policy.
"""
import functools
import json
import os
import re
import threading
import traceback
import zlib

from agent import guards, macro, potions, runctx, skillgate, tracker
from agent import screen as scr
from agent.args import Args
from agent.bridge import call
from agent.live import REPLAY_BAD, Live
from agent.runlog import RunLog

PICK_RECORD = guards.PICK_RECORD

# calculator outputs that priced nothing (a guard must not count them as the procedure having run); "routes: no node left before the boss" did price
PRICING_FAILED = ("ERR", "REFUSED", "reward: not", "no run", "need --enc", "routes: no act map", "routes: boss unknown", "eval: ")
PRICING = {"eval": "evaluate", "reward": "reward", "route": "route", "routes": "routes", "rmcalc": "rmcalc", "pickplan": "pickplan"}

# screen readers under their old names (agent.screen has the rules)
_floor, _kind, _act_index, _hp = scr.floor_key, scr.kind, scr.act_index, scr.hp


def _needs_run(reply):
    """A calculator that prices the run snapshot: `reply` is what it prints when there is no run (`runctx.NoRun` from `_run`)."""
    def deco(fn):
        @functools.wraps(fn)
        def wrapped(self, *a, **kw):
            try:
                return fn(self, *a, **kw)
            except runctx.NoRun:
                return reply
        return wrapped
    return deco


class Harness(Live):
    def __init__(self):
        self.engine = None
        self._eng_lock = threading.Lock()
        self.rp = None
        self.fight_id = None
        self.log = RunLog()
        self.last_state = ""
        self.priced = {}  # calculator -> floor ('A1 F5') it last ran on: the decision guards (`_decision_guard`) read it
        self.table_seed = 0  # seed of the tables on this screen (set per pricing call from the floor, `--seed N` adds N): `Engine.table_seed`
        self.reward_screen = None  # (floor, card names) of the card reward last priced with `reward` (the pick guard needs it)
        self.gate = False  # the daemon turns the skill gate on (`agent.skillgate`): no game action before the governing skills are loaded; tests build a bare Harness
        self.fight_hp0 = None
        self.fight_actions = 0
        self._ended = set()
        self.budget = None  # fixed seconds of search per decision (`budget <s>`); None = auto from the fight's predicted danger (`budget auto`)
        self.fight_budget = 1.0
        self._allowed = set()  # potions the live search may use in the current fight (`potion allow`, agent.live); none by default
        self._checked_turn = None
        self.potions_used = 0
        self._pred_q = None  # the predicted distribution of HP lost for this fight (calibration: where the real loss falls in it)
        self.hold = self._load_hold()  # potion ids the solver may not use (kept for the boss): `hold ID,ID`, `hold none`; saved with the run record, so a daemon restart keeps it
        self.fight_tol = 1.0  # HP of expected regret the search may leave on the table per decision
        self._rc = None  # the run context of this command (`_context`), dropped when an action runs

    # ------------------------------------------------------------------ plumbing

    def _hold_path(self):
        return os.path.join(self.log.dir, "hold.json")

    def _load_hold(self):
        try:
            with open(self._hold_path(), encoding="utf-8") as f:
                return set(json.load(f))
        except (OSError, ValueError):
            return set()

    def _save_hold(self):
        try:
            os.makedirs(self.log.dir, exist_ok=True)
            with open(self._hold_path(), "w", encoding="utf-8") as f:
                json.dump(sorted(self.hold), f)
        except OSError as e:
            self._bookkeeping_error("save hold", e)

    def _bookkeeping_error(self, where, e):
        """A failure that must not break the command, recorded in the run record instead of hidden."""
        try:
            self.log.event("harness_error", where=where, error=f"{type(e).__name__}: {e}"[:300])
        except Exception:  # noqa: BLE001  the record itself is what failed
            pass

    def eng(self):
        with self._eng_lock:
            if self.engine is None:
                from agent.engine import Engine
                self.engine = Engine()
            self.engine.table_seed = self.table_seed
            return self.engine

    def _send(self, line):
        """Every game action (`a ...`, `do ...`) goes through here: the screen changes, so the run context of this command is stale."""
        self._rc = None
        return call(line)

    def _new_run(self):
        """A new run record (the narrowing of the encounter pools reads it) and every per-run memory cleared: holds, priced floors, the last reward table."""
        self.log.new_run()
        self.hold = set()
        self.priced = {}
        self.reward_screen = None

    # ------------------------------------------------------------------ the run as the calculators see it

    def _deck_raw(self):
        """The run snapshot every calculator prices (`deck.json`) as text, with the potions I `hold` taken out of the belt (`potions.priced_view`); "null" = no run."""
        raw = call("deck.json").strip()
        if raw == "null" or not self.hold:
            return raw
        return json.dumps(potions.priced_view(json.loads(raw), self.hold))

    def _run(self):
        """The priced run snapshot as a dict; raises `runctx.NoRun` when no run is in progress."""
        raw = self._deck_raw()
        if raw == "null":
            raise runctx.NoRun()
        return json.loads(raw)

    def _cur_act(self, ctx):
        """0-based act for the pools. The act the map's boss belongs to wins over the state header: `peek` does not wait for the screen to settle and can come back
        without a header (right after a fight or an act change), which used to fall back to act 0 and price Act 1 bosses in Act 2."""
        act = runctx.act_of_bosses(ctx["bosses"])
        if act is not None:
            return act
        errs = []
        for _ in range(3):
            s = call("peek")
            act = scr.act_index(s)
            if act is not None:
                return act
            if s.startswith("ERR"):
                errs.append(s.split("\n")[0])
        if errs:  # act 0 because the bridge failed, not because the screen has no header: the pools may be the wrong act's
            self._bookkeeping_error("context: act", RuntimeError(errs[-1]))
        return 0

    def _context(self):
        """The run context (`runctx.RunContext`) of this command: the map's boss(es), the act, the encounters met this act (read from this run's record).
        Built once per command and dropped when an action runs; a failure to read the map or the record is logged and leaves that part empty."""
        if self._rc is not None:
            return self._rc
        bosses, map_text, seen = [], "", []
        try:
            map_text = call("m")
            if map_text.startswith("ERR"):  # the bridge reports its failures as text: no boss known, but say why in the record
                self._bookkeeping_error("context: map", RuntimeError(map_text.split("\n")[0]))
            bosses = runctx.bosses_from_map(map_text)
        except Exception as e:  # noqa: BLE001
            self._bookkeeping_error("context: map", e)
        act = self._cur_act(dict(bosses=bosses))
        try:
            for enc in runctx.encounters_met(os.path.join(self.log.dir, "events.jsonl"), act):
                seen.append(enc)
        except Exception as e:  # noqa: BLE001
            self._bookkeeping_error("context: run record", e)
        self._rc = runctx.RunContext(act, bosses, seen, map_text)
        return self._rc

    def _ctx(self):
        """What narrows the encounter pools (see `macro.narrow`): dict(seen=encounters met this act in order, bosses=the act's boss(es) in fight order)."""
        return self._context().ctx

    def _horizon(self):
        return self._context().horizon()

    def _future_encounters(self):
        return self._context().future()

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
            if not i and (before.startswith("ERR") or scr.busy(before)):
                before = self.state()  # mid-transition: wait for it to settle
            kind = scr.kind(before)
            if kind == "MENU" and not i and step.split()[0] == "0" and len(step.split()) >= 2:
                self._new_run()
            if i and self.gate:
                refusal = self._skill_refusal(before)
                if refusal:
                    return reply + f"[chain stopped before `{step}`: {refusal}]\n"
            if i and ((kind == "SELECT" and not step.startswith("~")) or kind == "MENU" or (kind == "COMBAT" and last_kind != "COMBAT")):
                return reply + f"[chain stopped before `{step}`: {kind}]\n"
            if kind == "MAP" and i < len(steps) - 1:
                return "REFUSED: a map choice must be the last step of a chain.\n" + before
            if i and scr.is_bare_number(step):
                return reply + f"REFUSED: `{step}` is an option number after an earlier step of the same chain: the list shifted when that step ran. Name the option (`~text`) or send it as its own call after reading the screen.\n"
            step = self._resolve(before, step)
            if step.startswith("ERR"):
                return step + "\n" + before
            guard = self._map_guard(before, step) or (self._decision_guard(before, step, why) if self.gate and os.environ.get("STS2_DECISION_GUARDS", "").lower() != "off" else None)
            if guard:
                return guard
            last_kind = kind
            reply = self._send("a " + step)
            self.log.event("macro", screen=before.split("\n")[0], state=before[:1500], choice=step, why=why, result=reply.split("\n")[0])
            self.last_state = reply
            if reply.startswith("ERR"):
                return reply
        if last_kind in ("COMBAT", "SELECT") and scr.kind(reply) not in ("COMBAT", "SELECT") and self.rp is not None:
            # a fight I finished by hand (manual drive, the killing blow, or my death): record its end NOW with the screen it left, not at the next sync (which read the HP
            # after a rest, and never came after a death: the game keeps exporting the finished fight on the GAME_OVER screen)
            try:
                self._fight_end(reply)
            except Exception as e:  # noqa: BLE001  never let bookkeeping break a command
                self._bookkeeping_error("fight end", e)
        if scr.kind(reply) == "COMBAT" and last_kind not in ("COMBAT", "SELECT"):  # a fight just started: decide auto or manual for it now
            try:
                self.sync()
                reply = reply.rstrip("\n") + "\n" + self._drive_line()
            except Exception as e:  # noqa: BLE001  never let bookkeeping break a command
                self._bookkeeping_error("fight start", e)
        if scr.kind(reply) == "COMBAT":
            reply = reply.rstrip("\n") + "\n" + self._combat_info()
        return self._public(reply, fight_start=scr.kind(reply) == "COMBAT" and last_kind not in ("COMBAT", "SELECT"))

    def _public(self, reply, fight_start=False):
        """Adds the public run counters (`agent/tracker.py`: potion drop chance, rare offset, unknown-room odds, removal price) to the screens where they
        bear on a choice: map, rewards, shop, event, treasure, rest, and the start of a fight (a potion spent now vs the chance of another after it)."""
        if scr.kind(reply) in ("MAP", "REWARDS", "CARD_REWARD", "SHOP", "EVENT", "TREASURE", "RESTSITE") or fight_start:
            try:
                return reply.rstrip("\n") + "\n" + tracker.from_record(os.path.join(self.log.dir, "events.jsonl")).line() + "\n"
            except Exception as e:  # noqa: BLE001  never let bookkeeping break a command
                self._bookkeeping_error("tracker", e)
        return reply

    _resolve = staticmethod(scr.resolve)

    def _map_guard(self, state, argline):
        return guards.map_guard(state, argline)

    def _decision_guard(self, state, step, why):
        return guards.decision_guard(state, step, why, self.priced, self.reward_screen)

    def _pick_guard(self, state, step, why):
        return guards.pick_guard(state, step, why, self.reward_screen)

    # ------------------------------------------------------------------ calculators (the decision guards ask which ran on this floor: PRICING)

    @_needs_run("no run in progress")
    def route(self, argline):
        """route <tokens> [--hp N] [--act Overgrowth] [--exclude ID,ID] [--attempts N]: HP budget along a planned route (see agent.macro.route_budget)."""
        a = Args(argline, valued=("--hp", "--act", "--exclude", "--attempts"))
        hp = a.get("--hp", None, int)
        excl = a.get("--exclude").split(",") if a.has("--exclude") else []
        deck = self._run()
        text = macro.route_budget(self.eng(), deck, a.pos, hp if hp is not None else deck["hp"], a.get("--act", "Overgrowth"), excl, a.get("--attempts", 48, int), ctx=self._ctx())
        self.log.event("route", nodes=a.pos, hp=hp, text=text)
        return text

    @_needs_run("no run in progress")
    def routes(self, argline):
        """routes [--attempts N] [--pf P] [--w E=4,M=1] [--hp N]: survival of every route on the act map and the price of each extra elite (agent.routes)."""
        from agent import routes
        a = Args(argline, valued=("--attempts", "--pf", "--w", "--hp"))
        deck = self._run()
        rc = self._context()
        state = call("peek")
        if a.has("--hp"):  # what-if start HP (rest vs smith: the HP after the rest vs now)
            state = re.sub(r"HP \d+/", f"HP {a.get('--hp', cast=int)}/", state, count=1)
        text = routes.analyse(self.eng(), deck, rc.map_text, state, rc.ctx, rc.names[0], a.get("--attempts", 24, int), a.get("--pf", 0.15, float), weights=a.weights())
        self.log.event("routes", text=text)
        return text

    @_needs_run("no run in progress\n")
    def rmcalc(self, argline):
        """rmcalc [--attempts N] [--hp full|current|N]: every removable card priced as a removal (boss smooth, elites, next act), ranked (macro.removal_report)."""
        a = Args(argline, valued=("--attempts", "--hp"))
        att, hp = a.get("--attempts", 64, int), a.hp()
        deck = self._run()  # before the engine: no run, no network loading
        text, res = macro.removal_report(self.eng(), deck, self._horizon(), att, hp, "all")
        self.log.event("rmcalc", text=text)
        return text + "\n"

    @_needs_run("no run in progress\n")
    def pickplan(self, argline):
        """pickplan [--screens K] [--elites E] [--shops S] [--slots N] [--rho R] [--attempts N]: how picky to be at a card reward given the offers still to come (agent.pickplan)."""
        from agent import pickplan
        a = Args(argline, valued=("--screens", "--elites", "--shops", "--slots", "--rho", "--attempts"))
        deck = self._run()
        hz = self._horizon()
        state = call("peek")
        offer = None
        if scr.kind(state) == "CARD_REWARD":
            opts, _skip = macro.parse_card_options(state)
            offer = {n: cid for _, n, cid, _ in opts if cid}
        text, res = pickplan.analyse(self.eng(), deck, hz, offer, a.get("--screens", 3, int), a.get("--elites", 0, int), a.get("--shops", 0, int), a.get("--slots", 6, int),
                                     a.get("--rho", 0.85, float), attempts=a.get("--attempts", 32, int), hold="all")
        self.log.event("pickplan", text=text)
        return text + "\n"

    @_needs_run("no run in progress\n")
    def reward(self, argline):
        """reward [--attempts N] [--hp full|current|N]: the card reward on screen, every option and skip priced in one call (boss smooth, elites, next act)."""
        state = call("peek")
        if scr.kind(state) != "CARD_REWARD":
            return f"reward: not a card reward screen ({scr.kind(state)}); use eval\n"
        a = Args(argline, valued=("--attempts", "--hp"))
        att, hp = a.get("--attempts", 96, int), a.hp()
        deck = self._run()
        opts, skip = macro.parse_card_options(state)
        hz = self._horizon()
        text, res = macro.reward_report(self.eng(), deck, opts, hz, att, hp, "all")
        self.reward_screen = guards.reward_key(state)
        self.log.event("reward_eval", options=[o[1] for o in opts], result={k: {str(i): v for i, v in macro.loggable(r).items()} for k, r in res.items()}, boss=hz["boss"])
        return text + f"\nskip is option {skip}; pick with `a <i> -- why`\n"

    @_needs_run("no run in progress")
    def price(self, argline):
        """`price [n]`: the options of this screen priced by paired run-model rollouts (`agent/price.py`)."""
        from agent import price as PR
        from predictor import Predictor
        from solver import PREDICTOR_CKPT
        if getattr(self, "_predictor", None) is None:
            self._predictor = Predictor(PREDICTOR_CKPT)
        state = call("peek")
        n = int(argline.split()[0]) if argline.split() and argline.split()[0].isdigit() else 128
        st = PR.run_state(self._run(), self._context(), os.path.join(self.log.dir, "events.jsonl"), state)
        opts = PR.options(st, state)
        if not opts:
            return f"price: nothing to price on a {scr.kind(state)} screen (events are not modelled yet: decide by judgment and note the gap)\n"
        res = PR.price(st, opts, self._predictor, n=n, seed=abs(hash(scr.floor_key(state) or "")) % 10_000)
        self.log.event("price", screen=scr.kind(state), options=[o[0] for o in opts],
                       result={k: {m: float(v.mean()) for m, v in r.items()} for k, r in res.items()})
        return PR.table(res) + f"\n({n} rollouts per option, paired; run model `docs/rebuild.md` S5)\n"

    def brief(self):
        state = call("peek")
        try:
            deck = self._run()
        except runctx.NoRun:
            return state
        return macro.brief_text(state, deck, self._horizon()) + "\n"

    EVAL_VALUED = ("--enc", "--pool", "--attempts", "--hp", "--v")
    EVAL_FLAGS = ("--boss", "--elites", "--next", "--all", "--future", "--smooth")

    @_needs_run("no run in progress")
    def evaluate(self, argline):
        argline = argline.strip()
        if argline.startswith("{"):
            spec = json.loads(argline)
        else:
            spec = dict(variants=[dict(name="baseline")], attempts=64)
            for t, val in Args(argline, self.EVAL_VALUED, self.EVAL_FLAGS).items:
                if t == "--enc":
                    spec["encounters"] = val.split(",")
                elif t == "--pool":
                    a, kind, *n = val.split(":")
                    spec["encounters"] = dict(act=a, kind=kind, n=int(n[0]) if n else 0)
                elif t in ("--boss", "--elites", "--next"):  # the horizon sets (see runctx.RunContext.horizon): the known boss, the elites that can still appear, the next act's elites + bosses
                    spec["encounters"] = self._horizon()[t[2:]]
                elif t == "--all":  # do not narrow the pool to the fights that can still appear
                    spec["all"] = True
                elif t == "--future":  # eval: the boss and elite pools of this act and every later act (horizon check)
                    spec["encounters"] = self._future_encounters()
                elif t == "--smooth":  # the deck-choice objective: win rate averaged over start HP x1 / 1.5 / 2 / 3 (macro.evaluate_smooth)
                    spec["smooth"] = True
                elif t == "--attempts":
                    spec["attempts"] = int(val)
                elif t == "--hp":
                    spec["hp"] = val if val in ("full", "current") else int(val)
                elif t == "--v":
                    parts = val.split("|")
                    v = dict(name=parts[0])
                    for p in parts[1:]:
                        k, x = p.split("=", 1)
                        v[k] = int(x) if k == "hp" else [y for y in x.split(",") if y]  # `potions=` (empty) = no potions
                    spec["variants"].append(v)
        if "encounters" not in spec:
            return "need --enc IDS or --pool Act:kind[:n]"
        spec.setdefault("hold", "all")  # non-boss fights are priced without potions (a lower bound: I spend one only when it is worth it); the boss with them
        if not spec.pop("all", False):
            spec["_ctx"] = self._ctx()
        deck = self._run()
        text, summary = macro.evaluate(self.eng(), deck, spec)
        self.log.event("eval", spec=spec, result=macro.loggable(summary))
        return text

    # ------------------------------------------------------------------ dispatch

    def status(self):
        st = dict(self.rp.stats) if self.rp else {}
        bad = {k: v for k, v in st.items() if k.startswith(REPLAY_BAD)}
        return (f"run {self.log.run_id}  engine {'loaded' if self.engine else 'not loaded'}  fight {self.fight_id}  actions {self.fight_actions}\n"
                f"replay: {st.get('end_turn_matched', 0)} enemy turns matched, {st.get('end_turn_unmatched', 0)} unmatched; divergences: {bad or 'none'}\n"
                f"potions held back from the solver: {sorted(self.hold) or 'none'}")

    def handle(self, line):
        self._rc = None  # every command reads the run afresh (the screen may have changed since the last one)
        out = self._handle(line)
        self._watch_run_end(out)
        return out

    def _watch_run_end(self, out):
        """Record how a run ended (once per run record): the screen after the last fight, the floor, the encounter that ended it. `improve review` reads it."""
        try:
            kind = scr.kind(out)
            first = out.splitlines()[0].lower() if out else ""
            if not (kind == "GAME_OVER" or "victory" in first):
                return
            if getattr(self, "_run_end_logged", None) == self.log.run_id:
                return
            self._run_end_logged = self.log.run_id
            self.log.event("run_end", screen=kind, header=scr.header_line(out, ""), last_encounter=getattr(self, "last_enc", None), text=out[:400])
        except Exception as e:  # noqa: BLE001  never let bookkeeping break a command
            self._bookkeeping_error("run end", e)

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
                return "REFUSED: " + why + "\n"
        try:
            if cmd in ("", "s"):
                return self._public(self.state())
            secs = float(rest) if cmd in ("adv", "turn", "combat", "budget") and rest.replace(".", "", 1).isdigit() else None
            if cmd == "hold":
                self.hold = potions.parse_hold(rest)
                self._save_hold()
                return f"held (out of the potion check, the search and every table): {sorted(self.hold) or 'nothing held'}\n"
            if cmd == "budget":
                if secs is not None:
                    self.budget = secs
                elif rest.strip() == "auto":
                    self.budget = None
                now = f"auto (this fight: {self.fight_budget}s)" if self.budget is None else f"{self.budget}s"
                return f"search budget {now} per combat decision\n"
            if cmd == "adv":
                adv = self.advice(secs)
                return self.state() + self._drive_line() + "advice: " + adv + "\n"
            words = rest.split()
            force = "!" in words
            ans = " ".join(w for w in words if w != "!")
            secs = float(ans) if cmd in ("turn", "combat") and ans.replace(".", "", 1).isdigit() else secs
            if cmd in ("turn", "combat"):
                if self.sync() is not None and getattr(self, "drive", ("auto",))[0] == "manual" and not force:
                    return "REFUSED: this fight is MANUAL.\n" + self._drive_line()
                return self.play(cmd == "combat", secs)
            if cmd == "potions":
                return self.potions_now()
            if cmd == "potion":
                return self.potion_cmd(rest)
            if cmd == "a":
                return self.act(rest)
            if cmd in PRICING:  # the decision guards ask which calculators ran on this floor
                here = scr.floor_key(call("peek"))  # the floor the calculator priced (read before it runs: the screen it saw)
                m = re.search(r"(?:^|\s)--seed\s+(\d+)", rest)  # `--seed N`: fresh draws on this screen (a re-run without it repeats the same ones)
                if m:
                    rest = rest[:m.start()] + rest[m.end():]
                self.table_seed = zlib.crc32((here or "").encode()) % 100_000 * 100 + (int(m.group(1)) if m else 0)
                out = getattr(self, PRICING[cmd])(rest)
                if here and not out.startswith(PRICING_FAILED):
                    self.priced[cmd] = here
                return out
            if cmd == "brief":
                return self.brief()
            if cmd == "price":
                return self.price(rest)
            if cmd == "plans":
                from agent import plans
                ch = scr.character(call("peek"))
                return "\n\n".join(plans.text(p) for p in plans.load() if not ch or p["character"] == ch) + "\n"
            if cmd == "relics":   # relic counters and saved state of the live fight (e.g. Pen Nib: attacks played so far, Book of Five Rings ...)
                raw = call("snap").strip()
                if raw == "null":
                    return "not in combat\n"
                st = json.loads(raw)
                return "\n".join(f"{r['id']}" + (f" counter {r['counter']}" if "counter" in r else "") + (f" {r['props']}" if "props" in r else "") for r in st["relics"]) + "\n"
            if cmd == "note":
                self.log.event("note", text=rest)
                return "noted\n"
            if cmd == "status":
                return self.status() + "\n"
            if cmd == "newrun":
                self._new_run()
                return f"run {self.log.run_id}\n"
            if cmd == "do" and self.gate:  # a raw bridge action skips every guard (held potions, map, decision record): play through `a` / `turn` / `combat`
                return "REFUSED: `do` sends a raw action past the harness's guards; use `a <i>`, `turn` or `combat`.\n"
            return call(line)
        except Exception:  # noqa: BLE001
            return "ERR harness: " + traceback.format_exc()
