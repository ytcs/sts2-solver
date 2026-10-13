import functools
import json
import os
import re
import threading
import time
import traceback
import zlib

from agent import bottleneck, guards, macro, pools, runctx, runlog, skillgate, tracker
from agent import screen as scr
from agent.args import Args
from agent.bridge import call
from agent.live import REPLAY_BAD, Live
from agent.runlog import RunLog

PRICING_FAILED = ("ERR", "REFUSED", "no run", "need --enc", "routes: no act map", "routes: boss unknown", "restcalc: not")
def _code_stamp():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return max(os.path.getmtime(os.path.join(root, d, f)) for d in ("agent", "rl") for f in os.listdir(os.path.join(root, d)) if f.endswith(".py"))


PRICING = {"eval": "evaluate", "routes": "routes", "restcalc": "restcalc"}
VALIDATION = ("validation (operator play: components, no verdict): the search check = near binding fights at current HP, the validated instrument "
              "(deck strength vs live boss wins, Spearman 0.73, E70); run-model rungs (P(win run), act clear, readiness) are weak: act-2 clear 0.36 vs 0.71 "
              "realised (E93, E94), far factors on act-1 decks have the wrong sign (E86); deck strength vs the final pair: `eval --pool Glory:boss --hp full --target`\n")
ENGINES = ("AGGRESSION", "BARRICADE", "CORRUPTION", "CRIMSON_MANTLE", "DARK_EMBRACE", "DEMON_FORM", "FEEL_NO_PAIN", "HELLRAISER", "INFERNO", "JUGGERNAUT", "RUPTURE")


def _engine_line(ids):
    hit = sorted(set(ids) & set(ENGINES))
    return (f"engine caveat ({', '.join(hit)}): the search's piloting of this engine is unvalidated (E96 validated search vs live only on the bot's "
            "own decks); the number is a lower bound (`prior` for the community split)\n") if hit else ""


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


def _check_lines(got, radius):
    def cell(a, m, w, mult):
        hp = f" at {mult[a]:g}x full HP" if mult.get(a, 1) != 1 else ""
        return f"{('A' + str(a + 1)) if isinstance(a, int) else a} win {w:.2f}{hp}" + ("" if abs(m - w) < 1e-9 else f" (scored on progress {m:.2f}: win floored)")
    rows = [f"  {lb[:40]:40s} {p:+.3f} +- {se:.3f}   " + "  ".join(cell(a, m, pw[a], mu) for a, m in sorted(pa.items(), key=lambda x: str(x[0])))
            for lb, (p, se, pa, _t, pw, _c, mu) in sorted(got.items(), key=lambda x: -x[1][0])]
    # both callers (`price`) run search_check with live=True
    return (f"search check ({macro.player_tag([{'player': 'live'}])}): log ratio vs skip summed over the factors of P(win run) (near binding fights at current HP, binding radius {radius}; future boss rungs at full HP), "
            "the belt each option leaves; per factor its win rate, and where skip's win rate is floored the progress score that ranked it\n" + "\n".join(rows) + "\n")


def _search_log(got):
    """price event `search`: {label: [score, se, {factor: scored}, {factor: win}, {factor: HP multiple}]}"""
    return {k: [round(v[0], 3), round(v[1], 3)] + [{str(a): round(x, 3) for a, x in d.items()} for d in (v[2], v[4], v[6])] for k, v in got.items()}


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
        self.code_stamp = _code_stamp()
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

    def _starts_new_run(self, state):
        return scr.kind(state) == "EVENT" and " A1 F1 " in f" {scr.header_line(state, '')} " and "\nNeow:" in state and self._log_has_fights()

    def _detect_new_run(self, state):
        if self._starts_new_run(state):
            self._new_run()

    def _unreviewed(self):
        """the self-improvement loop: a new run starts only after the previous one is reviewed (sts2-improve)"""
        if not self.gate or not self._log_has_fights():
            return None
        from agent import improve
        rid = self.log.run_id
        try:
            with open(improve.LEDGER, encoding="utf-8") as f:
                in_ledger = any(f'"run": "{rid}"' in l for l in f)
        except OSError:
            in_ledger = False
        if os.path.exists(os.path.join(self.log.dir, "review.md")) and in_ledger:
            return None
        return (f"REFUSED: run {rid} is not reviewed. Load `sts2-improve` and follow it: `python -m agent.improve review {rid}` "
                f"(writes runs/{rid}/review.md and its ledger rows), then the rule changes.\n")

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
        self.relics_seen = None

    def _relic_watch(self, state):
        """log the relic bar when it changes (pickups at Neow, events, rewards, chests, shops), not only at the next fight start"""
        try:
            now = [r["id"] for r in json.loads(self._deck_raw())["relics"]]
        except Exception:  # noqa: BLE001
            return
        before = getattr(self, "relics_seen", None)
        if before != now:
            self.log.event("relics", header=scr.header_line(state, ""), gained=[r for r in now if r not in (before or [])],
                           lost=[r for r in (before or []) if r not in now], relics=now)
            self.relics_seen = now


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
            if not s.startswith("ERR"):
                break
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


    def act(self, argline, first=None):
        why = None
        if " -- " in argline:
            argline, why = argline.split(" -- ", 1)
        steps = [t.strip() for t in argline.split(";") if t.strip()]
        reply, last_kind, warn = "", None, ""
        for i, step in enumerate(steps):
            before = self.last_state if i else first or call("peek")
            if not i and (before.startswith("ERR") or scr.busy(before)):
                before = self.state()
            kind = scr.kind(before)
            if not i and ((kind == "MENU" and step.split()[0] == "0" and scr.option_line(before, "0").startswith("0 new run")) or self._starts_new_run(before)):
                refusal = self._unreviewed()
                if refusal:
                    return refusal
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
            guard = guards.map_guard(before, step) or (guards.decision_guard(before, step, why, self.priced, self.reward_screen) if self.gate and os.environ.get("STS2_DECISION_GUARDS", "").lower() != "off" else None)
            if guard:
                return guard
            if self.gate and kind in ("CARD_REWARD", "SHOP") and runlog.target(self.log.dir):
                left = self._target_left(self._run())
                warn += guards.target_warning(before, step, [macro._card(c)["id"] for c in (left or {}).get("add", [])]) or ""
            last_kind = kind
            reply = self._send("a " + step)
            self.log.event("macro", screen=before.split("\n")[0], state=before[:1500], choice=step, why=why, result=reply.split("\n")[0])
            if kind != "COMBAT":
                self._relic_watch(reply)
            if reply.startswith("ERR"):
                return reply
            self.last_state = reply
            self.log.live(screen=reply)
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
        out = self._public(reply, fight_start=scr.kind(reply) == "COMBAT" and last_kind not in ("COMBAT", "SELECT"))
        # the warning trails the reply: live.json and the run-end watch read the screen from its first lines
        return out.rstrip("\n") + "\n" + warn if warn else out

    def _public(self, reply, fight_start=False):
        if scr.kind(reply) in ("MAP", "SHOP"):
            try:
                return reply.rstrip("\n") + "\n" + tracker.from_record(os.path.join(self.log.dir, "events.jsonl")).line() + "\n"
            except Exception as e:  # noqa: BLE001
                self._bookkeeping_error("tracker", e)
        return reply

    _resolve = staticmethod(scr.resolve)


    @_needs_run("no run in progress")
    def routes(self, argline, state):
        from agent import routes
        a = Args(argline, valued=("--attempts", "--pf", "--w", "--hp"))
        deck = self._run()
        rc = self._context()
        if a.has("--hp"):
            state = re.sub(r"HP \d+/", f"HP {a.get('--hp', cast=int)}/", state, count=1)
        text = routes.analyse(self.eng(), deck, rc.map_text, state, rc.ctx, rc.names[0], a.get("--attempts", 24, int), a.get("--pf", 0.15, float), weights=a.weights(), live=True)
        self.log.event("routes", text=text)
        return text

    def _price_check(self, PR, st, opts, state):
        """operator play, card reward / event: the validated search check only (`price --run` adds the run-model rollouts)"""
        import numpy as np
        stub = {o[0]: {"win": np.zeros(2), "act": np.zeros(2)} for o in opts}
        got, radius = PR.search_check(st, opts, stub, self.eng(), PR.screen_seed("live", state), top=len(opts), live=True)
        best = max(got, key=lambda lb: got[lb][0]) if got else None
        self.log.event("price", screen=scr.kind(state), options=[o[0] for o in opts], ranked_by="check", best=best,
                       result={k: {"check": round(v[0], 4)} for k, v in got.items()},
                       search=_search_log(got), radius=radius, boss_player="live")
        notes = PR.unmodelled_lines(state, got)
        return _check_lines(got, radius) + ("\n".join(notes) + "\n" if notes else "") + VALIDATION + self._plan_note(got, state) + self._synergy(state, self._run())

    def _plan_note(self, got, state):
        """the far rungs' deck (the target's when set) and the engine caveat for the offered cards"""
        far = ("far rungs `on target`: priced on the target deck (the pieces each option's deck lacks added; a target card's own far factor is then 0, "
               "its value assumed by the plan)\n") if any(k.endswith(" on target") for v in got.values() for k in v[2]) else ""
        return far + _engine_line(self._offered(state))

    @_needs_run("no run in progress")
    def price(self, argline, state):
        from agent import price as PR
        args = argline.split()
        hp_i = args.index("--hp") + 1 if "--hp" in args else None
        n = next((int(x) for i, x in enumerate(args) if x.isdigit() and i != hp_i), 128)
        deck = self._run()
        st = PR.run_state(deck, self._context(), os.path.join(self.log.dir, "events.jsonl"), state)
        if hp_i is not None and hp_i < len(args) and args[hp_i].isdigit():
            st.hp = min(st.max_hp, int(args[hp_i]))
        opts = PR.options(st, state)
        if not opts:
            why = " (this event is not catalogued: decide by judgment and note the gap)" if scr.kind(state) == "EVENT" else ""
            return f"price: nothing to price on a {scr.kind(state)} screen{why}\n"
        if scr.kind(state) == "CARD_REWARD":
            self.reward_screen = guards.reward_key(state)
        if len(opts) == 1:
            self.log.event("price", screen=scr.kind(state), options=[opts[0][0]], result=None)
            return f"price: one option on this screen ({opts[0][0]}): nothing to compare\n"
        if self.gate and scr.kind(state) in ("CARD_REWARD", "EVENT") and "--run" not in args:
            return self._price_check(PR, st, opts, state)
        if getattr(self, "_predictor", None) is None:
            from predictor import Predictor
            from solver import PREDICTOR_CKPT
            self._predictor = Predictor(PREDICTOR_CKPT)
        note = ""
        if scr.kind(state) == "SHOP":
            unpriced = []
            opts, note = PR.bundles(st, PR.shop_items(st, state, unpriced), self._predictor)
            note = f"; {note}" + (f"; not priced (no simulator id): {', '.join(unpriced)}" if unpriced else "")
        r = PR.decide(st, state, self._predictor, None if "--no-search" in args else self.eng(), opts, n=n, seed=PR.screen_seed("live", state), live=True)
        res, adj, got, override = r["res"], r["adj"], r["check"], r["override"]
        ranked_by, _why = PR.ladder(res)
        self.log.event("price", screen=scr.kind(state), options=[o[0] for o in opts], ranked_by=ranked_by, best=r["best"],
                       result={k: {m: float(v.mean()) for m, v in x.items()} for k, x in res.items()},
                       search=_search_log(got), override=override, terms=adj, radius=r["radius"], boss_player="live")
        ready = ("; next act ready: P(win) after the ancient's heal vs the next act's bosses x0.5 and elites x0.5, 0 on a death in this act"
                 if "ready" in next(iter(res.values())) else "")
        check = ""
        if adj:
            check += "macro terms (data/macro_rules.json, P units on " + ranked_by + "): " + ", ".join(f"{k} {v:+.3f}" for k, v in adj.items()) + "\n"
        if got:
            check += _check_lines(got, r["radius"]) + (f"  -> search overrides the run model: {r['best']} (+{override[0]} +- {override[1]})\n" if override else "")
        if self.gate:
            check += VALIDATION
        else:
            check += f"decision (shared with the scripted baseline): {r['best']}" + (" (card-reward deadline rule -> skip)" if r["terms_skip"] else "") + "\n"
        return PR.table(res) + f"\n({n} rollouts per option, paired; run model{ready}{note})\n" + check + self._plan_note(got, state) + self._synergy(state, deck)

    def _synergy(self, state, deck=None):
        """plan overlap of the items offered on this screen (counts, for the operator to judge; data/synergy_candidates_<char>.json)"""
        from agent import price as PR, synergy
        try:
            deck = deck or self._run()
        except runctx.NoRun:
            return ""
        ids = [c["id"] for c in deck["deck"]] + [r["id"] for r in deck["relics"]]
        rows = synergy.lines(dict.fromkeys(self._offered(state)), ids)
        return ("synergy (bundle: role, pieces you own; completes / needs: A + B pay off only with C; candidates, not measurements):\n" + "\n".join(rows) + "\n") if rows else ""

    @staticmethod
    def _offered(state):
        """ids of the cards (relic idents) offered on a card reward / shop / rewards screen"""
        from agent import price as PR
        items = []
        for _, label in scr.options(state):
            m = re.match(r"^(?:\d+g )?(card|relic) (.+?)(?:\(|:|$)", label) or re.match(r"^()(.+?)\(", label)
            if m:
                name = m.group(2).strip()
                cid = PR._card_id(name)[0] if m.group(1) != "relic" else None
                items.append(cid or PR._ident(name))
        return [i for i in items if i]

    def _target_left(self, deck):
        ev = runlog.target(self.log.dir)
        return macro.target_left(ev["variant"], deck, ev.get("base")) if ev else None

    def _plan_lines(self, deck):
        """plan nudges, never refusals: the target's missing pieces; from act 2 on, no target for the act ahead (set at the previous boss)"""
        ev, act = runlog.target(self.log.dir), self._context().horizon()["act"]
        out = [f"target missing: {macro.pieces(self._target_left(deck))}"] if ev else []
        if act >= 1 and (ev is None or ev.get("act", -1) < act):
            was = f" (the current one was set for act {ev['act'] + 1})" if ev and "act" in ev else ""
            out.append(f"no target deck for act {act + 1}{was}: set one with `target` (sts2-plan)")
        return "".join(x + "\n" for x in out)

    def brief(self):
        state = call("peek")
        try:
            deck = self._run()
        except runctx.NoRun:
            return state
        ev, ctx = os.path.join(self.log.dir, "events.jsonl"), self._context()
        bn = "\n".join(x for x in ((bottleneck.line(ev, ctx.act) or bottleneck.line(ev)), bottleneck.dead_line(ev)) if x) if os.path.exists(ev) else ""
        return self._plan_lines(deck) + macro.brief_text(state, deck, ctx.horizon()) + "\n" + (bn + "\n" if bn else "")

    EVAL_VALUED = ("--enc", "--pool", "--attempts", "--hp", "--v")
    EVAL_FLAGS = ("--boss", "--elites", "--next", "--all", "--future", "--smooth", "--target", "--potions")

    @_needs_run("no run in progress")
    def loot(self):
        """gold and potions (while a slot is free), only once the operator has decided the card reward, relics and any stolen card"""
        out = ""
        order = (r"(?i)^\d+ gold", r"(?i)^potion ")
        for _ in range(10):
            state = call("peek")
            if scr.kind(state) != "REWARDS":
                break
            opts = scr.options(state)
            card_done = self.reward_screen is not None and self.reward_screen[0] == scr.floor_key(state)
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

    @_needs_run("no run in progress\n")
    def restcalc(self, argline, state):
        """the rest decision's deciding number: the act boss(es) at the HP now vs after resting (Regal Pillow included)"""
        hp = scr.hp(state)
        m = re.search(r"Rest: Heal for \d+% of your Max HP \((\d+)\)", state)
        if not hp or not m:
            return "restcalc: not at a rest site with a Rest option\n"
        heal = int(m.group(1)) + (15 if any(r["id"] == "REGAL_PILLOW" for r in self._run()["relics"]) else 0)
        after = min(hp[1], hp[0] + heal)
        # one paired batch: the HP after resting is a variant of the HP now
        text = self.evaluate(f"--boss --hp {hp[0]} --attempts 128 --v rest|hp={after} {argline}")
        return f"rest heals {after - hp[0]} ({hp[0]} -> {after}/{hp[1]}); baseline = no rest\n" + text + "\n"

    @_needs_run("no run in progress")
    def evaluate(self, argline):
        argline = argline.strip()
        if argline.startswith("{"):
            spec = json.loads(argline)
        else:
            spec = dict(variants=[dict(name="baseline")])
            for t, val in Args(argline, self.EVAL_VALUED, self.EVAL_FLAGS).items:
                if t == "--enc":
                    spec["encounters"] = val.split(",")
                elif t == "--pool":
                    a, kind, *n = val.split(":")
                    spec["encounters"] = dict(act=a, kind=kind, n=int(n[0]) if n else 0)
                elif t in ("--boss", "--elites", "--next"):
                    spec["encounters"] = self._context().horizon()[t[2:]]
                elif t == "--all":
                    spec["all"] = True
                elif t == "--future":
                    spec["encounters"] = self._context().future()
                elif t == "--smooth":
                    spec["smooth"] = True
                elif t == "--attempts":
                    spec["attempts"] = int(val)
                elif t == "--hp":
                    spec["hp"] = val if val in ("full", "current") else int(val)
                elif t == "--v":
                    spec["variants"].append(self._variant(val))
                elif t == "--potions":
                    spec["hold"] = []
                elif t == "--target":
                    spec["target"] = runlog.target(self.log.dir)
                    if not spec["target"]:
                        return "eval --target: no target deck; set it with `target add=..|remove=..|upgrade=..|relics_add=..` (the plan's)\n"
        if "encounters" not in spec:
            return "need --enc IDS or --pool Act:kind[:n]"
        spec.setdefault("hold", "all")
        if not spec.pop("all", False):
            spec["_ctx"] = self._context().ctx
        # bosses play out on the live search (~5 jobs/s on the 4070): 128 attempts at the final pair, 64 elsewhere (E150)
        final = {b for n in pools.act_names(2) for b in pools.pool(n, "boss")}
        spec.setdefault("attempts", 128 if set(macro.resolve_encounters(spec)) <= final else 64)
        deck = self._run()
        if spec.get("target"):
            t = spec["target"]
            spec["target"] = macro.target_left(t["variant"], deck, t.get("base")) if "variant" in t else t
            deck = macro.apply_variant(deck, spec["target"])
        text, summary = macro.evaluate(self.eng(), deck, spec, live=True)
        self.log.event("eval", spec=spec, result=macro.loggable(summary))
        cav = _engine_line([macro._card(c)["id"] for v in spec["variants"] + [spec.get("target") or {}] for c in v.get("add", [])])
        return text.rstrip("\n") + "\n" + cav if cav else text


    @staticmethod
    def _variant(val):
        parts = val.split("|")
        v = dict(name=parts[0])
        for p in parts[1:]:
            k, x = p.split("=", 1)
            v[k] = int(x) if k == "hp" else [y for y in x.split(",") if y]
        return v

    @_needs_run("no run in progress\n")
    def target(self, rest):
        deck = self._run()
        if rest:
            self.log.event("target", variant=None if rest == "none" else self._variant("target|" + rest), act=self._context().horizon()["act"],
                           base=macro.deck_counts(deck["deck"]))
        t = runlog.target(self.log.dir)
        return (f"target deck (the plan's, a diff vs the deck it was set on; `eval --target` and `price`'s far rungs use it): {t['variant'] if t else 'none'}\n"
                + (f"target missing: {macro.pieces(self._target_left(deck))}\n" if t else ""))

    def prior(self, rest):
        """community prior (tools/community_lut.py): `prior KEY [ITEM ..]`, `prior pair [ITEM ..]` (the map's final pair); no run: `prior CHAR KEY ..`"""
        lut = getattr(self, "_lut", None)
        if lut is None:
            import importlib.util
            spec = importlib.util.spec_from_file_location("community_lut", os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tools", "community_lut.py"))
            lut = self._lut = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(lut)
        words = rest.split()
        try:
            char, bosses = self._run()["character"], self._context().bosses
        except runctx.NoRun:
            char, bosses, words = (words or [""])[0], [], words[1:]
        if not words:
            return "usage: prior KEY [CARD|RELIC ..] | prior pair [..] (KEY: an elite / boss id or prefix, ACT<N>, ACT<N>:<BOSS>)\n"
        if words[0].lower() != "pair":
            return lut.lines(char, words[0], words[1:])
        if len(bosses) < 2:
            return "prior pair: the final pair is not on the map yet; ACT3 pools every act-3 first boss\n" + lut.lines(char, "ACT3", words[1:])
        return "".join(lut.lines(char, k, words[1:], tag=not i) for i, k in enumerate((f"ACT3:{bosses[0]}", *bosses)))

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
        self.log.live(running=line, t_running=round(time.time(), 2))
        out = self._handle(line)
        if out.startswith("commands: s | a <i>") and line.split()[:1] not in (["help"], ["?"]):
            out = f"ERR unknown command `{line.split()[0]}`" + chr(10) + out
        ended = "-- combat over" in out or re.search(r"(?m)^REWARDS$", out)
        if self.gate and ended and "GAME_OVER" not in out and getattr(self, "_briefed", None) != self.fight_id:
            self._briefed = self.fight_id
            try:
                out = out.rstrip("\n") + "\n-- brief\n" + self.brief()
            except Exception:  # noqa: BLE001
                pass
        if _code_stamp() != self.code_stamp:
            out = "WARNING: harness code changed since this daemon started; `quit` reloads it (the reply below came from the old code)\n" + out
        self._watch_run_end(out)
        self.log.live(cmd=line, reply=out, screen=self.last_state, fight=getattr(self, "_last_f", None) if self.rp is not None else None, running=None, potion=self._potion_pending())
        return out

    def _potion_pending(self):
        pend = getattr(self, "_pending", None)
        try:
            return pend[2]["text"] if pend and self.rp is not None and pend[:2] == (self.fight_id, self._turn()) else None
        except Exception:  # noqa: BLE001
            return None

    def _watch_run_end(self, out):
        try:
            last = getattr(self, "last_state", "") or ""
            kind = "GAME_OVER" if "GAME_OVER" in (scr.kind(out), scr.kind(last)) or "\nGAME_OVER\n" in f"\n{out}\n" else scr.kind(out)
            first = out.splitlines()[0].lower() if out else ""
            if not (kind == "GAME_OVER" or "victory" in first):
                return
            out = last if scr.kind(last) == "GAME_OVER" else out[out.find("GAME_OVER"):] if "GAME_OVER" in out else out
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
        peeked = None
        if self.gate and cmd not in skillgate.READ_ONLY and cmd != "prior":  # `prior` reads no game state (READ_ONLY lives in skillgate.py)
            peeked = call("peek")
            why = self._skill_refusal(peeked)
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
                if cmd == "combat" and self.sync() is not None and getattr(self, "drive", ("auto",))[0] == "manual" and not force:
                    return "REFUSED: this fight is MANUAL: play it a turn at a time (`turn`, which prints each play's alternatives), or `combat !` with the reason.\n" + self._drive_line()
                return self.play(cmd == "combat", secs)
            if cmd == "potions":
                return self.potions_now()
            if cmd == "potion":
                return self.potion_cmd(rest)
            if cmd == "a":
                return self.act(rest, peeked)
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
                out = self.evaluate(rest) if cmd == "eval" else getattr(self, PRICING[cmd])(rest, now)
                if here and not out.startswith(PRICING_FAILED):
                    self.priced[cmd] = here
                return out
            if cmd == "brief":
                return self.brief()
            if cmd == "prior":
                return self.prior(rest)
            if cmd == "price":
                now = call("peek")
                self._detect_new_run(now)
                here = scr.floor_key(now)
                out = self.price(rest, now)
                if here and not out.startswith(PRICING_FAILED + ("price: nothing to price",)):
                    self.priced["price"] = here
                return out
            if cmd == "relics":
                raw = call("snap").strip()
                if raw == "null":
                    return "not in combat\n"
                st = json.loads(raw)
                return "\n".join(f"{r['id']}" + (f" counter {r['counter']}" if "counter" in r else "") + (f" {r['props']}" if "props" in r else "") for r in st["relics"]) + "\n"
            if cmd == "note":
                self.log.event("note", text=rest)
                return "noted\n"
            if cmd == "target":
                return self.target(rest.strip())
            if cmd == "status":
                return self.status() + "\n"
            if cmd == "newrun":
                refusal = self._unreviewed()
                if refusal:
                    return refusal
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
