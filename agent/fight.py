import collections, json, random

import sts2


RANDOM_PREFIXES = (".hand", ".draw", ".discard", ".exhaust")


def _alive(enemies):
    return [e for e in enemies if e.get("alive", True)]


def _foes(state):
    return [(e["id"], e["hp"]) for e in _alive(state["enemies"])]


def intents_of(state):
    out = []
    for e in _alive(state["enemies"]):
        its = tuple((i["type"], i.get("damage"), i.get("hits")) for i in e.get("intents", []))
        out.append((e.get("next_move"), its))
    return out


def category(line):
    path = line.split(":", 1)[0]
    parts = []
    for tok in path.replace("[", ".[").split("."):
        if tok:
            parts.append(tok)
        if len(parts) == 3 or (parts and parts[0] in ("energy", "turn", "round", "phase", "stars", "potions", "relics")):
            break
    out = ""
    for p in parts:
        out += p if p.startswith("[") else "." + p
    return out


class Replayer:
    def __init__(self, fight, seed=None, tries=300, start_tries=400):
        self.rng = random.Random(seed)
        self.tries = tries
        self.applied = 0
        self.stats = collections.Counter()
        self.examples = collections.defaultdict(list)
        self.errors = []
        state = (fight.get("states") or [None])[0] or (fight["state"] if not fight["log"] else None)
        if state is None:
            raise ValueError("the fight's opening state was not observed (the bridge records it when the first state is read)")
        want_ids = [e["id"] for e in state["enemies"]]
        want_intents = intents_of(state)
        best = None
        for k in range(start_tries):
            sc = dict(fight["scenario"])
            sc["seed"] = f"replay{self.rng.randrange(1 << 40)}"
            sim = sts2.Sim(json.dumps(sc), self.rng.randrange(1 << 62))
            snap = json.loads(sim.snapshot())
            ids_ok = [e["id"] for e in snap["enemies"]] == want_ids
            if ids_ok and best is None:
                best = sim
            if ids_ok and intents_of(snap) == want_intents:
                best = sim
                self.stats["start_tries"] += k + 1
                break
        else:
            self.stats["start_unmatched"] += 1
            self._note("start unmatched", f"want {want_ids} {want_intents}")
        if best is None:
            best = sim
        self.sim = best
        self.scenario = fight["scenario"]
        self._sync(state, None)


    def _note(self, key, text):
        self.stats[key] += 1
        if len(self.examples[key]) < 3:
            self.examples[key].append(text)

    def _compare_and_sync(self, state, action):
        if self.sim.stage() != "play":
            return
        pre = self.sim.diff(json.dumps(state))
        for line in pre:
            if line.startswith(RANDOM_PREFIXES):
                self.stats["random_pile_diffs"] += 1
                continue
            cat = category(line)
            self._note("diff " + cat, f"{action or 'start'} [{getattr(self, '_played', '')}]: {line[:200]}")
        self._sync(state, action)

    def _sync(self, state, action):
        rep = json.loads(self.sim.sync(json.dumps(state)))
        for k in ("created", "from_discard", "from_exhaust", "powers", "relics", "potions"):
            if rep.get(k):
                self._note("sync " + k, f"{action or 'start'} [{getattr(self, '_played', '')}]: {rep}")
        for n in rep["notes"]:
            self._note("sync note", n)
        post = self.sim.diff(json.dumps(state))
        for line in post:
            self._note("residual " + category(line), f"{action or 'start'} [{getattr(self, '_played', '')}]: {line[:200]}")
        if self.sim.missing():
            self._note("missing content", self.sim.missing())


    def _reroll(self, base, act, state):
        # random targets/effects (Serpent Form, Juggernaut...): re-roll until the enemies match the game
        for _k in range(64):
            s = base.copy()
            s.determinize(self.rng.randrange(1 << 62))
            try:
                s.apply(act)
            except Exception:  # noqa: BLE001
                continue
            if _foes(json.loads(s.snapshot())) == _foes(state):
                self.sim = s
                self.stats["random_effect_rerolls"] += 1
                return

    def _end_turn(self, state):
        act = '{"end_turn":true}'
        if state is None:
            self.sim.apply(act)
            return
        want = intents_of(state)
        first = None
        for k in range(self.tries):
            s = self.sim.copy()
            if k:
                s.determinize(self.rng.randrange(1 << 62))
            try:
                s.apply(act)
            except Exception as e:  # noqa: BLE001
                self.errors.append(f"end_turn: {e}")
                continue
            if s.stage() == "over":
                if not state.get("combat_in_progress", True):
                    self.sim = s
                    return
                self.stats["end_turn_over_rejected"] += 1
                continue
            snap = json.loads(s.snapshot())
            got = intents_of(snap)
            if first is None:
                first = s
            if got == want:
                self.sim = s
                self.stats["end_turn_tries"] += k + 1
                self.stats["end_turn_matched"] += 1
                return
        self.stats["end_turn_unmatched"] += 1
        self._note("intent unmatched", f"want {want} got {intents_of(json.loads(first.snapshot())) if first else '?'}")
        if first is not None:
            self.sim = first

    def _card_at(self, act):
        a = json.loads(act)
        if "play" not in a:
            return ""
        try:
            return json.loads(self.sim.snapshot())["hand"][a["play"]["hand_pos"]]["id"]
        except Exception:  # noqa: BLE001
            return ""

    def _map_choose(self, act, before):
        import re
        a = json.loads(act)
        if "choose" not in a or len(a["choose"]) != 1 or not before or self.sim.stage() != "choice":
            return act
        opts = [(i, m.group(2)) for _, t in self.sim.legal() for m in [re.match(r"pick (\d+) \((\w+)\)", t)] if m for i in [int(m.group(1))]]
        hand = [c["id"] for c in before.get("hand", [])]
        snap_hand = sorted(c["id"] for c in json.loads(self.sim.snapshot())["hand"])
        k = a["choose"][0]
        if sorted(o for _, o in opts) != snap_hand or k >= len(hand):
            return act
        if opts[k][1] == hand[k]:
            return act
        same = [i for i, o in opts if o == hand[k]]
        if not same:
            return act
        self.stats["choose_remapped"] += 1
        a["choose"] = [same[0]]
        return json.dumps(a)

    def _repair_target(self, act, state):
        import re
        a = json.loads(act)
        if "play" not in a or "target" in a["play"] or state is None:
            return False
        pos = a["play"]["hand_pos"]
        targets = [int(m.group(2)) for _, t in self.sim.legal() for m in [re.match(r"play \S+ #(\d+) -> e(\d+)", t)] if m and int(m.group(1)) == pos]
        best, best_n = None, None
        for k in targets:
            c = self.sim.copy()
            try:
                c.apply(json.dumps({"play": {"hand_pos": pos, "target": k}}))
            except Exception:  # noqa: BLE001
                continue
            n = sum(1 for line in c.diff(json.dumps(state)) if not line.startswith(RANDOM_PREFIXES) and "props.Skin" not in line)
            if best is None or n < best_n:
                best, best_n = c, n
        if best is None:
            return False
        self.sim = best
        self.stats["target_repaired"] += 1
        return True

    def resolve_phantom_choice(self, state=None, why="phantom choice"):
        n = 0
        while self.sim.stage() == "choice" and n < 40:
            legal = self.sim.legal()
            if not legal:
                break
            a = next((i for i, t in legal if t == "confirm"), legal[0][0])
            self.sim.step(a)
            n += 1
        if n:
            self._note("phantom choice", f"{why}: {n} answer(s) [{getattr(self, '_played', '')}]")
            if state is not None:
                self._sync(state, why)
        return n

    def advance(self, fight):
        log = fight["log"]
        states = fight.get("states")
        while self.applied < len(log):
            i = self.applied
            act = log[i] if isinstance(log[i], str) else json.dumps(log[i])
            last = i == len(log) - 1
            state = states[i + 1] if states and states[i + 1] is not None else (fight["state"] if last else None)
            if '"choose"' not in act and self.sim.stage() == "choice":
                self.resolve_phantom_choice(states[i] if states else None, "phantom choice before a logged action")
            if '"choose"' in act and self.sim.stage() != "choice":
                self._note("game-only selection", f"{act} [{getattr(self, '_played', '')}]")
                self.applied += 1
                if state is not None:
                    self._sync(state, act)
                continue
            try:
                if '"end_turn"' in act:
                    self._end_turn(state)
                else:
                    act = self._map_choose(act, states[i] if states else None)
                    self._played = self._card_at(act)
                    base = self.sim.copy()
                    try:
                        self.sim.apply(act)
                    except Exception:  # noqa: BLE001
                        if not self._repair_target(act, state):
                            raise
                    if state is not None and [i for i, _h in _foes(json.loads(self.sim.snapshot()))] != [i for i, _h in _foes(state)]:
                        self._reroll(base, act, state)
            except Exception as e:  # noqa: BLE001
                self.errors.append(f"{act}: {e}")
                self._note("action failed", f"{act}: {e}")
                self.applied = len(log)
                return False
            self.applied += 1
            if state is not None:
                self._compare_and_sync(state, act)
        return True
