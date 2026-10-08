import functools
import json
import os
import re
import threading
import traceback
import zlib

from agent import guards, macro, runctx, skillgate, tracker
from agent import screen as scr
from agent.args import Args
from agent.bridge import call
from agent.live import REPLAY_BAD, Live
from agent.runlog import RunLog

PICK_RECORD = guards.PICK_RECORD

PRICING_FAILED = ("ERR", "REFUSED", "reward: not", "no run", "need --enc", "routes: no act map", "routes: boss unknown", "eval: ")
PRICING = {"eval": "evaluate", "reward": "reward", "route": "route", "routes": "routes", "rmcalc": "rmcalc", "pickplan": "pickplan"}

_floor, _kind, _act_index, _hp = scr.floor_key, scr.kind, scr.act_index, scr.hp


def _needs_run(reply):
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
        self.priced = {}
        self.table_seed = 0
        self.reward_screen = None
        self.gate = False
        self.fight_hp0 = None
        self.fight_actions = 0
        self._ended = set()
        self.budget = None
        self.fight_budget = 1.0
        self._checked_turn = None
        self.potions_used = 0
        self._pred_q = None
        self.aside = self._load_aside()
        self.fight_tol = 1.0
        self._rc = None


    def _aside_path(self):
        return os.path.join(self.log.dir, "potion_aside.json")

    def _load_aside(self):
        try:
            with open(self._aside_path(), encoding="utf-8") as f:
                return set(json.load(f))
        except (OSError, ValueError):
            return set()

    def _save_aside(self):
        try:
            os.makedirs(self.log.dir, exist_ok=True)
            with open(self._aside_path(), "w", encoding="utf-8") as f:
                json.dump(sorted(self.aside), f)
        except OSError as e:
            self._bookkeeping_error("save potion aside", e)

    def _bookkeeping_error(self, where, e):
        try:
            self.log.event("harness_error", where=where, error=f"{type(e).__name__}: {e}"[:300])
        except Exception:  # noqa: BLE001
            pass

    def eng(self):
        with self._eng_lock:
            if self.engine is None:
                from agent.engine import Engine
                self.engine = Engine()
            self.engine.table_seed = self.table_seed
            return self.engine

    def _send(self, line):
        self._rc = None
        return call(line)

    def _new_run(self):
        self.log.new_run()
        self.aside = set()
        self.priced = {}
        self.reward_screen = None


    def _deck_raw(self):
        return call("deck.json").strip()

    def _run(self):
        raw = self._deck_raw()
        if raw == "null":
            raise runctx.NoRun()
        if raw.startswith("ERR"):
            raise RuntimeError(raw.split("\n")[0])
        return json.loads(raw)

    def _cur_act(self, ctx):
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
        if errs:
            self._bookkeeping_error("context: act", RuntimeError(errs[-1]))
        return 0

    def _context(self):
        if self._rc is not None:
            return self._rc
        bosses, map_text, seen = [], "", []
        try:
            map_text = call("m")
            if map_text.startswith("ERR"):
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
        return self._context().ctx

    def _horizon(self):
        return self._context().horizon()

    def _future_encounters(self):
        return self._context().future()


    def act(self, argline):
        why = None
        if " -- " in argline:
            argline, why = argline.split(" -- ", 1)
        steps = [t.strip() for t in argline.split(";") if t.strip()]
        reply, last_kind = "", None
        for i, step in enumerate(steps):
            before = self.last_state if i else call("peek")
            if not i and (before.startswith("ERR") or scr.busy(before)):
                before = self.state()
            kind = scr.kind(before)
            if kind == "MENU" and not i and step.split()[0] == "0" and len(step.split()) >= 2 and scr.option_line(before, "0").startswith("0 new run"):
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
            try:
                self._fight_end(reply)
            except Exception as e:  # noqa: BLE001
                self._bookkeeping_error("fight end", e)
        if scr.kind(reply) == "COMBAT" and last_kind not in ("COMBAT", "SELECT"):
            try:
                self.sync()
                reply = reply.rstrip("\n") + "\n" + self._drive_line()
            except Exception as e:  # noqa: BLE001
                self._bookkeeping_error("fight start", e)
        if scr.kind(reply) == "COMBAT":
            reply = reply.rstrip("\n") + "\n" + self._combat_info()
        return self._public(reply, fight_start=scr.kind(reply) == "COMBAT" and last_kind not in ("COMBAT", "SELECT"))

    def _public(self, reply, fight_start=False):
        if scr.kind(reply) in ("MAP", "REWARDS", "CARD_REWARD", "SHOP", "EVENT", "TREASURE", "RESTSITE") or fight_start:
            try:
                return reply.rstrip("\n") + "\n" + tracker.from_record(os.path.join(self.log.dir, "events.jsonl")).line() + "\n"
            except Exception as e:  # noqa: BLE001
                self._bookkeeping_error("tracker", e)
        return reply

    _resolve = staticmethod(scr.resolve)

    def _map_guard(self, state, argline):
        return guards.map_guard(state, argline)

    def _decision_guard(self, state, step, why):
        return guards.decision_guard(state, step, why, self.priced, self.reward_screen)

    def _pick_guard(self, state, step, why):
        return guards.pick_guard(state, step, why, self.reward_screen)


    @_needs_run("no run in progress")
    def route(self, argline):
        a = Args(argline, valued=("--hp", "--act", "--exclude", "--attempts"))
        hp = a.get("--hp", None, int)
        excl = a.get("--exclude").split(",") if a.has("--exclude") else []
        deck = self._run()
        text = macro.route_budget(self.eng(), deck, a.pos, hp if hp is not None else deck["hp"], a.get("--act", "Overgrowth"), excl, a.get("--attempts", 48, int), ctx=self._ctx())
        self.log.event("route", nodes=a.pos, hp=hp, text=text)
        return text

    @_needs_run("no run in progress")
    def routes(self, argline):
        from agent import routes
        a = Args(argline, valued=("--attempts", "--pf", "--w", "--hp"))
        deck = self._run()
        rc = self._context()
        state = call("peek")
        if a.has("--hp"):
            state = re.sub(r"HP \d+/", f"HP {a.get('--hp', cast=int)}/", state, count=1)
        text = routes.analyse(self.eng(), deck, rc.map_text, state, rc.ctx, rc.names[0], a.get("--attempts", 24, int), a.get("--pf", 0.15, float), weights=a.weights())
        self.log.event("routes", text=text)
        return text

    @_needs_run("no run in progress\n")
    def rmcalc(self, argline):
        a = Args(argline, valued=("--attempts", "--hp"))
        att, hp = a.get("--attempts", 64, int), a.hp()
        deck = self._run()
        text, res = macro.removal_report(self.eng(), deck, self._horizon(), att, hp, "all")
        self.log.event("rmcalc", text=text)
        return text + "\n"

    @_needs_run("no run in progress\n")
    def pickplan(self, argline):
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
        from agent import price as PR
        state = call("peek")
        args = argline.split()
        n = int(args[0]) if args and args[0].isdigit() else 128
        sat = float(args[args.index("--sat") + 1]) if "--sat" in args[:-1] else PR.ACT_SATURATED
        st = PR.run_state(self._run(), self._context(), os.path.join(self.log.dir, "events.jsonl"), state)
        opts = PR.options(st, state)
        if not opts:
            why = " (this event is not catalogued: decide by judgment and note the gap)" if scr.kind(state) == "EVENT" else ""
            return f"price: nothing to price on a {scr.kind(state)} screen{why}\n"
        if len(opts) == 1:
            self.log.event("price", screen=scr.kind(state), options=[opts[0][0]], result=None)
            return f"price: one option on this screen ({opts[0][0]}): nothing to compare\n"
        if getattr(self, "_predictor", None) is None:
            from predictor import Predictor
            from solver import PREDICTOR_CKPT
            self._predictor = Predictor(PREDICTOR_CKPT)
        note = ""
        if scr.kind(state) == "SHOP":
            unpriced = []
            opts, note = PR.bundles(st, PR.shop_items(st, state, unpriced), self._predictor)
            note = f"; {note}" + (f"; not priced (no simulator id): {', '.join(unpriced)}" if unpriced else "")
        res = PR.price(st, opts, self._predictor, n=n, seed=abs(hash(scr.floor_key(state) or "")) % 10_000)
        ranked_by, _why = PR.ladder(res, sat)
        self.log.event("price", screen=scr.kind(state), options=[o[0] for o in opts], ranked_by=ranked_by,
                       result={k: {m: float(v.mean()) for m, v in r.items()} for k, r in res.items()})
        ready = ("; next act ready: P(win) after the ancient's heal vs the next act's bosses x0.5 and elites x0.5, 0 on a death in this act"
                 if "ready" in next(iter(res.values())) else "")
        return PR.table(res, sat) + f"\n({n} rollouts per option, paired; run model `docs/rebuild.md` S5{ready}{note})\n"

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
                elif t in ("--boss", "--elites", "--next"):
                    spec["encounters"] = self._horizon()[t[2:]]
                elif t == "--all":
                    spec["all"] = True
                elif t == "--future":
                    spec["encounters"] = self._future_encounters()
                elif t == "--smooth":
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
                        v[k] = int(x) if k == "hp" else [y for y in x.split(",") if y]
                    spec["variants"].append(v)
        if "encounters" not in spec:
            return "need --enc IDS or --pool Act:kind[:n]"
        spec.setdefault("hold", "all")
        if not spec.pop("all", False):
            spec["_ctx"] = self._ctx()
        deck = self._run()
        text, summary = macro.evaluate(self.eng(), deck, spec)
        self.log.event("eval", spec=spec, result=macro.loggable(summary))
        return text


    def status(self):
        st = dict(self.rp.stats) if self.rp else {}
        bad = {k: v for k, v in st.items() if k.startswith(REPLAY_BAD)}
        return (f"run {self.log.run_id}  engine {'loaded' if self.engine else 'not loaded'}  fight {self.fight_id}  actions {self.fight_actions}\n"
                f"replay: {st.get('end_turn_matched', 0)} enemy turns matched, {st.get('end_turn_unmatched', 0)} unmatched; divergences: {bad or 'none'}\n"
                f"potions set aside for the boss: {sorted(self.aside) or 'none'}")

    def handle(self, line):
        self._rc = None
        out = self._handle(line)
        self._watch_run_end(out)
        self.log.live(cmd=line, reply=out, screen=self.last_state, fight=getattr(self, "_last_f", None) if self.rp is not None else None)
        return out

    def _watch_run_end(self, out):
        try:
            kind = scr.kind(out)
            first = out.splitlines()[0].lower() if out else ""
            if not (kind == "GAME_OVER" or "victory" in first):
                return
            if getattr(self, "_run_end_logged", None) == self.log.run_id:
                return
            self._run_end_logged = self.log.run_id
            self.log.event("run_end", screen=kind, header=scr.header_line(out, ""), last_encounter=getattr(self, "last_enc", None), text=out[:400])
        except Exception as e:  # noqa: BLE001
            self._bookkeeping_error("run end", e)

    def _skill_refusal(self, state_text):
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
            if cmd == "hold":
                return "REFUSED: `hold` is retired: potions are proposed every turn and committed one at a time; `potion aside <name>` keeps one for the boss.\n"
            if cmd == "a":
                return self.act(rest)
            if cmd in PRICING:
                here = scr.floor_key(call("peek"))
                m = re.search(r"(?:^|\s)--seed\s+(\d+)", rest)
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
            if cmd == "relics":
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
            if cmd == "do" and self.gate:
                return "REFUSED: `do` sends a raw action past the harness's guards; use `a <i>`, `turn` or `combat`.\n"
            if cmd == "draw":
                out = call(line)
                try:
                    self.log.event("route_plan", nodes=re.findall(r"r\d+c\d+", rest))
                except Exception as e:  # noqa: BLE001
                    self._bookkeeping_error("route plan", e)
                return out
            return call(line)
        except Exception:  # noqa: BLE001
            return "ERR harness: " + traceback.format_exc()
