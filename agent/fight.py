"""Follow a live fight in the simulator (the micro layer's view of the game).

`Replayer` builds `sts2.Sim` from the bridge's fight-start scenario, applies the actions the bridge logged (oracle script vocabulary) and after each one
puts the simulator's *visible* state on the observed one (`Sim.sync`). Hidden information is the simulator's own random sample (it never sees the real
shuffle or RNG); enemy turns are resampled until the enemies' announced intents match what the game shows, which is observed information.

  r = Replayer(fight)            # fight = json.loads(bridge "fight") at the start of a combat
  r.advance(fight)               # after every real action
  r.sim                          # the aligned simulator state; r.stats / r.examples = how often the prediction was wrong
"""
import collections, json, random

import sts2

# pre-sync differences under these prefixes are expected whenever cards were drawn (the simulator drew other cards than the real game)
RANDOM_PREFIXES = (".hand", ".draw", ".discard", ".exhaust")


def _alive(enemies):
    return [e for e in enemies if e.get("alive", True)]


def intents_of(state):
    """What the game announces for the enemies' next moves: (move id, intents) per living enemy."""
    out = []
    for e in _alive(state["enemies"]):
        its = tuple((i["type"], i.get("damage"), i.get("hits")) for i in e.get("intents", []))
        out.append((e.get("next_move"), its))
    return out


def category(line):
    """First path components of a diff line, e.g. '.enemies[0].hp'."""
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
        # The spawn variants and the opening moves are random rolls the screen shows: resample the fight start until it agrees with what is visible.
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

    # ------------------------------------------------------------------ bookkeeping

    def _note(self, key, text):
        self.stats[key] += 1
        if len(self.examples[key]) < 3:
            self.examples[key].append(text)

    def _compare_and_sync(self, state, action):
        """Pre-sync diff (what the simulator predicted wrongly), then align."""
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
        for k in ("created", "from_discard", "from_exhaust"):
            if rep[k]:
                self._note("sync " + k, f"{action or 'start'} [{getattr(self, '_played', '')}]: {rep}")
        for n in rep["notes"]:
            self._note("sync note", n)
        post = self.sim.diff(json.dumps(state))
        for line in post:
            self._note("residual " + category(line), f"{action or 'start'} [{getattr(self, '_played', '')}]: {line[:200]}")
        if self.sim.missing():
            self._note("missing content", self.sim.missing())

    # ------------------------------------------------------------------ actions

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
                self.sim = s
                return
            got = intents_of(json.loads(s.snapshot()))
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
        """Id of the card a `play` action plays (read from the simulator's hand before the action), for the divergence notes."""
        a = json.loads(act)
        if "play" not in a:
            return ""
        try:
            return json.loads(self.sim.snapshot())["hand"][a["play"]["hand_pos"]]["id"]
        except Exception:  # noqa: BLE001
            return ""

    def _map_potion(self, act):
        """The scenario lists potions in slot order and the simulator packs them into slots 0..n-1; the game keeps their real slots (a lone potion in the
        second slot is slot 1). Translate a logged `use_potion` slot to the simulator's."""
        a = json.loads(act)
        if "use_potion" not in a:
            return act
        slots = sorted(p.get("slot", i) for i, p in enumerate(self.scenario["potions"]))
        s = a["use_potion"]["slot"]
        if s in slots:
            a["use_potion"]["slot"] = slots.index(s)
        return json.dumps(a)

    def _repair_target(self, act, state):
        """A logged `play` without a target (the bridge could not find the enemy any more, e.g. it died from the very hit): try every legal target of that
        card and keep the one whose result matches the observed state best. Returns True if the action was applied."""
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

    def advance(self, fight):
        log = fight["log"]
        states = fight.get("states")
        while self.applied < len(log):
            i = self.applied
            act = log[i] if isinstance(log[i], str) else json.dumps(log[i])
            last = i == len(log) - 1
            # the state observed after this action (the bridge records one after every action it settles); the current one for the latest
            state = states[i + 1] if states and states[i + 1] is not None else (fight["state"] if last else None)
            try:
                if '"end_turn"' in act:
                    self._end_turn(state)
                else:
                    act = self._map_potion(act)
                    self._played = self._card_at(act)
                    try:
                        self.sim.apply(act)
                    except Exception:  # noqa: BLE001
                        if not self._repair_target(act, state):
                            raise
            except Exception as e:  # noqa: BLE001
                self.errors.append(f"{act}: {e}")
                self._note("action failed", f"{act}: {e}")
                self.applied = len(log)
                return False
            self.applied += 1
            if state is not None:
                self._compare_and_sync(state, act)
        return True
