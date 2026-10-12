import collections, itertools, json, random

import sts2


RANDOM_PREFIXES = (".hand", ".draw", ".discard", ".exhaust")


def _alive(enemies):
    return [e for e in enemies if e.get("alive", True)]


def _cards(state, pile):
    return collections.Counter((c["id"], c.get("upgrade", 0)) for c in state.get(pile) or [])


def _left_draw(before, after):
    # the cards that left the draw pile between two observed states (drawn or auto-played: Mayhem, Cascade); None when a reshuffle hides it
    if not before or "draw" not in before or "draw" not in after:
        return None
    b, a = _cards(before, "draw"), _cards(after, "draw")
    return None if a - b else list((b - a).elements())


def _wants_choice(state, nxt):
    if nxt is not None:
        return '"choose"' in nxt
    return bool(state.get("play_pile")) or state.get("phase") == "AutoPrePlay"


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


    def _mismatch(self, s, state, real, want_choice):
        # distance of a candidate from the observed state: visible fields, pile contents, a pending selection, a fight the game has not ended
        if s.stage() == "over":
            return 0 if not state.get("combat_in_progress", True) else 1000
        n = sum(1 for line in s.diff(real) if not line.startswith(RANDOM_PREFIXES) and "props.Skin" not in line)
        snap = json.loads(s.snapshot())
        for pile in ("hand", "discard", "exhaust"):
            if pile in state:
                a, b = _cards(snap, pile), _cards(state, pile)
                n += sum(((a - b) + (b - a)).values())
        if want_choice is not None and (s.stage() == "choice") != want_choice:
            n += 5
        return n

    def _candidates(self, base, act, left, tries):
        # hidden-state re-rolls (random targets / effects, pile order); the cards that left the draw pile go on top in a sampled order, so draw-order effects (Mayhem, Cascade) can match
        for k in range(tries):
            s = base.copy()
            s.determinize(self.rng.randrange(1 << 62))
            if left:
                order = list(left)
                self.rng.shuffle(order)
                s.stack_draw(order)
            try:
                s.apply(act)
            except Exception:  # noqa: BLE001
                continue
            yield k, s

    def _rematch(self, base, act, state, before, want_choice, tries=96):
        real = json.dumps(state)
        best = start = self._mismatch(self.sim, state, real, want_choice)
        if best == 0:
            return
        for _k, s in self._candidates(base, act, _left_draw(before, state), tries):
            n = self._mismatch(s, state, real, want_choice)
            if n < best:
                self.sim, best = s, n
                if n == 0:
                    break
        if best < start:
            self.stats["random_effect_rerolls"] += 1
        if self.sim.stage() == "over" and state.get("combat_in_progress", True):
            # no world found keeps the fight going (hidden per-turn state, e.g. Hardened Shell's damage taken): take the game's state over the action
            self.sim = base
            self.resolve_phantom_choice()
            self._note("simulator-only kill", f"{act} [{getattr(self, '_played', '')}]")

    def _end_turn(self, state, before=None, want_choice=None):
        act = '{"end_turn":true}'
        if state is None:
            self.sim.apply(act)
            return
        want, real = intents_of(state), json.dumps(state)
        best = best_key = matched = err = None
        first = self.sim.copy()
        try:
            first.apply(act)
            cands = [(0, first)]
        except Exception as e:  # noqa: BLE001
            cands, err = [], f"end_turn: {e}"
        for k, s in itertools.chain(cands, ((k + 1, s) for k, s in self._candidates(self.sim, act, _left_draw(before, state), self.tries - 1))):
            if s.stage() == "over" and state.get("combat_in_progress", True):
                self.stats["end_turn_over_rejected"] += 1
                continue
            hit = s.stage() == "over" or intents_of(json.loads(s.snapshot())) == want
            key = (not hit, self._mismatch(s, state, real, want_choice))
            if best_key is None or key < best_key:
                best, best_key = s, key
            if hit and matched is None:
                matched = k
            if key == (False, 0) or (matched is not None and k - matched >= 64):
                break
        if best is None:
            self.errors.append(err or "end_turn: no simulated end of turn keeps the fight going")
            return
        self.sim = best
        if best_key[0]:
            self.stats["end_turn_unmatched"] += 1
            self._note("intent unmatched", f"want {want} got {intents_of(json.loads(best.snapshot()))}")
        else:
            self.stats["end_turn_tries"] += matched + 1
            self.stats["end_turn_matched"] += 1

    def _card_at(self, act):
        a = json.loads(act)
        if "play" not in a:
            return ""
        try:
            return json.loads(self.sim.snapshot())["hand"][a["play"]["hand_pos"]]["id"]
        except Exception:  # noqa: BLE001
            return ""

    def _map_choose(self, act, before):
        # the game's index is into its own list: the source pile in pile order (hand, discard, exhaust), the draw pile sorted (rarity, id) like the
        # simulator's view; the simulator's pick is into its candidates in its own pile order, which can differ: map by card
        import re
        a = json.loads(act)
        if "choose" not in a or not before or self.sim.stage() != "choice":
            return act
        opts = sorted((self.sim.pick_game_index(int(m.group(1))), int(m.group(1)), m.group(2)) for _, t in self.sim.legal() for m in [re.match(r"pick (\d+) \((\w+)\)", t)] if m)
        names = collections.Counter(n for _, _, n in opts)
        game = next((seq for pile in ("hand", "discard", "exhaust") for seq in [[c["id"] for c in before.get(pile) or [] if c["id"] in names]] if collections.Counter(seq) == names), None)
        if game is None:
            game = [n for _, _, n in sorted(opts, key=lambda o: o[1])]
        picks = []
        for k in a["choose"]:
            g = next((g for g, _, n in opts if k < len(game) and n == game[k] and g not in picks), None)
            if g is None:
                return act
            picks.append(g)
        if picks != a["choose"]:
            self.stats["choose_remapped"] += 1
            a["choose"] = picks
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
            before = states[i] if states else None
            nxt = None if last else log[i + 1] if isinstance(log[i + 1], str) else json.dumps(log[i + 1])
            want_choice = None if state is None else _wants_choice(state, nxt)
            if '"choose"' not in act and self.sim.stage() == "choice":
                self.resolve_phantom_choice(before, "phantom choice before a logged action")
            if '"choose"' in act and self.sim.stage() != "choice":
                self._note("game-only selection", f"{act} [{getattr(self, '_played', '')}]")
                self.applied += 1
                if state is not None:
                    self._sync(state, act)
                continue
            try:
                if '"end_turn"' in act:
                    self._end_turn(state, before, want_choice)
                else:
                    act = self._map_choose(act, before)
                    self._played = self._card_at(act)
                    base = self.sim.copy()
                    try:
                        self.sim.apply(act)
                    except Exception:  # noqa: BLE001
                        if not self._repair_target(act, state):
                            raise
                    if state is not None:
                        self._rematch(base, act, state, before, want_choice)
            except Exception as e:  # noqa: BLE001
                self.errors.append(f"{act}: {e}")
                self._note("action failed", f"{act}: {e}")
                self.applied = len(log)
                return False
            self.applied += 1
            if state is not None:
                self._compare_and_sync(state, act)
        return True
