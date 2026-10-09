"""Headless baseline: our agent's A10 win rate on the game's own code (plan stage 1, macro side).

One process drives N `OracleCombat serve` games (own port, --state-dir, APPDATA/TEMP each) round-robin with ONE shared search Engine
(models/current.json, cover + exact turn search, hp_cap default); `agent.bridge.OVERRIDE` points every bridge call at the stepping game.
Never the real game: the bridge never falls back to it while OVERRIDE is set.

Combat: the live player (`Harness.play`), a fixed number of search rounds per decision (--rounds, not seconds); potions only when the
proposal rule fires (`potion use` the proposed one, one per commit). No fight-start predictions, no DRIVE thresholds. A DIFFERS line does
not stop play (counted; a potion-belt DIFFERS, e.g. Entropic Brew's random potions, turns proposals off for that fight); a DESYNC
(or a search error, or combat_in_progress differing) rebuilds the replayer once per fight, then plays `fallback_action` (first playable
card, else end turn; run tagged `fallback`; also every fight the simulator cannot build, tagged `unplayable`); a combat selection the game
rejects is answered by the macro SELECT rule.
Macro = runmodel.BasePolicy, the rules `price` rollouts play, applied to screens (no price/reward/routes calls):
  map BasePolicy.node; card reward / card offers: predictor screen of each deck vs the act boss at max HP and a random elite at 70% (argmax
  of runmodel.worth, skip included); rest: rest below 50% HP else smith BasePolicy.smith order; shop: remove a card if affordable, then the
  best affordable card/relic by the same screen; catalogued events: BasePolicy.event on the first page, follow-up pages as rollouts resolve
  them (events.default_choose: the exit option, else the first modelled); Neow/ancients: the predictor screen of each option
  applied (events.apply_ancient), fights at min(reference HP, HP after the option); removals/transforms: curses, Strikes, Defends first;
  upgrades BasePolicy.smith order; other selections the first k; rewards: gold, relics, cards, potions only into a free slot; treasure:
  open, take; bundle: the first; Crystal Sphere: tools/serve_harness_run heuristic. No potion use outside combat.
  Unknown events (not in data/events.json): the first option starting Leave/Exit/Decline/Ignore/Abstain/Give Up/Proceed/Continue/Skip, else 0.
Win = the EVENT "The Architect" is reached (the GAME_OVER page is never read for the result); its pages and GAME_OVER are logged.

usage: python tools/baseline.py [--n 20] [--seeds S1,S2] [--games 2] [--rounds 16] [--port 15820] [--tag NAME] [--character ironclad]
Seeds: BASE0001..BASE0020 by default. Per game: target/baseline/<tag>/<seed>/ (events.jsonl, server log); per run one JSON line in
evals/baseline/<tag>.jsonl (re-running skips seeds already there); summary table at the end. Builds Harness() directly (no skill gate).
"""
import json, os, random, re, socket, subprocess, sys, threading, time, traceback, zlib

os.environ.setdefault("STS2_DEVICE", "cuda")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))

from agent import bridge, events as EV, price as PR, runmodel as R, screen as scr  # noqa: E402
from agent.harness import Harness  # noqa: E402
from agent.runlog import RunLog  # noqa: E402

DLL = os.environ.get("ORACLE_DLL") or os.path.join(ROOT, "oracle", "combat", "bin", "Release", "net9.0", "OracleCombat.dll")
DOTNET = os.environ.get("DOTNET", os.path.expanduser("~/.dotnet/dotnet.exe" if os.name == "nt" else "~/.dotnet/dotnet"))
LEAVE = ("leave", "exit", "decline", "ignore", "abstain", "give up", "proceed", "continue", "skip")
MAP_TYPE = {"Monster": "M", "Elite": "E", "Unknown": "?", "Rest": "R", "RestSite": "R", "Shop": "$", "Merchant": "$", "Treasure": "T"}
CURSES = {c["id"] for c in R.CAT["cards"].get("CURSE", [])}
SHUFFLES, PSEED = 4, 1000


class GameLog(RunLog):
    """RunLog in a fixed directory; never reads or writes runs/CURRENT."""

    def __init__(self, d):
        self.run_id, self.dir, self._f, self._live = os.path.basename(d), d, None, {}

    def new_run(self):
        pass


class BaselineHarness(Harness):
    def __init__(self, log, engine, rounds, seed):
        super().__init__(log=log)
        self.engine, self.rounds, self._seed = engine, rounds, zlib.crc32(seed.encode()) * 1000
        self.stats = dict(decisions=0, decide_s=0.0, proposals_s=0.0, differs=0, desync=0, select_fallback=0, fallback_actions=0)

    def _decide(self, scenario, sim, budget, **kw):
        e = self.eng()
        e.seed, t0 = self._seed, time.perf_counter()
        d = e.decide(scenario, sim, budget, keep_potions=True, worth=getattr(self, "fight_worth", None), rounds=self.rounds)
        self._seed = e.seed
        self.stats["decisions"] += 1
        self.stats["decide_s"] += time.perf_counter() - t0
        return d

    def _proposal_check(self, force=False):
        if getattr(self, "_belt_off", None) == self.fight_id:
            return None
        t0 = time.perf_counter()
        try:
            return super()._proposal_check(force)
        finally:
            self.stats["proposals_s"] += time.perf_counter() - t0

    def _fight_start(self, f):
        sc = f["scenario"]
        self._checked_turn, self._proposal_rows, self.potions_used, self._pred_q = None, [], 0, None
        self.fight_worth, self.fight_objective = self._fight_objective(sc)
        self.fight_hp0, self.last_enc = (sc["hp"], sc["max_hp"]), sc.get("encounter")
        self.drive = ("auto", "baseline: no fight-start prediction")
        self.log.event("fight_start", id=f["id"], encounter=sc["encounter"], hp=sc["hp"], max_hp=sc["max_hp"], act=sc.get("act"),
                       floor=scr.floor_key(bridge.call("peek")), deck=len(sc["deck"]), relics=[r["id"] for r in sc["relics"]],
                       potions=[p["id"] for p in sc["potions"]], objective=self.fight_objective)

    def _sync_problem(self, f):
        bad = super()._sync_problem(f)
        if bad and "DIFFERS" in bad and not self.rp.errors:
            self.stats["differs"] += 1
            self.log.event("divergence", what=bad[:200])
            if ".combat_in_progress" in bad:
                return "SIMULATOR DESYNC (combat_in_progress): " + bad
            if ".potions" in bad:
                self._belt_off = self.fight_id
            return None
        return bad


def _tree(options):
    """every option of an event, nested follow-up pages included"""
    for o in options:
        yield o
        for e in o["effects"]:
            for sub in [e] + e.get("then", []) + e.get("else", []):
                if "choice" in sub:
                    yield from _tree(sub["choice"])


def fallback_action(s):
    """when the simulator cannot follow the fight: the first playable card (first living enemy as target), else end turn"""
    opts = scr.options(s)
    foes = [m.group(1) for l in s.split("\n") for m in [re.match(r"^e(\d+) ", l)] if m and "[untargetable]" not in l]
    end = next((n for n, t in opts if t == "end turn"), opts[-1][0] if opts else "0")
    play = next(((n, t) for n, t in opts if not t.startswith(("(x)", "potion", "end turn", "discard"))), None)
    if play is None:
        return end
    return play[0] + (f" e{foes[0]}" if play[1].rstrip().endswith("->e") and foes else "")


def _name(label):
    m = re.match(r"^(.+?)\(", label)
    return (m.group(1) if m else label.split(":", 1)[0]).strip()


class Macro:
    """runmodel.BasePolicy on the game's screens (module docstring)."""

    def __init__(self, h, predictor, seed):
        self.h, self.pred, self.seed, self.pol = h, predictor, seed, R.BasePolicy()
        self.declined, self.opened, self.shop_plan, self.won = set(), None, {}, None

    def rng(self, s, what):
        return random.Random(zlib.crc32(f"{self.seed}|{scr.floor_key(s)}|{what}".encode()))

    def st(self, s):
        h = self.h
        return PR.run_state(h._run(), h._context(), os.path.join(h.log.dir, "events.jsonl"), s)

    def screen(self, st, variants, rng, hp_min=False):
        sts = []
        for f in variants:
            x = st.copy()
            if f is not None:
                f(x)
            sts.append(x)
        refs = R.reference_fights(st, rng)
        scen = [x.scenario(e, min(hp, x.hp) if hp_min else hp, potions=[]) for x in sts for e, hp in refs]
        P = self.pred.fight_start(scen, SHUFFLES, PSEED)
        return R.worth(P, st.max_hp).reshape(len(sts), len(refs)).mean(1)

    def drive(self, gen):
        try:
            req = next(gen)
            while True:
                req = gen.send(self.pred.fight_start(req, SHUFFLES, PSEED))
        except StopIteration as e:
            return e.value

    def decide(self, s, opts):
        kind = scr.kind(s)
        lab = [(int(n), t) for n, t in opts]
        first = lambda pred: next((i for i, t in lab if pred(t)), None)  # noqa: E731
        proceed = first(lambda t: t.lower().startswith("proceed"))
        if kind == "MAP":
            return self.map(s, lab) + " !"
        if kind == "CARD_REWARD":
            return self.card_reward(s, lab)
        if kind == "REWARDS":
            fk = scr.floor_key(s)
            for i, t in lab:
                if t.startswith("proceed") or (t.startswith("potion") and "slots full" in t):
                    continue
                if t.startswith("card"):
                    if (fk, t) in self.declined:
                        continue
                    self.opened = (fk, t)
                return str(i)
            return str(proceed if proceed is not None else lab[-1][0])
        if kind == "RESTSITE":
            if len(lab) == 1:
                return str(lab[0][0])
            want = self.pol.rest(self.st(s))
            i = first(lambda t: t.lower().startswith(want)) or first(lambda t: t.lower().startswith(("rest", "smith")))
            return str(i if i is not None else proceed if proceed is not None else lab[0][0])
        if kind == "SHOP":
            return self.shop(s, lab)
        if kind == "EVENT":
            return self.event(s, lab)
        if kind == "SELECT":
            return self.select(s, lab)
        if kind == "TREASURE":
            i = first(lambda t: t.startswith(("open", "take")))
            return str(i if i is not None else proceed if proceed is not None else first(lambda t: not t.startswith("potion")))
        if kind == "CHOOSE_BUNDLE":
            return str(lab[0][0])
        from serve_harness_run import macro_choice
        return macro_choice(s)

    def map(self, s, lab):
        st = self.st(s)
        keyed = {}
        for i, t in lab:
            m = re.match(r"(\w+) r(\d+)c(\d+)", t)
            if not m:
                return str(i)
            k = (int(m.group(2)), int(m.group(3)))
            typ = (st.nodes or {}).get(k, {}).get("type") or MAP_TYPE.get(m.group(1), "M")
            keyed[k] = (i, typ)
        k = self.pol.node(st, [(k, typ) for k, (_, typ) in keyed.items()])
        return str(keyed[k][0])

    def card_reward(self, s, lab):
        skip = next((i for i, t in lab if t.startswith("Skip")), None)
        cards = []
        for i, t in lab:
            cid, up = PR._card_id(_name(t)) if "(" in t else (None, 0)
            if cid:
                cards.append((i, cid, up))
        if not cards:
            choice = skip if skip is not None else lab[0][0]
        else:
            st = self.st(s)
            variants = ([None] if skip is not None else []) + [lambda x, c=c, u=u: x.deck.append({"id": c, "upgrade": u}) for _, c, u in cards]
            w = self.screen(st, variants, self.rng(s, "card"))
            best = int(w.argmax())
            choice = (skip if best == 0 else cards[best - 1][0]) if skip is not None else cards[best][0]
        if choice == skip and self.opened:
            self.declined.add(self.opened)
        return str(choice)

    def shop(self, s, lab):
        fk = scr.floor_key(s)
        if fk not in self.shop_plan:
            st = self.st(s)
            items = []
            for _, t in lab:
                if "can't afford" in t:
                    continue
                m = re.match(r"^(\d+)g (card|relic) (.+?)(?:\(|:)", t)
                if m:
                    price, what, name = int(m.group(1)), m.group(2), m.group(3).strip()
                    iid = PR._card_id(name)[0] if what == "card" else PR._ident(name)
                    if iid and (what == "card" or iid in PR._ids("relics")):
                        items.append((what, iid, price))
                m = re.match(r"^(\d+)g remove a card", t)
                if m:
                    items.append(("remove", None, int(m.group(1))))
            buys = self.drive(self.pol.shop(st, items, self.rng(s, "shop"))) or []
            self.shop_plan[fk] = [(k, i) for k, i, _ in buys]
        plan = self.shop_plan[fk]
        while plan:
            k, iid = plan.pop(0)
            for i, t in lab:
                if "can't afford" in t:
                    continue
                if k == "remove" and re.match(r"^\d+g remove a card", t):
                    return str(i)
                m = re.match(r"^\d+g (card|relic) (.+?)(?:\(|:)", t)
                if m and m.group(1) == k and (PR._card_id(m.group(2).strip())[0] if k == "card" else PR._ident(m.group(2))) == iid:
                    return str(i)
        leave = next((i for i, t in lab if t == "leave shop"), lab[-1][0])
        return str(leave)

    def event(self, s, lab):
        lines = s.split("\n")
        title = lines[2].split(":", 1)[0].strip() if len(lines) > 2 else ""
        if title == "The Architect" and self.won is None:
            self.won = dict(floor=scr.floor_key(s), hp=scr.hp(s))
            self.h.log.event("victory", screen=s[:1500])
        if len(lab) == 1:
            return str(lab[0][0])
        labels = [t for _, t in lab]
        anc = [(i, EV.ancient_option(t)[0]) for i, t in lab]
        if any(r for _, r in anc) and EV.get(title) is None:
            st = self.st(s)
            ok = [(i, r) for i, r in anc if r]
            variants, keep = [], []
            for i, r in ok:
                def f(x, r=r):
                    EV.apply_ancient(x, r, R.Draws(random.Random(0), st.base["character"], st.act))
                try:
                    f(st.copy())
                except Exception:  # noqa: BLE001
                    continue
                variants.append(f)
                keep.append(i)
            if keep:
                w = self.screen(st, variants, self.rng(s, "ancient"), hp_min=True)
                return str(keep[int(w.argmax())])
        entry = EV.get(title)
        if entry is not None:
            matched = EV.match(title, labels)
            sub = [o for o in matched if o is not None and not o["key"].endswith("_LOCKED")]
            if sub:
                j = self.pol.event(self.st(s), {"options": sub})
                return str(lab[matched.index(sub[j])][0])
            nested = [next((o for o in _tree(entry["options"]) if EV._pattern(o["label"]).match(t.split(":", 1)[0].strip()) or EV._pattern(o["label"]).match(t)), None)
                      for t in labels]
            if any(nested):
                j = EV.default_choose(None, [o or {"effects": [{"unmodelled": "?"}]} for o in nested])
                return str(lab[j][0])
        i = next((i for i, t in lab if t.lower().startswith(LEAVE)), lab[0][0])
        return str(i)

    def select(self, s, lab):
        lines = s.split("\n")
        m = re.match(r"SELECT (\d+)(?:-(\d+))?", lines[0])
        lo = int(m.group(1)) if m else 1
        hi = int(m.group(2) or lo) if m else 1
        k = max(lo, 1)
        prompt = (lines[2] if len(lines) > 2 else "").lower()
        names = [(i, _name(t)) for i, t in lab]
        if "remove" in prompt or "transform" in prompt:
            rank = lambda n: 0 if PR._ident(n) in CURSES else 1 if n == "Strike" else 2 if n == "Defend" else 9  # noqa: E731
            order = sorted(names, key=lambda x: rank(x[1]))
            if lo == 0 and rank(order[0][1]) == 9:
                return "-"
            return " ".join(str(i) for i, _ in order[:k])
        if "upgrade" in prompt:
            free = [(i, n) for i, n in names if not n.endswith("+") and not re.search(r"\+\d*$", n)]
            order = [x for x in free if x[1] not in ("Strike", "Defend")] + [x for x in free if x[1] in ("Strike", "Defend")] + [x for x in names if x not in free]
            return " ".join(str(i) for i, _ in order[:k])
        if "add" in prompt or prompt.strip() == "choose a card":
            cards = [(i, *PR._card_id(n)) for i, n in names]
            cards = [(i, c, u) for i, c, u in cards if c]
            if cards:
                st = self.st(s)
                w = self.screen(st, [None] + [lambda x, c=c, u=u: x.deck.append({"id": c, "upgrade": u}) for _, c, u in cards], self.rng(s, "offer"))
                gain = sorted(range(len(cards)), key=lambda j: -w[j + 1])
                take = [cards[j][0] for j in gain][:lo] if lo else [cards[j][0] for j in gain if w[j + 1] > w[0]][:hi]
                return " ".join(map(str, take)) if take else "-"
            return "-" if lo == 0 else " ".join(str(i) for i, _ in lab[:k])
        return "-" if lo == 0 and not names else " ".join(str(i) for i, _ in lab[:k])


class Game:
    def __init__(self, seed, port, out, engine, predictor, rounds, character):
        self.seed, self.port, self.ep = seed, port, f"127.0.0.1:{port}"
        self.dir = os.path.join(out, seed)
        os.makedirs(self.dir, exist_ok=True)
        for f in ("events.jsonl", "live.json"):
            if os.path.exists(os.path.join(self.dir, f)):
                os.remove(os.path.join(self.dir, f))
        sd = os.path.join(self.dir, "server")
        env = dict(os.environ, APPDATA=os.path.join(sd, "appdata"), LOCALAPPDATA=os.path.join(sd, "localappdata"), TEMP=os.path.join(sd, "tmp"),
                   TMP=os.path.join(sd, "tmp"))
        for k in ("APPDATA", "LOCALAPPDATA", "TEMP"):
            os.makedirs(env[k], exist_ok=True)
        self.slog = open(os.path.join(sd, "server.log"), "w")
        self.proc = subprocess.Popen([DOTNET, os.path.abspath(DLL), "serve", "--port", str(port), "--state-dir", os.path.join(sd, "saves"), "--seed", seed,
                                      "--character", character, "--ascension", "10"], stdout=self.slog, stderr=self.slog, cwd=sd, env=env)
        for _ in range(600):
            try:
                socket.create_connection(("127.0.0.1", port), timeout=1).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError(f"server on {port} did not start")
        self.h = BaselineHarness(GameLog(self.dir), engine, rounds, seed)
        self.macro = Macro(self.h, predictor, seed)
        self.t0, self.own, self.steps, self.errors, self.stuck = time.time(), 0.0, 0, 0, 0
        self.bad, self.last, self.final, self.tags = {}, None, None, set()
        self.potions, self.desync_turn = 0, None

    def step(self):
        t0 = time.perf_counter()
        try:
            return self._step()
        finally:
            self.own += time.perf_counter() - t0
            self.steps += 1

    def _step(self):
        h = self.h
        s = bridge.call("s")
        kind = scr.kind(s)
        if s.startswith("ERR"):
            self.errors += 1
            return self.errors > 50
        if kind == "GAME_OVER":
            self.final = s
            h.log.event("game_over", screen=s[:1500])
            return True
        if kind == "MENU":
            self.final = s
            self.tags.add("menu")
            return True
        if scr.busy(s):
            time.sleep(0.02)
            return False
        if kind == "COMBAT":
            self.last_combat = (scr.floor_key(s), [m.group(1) for l in s.split("\n") for m in [re.match(r"^e\d+ (.+?) \d+/\d+ ", l)] if m])
        if kind in ("COMBAT", "SELECT"):
            try:
                f = h.sync()
            except Exception as e:  # noqa: BLE001  the simulator cannot build this fight (unmodelled content): fallback play
                self.tags.add("unplayable")
                h.stats["fallback_actions"] += 1
                h.log.event("unplayable", error=f"{type(e).__name__}: {e}"[:300])
                h.handle("a " + (fallback_action(s) if kind == "COMBAT" else self.macro.select(s, [(int(n), t) for n, t in scr.options(s)])))
                return False
            if kind == "COMBAT" or f is not None:
                return self.combat()
        key = zlib.crc32(s.encode())
        self.stuck = self.stuck + 1 if key == self.last else 0
        self.last = key
        if self.stuck > 40:
            self.tags.add("stuck")
            h.log.event("stuck", screen=s[:1500])
            return True
        bad = self.bad.setdefault(key, set())
        opts = [(n, t) for n, t in scr.options(s) if n not in bad]
        if not opts:
            opts = scr.options(s)
        choice = self.macro.decide(s, opts)
        r = h.handle("a " + choice)
        if r.startswith(("ERR", "REFUSED")):
            self.errors += 1
            bad.add(choice.split()[0])
            h.log.event("baseline_error", screen=s[:800], choice=choice, reply=r[:300])
        return False

    def combat(self):
        h = self.h
        out = h.handle("combat !")
        m = re.search(r"POTION PROPOSAL \(turn \d+\): (\S+) now", out)
        if m:
            r = h.handle(f"potion use {m.group(1)}")
            self.potions += not r.startswith("ERR")
            return False
        bad = "SIMULATOR DESYNC" in out or "SIMULATOR CHOICE DIFFERS" in out or "SIMULATOR DIFFERS" in out
        if not (bad or out.startswith(("ERR", "REFUSED")) or "\nERR" in out):
            return False
        h.log.event("baseline_error", reply=out[-600:])
        if bad:
            h.stats["desync"] += 1
            self.tags.add("desync")
        else:
            self.errors += 1
        s = bridge.call("s")
        if scr.kind(s) == "SELECT":
            h.stats["select_fallback"] += 1
            h.handle("a " + self.macro.select(s, [(int(n), t) for n, t in scr.options(s)]))
        elif scr.kind(s) == "COMBAT":
            if self.desync_turn != h.fight_id:
                self.desync_turn = h.fight_id
                h.rp = None
            else:
                h.stats["fallback_actions"] += 1
                self.tags.add("fallback")
                h.handle("a " + fallback_action(s))
        if self.errors > 50:
            self.tags.add("errors")
            return True
        return False

    def result(self):
        try:
            bridge.call("shutdown")
        except Exception:  # noqa: BLE001
            pass
        try:
            self.proc.wait(10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
        self.slog.close()
        ev = [json.loads(l) for l in open(os.path.join(self.dir, "events.jsonl"), encoding="utf-8")] if os.path.exists(os.path.join(self.dir, "events.jsonl")) else []
        starts = {e["id"]: e for e in ev if e["kind"] == "fight_start"}
        fights, act_end = [], []
        for e in ev:
            if e["kind"] != "fight_end" or e["id"] not in starts:
                continue
            f0 = starts[e["id"]]
            hp1 = (e.get("hp") or [0])[0]
            fights.append(dict(floor=f0.get("floor"), enc=f0["encounter"], hp0=f0["hp"], hp1=hp1, lost=f0["hp"] - hp1, potions=e.get("potions_used", 0)))
            if f0["encounter"].endswith("_BOSS"):
                act_end.append((f0.get("act"), f0["encounter"], hp1))
        won = self.macro.won is not None
        hdr = scr.header_line(self.final or self.h.last_state or "", "") or ""
        m = re.search(r"Floors Climbed: (\d+)", self.final or "")
        fm = re.search(r"F(\d+)", hdr)
        floor = int(m.group(1)) if m else (int(fm.group(1)) if fm else None)
        died = None
        if not won:
            last = fights[-1] if fights else None
            lc = getattr(self, "last_combat", None)
            if last and last["hp1"] <= 0:
                died = last["enc"]
            elif lc and lc[0] == scr.floor_key(hdr) and (not last or last["floor"] != lc[0]):
                died = "unplayable fight: " + ",".join(lc[1])
            elif self.final is None:
                died = "aborted: " + ",".join(sorted(self.tags))
            else:
                died = next(((e.get("state", "").split("\n") + ["", "", ""])[2][:40] for e in reversed(ev)
                             if e["kind"] == "macro" and e.get("screen", "").startswith("EVENT")), None)
        acts = {}
        for a, enc, hp in act_end:
            acts[a] = hp
        st = self.h.stats
        return dict(seed=self.seed, character="IRONCLAD", ascension=10, result="win" if won else "loss", floor=floor, died_at=died,
                    act_end_hp={str((a or 0) + 1): hp for a, hp in sorted(acts.items(), key=lambda x: x[0] or 0)}, fights=fights,
                    hp_lost=sum(f["lost"] for f in fights), potions=self.potions, tags=sorted(self.tags), victory=self.macro.won,
                    final_screen=(self.final or "")[:400], wall_s=round(time.time() - self.t0, 1), own_s=round(self.own, 1), steps=self.steps,
                    decisions=st["decisions"], decide_s=round(st["decide_s"], 1), proposals_s=round(st["proposals_s"], 1), differs=st["differs"],
                    desync=st["desync"], select_fallback=st["select_fallback"],
                    fallback_actions=st["fallback_actions"], errors=self.errors)


class GpuSampler(threading.Thread):
    def __init__(self):
        super().__init__(daemon=True)
        self.util, self.mem, self.p = [], [], None

    def run(self):
        try:
            self.p = subprocess.Popen(["nvidia-smi", "--query-gpu=utilization.gpu,memory.used", "--format=csv,noheader,nounits", "-l", "1"],
                                      stdout=subprocess.PIPE, text=True)
            for line in self.p.stdout:
                u, m = (float(x) for x in line.split(","))
                self.util.append(u)
                self.mem.append(m)
        except Exception:  # noqa: BLE001
            pass

    def stop(self):
        if self.p:
            self.p.kill()


def summary(rows, gpu=None):
    import math
    out = [f"{'seed':10s} {'result':6s} {'floor':>5s} {'died at':34s} {'act HP':14s} {'HP lost':>7s} {'fights':>6s} {'pots':>4s} {'own min':>7s} {'wall min':>8s} tags"]
    for r in rows:
        acts = "/".join(str(r["act_end_hp"].get(str(a), "-")) for a in (1, 2, 3))
        out.append(f"{r['seed']:10s} {r['result']:6s} {r['floor'] or 0:5d} {str(r['died_at'] or '')[:34]:34s} {acts:14s} {r['hp_lost']:7d} {len(r['fights']):6d} "
                   f"{r['potions']:4d} {r['own_s'] / 60:7.1f} {r['wall_s'] / 60:8.1f} {','.join(r['tags'])}")
    n = len(rows)
    if n:
        p = sum(r["result"] == "win" for r in rows) / n
        out.append(f"win rate {p:.3f} +- {math.sqrt(p * (1 - p) / n):.3f} (n {n}); mean floor {sum(r['floor'] or 0 for r in rows) / n:.1f}; "
                   f"own time per run {sum(r['own_s'] for r in rows) / n / 60:.1f} min; decide {sum(r['decide_s'] for r in rows) / max(1, sum(r['decisions'] for r in rows)):.3f} s/decision")
    if gpu and gpu.util:
        out.append(f"GPU utilisation mean {sum(gpu.util) / len(gpu.util):.0f}% (samples {len(gpu.util)}), memory max {max(gpu.mem):.0f} MiB")
    return "\n".join(out)


def main():
    a = sys.argv[1:]
    get = lambda k, d: a[a.index(k) + 1] if k in a else d  # noqa: E731
    n, games, rounds, base = int(get("--n", 20)), int(get("--games", 2)), int(get("--rounds", 16)), int(get("--port", 15820))
    character = get("--character", "ironclad")
    seeds = get("--seeds", ",".join(f"BASE{i:04d}" for i in range(1, n + 1))).split(",")
    tag = get("--tag", f"r{rounds}")
    out = os.path.join(ROOT, "target", "baseline", tag)
    res_path = os.path.join(ROOT, "evals", "baseline", f"{tag}.jsonl")
    os.makedirs(os.path.dirname(res_path), exist_ok=True)
    done = {json.loads(l)["seed"]: json.loads(l) for l in open(res_path, encoding="utf-8")} if os.path.exists(res_path) else {}
    pending = [s for s in seeds if s not in done]
    assert all(base + i not in (15555, 15556) for i in range(games))
    from agent.engine import Engine
    from predictor import Predictor
    engine = Engine()
    predictor = Predictor(engine.solver.net)
    gpu = GpuSampler()
    gpu.start()
    slots = [None] * games
    print(f"baseline {tag}: {len(pending)} runs pending of {len(seeds)}, {games} games round-robin, {rounds} rounds per decision", flush=True)
    try:
        while pending or any(slots):
            for i in range(games):
                if slots[i] is None and pending:
                    bridge.OVERRIDE = f"127.0.0.1:{base + i}"
                    slots[i] = Game(pending.pop(0), base + i, out, engine, predictor, rounds, character)
                g = slots[i]
                if g is None:
                    continue
                bridge.OVERRIDE = g.ep
                try:
                    end = g.step()
                except Exception:  # noqa: BLE001
                    g.errors += 1
                    g.h.log.event("baseline_exception", tb=traceback.format_exc()[-2000:])
                    end = g.errors > 50
                    if end:
                        g.tags.add("errors")
                if end:
                    try:
                        r = g.result()
                    except Exception:  # noqa: BLE001
                        r = dict(seed=g.seed, result="loss", floor=None, died_at="result error: " + traceback.format_exc()[-300:], act_end_hp={}, fights=[],
                                 hp_lost=0, potions=g.potions, tags=sorted(g.tags | {"result_error"}), wall_s=round(time.time() - g.t0, 1),
                                 own_s=round(g.own, 1), decisions=0, decide_s=0.0)
                    done[r["seed"]] = r
                    with open(res_path, "a", encoding="utf-8") as f:
                        f.write(json.dumps(r) + "\n")
                    print(summary([r]).split("\n")[1], flush=True)
                    slots[i] = None
    finally:
        bridge.OVERRIDE = None
        gpu.stop()
        for g in slots:
            if g is not None:
                g.proc.kill()
    rows = [done[s] for s in seeds if s in done]
    text = summary(rows, gpu)
    print(text)
    with open(os.path.join(ROOT, "evals", "baseline", f"{tag}.txt"), "w", encoding="utf-8") as f:
        f.write(text + "\n")


if __name__ == "__main__":
    main()
