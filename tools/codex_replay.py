"""Replay logs (spire-codex /api/runs/{hash}/replay, replay_version 7) -> combats. The one reconstruction for every consumer (fidelity, search-gap
mining, collection roots, macro extraction): `fights(path)` yields each combat's start scenario (deck by card instance, relics + saved state,
potions by slot, HP, gold, the logged per-stream RNG counters), its events and the human actions; `Board` folds a fight's events into the logged
state (snapshot schema of sts2diff, as `Sim.sync` / `Sim.diff` read it) at any cut. Per-combat RNG counters and draw order are hidden in-game:
offline fidelity / roots only, never an input to a scored decision.
fidelity: each attempt-0 combat of clean replays in `Sim.exact` (logged counters), the human's actions applied, the sim diffed against the log
before every human play / potion and re-synced on a difference (one divergence per step); first divergence per combat + per-field counts.
usage: python tools/codex_replay.py fidelity target/codex/replays --out target/review/replay_fidelity [--character ironclad] [--workers 4]
       python tools/codex_replay.py corpus target/codex/replays --out target/review/replay_corpus.json"""
import collections, glob, gzip, json, os, re, sys

CHARS = {"IRONCLAD", "SILENT", "DEFECT", "NECROBINDER", "REGENT"}
STREAMS = ("shuffle", "combat_card_generation", "combat_potion_generation", "combat_card_selection", "combat_energy_costs", "combat_targets", "monster_ai",
           "niche", "combat_orbs")
HUMAN = ("play", "potion_start", "end_turn")


def events(path):
    with gzip.open(path, "rt", encoding="utf-8") as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


def _pascal(k):
    return "".join(p.capitalize() for p in k.split("_"))


def _card(d):
    out = dict(id=d["id"], upgrade=int(d.get("up") or 0), c=d.get("c"))
    if d.get("enchantment"):
        out["enchantment"] = dict(id=d["enchantment"], amount=int(d.get("amount") or 1))
    return out


class Run:
    """run state between combats, from the event stream (headers after a resume are authoritative)"""

    def __init__(self):
        self.deck, self.relics, self.potions, self.slots = [], [], {}, 3
        self.hp = self.max_hp = None
        self.gold, self.char, self.seed = 99, None, None

    def apply(self, e):
        t = e["t"]
        if t == "header":
            self.char, self.seed = e.get("character"), e.get("seed")
            self.deck = list(e.get("starting_deck") or [])
            self.relics = [dict(id=r["id"], props={_pascal(k): v for k, v in (r.get("state") or {}).items()}) for r in e.get("relics") or []]
            pots = e.get("potions") or []
            self.slots = len(pots) or self.slots
            self.potions = {p["slot"]: p["id"] for p in pots if p.get("id")}
            self.max_hp = e.get("starting_max_hp") or self.max_hp
        elif t == "resume":
            self.hp, self.gold = e.get("hp", self.hp), e.get("gold", self.gold)
            self.relics = [dict(id=r["id"], props={_pascal(k): v for k, v in (r.get("state") or {}).items()}) for r in e.get("relics") or []] or self.relics
            pots = e.get("potions") or []
            if pots:
                self.slots, self.potions = len(pots), {p["slot"]: p["id"] for p in pots if p.get("id")}
        elif t == "deck":
            self.deck = list(e["cards"])
        elif t == "relic":
            self.relics.append(dict(id=e["id"], props={}))
        elif t == "relic_lost":
            i = next((i for i, r in enumerate(self.relics) if r["id"] == e["id"]), None)
            if i is not None:
                self.relics.pop(i)
        elif t == "relic_counter":
            for r in self.relics:
                if r["id"] == e["id"]:
                    r["counter"] = e["n"]
        elif t == "potion_got":
            free = [s for s in range(max(self.slots, len(self.potions) + 1)) if s not in self.potions]
            self.potions[e.get("slot", free[0])] = e["id"]
        elif t in ("potion_used", "potion_dropped"):
            s = next((s for s, p in sorted(self.potions.items()) if p == e["id"]), None)
            if s is not None:
                del self.potions[s]
        elif t == "hp" and e.get("dst") == "player":
            self.hp = e["hp"]
        elif t == "max_hp" and e.get("dst", "player") == "player":
            self.max_hp, self.hp = e["max_hp"], e.get("hp", self.hp)
        elif t == "hit" and e.get("dst") == "player":
            self.hp = max(0, (self.hp or 0) - (e.get("dmg") or 0))
        elif t == "hp_loss":
            self.hp = max(0, (self.hp or 0) - (e.get("dmg") or 0))
        elif t == "gold":
            self.gold = e["gold"]

    def scenario(self, start):
        return dict(character=self.char, seed=self.seed, ascension=10, encounter=start["encounter"], act=start["act"] - 1, total_floor=start["floor"],
                    hp=self.hp, max_hp=self.max_hp, gold=self.gold, max_energy=3, max_potion_slots=self.slots,
                    base_orb_slots=3 if self.char == "DEFECT" else 0, deck=[_card(c) for c in self.deck],
                    relics=[dict(r, props=dict(r["props"])) for r in self.relics],
                    potions=[dict(slot=s, id=p) for s, p in sorted(self.potions.items())],
                    rng={k: start["rng_state"][k] for k in STREAMS if k in start.get("rng_state", {})})


def fights(path):
    """(scenario, start event, combat events, combat_end or None) per combat_start in log order; a retry after a reload repeats the combat_id.
    Card events logged between room entry and combat_start (room-entry relics on combat cards: Stone Cracker) follow draw_order in the events."""
    run, cur, entry = Run(), None, []
    for e in events(path):
        t = e["t"]
        if cur is not None:
            if t in ("combat_end", "end", "header"):
                yield cur[0], cur[1], cur[2], e if t == "combat_end" else None
                cur = None
            else:
                cur[2].append(e)
                if t == "draw_order":
                    cur[2].extend(entry)
        if t == "room":
            entry = []
        elif t in ("upgrade", "enchant", "afflict") and cur is None:
            entry.append(e)
        if t == "combat_start":
            cur = (run.scenario(e), e, [])
        run.apply(e)
    if cur is not None:
        yield cur[0], cur[1], cur[2], None


def _next_piles(evs):
    """(event index, card) -> pile for generated and picked cards, read from the next event naming the card (the log does not say where they went);
    generated card -> upgrade, from its first play"""
    out, pending, gen, up = {}, {}, set(), {}
    for i, e in enumerate(evs):
        t = e["t"]
        if pending:
            named = {e.get("c"), e.get("from_c")} | set(e.get("flushed_c") or []) | set(e.get("retained_c") or [])
            for c, j in list(pending.items()):
                if t == "shuffle" and c in (e.get("order_c") or []):
                    out[j, c] = "disc"
                elif c in named:
                    out[j, c] = "draw" if t == "draw" else "hand" if evs[j]["t"] == "generate" or t in ("play", "flush", "discard") else None
                else:
                    continue
                del pending[c]
        if t == "generate":
            pending[e["c"]] = i
            gen.add(e["c"])
        elif t == "pick":
            pending.update({c["c"]: i for c in e.get("cards") or [] if c.get("c") is not None})
        elif t == "play" and e.get("c") in gen and e["c"] not in up:
            up[e["c"]] = int(e.get("up") or 0)
        elif t == "upgrade":
            gen.discard(e.get("c"))
    return out, up


class Board:
    """the logged combat state, folded event by event; snapshot() is the sts2diff schema (piles top-last as the game lists them)"""

    def __init__(self, sc, start, evs=()):
        self.next_pile, self.gen_up, self.i, self.spent, self.unknown_cards = *_next_piles(evs), -1, False, []
        self.p = dict(hp=sc["hp"], max_hp=sc["max_hp"], block=0, powers={})
        self.en = {}
        for x in sorted(start.get("enemies") or [], key=lambda x: x.get("i", 0)):
            self.en[x["cid"]] = dict(id=x["id"], hp=x["hp"], max_hp=x["max_hp"], block=0, alive=True, powers={}, move=None, intent=None)
        self.cards, self.draw, self.hand, self.disc, self.exh = {}, [], [], [], []
        self.energy, self.stars, self.potions = 0, 0, dict((p["slot"], p["id"]) for p in sc["potions"])
        self.deck, self.deck_to_c = {c["c"]: c for c in sc["deck"]}, {}
        self.char = sc["character"]

    def _pile_of(self, c):
        for pile in (self.hand, self.draw, self.disc, self.exh):
            if c in pile:
                return pile
        return None

    def _move(self, c, dst):
        src = self._pile_of(c)
        if src is not None:
            src.remove(c)
        if dst is not None:
            dst.append(c)

    def _who(self, e, key="tgt"):
        cid = e.get(key + "_cid")
        if cid in self.en:
            return self.en[cid]
        name = e.get(key)
        return self.p if name in (None, "player") or name == self.char or cid is None and name not in [x["id"] for x in self.en.values()] else None

    def apply(self, e):
        t = e["t"]
        self.i += 1
        if t == "draw_order":
            self.deck_to_c = dict(zip(e["order_deck_c"], e["order_c"]))
            for c, dc in zip(e["order_c"], e["order_deck_c"]):
                self.cards[c] = {k: v for k, v in (self.deck.get(dc) or dict(id="?", upgrade=0)).items() if k != "c"}
            self.draw = list(reversed(e["order_c"]))
        elif t == "shuffle":
            for c in e["order_c"]:
                self._move(c, None)
            self.draw = list(reversed(e["order_c"])) + self.draw
        elif t == "draw":
            self.cards.setdefault(e["c"], dict(id=e["id"], upgrade=0))
            self._move(e["c"], self.hand)
        elif t == "generate":
            placed = self._pile_of(e["c"]) is not None
            self.cards[e["c"]] = dict(self.cards.get(e["c"]) or {}, id=e["id"], upgrade=self.gen_up.get(e["c"], e.get("up")))
            pile = None if placed else self.next_pile.get((self.i, e["c"]))
            if placed:
                pass
            elif pile == "draw":
                self.draw.insert(0, e["c"])
            elif pile is not None:
                getattr(self, pile).append(e["c"])
            else:
                self.unknown_cards.append(e["c"])
        elif t == "pick":
            for c in e.get("cards") or []:
                pile = self.next_pile.get((self.i, c.get("c")))
                if pile:
                    self._move(c["c"], getattr(self, pile))
        elif t == "play":
            self.cards.setdefault(e["c"], dict(id=e["id"], upgrade=int(e.get("up") or 0)))["upgrade"] = int(e.get("up") or 0)
            if e.get("play_index", 0) == 0 and not self.spent:
                self.energy -= e.get("cost_paid") or 0
                self.stars -= e.get("stars_paid") or 0
            self.spent = False
            rp = e.get("result_pile")
            self._move(e["c"], {"discard": self.disc, "exhaust": self.exh, "hand": self.hand, "draw": self.draw}.get(rp))
        elif t == "discard":
            self._move(e["c"], self.disc)
        elif t == "exhaust":
            self._move(e["c"], self.exh)
        elif t == "flush":
            for c in e.get("flushed_c") or []:
                self._move(c, self.disc)
        elif t in ("upgrade", "downgrade") and e.get("c") in self.cards:
            self.cards[e["c"]]["upgrade"] = int(e.get("up", (self.cards[e["c"]]["upgrade"] or 0) + 1) if t == "upgrade" else e.get("up", 0))
        elif t == "transform" and e.get("from_c") in self.cards:
            pile = self._pile_of(e["from_c"])
            self.cards[e["to_c"]] = dict(id=e["to_id"], upgrade=0)
            if pile is not None:
                pile[pile.index(e["from_c"])] = e["to_c"]
        elif t == "afflict" and e.get("c") in self.cards:
            self.cards[e["c"]]["affliction"] = dict(id=e["affliction"], amount=e.get("amount_total", e.get("amount")))
        elif t == "unafflict" and e.get("c") in self.cards:
            self.cards[e["c"]].pop("affliction", None)
        elif t == "enchant" and e.get("c") in self.cards:
            self.cards[e["c"]]["enchantment"] = dict(id=e["enchantment"], amount=e.get("amount_total", e.get("amount")))
        elif t == "energy":
            self.energy, self.spent = e["energy"], "src" not in e and (e.get("d") or 0) < 0
        elif t == "stars":
            self.stars = e["stars"]
            self.spent = self.spent or (e.get("d") or 0) < 0
        elif t == "block":
            w = self.en.get(e.get("src_cid")) if e.get("src_cid") is not None else (self.p if e.get("src") == "player" else None)
            if w is not None and "left" in e:
                w["block"] = e["left"]
        elif t == "hit":
            w = self.en.get(e.get("dst_cid")) if e.get("dst_cid") is not None else (self.p if e.get("dst") == "player" else None)
            if w is not None:
                w["hp"], w["block"] = max(0, w["hp"] - (e.get("dmg") or 0)), max(0, w["block"] - (e.get("blocked") or 0))
        elif t == "hp_loss":
            self.p["hp"] = max(0, self.p["hp"] - (e.get("dmg") or 0))
        elif t == "hp":
            w = self.en.get(e.get("dst_cid")) if e.get("dst_cid") is not None else (self.p if e.get("dst") == "player" else None)
            if w is not None:
                w["hp"] = e["hp"]
                if w is not self.p and e["hp"] > 0:
                    w["alive"] = True
        elif t == "forge":
            for bl in e.get("blades") or []:
                if bl.get("c") is not None and self._pile_of(bl["c"]) is None:
                    self.cards.setdefault(bl["c"], dict(id="SOVEREIGN_BLADE", upgrade=0))
                    self.hand.append(bl["c"])
        elif t == "remove":
            self._move(self.deck_to_c.get(e.get("c"), e.get("c")), None)
        elif t == "max_hp":
            w = self.en.get(e.get("dst_cid")) if e.get("dst_cid") is not None else self.p
            if w is not None:
                w["max_hp"], w["hp"] = e["max_hp"], e.get("hp", w["hp"])
        elif t in ("power", "power_lost"):
            w = self._who(e)
            if w is not None:
                w.setdefault("logged", set()).add(e["id"])
                if e.get("amount"):
                    w["powers"][e["id"]] = e["amount"]
                else:
                    w["powers"].pop(e["id"], None)
        elif t == "death" and e.get("tgt_cid") in self.en:
            self.en[e["tgt_cid"]]["alive"] = bool(e.get("removal_prevented"))
        elif t == "escape" and e.get("tgt_cid") in self.en:
            self.en[e["tgt_cid"]]["alive"] = False
        elif t == "spawn" and e.get("side", "enemy") == "enemy":
            self.en[e["cid"]] = dict(id=e["id"], hp=e["hp"], max_hp=e["max_hp"], block=0, alive=True, powers={}, move=None, intent=None)
        elif t == "intent" and e.get("src_cid") in self.en:
            self.en[e["src_cid"]].update(move=e["id"], intent=(e.get("dmg"), e.get("hits")) if "dmg" in e else None)
        elif t == "potion_used" or t == "potion_dropped":
            s = next((s for s, p in sorted(self.potions.items()) if p == e["id"]), None)
            if s is not None:
                del self.potions[s]
        elif t == "potion_got":
            free = [s for s in range(8) if s not in self.potions]
            self.potions[e.get("slot", free[0])] = e["id"]

    def seed_powers(self, sn):
        """powers a creature enters the fight with (relics, monster innates, spawns) are not logged: at its first cut take the sim's, per power id never logged"""
        pairs = [(self.p, sn["player"])] + [(x, sn["enemies"][j]) for x, j in zip(self.en.values(), _pair(list(self.en.values()), sn["enemies"])) if j is not None]
        for x, y in pairs:
            if not x.get("seen"):
                x["seen"] = True
                for p in y["powers"]:
                    if p["id"] not in x.get("logged", ()):
                        x["powers"][p["id"]] = p["amount"]

    def snapshot(self):
        cards = lambda pile: [self.cards.get(c, dict(id="?", upgrade=0)) for c in pile]
        pw = lambda d: [dict(id=k, amount=v) for k, v in d.items()]
        en = [dict(id=x["id"], hp=x["hp"], max_hp=x["max_hp"], block=x["block"], alive=x["alive"], powers=pw(x["powers"]), next_move=x["move"], intent=x["intent"])
              for x in self.en.values()]
        return dict(energy=self.energy, stars=self.stars, player=dict(hp=self.p["hp"], max_hp=self.p["max_hp"], block=self.p["block"], powers=pw(self.p["powers"])),
                    enemies=en, hand=cards(self.hand), draw=cards(self.draw), discard=cards(self.disc), exhaust=cards(self.exh),
                    potions=[dict(slot=s, id=p) for s, p in sorted(self.potions.items())], unknown=[self.cards[c] for c in self.unknown_cards])


ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
UNPORTED = re.compile(r"unknown (card|relic|potion|encounter|enchantment) id ([A-Z0-9_.]+)|Unimplemented(Card|Relic|Potion|Encounter)\(\"([A-Z0-9_]+)\"\)")


def exact_sim(sc):
    """Sim on the logged RNG: combat_start counters are post-init (opening shuffle, HP rolls), so the streams start at counter - the sim's own init draws.
    -> (sim, scenario as built, streams whose post-init counter misses the log) or raises ValueError"""
    import sts2
    sc = dict(sc, relics=[dict(r) for r in sc["relics"]])
    for _ in range(12):
        try:
            sim = sts2.Sim.exact(json.dumps(sc))
            break
        except ValueError as ex:
            m = re.search(r"relic ([A-Z0-9_]+) has no modelled saved property (\w+)|relic prop (\w+):", str(ex))
            if not m:
                raise
            key = m.group(2) or m.group(3)
            for r in sc["relics"]:
                r["props"] = {k: v for k, v in r.get("props", {}).items() if k != key}
    # a logged relic counter (relic_counter.n) goes into the relic's one saved int property when the sim displays something else
    fixed = [dict(r) for r in sc["relics"]]
    for r, (rid, _, shown, props) in zip(fixed, json.loads(sim.diag())["relics"]):
        if r.get("counter", -1) >= 0 and shown != r["counter"] and len(props) == 1:
            r["props"] = dict(r.get("props") or {}, **{props[0][0]: r["counter"]})
    if fixed != sc["relics"]:
        try:
            sim, sc = sts2.Sim.exact(json.dumps(dict(sc, relics=fixed))), dict(sc, relics=fixed)
        except ValueError:
            pass
    got = dict(sim.rng_counters())
    pre = dict(sc, rng={k: max(0, v - (got[k] - v)) for k, v in sc["rng"].items()})
    sim = sts2.Sim.exact(json.dumps(pre))
    return sim, pre, sorted(k for k, v in sim.rng_counters() if k in sc["rng"] and v != sc["rng"][k])


def _ms(cards):
    return collections.Counter((c["id"], c.get("upgrade", 0)) for c in cards)


def _pile_diff(sim_cards, log_cards, unknown):
    """(sim extra, log extra) multisets; a log card with upgrade None (generated, never played) matches its id at any upgrade; sim extras that are
    generated cards of unlogged pile are tolerated"""
    a, b = _ms(sim_cards), _ms(c for c in log_cards if c.get("upgrade") is not None)
    sx, lx = a - b, b - a
    for c in log_cards:
        if c.get("upgrade") is None:
            k = next((k for k in sx if k[0] == c["id"]), None)
            if k:
                sx[k] -= 1
                sx = +sx
            else:
                lx[(c["id"], None)] += 1
    loose = collections.Counter(c["id"] for c in unknown)
    for k in list(sx):
        n = min(sx[k], loose[k[0]])
        sx[k] -= n
        loose[k[0]] -= n
    return +sx, +lx


def _pair(log_en, sim_en):
    """board enemies -> sim enemy index: alive ones by monster id and rank among the alive of that id (list order); dead ones unpaired"""
    out, used = [], collections.Counter()
    for x in log_en:
        if not x["alive"]:
            out.append(None)
            continue
        k = used[x["id"]]
        used[x["id"]] += 1
        same = [i for i, y in enumerate(sim_en) if y["id"] == x["id"] and y["alive"]]
        out.append(same[k] if k < len(same) else None)
    return out


def diff(sim, log, intents=True, energy=True):
    """field -> (sim, log); powers as dicts, piles as multisets; logged intent damage is the turn-start display, so `intents` only before the turn's first action;
    energy gains are not logged (spends are, absolute, just before the play), so `energy` only when the action spends"""
    out = {}

    def put(k, a, b):
        if a != b:
            out[k] = (a, b)
    if energy:
        put("energy", sim["energy"], log["energy"])
        if log.get("stars"):
            put("stars", sim.get("stars", 0), log["stars"])
    for k in ("hp", "max_hp", "block"):
        put(f"player.{k}", sim["player"][k], log["player"][k])
    sp, lp = {p["id"]: p["amount"] for p in sim["player"]["powers"]}, {p["id"]: p["amount"] for p in log["player"]["powers"]}
    for k in set(sp) | set(lp):
        put(f"player.power.{k}", sp.get(k, 0), lp.get(k, 0))
    pairs = _pair(log["enemies"], sim["enemies"])
    for x, j in zip(log["enemies"], pairs):
        if j is None:
            if x["alive"]:
                put(f"enemy.missing.{x['id']}", None, x["id"])
            continue
        y, tag = sim["enemies"][j], x["id"]
        if not x["alive"] and not y["alive"]:
            continue
        put(f"enemy.alive.{tag}", y["alive"], x["alive"])
        if not x["alive"] or not y["alive"]:
            continue
        for k in ("hp", "max_hp", "block"):
            put(f"enemy.{k}.{tag}", y[k], x[k])
        sp, lp = {p["id"]: p["amount"] for p in y["powers"]}, {p["id"]: p["amount"] for p in x["powers"]}
        for k in set(sp) | set(lp):
            put(f"enemy.power.{k}.{tag}", sp.get(k, 0), lp.get(k, 0))
        if x["next_move"] not in (None, "STUNNED"):
            put(f"enemy.move.{tag}", y.get("next_move"), x["next_move"])
            atk = [i for i in y.get("intents") or [] if i.get("type") in ("Attack", "DeathBlow") and "damage" in i]
            if intents and x["intent"] and atk and y.get("next_move") == x["next_move"]:
                put(f"enemy.intent.{tag}", [atk[0]["damage"], atk[0]["hits"]], list(x["intent"]))
    extra = [y["id"] for j, y in enumerate(sim["enemies"]) if j not in pairs and y["alive"]]
    if extra:
        out["enemy.extra"] = (extra, [])
    for pile in ("hand", "draw", "discard", "exhaust"):
        sx, lx = _pile_diff(sim.get(pile) or [], log[pile], log.get("unknown") or [])
        if sx or lx:
            out[pile] = (sorted(sx.elements()), sorted(lx.elements(), key=str))
    a, b = collections.Counter(p["id"] for p in sim["potions"]), collections.Counter(p["id"] for p in log["potions"])
    put("potions", sorted(a.elements()), sorted(b.elements()))
    return out


GROUP = {"energy": "energy", "stars": "stars", "player": "player", "enemy": "enemies", "hand": "hand", "draw": "draw", "discard": "discard", "exhaust": "exhaust",
         "potions": "potions"}


def field_kind(f):
    """divergence key without the monster tag: enemy.hp, enemy.power.X, enemy.move.M (monster kept for moves / intents)"""
    p = f.split(".")
    if p[0] == "enemy" and p[1] == "power":
        return ".".join(p[:3])
    if p[0] == "enemy" and p[1] in ("hp", "max_hp", "block", "alive"):
        return ".".join(p[:2])
    return f


def _sync(sim, sn, log, fields):
    real = dict(sn)
    for f in fields:
        real[GROUP[f.split(".")[0]]] = log[GROUP[f.split(".")[0]]]
    real.pop("relics", None)
    sim.sync(json.dumps(real))


def _label(e):
    return e["id"] if e["t"] in ("play", "potion_start") else "end_turn"


REPLAY_POWERS = {"BURST_POWER", "DUPLICATION_POWER", "ECHO_FORM_POWER"}


def _pre_play(v):
    """logged just before the play event but part of it: the energy / star spend (absolute), a play-doubling power used up (Burst, Duplication)"""
    if v["t"] in ("energy", "stars"):
        return "src" not in v and (v.get("d") or 0) < 0
    return v["t"] in ("power", "power_lost") and v.get("id") in REPLAY_POWERS and (v.get("n") or 0) < 0 and "tgt_cid" not in v


def human_actions(evs):
    return [i for i, e in enumerate(evs) if (e["t"] == "play" and not e.get("auto") and not e.get("play_index")) or e["t"] == "potion_start"
            or (e["t"] == "end_turn" and e.get("side") == "player")]


def replay_fight(sc, start, evs, end, keep_ops=False):
    """one combat in Sim.exact with the human's actions; before each play / potion the sim is diffed against the log and synced on a difference
    -> (result row, (scenario, ops) for a repro when keep_ops)"""
    row = dict(enc=start["encounter"], floor=start["floor"], act=start["act"], char=sc["character"], cuts=0, divs=[], fields=collections.Counter(),
               actions=0, failed=None, end=None, deck=sorted({c["id"] for c in sc["deck"]}), relics=[r["id"] for r in sc["relics"]])
    try:
        sim, pre, bad = exact_sim(sc)
    except ValueError as ex:
        m = UNPORTED.search(str(ex))
        row["unported"] = f"{(m.group(1) or m.group(3)).lower()}:{m.group(2) or m.group(4)}" if m else None
        row["error"] = str(ex)[:160]
        return row, None
    row["init_rng_miss"], row["rng_pre"] = bad, pre["rng"]
    b, ops = Board(sc, start, evs), []
    human = human_actions(evs)
    picks = {-1: [e for e in evs[:human[0] if human else len(evs)] if e["t"] == "pick" and e.get("selector", "human") == "human" and not e.get("decision_type")]}
    for k, i in enumerate(human):
        nxt = human[k + 1] if k + 1 < len(human) else len(evs)
        picks[i] = [e for e in evs[i + 1:nxt] if e["t"] == "pick" and e.get("selector", "human") == "human" and not e.get("decision_type")]
    starts = {}
    for i in human:
        j = i
        while j > 0 and evs[i]["t"] == "play" and _pre_play(evs[j - 1]):
            j -= 1
        starts[j] = i
    turn, last, moves, cascade = 0, "start", [], False

    def step(j):
        sim.step(j)
        if keep_ops:
            ops.append(("step", j))

    def resolve(pks):
        """the window's logged picks on the sim's pending selections, then any selection the log did not show (confirm / first option)"""
        for pk in pks:
            if sim.stage() != "choice":
                row["fields"]["choice.unasked"] += 1
                break
            for c in pk.get("cards") or []:
                opts = [j for j, t in sim.legal() if t.startswith("pick ") and t.endswith(f"({c['id']})")]
                if not opts:
                    row["fields"]["choice.option_missing"] += 1
                    continue
                step(opts[0])
                if sim.stage() != "choice":
                    break
            conf = [j for j, t in sim.legal() if t == "confirm"] if sim.stage() == "choice" else []
            if conf:
                step(conf[0])
        for _ in range(8):
            if sim.stage() != "choice":
                break
            row["fields"]["choice.unlogged"] += 1
            leg = sim.legal()
            if not leg:
                break
            step(next((j for j, t in leg if t == "confirm"), leg[0][0]))
    resolve(picks[-1])
    for i, e in enumerate(evs):
        if e["t"] == "turn" and e.get("side") == "player":
            turn = e.get("n", turn)
        if e["t"] == "move":
            moves.append(e.get("id"))
        if i in starts:
            ae = evs[starts[i]]
            if ae["t"] != "end_turn":
                if sim.stage() != "play":
                    row["failed"] = dict(step=row["actions"], turn=turn, why=f"sim stage {sim.stage()} before {_label(ae)}", after=last)
                    break
                sn = json.loads(sim.snapshot())
                b.seed_powers(sn)
                spend = [v for v in evs[i:starts[i]] if v["t"] in ("energy", "stars") and "src" not in v]
                for v in spend:
                    setattr(b, v["t"], v[v["t"]] - v["d"])
                log = b.snapshot()
                d = diff(sn, log, last in ("start", "end_turn"), bool(spend))
                row["cuts"] += 1
                if d:
                    for f, (x, y) in d.items():
                        row["fields"][field_kind(f)] += 1
                        if len(row["divs"]) < 12:
                            row["divs"].append(dict(field=f, sim=x, log=y, step=row["actions"], turn=turn, after=last, moves=moves[-4:], cascade=cascade))
                    cascade = cascade or any(f.startswith(("hand", "draw", "enemy.move")) for f in d)
                    if keep_ops:
                        ops.append(("expect", log, sorted(d)))
                    _sync(sim, sn, log, d)
                    if keep_ops:
                        ops.append(("sync", {GROUP[f.split(".")[0]]: log[GROUP[f.split(".")[0]]] for f in d}))
            try:
                a = _act(sim, ae, b)
            except ValueError as ex:
                row["failed"] = dict(step=row["actions"], turn=turn, why=str(ex)[:120], after=_label(ae))
                break
            if keep_ops:
                ops.append(("apply", a))
            row["actions"] += 1
            last, moves = _label(ae), []
            resolve(picks.get(starts[i], []))
        b.apply(e)
    if not row["failed"]:
        row["end"] = dict(log=(end or {}).get("result"), sim={1: "victory", -1: "loss"}.get(sim.outcome(), "ongoing"))
        row["rng_end"] = dict(sim.rng_counters())
    row["fields"] = dict(row["fields"])
    return row, ((pre, ops) if keep_ops else None)


def _act(sim, e, b):
    """the human's action on the sim -> its Sim.apply JSON; ValueError when the sim cannot take it"""
    t = e["t"]
    if t == "end_turn":
        sim.end_turn()
        return {"end_turn": True}
    sn = json.loads(sim.snapshot())
    tgt = None
    if e.get("target_cid") in b.en:
        cid = e["target_cid"]
        rank = [c for c, x in b.en.items() if x["id"] == b.en[cid]["id"] and (x["alive"] or c == cid)].index(cid)
        same = [j for j, y in enumerate(sn["enemies"]) if y["id"] == b.en[cid]["id"] and y["alive"]]
        if rank >= len(same):
            raise ValueError(f"target {b.en[cid]['id']} not in sim")
        tgt = same[rank]
    if t == "potion_start":
        slot = next((s for s, p in sim.belt() if p == e["id"]), None)
        if slot is None:
            raise ValueError(f"potion {e['id']} not on the sim belt")
        try:
            sim.use_potion(slot, tgt)
        except ValueError:
            sim.use_potion(slot, None)
            tgt = None
        return {"use_potion": {"slot": slot, "target": tgt}}
    up, hand = int(e.get("up") or 0), sn["hand"]
    cand = [j for j, c in enumerate(hand) if c["id"] == e["id"] and c["upgrade"] == up] or [j for j, c in enumerate(hand) if c["id"] == e["id"]]
    if not cand:
        raise ValueError(f"card {e['id']} not in sim hand")
    err = None
    for j in sorted(cand, key=lambda j: hand[j].get("cost") != e.get("cost_value")):
        for tg in dict.fromkeys([tgt, None]):
            try:
                sim.play(j, tg)
                return {"play": {"hand_pos": j, "target": tg}}
            except ValueError as ex:
                err = err or ex
    raise ValueError(f"play {e['id']}: {err}")




def _first_attempts(path):
    seen = set()
    for sc, st, evs, end in fights(path):
        if st.get("combat_id") not in seen:
            seen.add(st.get("combat_id"))
            yield sc, st, evs, end


def fidelity_replay(path):
    rows = []
    for k, (sc, st, evs, end) in enumerate(_first_attempts(path)):
        if sc["character"] not in CHARS:
            break
        if "rng_state" not in st or not any(e["t"] == "draw_order" for e in evs):
            row = dict(enc=st["encounter"], char=sc["character"], error="replay format: no rng_state / draw_order", fields={}, divs=[], relics=[])
        else:
            try:
                row, _ = replay_fight(sc, st, evs, end)
            except (KeyError, TypeError, IndexError, ValueError) as ex:
                row = dict(enc=st["encounter"], char=sc["character"], error=f"reconstruction crash: {ex!r}"[:160], fields={}, divs=[], relics=[])
        row.update(run=os.path.basename(path)[:-9], k=k, plays=dict(collections.Counter(e["id"] for e in evs if e["t"] == "play" and not e.get("auto"))))
        rows.append(row)
    return rows


def _clean(src, character):
    idx = [json.loads(l) for l in open(os.path.join(src, "index.jsonl"), encoding="utf-8")]
    return [(os.path.join(src, x["run_hash"] + ".jsonl.gz"), x) for x in idx if x.get("clean") and (character is None or x.get("character") == character)
            and os.path.exists(os.path.join(src, x["run_hash"] + ".jsonl.gz"))]


def repro(path, k, field):
    """the ops of fight k of a replay up to the first difference in `field` (scenario + actions / syncs / expected states)"""
    sc, st, evs, end = list(_first_attempts(path))[k]
    _, (pre, ops) = replay_fight(sc, st, evs, end, keep_ops=True)
    for n, op in enumerate(ops):
        if op[0] == "expect" and field in op[2]:
            return dict(run=os.path.basename(path)[:-9], k=k, encounter=st["encounter"], field=field, scenario=pre, ops=ops[:n + 1])
    return None


def run_ops(scenario, ops, stop=None, show=False):
    """Sim.exact on a scenario through recorded ops (apply / step / sync / expect); stops before the `stop`-th apply"""
    import sts2
    sim, n = sts2.Sim.exact(json.dumps(scenario)), 0
    for op in ops:
        if op[0] == "apply":
            if n == stop:
                break
            n += 1
            sim.apply(json.dumps(op[1]))
        elif op[0] == "step":
            sim.step(op[1])
        elif op[0] == "sync":
            real = dict(json.loads(sim.snapshot()), **op[1])
            real.pop("relics", None)
            sim.sync(json.dumps(real))
        elif show:
            d = {f: v for f, v in diff(json.loads(sim.snapshot()), op[1]).items() if f in op[2]}
            print(f"expect: {len(d)} of the recorded difference(s)" + "".join(f"\n  {f}: sim {v[0]} log {v[1]}" for f, v in sorted(d.items())))
    return sim


def sim_at(path, k, step):
    """fight k (first attempts) of a replay as a Sim right before the human's action number `step`: logged RNG, the human's actions, the log's
    state synced in wherever the sim differed (a mid-fight search / collection root; draw order and RNG are hidden in-game)"""
    sc, st, evs, end = list(_first_attempts(path))[k]
    _, (pre, ops) = replay_fight(sc, st, evs, end, keep_ops=True)
    return run_ops(pre, ops, stop=step)


def run_repro(file):
    r = json.load(open(file, encoding="utf-8"))
    print(f"{r['run']} fight {r['k']} {r['encounter']}: first {r['field']}")
    run_ops(r["scenario"], r["ops"], show=True)


BLOCKING = ("TEST_SUBJECT_BOSS", "WATERFALL_GIANT_BOSS", "QUEEN_BOSS", "AEONGLASS_BOSS")


def _table(head, rows):
    return "| " + " | ".join(head) + " |\n|" + "---|" * len(head) + "\n" + "".join("| " + " | ".join(str(c) for c in r) + " |\n" for r in rows)


def report(rows, meta, out):
    """replay_fidelity.md: coverage, first divergence per fight, per field / encounter / card / power / relic, RNG consumption between fights; repros"""
    C = collections.Counter
    built = [r for r in rows if "error" not in r]
    exact = [r for r in built if not r["fields"] and not r["failed"] and (r["end"] or {}).get("log") in (None, (r["end"] or {}).get("sim"))]
    first, steps, after_card, plays, by_enc, by_rel, fails = C(), C(), C(), C(), collections.defaultdict(C), collections.defaultdict(C), C()
    ex_first, clus, clus_enc = {}, C(), collections.defaultdict(C)
    for r in built:
        plays.update(r.get("plays") or {})
        divs = [d for d in r["divs"] if not d["cascade"]]
        if r["divs"]:
            f = field_kind(r["divs"][0]["field"])
            first[f] += 1
            clus[f, r["divs"][0]["after"]] += 1
            clus_enc[f, r["divs"][0]["after"]][r["enc"]] += 1
            ex_first.setdefault(f, (meta[r["run"]][0], r["k"], r["divs"][0]))
        for d in divs:
            steps[field_kind(d["field"])] += 1
            if d["after"] not in ("start", "end_turn"):
                after_card[d["after"]] += 1
        e = by_enc[r["enc"]]
        e["fights"] += 1
        e["exact"] += r in exact
        e["fail"] += bool(r["failed"])
        if r["divs"]:
            e["first:" + field_kind(r["divs"][0]["field"])] += 1
        if r["failed"]:
            fails[re.sub(r"[A-Z][A-Z0-9_]{3,}", "X", r["failed"]["why"])[:60]] += 1
        for rel in set(r["relics"]):
            by_rel[rel]["fights"] += 1
            by_rel[rel]["div"] += bool(r["fields"])
            by_rel[rel]["start"] += any(d["after"] == "start" for d in r["divs"])
    chars = C(r["char"] for r in built)
    n_start = max(1, sum(any(d["after"] == "start" for d in r["divs"]) for r in built))
    L = [f"# Replay fidelity (spire-codex A10 v0.111.0 replays, Sim.exact on logged RNG)\n",
         f"clean replays {len(meta)}; first-attempt fights {len(rows)}; built {len(built)}; unported / errors {len(rows) - len(built)}; "
         f"action not takeable {sum(bool(r['failed']) for r in built)}; exact (no divergence, same outcome) {len(exact)} ({len(exact) / max(1, len(built)):.3f})\n",
         _table(["character", "fights", "exact", "share"], [(c, n, sum(r['char'] == c for r in exact), f"{sum(r['char'] == c for r in exact) / n:.3f}") for c, n in chars.most_common()]),
         "\n## Opening state (logged counters minus the sim's own init draws)\n",
         f"post-init counter misses the log: {dict(C(k for r in built for k in r.get('init_rng_miss') or []))}; opening hand differs: "
         f"{sum(any(d['field'] == 'hand' and d['after'] == 'start' for d in r['divs']) for r in built)} of {len(built)}\n",
         "\n## Unported content (scenario refused)\n", _table(["id", "fights"], C(r.get("unported") or r.get("error", "")[:60] for r in rows if "error" in r).most_common(25)),
         "\n## Action not takeable (sim refused the human's action)\n", _table(["reason", "fights"], fails.most_common(15)),
         "\n## First divergence per fight\n", _table(["field", "fights", "example (run k: step turn after: sim vs log)"],
                                                       [(f, n, f"{os.path.basename(ex_first[f][0])[:16]} {ex_first[f][1]}: {ex_first[f][2]['step']} t{ex_first[f][2]['turn']} {ex_first[f][2]['after']}: {json.dumps(ex_first[f][2]['sim'])[:60]} vs {json.dumps(ex_first[f][2]['log'])[:60]}")
                                                        for f, n in first.most_common(40)]),
         "\n## First divergence clusters (field x what preceded it: card / potion played, end_turn = enemy turn + turn start, start = combat start)\n",
         _table(["field", "after", "fights", "encounters"], [(f, a, n, ", ".join(f"{e} {m}" for e, m in clus_enc[f, a].most_common(3))) for (f, a), n in clus.most_common(50)]),
         "\n## Divergent steps by field (cascades after a draw / move divergence excluded)\n", _table(["field", "steps"], steps.most_common(60)),
         "\n## Per encounter (blocking: Test Subject, Waterfall Giant, act-3 bosses)\n",
         _table(["encounter", "fights", "exact", "fail", "top first divergence", "blocking"],
                [(k, v["fights"], v["exact"], v["fail"], max(((f[6:], n) for f, n in v.items() if f.startswith("first:")), key=lambda x: x[1], default=("", 0)), "BLOCKING" if k in BLOCKING else "")
                 for k, v in sorted(by_enc.items(), key=lambda kv: -(kv[1]["fights"] - kv[1]["exact"]))]),
         "\n## Per card: divergences right after a play / plays\n",
         _table(["card", "div", "plays", "rate"], [(c, n, plays[c], f"{n / max(1, plays[c]):.3f}") for c, n in sorted(after_card.items(), key=lambda kv: -kv[1])[:40]]),
         "\n## Relic lift on combat-start divergences (share of start-divergent fights holding the relic / share of all fights; >= 15 start-divergent fights)\n",
         _table(["relic", "lift", "start-divergent fights", "fights"], [(k, f"{(v['start'] / n_start) / (v['fights'] / len(built)):.2f}", v["start"], v["fights"])
                                                                    for k, v in sorted(by_rel.items(), key=lambda kv: -(kv[1]["start"] / kv[1]["fights"])) if v["start"] >= 15][:30]),
         "\n## Per relic: fights with any divergence / with a start divergence (baseline all fights)\n",
         _table(["relic", "fights", "div share", "start div share"],
                [("ALL", len(built), f"{sum(bool(r['fields']) for r in built) / max(1, len(built)):.3f}", f"{sum(any(d['after'] == 'start' for d in r['divs']) for r in built) / max(1, len(built)):.3f}")] +
                [(k, v["fights"], f"{v['div'] / v['fights']:.3f}", f"{v['start'] / v['fights']:.3f}") for k, v in sorted(by_rel.items(), key=lambda kv: -kv[1]["start"]) if v["fights"] >= 10][:40])]
    by_run = collections.defaultdict(list)
    for r in built:
        by_run[r["run"]].append(r)
    cons = collections.defaultdict(C)
    for rs in by_run.values():
        rs.sort(key=lambda r: r["k"])
        for x, y in zip(rs, rs[1:]):
            if x in exact and x.get("rng_end") and y.get("rng_pre") and y["k"] == x["k"] + 1:
                for k, v in x["rng_end"].items():
                    cons[k]["pairs"] += 1
                    cons[k]["match"] += y["rng_pre"].get(k) == v
    L += ["\n## RNG consumption over exact fights (sim counter at the end == the next fight's start; non-combat draws between fights also count as misses)\n",
          _table(["stream", "pairs", "match"], [(k, v["pairs"], v["match"]) for k, v in cons.items()])]
    rdir = out + "_repro"
    os.makedirs(rdir, exist_ok=True)
    for f, (path, k, d) in ex_first.items():
        r = repro(path, k, d["field"])
        if r:
            json.dump(r, open(os.path.join(rdir, re.sub(r"[^A-Za-z0-9_.]", "_", f) + ".json"), "w", encoding="utf-8"))
    open(out + ".md", "w", encoding="utf-8").write("".join(L))
    print("".join(L[:3]))


def corpus(files, out):
    """task-4 counts without the sim: human decision states per encounter (first attempts), macro decisions with their option sets"""
    C = collections.Counter
    enc, macro, strong_enc = collections.defaultdict(C), collections.defaultdict(C), C()
    for path, x in files:
        for sc, st, evs, end in _first_attempts(path):
            e = enc[st["encounter"]]
            e["fights"] += 1
            e["wins"] += (end or {}).get("result") == "victory"
            e["states"] += len(human_actions(evs)) + sum(1 for v in evs if v["t"] == "pick")
            e["turns"] += sum(1 for v in evs if v["t"] == "turn" and v.get("side") == "player")
            strong_enc[st["encounter"]] += x.get("strong", False)
        for v in events(path):
            if v["t"] == "decision":
                m = macro[v.get("decision_type")]
                m["n"] += 1
                m["options"] += v.get("n_presented") or len(v.get("options") or [])
            elif v["t"] == "shop":
                m = macro["shop (inventory + removal)"]
                m["n"] += 1
                m["options"] += sum(len(v.get(k) or []) for k in ("cards", "relics", "potions")) + bool(v.get("removal_stocked"))
    res = dict(encounters={k: dict(v, strong_fights=strong_enc[k]) for k, v in enc.items()}, macro={k: dict(v) for k, v in macro.items()})
    json.dump(res, open(out, "w", encoding="utf-8"), indent=1)
    print(_table(["encounter", "fights", "wins", "states", "turns", "strong fights"],
                 [(k, v["fights"], v["wins"], v["states"], v["turns"], strong_enc[k]) for k, v in sorted(enc.items(), key=lambda kv: -kv[1]["states"])]))
    print(_table(["decision", "n", "mean options"], [(k, v["n"], f"{v['options'] / v['n']:.1f}") for k, v in sorted(macro.items(), key=lambda kv: -kv[1]["n"])]))


def main():
    import argparse, multiprocessing
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["fidelity", "corpus", "repro"])
    ap.add_argument("src")
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "review", "replay_fidelity"))
    ap.add_argument("--character")
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    if a.cmd == "repro":
        return run_repro(a.src)
    files = _clean(a.src, a.character)[: a.limit or None]
    if a.cmd == "corpus":
        return corpus(files, a.out)
    os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
    rows_path = a.out + ".jsonl"
    done = set()
    if os.path.exists(rows_path):
        done = {json.loads(l)["run"] for l in open(rows_path, encoding="utf-8")}
    todo = [p for p, _ in files if os.path.basename(p)[:-9] not in done]
    print(f"{len(files)} clean replays, {len(todo)} to run", flush=True)
    with multiprocessing.Pool(a.workers) as pool, open(rows_path, "a", encoding="utf-8") as f:
        for i, rows in enumerate(pool.imap_unordered(fidelity_replay, todo, chunksize=4)):
            for r in rows:
                f.write(json.dumps(r) + "\n")
            if (i + 1) % 200 == 0:
                print(f"  {i + 1}/{len(todo)}", flush=True)
    meta = {os.path.basename(p)[:-9]: (p, x) for p, x in files}
    report([json.loads(l) for l in open(rows_path, encoding="utf-8") if json.loads(l)["run"] in meta], meta, a.out)


if __name__ == "__main__":
    sys.path.insert(0, ROOT)
    main()
