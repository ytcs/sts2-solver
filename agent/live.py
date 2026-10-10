import json
import os
import re
import time

from agent import potions, proposal
from agent import screen as scr
from agent.bridge import call
from agent.fight import Replayer

PRED_ATTEMPTS = 320
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPLAY_BAD = ("diff", "residual", "action failed", "intent unmatched", "start unmatched", "missing")


class Live:
    MANUAL_WIN = 0.90
    MANUAL_Q90 = 0.40
    COSTLY = 0.30
    TAIL = 0.90


    def state(self):
        self.last_state = call("s")
        if scr.kind(self.last_state) == "COMBAT":
            text = self.last_state.rstrip("\n") + "\n" + self._combat_info()
            self.log.live(screen=text, fight=getattr(self, "_last_f", None))
            return text
        return self.last_state

    def _combat_info(self):
        try:
            if self.sync() is None:
                return ""
            lines = []
            draw = call("p draw").strip().split("\n")
            if draw and draw[0].startswith("draw"):
                lines.append(draw[0].rstrip(":") + ": " + ", ".join(l.strip() for l in draw[1:] if l.strip()))
            for i, move, text in self.rp.sim.intent_now():
                lines.append(f"e{i} now: {move} {text}")
            for i, rows in self.rp.sim.intent_plan():
                turns = ["+%d %s" % (h + 1, " | ".join(f"{n} {t}" + (f" ({p:.0%})" if p < 0.995 else "") for n, p, t in r)) for h, r in enumerate(rows)]
                lines.append(f"e{i} plan: " + "  ".join(turns))
            return "\n".join(lines) + "\n" if lines else ""
        except Exception as e:  # noqa: BLE001
            return f"(combat info unavailable: {str(e)[:80]})\n"

    def _fight_json(self):
        raw = call("fight").strip()
        return None if raw == "null" else json.loads(raw)

    def sync(self):
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

    @staticmethod
    def _dist(r, hp):
        import numpy as np
        ends = np.array(r.get("ends") or [])
        if not len(ends):
            return None
        return [round(float(x), 1) for x in np.percentile(hp - ends, np.linspace(0, 100, 21))]

    def _fight_start(self, f):
        sc = f["scenario"]
        self._checked_turn = None
        self._proposal_rows = []
        self.potions_used = 0
        self.fight_worth, self.fight_objective = self._fight_objective(sc)
        try:
            r = self.eng().solve([dict(sc, name="start", potions=[])], attempts=PRED_ATTEMPTS, worth=self.fight_worth)[0]
            q = self._dist(r, sc["hp"])
            pred = dict(win=round(r["win"], 3), win_se=round(r["win_se"], 3), hp_lost=round(r["hp_lost"] or 0, 3), hp_lost_se=round(r.get("hp_lost_se") or 0, 3), n=r["attempts"], lost_q=q,
                        potions="none")
            self._pred_q = q
        except Exception as e:  # noqa: BLE001
            pred = dict(error=str(e)[:80])
            self._pred_q = None
        self.fight_hp0 = (sc["hp"], sc["max_hp"])
        self.last_enc = sc.get("encounter")
        self.fight_budget, self.fight_tol = self._auto_budget(pred, sc["hp"], sc["max_hp"])
        self.drive = self._drive_mode(sc.get("encounter", ""), pred, sc["hp"])
        try:
            mt = self._context().map_text
            if mt and mt != getattr(self, "_logged_map", None):
                self.log.event("map", text=mt)
                self._logged_map = mt
        except Exception:  # noqa: BLE001
            pass
        self.log.event("fight_start", id=f["id"], encounter=sc["encounter"], hp=sc["hp"], max_hp=sc["max_hp"], deck=len(sc["deck"]), relics=[r["id"] for r in sc["relics"]],
                       potions=[p["id"] for p in sc["potions"]], scenario=sc, predicted=pred, budget=self.fight_budget, tol_hp=self.fight_tol,
                       drive=self.drive, objective=self.fight_objective, worth=(self.fight_worth or {}).get("kind", "linear"))

    def _drive_mode(self, enc, pred, hp):
        try:
            with open(os.path.join(ROOT, "data", "drive_manual.json")) as fh:
                listed = json.load(fh)
        except Exception:  # noqa: BLE001
            listed = {}
        if enc in listed:
            return "manual", f"listed: {listed[enc]}"
        if enc.endswith(("_ELITE", "_BOSS")):
            return "manual", "an elite or boss"
        if "win" in pred and pred["win"] < self.MANUAL_WIN:
            return "manual", f"predicted win {pred['win']:.2f} < {self.MANUAL_WIN}"
        q = pred.get("lost_q")
        if q and q[18] >= self.MANUAL_Q90 * hp:
            return "manual", f"q90 predicted loss {q[18]:.0f} HP >= {self.MANUAL_Q90:.0%} of {hp}"
        return "auto", f"predicted win {pred.get('win', '?')}, q90 loss {q[18] if q else '?'} HP"

    def _drive_line(self):
        d = getattr(self, "drive", None)
        if not d:
            return ""
        how = "play each decision with `adv` then `a <i>` (`combat !` / `turn !` to auto-play anyway, with the reason)" if d[0] == "manual" else "`combat` is fine"
        return f"DRIVE: {d[0].upper()} ({d[1]}): {how}\nSEARCH OBJECTIVE: {getattr(self, 'fight_objective', 'linear')}\n"

    @staticmethod
    def _auto_budget(pred, hp, max_hp):
        if "win" not in pred:
            return 5.0, 0.5
        danger = (1 - pred["win"] + pred["win_se"]) * 2 + (pred["hp_lost"] + pred["hp_lost_se"]) * max_hp / max(hp, 1)
        return round(min(15.0, 0.5 + 12 * danger), 1), round(max(0.25, 1 / (1 + 3 * danger)), 2)

    def _budget(self, override=None):
        return override if override is not None else (self.budget if self.budget is not None else self.fight_budget)

    def _tol(self, override=None):
        return 0.0 if (override is not None or self.budget is not None) else self.fight_tol

    @staticmethod
    def _foes(f):
        try:
            es = [e for e in (f.get("state") or {}).get("enemies") or [] if e.get("alive", True)]
            return [e.get("id") for e in sorted(es, key=lambda e: e.get("index", 0))]
        except Exception:  # noqa: BLE001
            return None

    def _fight_end(self, text=None):
        if self.rp is None or self.fight_id in self._ended:
            return
        self._ended.add(self.fight_id)
        text = text or self.last_state
        hp = scr.hp(text) if text else None
        pit = None
        if hp and self.fight_hp0 and self._pred_q:
            import numpy as np
            lost = self.fight_hp0[0] - hp[0]
            pit = round(float(np.interp(lost, self._pred_q, np.linspace(0, 1, len(self._pred_q)))), 3)
        self._save_costly_fight(hp, pit)
        self.log.event("fight_end", id=self.fight_id, hp=hp, pit=pit, potions_used=getattr(self, "potions_used", 0), pred_lost_q=self._pred_q, screen=scr.kind(text) if text else None, hp_start=self.fight_hp0, actions=self.fight_actions, replay=dict(self.rp.stats),
                       errors=self.rp.errors[:3], diff_examples={k: v for k, v in self.rp.examples.items() if not k.startswith("random")})

    def _save_costly_fight(self, hp, pit=None):
        f = getattr(self, "_last_f", None)
        if not hp or f is None or f.get("id") != self.fight_id or not self.fight_hp0:
            return
        lost = self.fight_hp0[0] - hp[0]
        bad = any(k.startswith(REPLAY_BAD) for k in self.rp.stats)
        if lost < self.COSTLY * self.fight_hp0[1] and hp[0] > 0 and not bad and (pit is None or pit < self.TAIL):
            return
        try:
            d = os.path.join(self.log.dir, "fights")
            os.makedirs(d, exist_ok=True)
            enc = self.rp.scenario.get("encounter", "?") if self.rp is not None else "?"
            with open(os.path.join(d, f"{self.fight_id}_{enc}.json"), "w") as fh:
                json.dump(dict(id=self.fight_id, encounter=enc, hp_start=self.fight_hp0, hp_end=hp, scenario=self.rp.scenario, fight=f), fh)
        except Exception as e:  # noqa: BLE001
            self._bookkeeping_error("save costly fight", e)

    def _sync_problem(self, f):
        if self.rp.errors:
            return f"SIMULATOR DESYNC ({self.rp.errors[-1][:120]}). Do not trust advice; play by hand or restart the fight tracking with `status`."
        from agent.fight import RANDOM_PREFIXES
        bad = [l for l in self.rp.sim.diff(json.dumps(f["state"])) if not l.startswith(RANDOM_PREFIXES) and "props.Skin" not in l]
        return f"SIMULATOR DIFFERS FROM THE GAME: {bad[0][:140]}" if bad else None


    def _label(self, text):
        m = re.match(r"pick (\d+)(.*)", str(text))
        if not m or self.rp is None:
            return text
        try:
            return f"pick {self.rp.sim.pick_game_index(int(m.group(1)))}{m.group(2)}"
        except Exception:  # noqa: BLE001
            return text

    def _advice_text(self, d):
        opts = sorted(d["options"], key=lambda o: -(o["q"] if o["q"] is not None else -9))
        alts = "; ".join(f"{self._label(o['text'])} q{o['q']}" for o in opts[:3] if o["action"] != d["action"])
        mine = next((o for o in d["options"] if o["action"] == d["action"]), None)
        q = f" q{mine['q']}" if mine and mine["q"] is not None else ""
        return f"{self._label(d['text'])}{q}" + (f"   [alt: {alts}]" if alts else "")

    def _outlook(self):
        rows = self.rp.sim.lookahead()
        return f"outlook (expected damage, next {len(rows[0][1])} turns): " + "  ".join(f"e{i} " + "/".join(f"{x:.0f}" for x in v) for i, v in rows) if rows else ""

    def advice(self, budget=None):
        if scr.kind(call("peek")) == "GAME_OVER":
            self._fight_end(call("peek"))
            return "not in combat"
        f = self.sync()
        if f is None:
            return "not in combat"
        bad = self._sync_problem(f)
        if bad:
            return bad
        screen = call("peek")
        if self.rp.sim.stage() == "choice" and scr.kind(screen) == "COMBAT":
            self.rp.resolve_phantom_choice(f["state"], "phantom choice on a COMBAT screen")
        if self.rp.sim.stage() == "choice" and scr.kind(screen) == "SELECT":
            bad = self._choice_mismatch(screen)
            if bad:
                return bad
        self._proposal_table = None
        stop = self._proposal_check() if self.rp.sim.stage() == "play" else None
        fresh = None if stop or not self._proposal_table else "\n".join(self._proposal_table + ["no potion proposed this turn"])
        d = self._decide(self.rp.scenario, self.rp.sim, self._budget(budget), tol_hp=self._tol(budget))
        self.log.event("advice", fight=self.fight_id, text=d["text"], options=[dict(text=o["text"], q=o["q"]) for o in d["options"][:6]], drive=getattr(self, "drive", None), foes=self._foes(f))
        return self._advice_text(d) + f"   ({d['rounds']} rounds, {d['seconds']}s)\n" + self._outlook() + (f"\n{stop}" if stop else "") + (f"\n{fresh}" if fresh else "")

    def _decide(self, scenario, sim, budget, **kw):
        e = self.eng()
        keep = e.keep(getattr(self, "aside", set()), self._is_boss())
        return e.decide(scenario, sim, budget, keep_potions=keep, worth=getattr(self, "fight_worth", None), **kw)

    def _fight_objective(self, sc):
        if not getattr(self.eng(), "worth_ok", True):
            return None, "linear (the adopted networks have no outcome head: no per-fight table)"
        bosses, seen = [], []
        try:
            rc = self._context()
            bosses, seen = rc.bosses, rc.seen
        except Exception as e:  # noqa: BLE001
            self._bookkeeping_error("fight objective", e)
        return proposal.fight_objective(sc, bosses, seen)


    def _sim_potions(self):
        return potions.live_slots(self.rp.sim) if self.rp is not None else []

    def _belt(self):
        return scr.belt(call("peek"))

    def _is_boss(self):
        return self.rp is not None and str(self.rp.scenario.get("encounter", "")).endswith("_BOSS")

    def _proposal_check(self, force=False):
        if self.rp is None or self.rp.sim.stage() != "play":
            return None
        if not self._sim_potions():
            return "no potion to price" if force else None
        snap = json.loads(self.rp.sim.snapshot())
        turn = snap.get("turn")
        if not force and turn == getattr(self, "_checked_turn", None):
            return None
        self._checked_turn = turn
        worth = getattr(self, "fight_worth", None)
        why = getattr(self, "fight_objective", "linear")
        rows = proposal.price(self.eng(), self.rp.scenario, self.rp.sim, worth=worth, seed=int(self.fight_id or 0) * 100 + int(turn or 0))
        self._proposal_rows = rows
        boss, aside = self._is_boss(), getattr(self, "aside", set())
        pick, reason = proposal.proposal(rows, aside, boss)
        self.log.event("potion_proposal", fight=self.fight_id, turn=turn, objective=why, rows=rows, proposed=pick["id"] if pick else None,
                       target=pick["text"] if pick else None, why=reason)
        self._proposal_table = proposal.table(rows, turn, why, aside, boss)
        if pick is None and not force:
            return None
        lines = list(self._proposal_table)
        if pick is not None:
            lines.append(f"POTION PROPOSAL (turn {turn}): {pick['id']} now ({pick['text']}): {reason}.")
        else:
            lines.append("no potion proposed: none beats keep and save beyond noise, and none changes this fight's win beyond noise")
        now = call("peek")
        hp_s = scr.hp(now) or (snap["player"]["hp"], snap["player"]["max_hp"])
        lines.append(f"  HP {hp_s[0]}/{hp_s[1]} now; belt: {', '.join(scr.belt(now)) or 'none'}; set aside for the boss: {', '.join(sorted(aside)) or 'none'}")
        lines.append("Your call, ONE potion per commit (the next turn re-prices the rest): `potion use <name>` throws it at the priced target, or `a <i>` "
                     "(its option on the screen); or go on without it (`turn` / `combat`: no new proposal this turn). `potion aside <name>` keeps it for the boss.")
        return "\n".join(lines)

    def potions_now(self):
        f = self.sync()
        if f is None:
            return "not in combat\n"
        return (self._proposal_check(force=True) or "no potion to price") + "\n"

    def potion_cmd(self, rest):
        words = rest.split(None, 1)
        verb, arg = (words[0].lower() if words else ""), (words[1] if len(words) > 1 else "")
        if verb == "use":
            return self._commit(arg)
        if verb == "aside":
            self.aside = potions.parse_names(arg)
            self._save_aside()
            self.log.event("potion_aside", aside=sorted(self.aside))
            return f"set aside for the boss: {sorted(self.aside) or 'nothing'}\n"
        if verb:
            return "usage: potion use <name> | potion aside <name>[, name] | potion aside none\n"
        return f"set aside for the boss: {sorted(getattr(self, 'aside', set())) or 'nothing'}\n"

    def _commit(self, name):
        f = self.sync()
        if f is None:
            return "ERR not in combat\n"
        bad = self._sync_problem(f)
        if bad:
            return "ERR " + bad + "\n"
        sim = self.rp.sim
        if sim.stage() != "play":
            return "ERR a selection is pending: answer it first\n"
        want = potions.parse_names(name)
        pots = self._sim_potions()
        match = [(i, pid) for i, pid in pots if pid in want]
        if len(want) != 1 or not match:
            return f"ERR name one usable potion: {', '.join(pid for _, pid in pots) or 'none usable'}\n"
        i, pid = match[0]
        legal = dict(sim.legal())
        row = next((r for r in getattr(self, "_proposal_rows", []) if r["i"] == i and legal.get(r["action"]) == r["text"]), None)
        if row is not None:
            a = row["action"]
        else:
            acts = [a for a, t in sim.legal() if re.match(rf"potion {i}( |$)", t)]
            if len(acts) != 1:
                return f"ERR {pid} needs a target and no proposal priced one this turn: `potions` prices it, or throw it by hand (`a <i> e<target>`)\n"
            a = acts[0]
        j = sim.action_json(a)
        self.potions_used += 1
        self.fight_actions += 1
        self.log.event("potion_commit", fight=self.fight_id, id=pid, text=legal[a], json=j, priced=row is not None, verdict=row["verdict"] if row is not None else None)
        reply = self._send("do " + j)
        if not reply.startswith("ERR"):
            self.last_state = reply
        return reply


    @staticmethod
    def _base(name):
        cid = re.sub(r"[^A-Z0-9]+", "_", name.strip().rstrip("+").upper().replace("'", "")).strip("_")
        return re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", cid)

    def _choice_mismatch(self, screen):
        labels = [m.group(1) for _, label in scr.options(screen) for m in [re.match(r"(.+?)\(", label)] if m]
        game = sorted(self._base(x) for x in labels)
        sim_cands = lambda: sorted(self._base(m.group(1)) for _, t in self.rp.sim.legal() for m in [re.match(r"pick \d+ \((\w+)\)", t)] if m)  # noqa: E731
        sim = sim_cands()
        if not game or not sim or game == sim:
            return None
        real = (getattr(self, "_last_f", None) or {}).get("state") or {}
        hand, used, opts = real.get("hand") or [], set(), []
        for x in labels:
            up = int(x.strip().endswith("+"))
            j = next((j for j, c in enumerate(hand) if j not in used and self._base(c.get("id", "")) == self._base(x) and int(c.get("upgrade", 0) > 0) == up), None)
            if j is None:
                break
            used.add(j)
            opts.append((hand[j]["id"], int(hand[j].get("upgrade", 0))))
        offer = len(opts) < len(labels)
        if offer:
            from agent import macro
            opts = [macro.card_from_name(x) for x in labels]
        if all(cid for cid, _ in opts) and self.rp.sim.sync_choice(json.dumps(real), opts):
            sim = sim_cands()
            if game == sim:
                self.log.event("sync", what="selection re-synced to the game's " + ("offer" if offer else "hand"))
                return None
        self.log.event("divergence", what="selection options differ", game=game, sim=sim)
        return f"SIMULATOR CHOICE DIFFERS: the game offers {', '.join(game)}, the simulator {', '.join(sim)}. Answer by hand (`a <i>`); the solver's pick does not apply."

    def _answer_selection(self, screen=""):
        sim = self.rp.sim
        if sim.stage() != "choice":
            self.log.event("divergence", what="the game asks for a selection, the simulator does not")
            return self._send("a 0")
        bad = self._choice_mismatch(screen)
        if bad:
            return "ERR " + bad
        m = re.match(r"SELECT (\d+)(?:-(\d+))?", screen)
        hi = int(m.group(2) or m.group(1)) if m else 99
        s2, picks = sim.copy(), []
        cands = lambda: [t for _, t in s2.legal() if t.startswith("pick")]  # noqa: E731
        prompt = cands()
        def gi(a):
            aj = json.loads(s2.action_json(a))
            return s2.pick_game_index(aj["pick"]) if "pick" in aj else None
        for _ in range(40):
            d = self._decide(self.rp.scenario, s2, min(self._budget(), 0.3))
            fresh = [o["action"] for o in sorted(d["options"], key=lambda o: -(o["q"] if o["q"] is not None else -9)) if gi(o["action"]) not in picks]
            a = d["action"] if gi(d["action"]) not in picks else next(iter(fresh), None)
            if a is None or gi(a) is None:
                break
            picks.append(gi(a))
            s2.step(a)
            if s2.stage() != "choice" or len(picks) >= hi or cands() != prompt:
                break
        self.log.event("action", kind_="choose", picks=picks, text=f"choose {picks}")
        return self._send("do " + json.dumps({"choose": picks}))

    def play(self, whole_fight=False, budget=None, max_actions=120):
        budget = self._budget(budget)
        out = []
        tm = dict(state=0.0, sync=0.0, decide=0.0, do=0.0)
        T = time.perf_counter
        for _ in range(max_actions):
            t0 = T()
            txt = self.state()
            tm["state"] += T() - t0
            k = scr.kind(txt)
            if k not in ("COMBAT", "SELECT"):
                self._fight_end(txt)
                self.sync()
                out.append("-- combat over  (secs: " + " ".join(f"{k} {v:.1f}" for k, v in tm.items()) + ")")
                out.append(txt)
                return "\n".join(out)
            if scr.busy(txt):
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
                reply = self._answer_selection(txt)
                out.append("  choose")
            else:
                if self.rp.sim.stage() == "choice":
                    self.rp.resolve_phantom_choice(f["state"], "phantom choice on a COMBAT screen")
                t0 = T()
                stop = self._proposal_check()
                tm["potions"] = tm.get("potions", 0.0) + (T() - t0)
                if stop:
                    out.append(stop)
                    out.append(txt)
                    return "\n".join(out)
                t0 = T()
                d = self._decide(self.rp.scenario, self.rp.sim, budget, tol_hp=self._tol())
                tm["decide"] += T() - t0
                if str(d["text"]).startswith("potion"):
                    self.potions_used += 1
                self.fight_actions += 1
                self.log.event("action", fight=self.fight_id, text=d["text"], json=d["json"], searched=d["searched"], options=d["options"], foes=self._foes(f))
                out.append("  " + self._advice_text(d))
                t0 = T()
                reply = self._send("do " + d["json"])
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
