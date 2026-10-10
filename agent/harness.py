import functools
import json
import os
import re
import threading
import traceback
import zlib

from agent import bottleneck, guards, macro, runctx, skillgate, tracker
from agent import screen as scr
from agent.args import Args
from agent.bridge import call
from agent.live import REPLAY_BAD, Live
from agent.runlog import RunLog

PICK_RECORD = guards.PICK_RECORD

PRICING_FAILED = ("ERR", "REFUSED", "reward: not", "no run", "need --enc", "routes: no act map", "routes: boss unknown", "eval: ")
PRICING = {"eval": "evaluate", "reward": "reward", "route": "route", "routes": "routes", "rmcalc": "rmcalc", "pickplan": "pickplan", "restcalc": "restcalc"}

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
    def __init__(self, log=None):
        self.engine = None
        self._eng_lock = threading.Lock()
        self.rp = None
        self.fight_id = None
        self.log = log or RunLog()
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

    def _detect_new_run(self, state):
        if scr.kind(state) == "EVENT" and " A1 F1 " in f" {scr.header_line(state, '')} " and "\nNeow:" in state and self._log_has_fights():
            self._new_run()

    def _log_has_fights(self):
        """a headless game starts at Neow without the main menu: a Neow screen after logged fights is a new run"""
        p = os.path.join(self.log.dir, "events.jsonl")
        try:
            with open(p, encoding="utf-8") as f:
                return any('"kind": "fight_start"' in l for l in f)
        except OSError:
            return False

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
            elif not i:
                self._detect_new_run(before)
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
        if scr.kind(reply) in ("MAP", "SHOP"):
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
        return text + "\n" + self._synergy(state, deck) + f"skip is option {skip}; pick with `a <i> -- why`\n"

    @_needs_run("no run in progress")
    def price(self, argline):
        from agent import price as PR
        state = call("peek")
        args = argline.split()
        n = int(args[0]) if args and args[0].isdigit() else 128
        sat = float(args[args.index("--sat") + 1]) if "--sat" in args[:-1] else PR.ACT_SATURATED
        cont = args[args.index("--cont") + 1] if "--cont" in args[:-1] else os.environ.get("STS2_PRICE_CONT") or None
        if cont is not None and cont not in PR.R.RULES:
            return f"price: --cont takes one of {', '.join(PR.R.RULES)}\n"
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
        res = PR.price(st, opts, self._predictor, n=n, seed=abs(hash(scr.floor_key(state) or "")) % 10_000, cont=cont)
        gates = "" if cont is None else PR.gates_text(opts, PR.closed_gates(st, opts, self._predictor), cont) + "\n"
        ranked_by, _why = PR.ladder(res, sat)
        self.log.event("price", screen=scr.kind(state), options=[o[0] for o in opts], ranked_by=ranked_by,
                       result={k: {m: float(v.mean()) for m, v in r.items()} for k, r in res.items()})
        ready = ("; next act ready: P(win) after the ancient's heal vs the next act's bosses x0.5 and elites x0.5, 0 on a death in this act"
                 if "ready" in next(iter(res.values())) else "")
        check = ""
        adj = PR.event_adjust(st, state)
        if adj:
            b, _m = PR.best(res, sat, adj)
            check += ("macro terms (data/macro_rules.json, P units on " + ranked_by + "): " + ", ".join(f"{k} {v:+.3f}" for k, v in adj.items())
                      + f" -> best with terms: {b}\n")
        if scr.kind(state) in ("CARD_REWARD", "SHOP", "EVENT") and "--no-search" not in args:
            got = PR.search_check(st, opts, res, self.eng())
            if got:
                pick, _ranked, override = PR.hybrid_best(res, got, adj)
                rows = [f"  {lb[:44]:44s} {p:.3f} +- {se:.3f}" for lb, (p, se) in sorted(got.items(), key=lambda x: -x[1][0])]
                check += ("search check (the predictor is near-blind to small deck edits): deck vs " + " + ".join(st.bosses) + " at full HP, no potions\n"
                         + "\n".join(rows) + (f"\n  -> search overrides the run model: {pick} (+{override[0]} +- {override[1]})" if override else "") + "\n")
        return PR.table(res, sat) + f"\n({n} rollouts per option, paired; run model{ready}{note})\n" + gates + check + self._synergy(state)

    def _synergy(self, state, deck=None):
        """plan overlap of the items offered on this screen (counts, for the operator to judge; data/synergy_candidates.json)"""
        from agent import price as PR, synergy
        try:
            deck = deck or self._run()
        except runctx.NoRun:
            return ""
        ids = [c["id"] for c in deck["deck"]] + [r["id"] for r in deck["relics"]]
        items = []
        for _, label in scr.options(state):
            m = re.match(r"^(?:\d+g )?(card|relic) (.+?)(?:\(|:|$)", label) or re.match(r"^()(.+?)\(", label)
            if m:
                name = m.group(2).strip()
                cid = PR._card_id(name)[0] if m.group(1) != "relic" else None
                items.append(cid or PR._ident(name))
        rows = synergy.lines(dict.fromkeys(i for i in items if i), ids)
        return ("synergy (bundle: role, pieces you own; candidates, not measurements):\n" + "\n".join(rows) + "\n") if rows else ""

    def brief(self):
        state = call("peek")
        try:
            deck = self._run()
        except runctx.NoRun:
            return state
        ev = os.path.join(self.log.dir, "events.jsonl")
        bn = bottleneck.line(ev) if os.path.exists(ev) else ""
        return macro.brief_text(state, deck, self._horizon()) + "\n" + (bn + "\n" if bn else "")

    EVAL_VALUED = ("--enc", "--pool", "--attempts", "--hp", "--v")
    EVAL_FLAGS = ("--boss", "--elites", "--next", "--all", "--future", "--smooth")

    @_needs_run("no run in progress")
    def loot(self):
        """the rewards that need no judgement (gold, potions while a slot is free), after the operator has decided the card reward (War Paint-like
        pickups act on the new card), relics (a free relic can be anti-synergy, e.g. Tungsten Rod vs Rupture/Inferno; relics before gold for
        Bowler Hat-like ones) and a stolen card (maybe a card to drop anyway, or the card reward offers it upgraded)"""
        out = ""
        order = (r"(?i)^\d+ gold", r"(?i)^potion ")
        for _ in range(10):
            state = call("peek")
            if scr.kind(state) != "REWARDS":
                break
            opts = scr.options(state)
            card_done = self.priced.get("reward") == scr.floor_key(state)
            pending = [label.split(":")[0] for _, label in opts if label.lower().startswith(("relic ", "take your stolen")) or (label.lower().startswith("card") and not card_done)]
            if pending:
                return ((out.rstrip("\n") + "\n" if out else "") + self._synergy(state) + f"loot: decide {', '.join(pending)} first (the card reward before relics; a relic can be "
                        "anti-synergy; relics before gold), then `loot`\n" + state)
            free = "-" in scr.belt(state)
            pick = next((i for pat in order for i, label in opts if re.match(pat, label) and (free or "potion" not in pat)), None)
            if pick is None:
                break
            out = self.act(f"{pick} -- loot: free reward")
            if out.startswith(("ERR", "REFUSED")):
                break
        return out or call("peek")

    def restcalc(self, argline):
        """the rest decision's deciding number: the act boss(es) at the HP now vs after resting (Regal Pillow included)"""
        state = call("peek")
        hp = scr.hp(state)
        m = re.search(r"Rest: Heal for \d+% of your Max HP \((\d+)\)", state)
        if not hp or not m:
            return "restcalc: not at a rest site with a Rest option\n"
        heal = int(m.group(1)) + (15 if "Regal Pillow:" in self.handle("d") else 0)
        after = min(hp[1], hp[0] + heal)
        out = [f"rest heals {after - hp[0]} ({hp[0]} -> {after}/{hp[1]})"]
        for h in (hp[0], after):
            rows = [l for l in self.evaluate(f"--boss --hp {h} --attempts 128 {argline}").splitlines() if l.startswith("baseline")]
            out.append(f"  at {h} HP: " + (rows[0].split(None, 1)[1] if rows else "?"))
        return "\n".join(out) + "\n"

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


    def reenact(self, cmd, rest):
        from agent.reenact import Reenactor
        a = Args(rest, valued=("--steps", "--until"), flags=("--custom",))
        if not a.pos:
            return f"usage: {cmd} <record.compact.jsonl> [--steps N] [--until FLOOR] [--custom]\n"
        return Reenactor(self, os.path.abspath(a.pos[0]), custom=a.has("--custom")).run(a.get("--steps", None, int), a.get("--until", None, int), seedcheck=cmd == "seedcheck")

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
            if cmd == "a":
                return self.act(rest)
            if cmd == "loot":
                return self.loot()
            if cmd in ("replay", "seedcheck"):
                return self.reenact(cmd, rest)
            if cmd in PRICING:
                now = call("peek")
                self._detect_new_run(now)
                here = scr.floor_key(now)
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
                now = call("peek")
                self._detect_new_run(now)
                here = scr.floor_key(now)
                out = self.price(rest)
                if here and not out.startswith(("ERR", "price: nothing to price")):
                    self.priced["price"] = here
                return out
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
