"""The live fight (micro): follow it in the simulator, advise, play turns, answer selections, and stop at every potion the solver proposes.

Potions (`agent.proposal`): the card-play search plans without them and never plays one; at the start of every player turn each potion is priced three
ways (use now / keep for later in this fight / save), `turn` / `combat` stop on a proposal, and only the operator commits a potion, one per commit
(`potion use <name>` or `a <i>`). The fight's objective (`proposal.fight_objective`: win only for an act boss followed by the ancient's heal, else
linear) is the same for the fight-start prediction, the live search and the proposal's arms.

`Live` is a mixin of `agent.harness.Harness` (the state owner): it reads and writes the harness's fight state (`rp`, `fight_id`, the potion answers ...)
and calls `self.sync` / `self._sync_problem` through the instance, so the sweeps that patch those on a Harness (`agent.fidelity_sweep`, `agent.calibrate`)
keep working. Every action sent to the game goes through `self._send` (the harness drops its run context there).
"""
import json
import os
import re
import time

from agent import potions, proposal
from agent import screen as scr
from agent.bridge import call
from agent.fight import Replayer

PRED_ATTEMPTS = 320  # fights played from the start for the prediction (~1.2 s, same as 48): win rate +-0.03 at worst, HP cost +-0.5% of max HP
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPLAY_BAD = ("diff", "residual", "action failed", "intent unmatched", "start unmatched", "missing")  # replay stats that are fidelity problems


class Live:
    MANUAL_WIN = 0.90  # a fight predicted below this win rate is driven by hand
    MANUAL_Q90 = 0.40  # ... and so is one whose 90th-percentile predicted loss is this share of my HP or more
    COSTLY = 0.30  # a fight that loses this share of max HP (or is lost) is kept whole for the hindsight review (`python -m agent.hindsight`)
    TAIL = 0.90  # ... and so is one whose loss falls in the worst 10% of the prediction (the simulation-game gap shows there first)

    # ------------------------------------------------------------------ following the fight

    def state(self):
        self.last_state = call("s")
        if scr.kind(self.last_state) == "COMBAT":
            text = self.last_state.rstrip("\n") + "\n" + self._combat_info()
            self.log.live(screen=text, fight=getattr(self, "_last_f", None))  # dashboard feed, every action of `turn` / `combat` (no bridge call)
            return text
        return self.last_state

    def _combat_info(self):
        """What a good player reads every turn besides the hand: the draw pile (unordered) and each enemy's move plan after the shown intent, with what each
        possible move does (damage at today's modifiers, statuses into my piles, debuffs on me, its own buffs / block). From the synced simulator."""
        try:
            if self.sync() is None:
                return ""
            lines = []
            draw = call("p draw").strip().split("\n")
            if draw and draw[0].startswith("draw"):
                lines.append(draw[0].rstrip(":") + ": " + ", ".join(l.strip() for l in draw[1:] if l.strip()))
            for i, rows in self.rp.sim.intent_plan():
                turns = ["+%d %s" % (h + 1, " | ".join(f"{n} {t}" + (f" ({p:.0%})" if p < 0.995 else "") for n, p, t in r)) for h, r in enumerate(rows)]
                lines.append(f"e{i} plan: " + "  ".join(turns))
            return "\n".join(lines) + "\n" if lines else ""
        except Exception as e:  # noqa: BLE001  never let the display break a command
            return f"(combat info unavailable: {str(e)[:80]})\n"

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

    @staticmethod
    def _dist(r, hp):
        """The predicted distribution of HP lost from one `solve` result: 21 quantiles (0, 5 ... 100 %) of the start HP minus the end HP (a loss counts as the whole start HP)."""
        import numpy as np
        ends = np.array(r.get("ends") or [])
        if not len(ends):
            return None
        return [round(float(x), 1) for x in np.percentile(hp - ends, np.linspace(0, 100, 21))]

    def _fight_start(self, f):
        sc = f["scenario"]
        self._checked_turn = None  # the turn the potion proposal last priced
        self._proposal_rows = []  # the latest proposal's rows (`potion use` commits the target it priced)
        self.potions_used = 0
        self.fight_worth, self.fight_objective = self._fight_objective(sc)  # before the prediction: it plays the fight under the same objective as live play
        try:
            # the prediction is the no-potion lower bound: a potion is used only when I commit it
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
        try:  # the act map as it stood (once per change): offline replays of route decisions need it
            mt = self._context().map_text
            if mt and mt != getattr(self, "_logged_map", None):
                self.log.event("map", text=mt)
                self._logged_map = mt
        except Exception:  # noqa: BLE001  never let bookkeeping break a fight
            pass
        self.log.event("fight_start", id=f["id"], encounter=sc["encounter"], hp=sc["hp"], max_hp=sc["max_hp"], deck=len(sc["deck"]), relics=[r["id"] for r in sc["relics"]],
                       potions=[p["id"] for p in sc["potions"]], scenario=sc, predicted=pred, budget=self.fight_budget, tol_hp=self.fight_tol,
                       drive=self.drive, objective=self.fight_objective, worth=(self.fight_worth or {}).get("kind", "linear"))

    def _drive_mode(self, enc, pred, hp):
        """("auto" | "manual", why) for this fight. Manual = I play every decision from `adv` (the solver's options and values are the input, the choice is mine), so its gaps
        show up as disagreements instead of hiding inside `combat`. Manual: an elite or boss, an encounter listed in data/drive_manual.json, a predicted win under MANUAL_WIN, or a
        90th-percentile predicted loss of MANUAL_Q90 of my HP or more. `combat` / `turn` refuse in a manual fight unless given `!`."""
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

    def _tol(self, override=None):
        """Early-stop tolerance of the search: an explicit time (`adv N`, `budget N`) is searched in full (no stop on a near-tie), the automatic budget
        stops once the expected regret is below `fight_tol` HP."""
        return 0.0 if (override is not None or self.budget is not None) else self.fight_tol

    def _fight_end(self, text=None):
        """Record the end of the current fight. `text` = the screen right after it (HP is read from it); without it the HP is unknown."""
        if self.rp is None or self.fight_id in self._ended:
            return
        self._ended.add(self.fight_id)
        text = text or self.last_state  # a fight finished by a hand `a` (sim desync) ends here without the screen: the reply of that `a` is the screen after it
        hp = scr.hp(text) if text else None
        pit = None
        if hp and self.fight_hp0 and self._pred_q:
            import numpy as np
            lost = self.fight_hp0[0] - hp[0]
            pit = round(float(np.interp(lost, self._pred_q, np.linspace(0, 1, len(self._pred_q)))), 3)  # where the real loss falls in the predicted distribution (0 = best case, 1 = worse than predicted)
        self._save_costly_fight(hp, pit)
        self.log.event("fight_end", id=self.fight_id, hp=hp, pit=pit, potions_used=getattr(self, "potions_used", 0), pred_lost_q=self._pred_q, screen=scr.kind(text) if text else None, hp_start=self.fight_hp0, actions=self.fight_actions, replay=dict(self.rp.stats),
                       errors=self.rp.errors[:3], diff_examples={k: v for k, v in self.rp.examples.items() if not k.startswith("random")})

    def _save_costly_fight(self, hp, pit=None):
        """Keep the full export (scenario, action log, observed state after every action) of a costly fight: `runs/<run>/fights/<id>_<encounter>.json`. The log may
        end one action before the last (the final sync happens before the killing blow)."""
        f = getattr(self, "_last_f", None)
        if not hp or f is None or f.get("id") != self.fight_id or not self.fight_hp0:
            return
        lost = self.fight_hp0[0] - hp[0]
        bad = any(k.startswith(REPLAY_BAD) for k in self.rp.stats)  # fidelity cases are kept too
        if lost < self.COSTLY * self.fight_hp0[1] and hp[0] > 0 and not bad and (pit is None or pit < self.TAIL):
            return
        try:
            d = os.path.join(self.log.dir, "fights")
            os.makedirs(d, exist_ok=True)
            enc = self.rp.scenario.get("encounter", "?") if self.rp is not None else "?"
            with open(os.path.join(d, f"{self.fight_id}_{enc}.json"), "w") as fh:
                json.dump(dict(id=self.fight_id, encounter=enc, hp_start=self.fight_hp0, hp_end=hp, scenario=self.rp.scenario, fight=f), fh)
        except Exception as e:  # noqa: BLE001  never let bookkeeping break a fight
            self._bookkeeping_error("save costly fight", e)

    def _sync_problem(self, f):
        """None when the simulator matches the game, else a loud description. Advice from a desynced simulator is stale or wrong: never use it silently."""
        if self.rp.errors:
            return f"SIMULATOR DESYNC ({self.rp.errors[-1][:120]}). Do not trust advice; play by hand or restart the fight tracking with `status`."
        from agent.fight import RANDOM_PREFIXES
        bad = [l for l in self.rp.sim.diff(json.dumps(f["state"])) if not l.startswith(RANDOM_PREFIXES) and "props.Skin" not in l]  # a relic's random cosmetic skin (Pael's Legion) is not game state
        return f"SIMULATOR DIFFERS FROM THE GAME: {bad[0][:140]}" if bad else None

    # ------------------------------------------------------------------ advice

    def _label(self, text):
        """A selection pick in the GAME's numbering: the simulator numbers the choices in its own order (`pick 4 (STRIKE)` could be option 1 on the screen)."""
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
        """Expected enemy damage over the next turns after the shown intent (the move pattern humans know by heart)."""
        rows = self.rp.sim.lookahead()
        return f"outlook (expected damage, next {len(rows[0][1])} turns): " + "  ".join(f"e{i} " + "/".join(f"{x:.0f}" for x in v) for i, v in rows) if rows else ""

    def advice(self, budget=None):
        if scr.kind(call("peek")) == "GAME_OVER":  # the game still exports the finished fight there: no decision to search
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
        self.log.event("advice", fight=self.fight_id, text=d["text"], options=[dict(text=o["text"], q=o["q"]) for o in d["options"][:6]], drive=getattr(self, "drive", None))  # manual fights: my choice (the next `macro` event) vs this
        return self._advice_text(d) + f"   ({d['rounds']} rounds, {d['seconds']}s)\n" + self._outlook() + (f"\n{stop}" if stop else "") + (f"\n{fresh}" if fresh else "")

    def _decide(self, scenario, sim, budget, **kw):
        """Every live search goes through here: under the fight's objective (`fight_worth`), and without potions (`keep_potions=True`: the search never plays
        one and plans no line that counts on one; potions are proposed by `_proposal_check` and committed by me)."""
        return self.eng().decide(scenario, sim, budget, keep_potions=True, worth=getattr(self, "fight_worth", None), **kw)

    def _fight_objective(self, sc):
        """(worth table or None, why) of this fight (`proposal.fight_objective`): the act's boss(es) from the map and the encounters met this act."""
        if not getattr(self.eng(), "worth_ok", True):
            return None, "linear (the adopted networks have no outcome head: no per-fight table)"
        bosses, seen = [], []
        try:
            rc = self._context()
            bosses, seen = rc.bosses, rc.seen
        except Exception as e:  # noqa: BLE001  never let bookkeeping break a fight
            self._bookkeeping_error("fight objective", e)
        return proposal.fight_objective(sc, bosses, seen)

    # ------------------------------------------------------------------ potions: the solver proposes (agent.proposal), I commit one at a time

    def _sim_potions(self):
        """[(slot, id)] of the potions usable now (`potions.live_slots`: the simulator's slots, fixed for the fight)."""
        return potions.live_slots(self.rp.scenario, self.rp.sim) if self.rp is not None else []

    def _game_json(self, j):
        """The simulator numbers potions by position in its own list; the game by slot (an empty first slot makes them differ). Translate a `use_potion` action."""
        return potions.to_game_action(self.rp.scenario, j)

    def _belt(self):
        """The potions in the game's belt, slot order (the header's `pots[...]`; `-` = empty slot). Relic-made potions (Potion-Shaped Rock) are there, not in the fight scenario."""
        return scr.belt(call("peek"))

    def _potion_name(self, text):
        """The belt name of the simulator's `potion N`: N is the packed index, the belt is by game slot (`pots[-, Fire Potion]`: potion 0 is slot 1)."""
        i = potions.text_index(text)
        if i is not None and self.rp is not None:
            i = potions.game_slot(self.rp.scenario, i)
        belt = self._belt()
        return belt[i] if i is not None and i < len(belt) else "?"

    def _is_boss(self):
        return self.rp is not None and str(self.rp.scenario.get("encounter", "")).endswith("_BOSS")

    def _proposal_check(self, force=False):
        """At the start of each player turn (once per turn; `force`: now): every potion priced use now / keep / save (`proposal.price`), the table kept in
        `_proposal_table`. Returns the stop text when a potion is proposed (`proposal.proposal`), with `force` the table in any case; else None."""
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
        """potions: the turn's proposal on demand (read-only): every potion priced use now / keep / save."""
        f = self.sync()
        if f is None:
            return "not in combat\n"
        return (self._proposal_check(force=True) or "no potion to price") + "\n"

    def potion_cmd(self, rest):
        """potion use <name>: commit ONE potion (at the target the turn's proposal priced). potion aside <name>[, name] | none: potions kept for the boss
        (outside boss fights they stop `turn` / `combat` only when the win is at stake; saved with the run record). potion: what is set aside."""
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
        """Throw ONE potion now, at the target the turn's proposal priced for it (a potion with a single action needs no proposal)."""
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
        reply = self._send("do " + self._game_json(j))
        if not reply.startswith("ERR"):
            self.last_state = reply
        return reply

    # ------------------------------------------------------------------ selections and the play loop

    @staticmethod
    def _base(name):
        """A card's identity without the character suffix and the upgrade mark: 'Strike+' and STRIKE_SILENT -> 'STRIKE'."""
        cid = re.sub(r"[^A-Z0-9]+", "_", name.strip().rstrip("+").upper().replace("'", "")).strip("_")
        return re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", cid)

    def _choice_mismatch(self, screen):
        """None when the simulator's pending selection offers the same cards as the game's SELECT screen, else a description. Random offers (Colorless /
        Attack / Skill / Power Potion, Discovery) roll differently in the simulator: its pending choice is re-synced to the game's offered cards
        (`Sim.sync_choice` creates them as the candidates). A prompt from the hand after a draw (Survivor, Dagger Throw) is re-synced the same way:
        the simulator drew its own sample, the game's hand is visible."""
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
        offer = len(opts) < len(labels)  # not all in the hand: a random offer of generated cards, named on screen
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
        """A card-selection prompt: the search picks card by card on a copy of the simulator; the whole answer goes to the game at once."""
        sim = self.rp.sim
        if sim.stage() != "choice":
            self.log.event("divergence", what="the game asks for a selection, the simulator does not")
            return self._send("a 0")
        bad = self._choice_mismatch(screen)
        if bad:
            return "ERR " + bad
        s2, picks = sim.copy(), []
        for _ in range(40):
            d = self._decide(self.rp.scenario, s2, min(self._budget(), 0.3))
            aj = json.loads(d["json"])
            if "pick" in aj:
                picks.append(s2.pick_game_index(aj["pick"]))
                s2.step(d["action"])
            else:
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
                if self.rp.sim.stage() == "choice":  # no selection on the screen, one in the simulator: settle it and re-sync (else it sends `pick` forever)
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
                self.log.event("action", fight=self.fight_id, text=d["text"], json=d["json"], searched=d["searched"], options=d["options"])
                out.append("  " + self._advice_text(d))
                t0 = T()
                reply = self._send("do " + self._game_json(d["json"]))
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
