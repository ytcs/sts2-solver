import json
import os
import re

from agent import screen as scr
from agent.bridge import call

PINNED_BUILD = "v0.111.0"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
AUTO = ("continue dialogue", "proceed", "proceed (skip the rest)", "open chest", "continue")
ROOM_OF_ENCOUNTER = (("_WEAK", "Monster"), ("_NORMAL", "Monster"), ("_ELITE", "Elite"), ("_BOSS", "Boss"))
MAP_CODES = {"M": "Monster", "E": "Elite", "R": "RestSite", "$": "Shop", "T": "Treasure", "?": "Unknown", "B": "Boss", "A": "Ancient"}
ROOM_ALLOWS = dict(Monster={"Monster", "Unknown"}, Elite={"Elite"}, Boss={"Boss"}, RestSite={"RestSite"}, Shop={"Shop", "Unknown"},
                   Treasure={"Treasure", "Unknown"}, Event={"Unknown"}, Ancient={"Ancient"})
SCREEN_ROOM = dict(EVENT="Event", RESTSITE="RestSite", SHOP="Shop", TREASURE="Treasure")


def load(path):
    with open(path, encoding="utf-8") as f:
        if not path.endswith(".jsonl"):
            return json.load(f)
        lines = [json.loads(x) for x in f if x.strip()]
    return dict(lines[0], steps=lines[1:])


def build_ok(rec):
    b = str(rec.get("build", "")).split()
    return bool(b) and b[0] == PINNED_BUILD


def creator_dir(rec):
    return os.path.join(ROOT, "data", "expert", rec["video"]["creator"])


def base(name):
    s = re.sub(r"\+\d*$", "", str(name).strip())
    s = re.sub(r"[^A-Z0-9]+", "_", s.upper().replace("'", "").replace("’", "")).strip("_")
    return re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", s)


def token(tok, aliases):
    tok = str(tok)
    pos = None
    if "@" in tok:
        tok, p = tok.split("@")
        pos = int(p)
    ench = tok.endswith("*")
    tok = tok.rstrip("*")
    up = int(tok.endswith("+"))
    b = tok.rstrip("+")
    return aliases.get(b, b), up, ench, pos


def label_card(label):
    m = re.match(r"^(?:\(x\) )?(.+?)\([^()]*\)", label)
    if not m:
        return None, 0
    title = m.group(1).strip()
    return base(title), int(bool(re.search(r"\+\d*$", title)))


def room_of(step):
    if step.get("room"):
        return step["room"]
    enc = (step.get("fight") or {}).get("encounter") or step.get("encounter") or ""
    for suf, room in ROOM_OF_ENCOUNTER:
        if enc.endswith(suf):
            return room
    return SCREEN_ROOM.get(step.get("screen"))


def fight_actions(spec, aliases, built=None):
    pots = {p.get("slot", i): p["id"] for i, p in enumerate((spec.get("scenario") or {}).get("potions", []))}
    states = built["fight"].get("states") if built else None
    out, i = [], 0
    for ti, t in enumerate(spec["turns"]):
        times = t.get("times") or []
        for ai, a in enumerate(t["acts"]):
            a = [x for x in a if not isinstance(x, dict)]
            act = dict(fight=spec["id"], i=i, t=times[ai] if ai < len(times) else None, turn=ti + 1)
            if a[0] == "p":
                cid, up, ench, pos = token(a[1], aliases)
                act.update(kind="play", card=cid, upgrade=up, enchanted=ench, pos=pos, target=a[2] if len(a) > 2 else None)
            elif a[0] == "c":
                act.update(kind="choose", cards=[list(token(x, aliases)[:2]) for x in a[1:]])
            elif a[0] == "pot":
                act.update(kind="potion", slot=a[1], potion=a[3] if len(a) > 3 else pots.get(a[1]), target=a[2] if len(a) > 2 else None)
            elif a[0] == "e":
                act.update(kind="end")
            else:
                raise ValueError(f"{spec['id']} T{ti + 1}.{ai + 1}: unknown act {a[0]!r}")
            if states and act.get("target") is not None and i < len(states) and states[i]:
                es = states[i].get("enemies") or []
                if act["target"] < len(es):
                    act["expect"] = es[act["target"]]["id"]
            out.append(act)
            i += 1
    return out


def built_record(rec, spec_id):
    d = creator_dir(rec)
    for p in (os.path.join(d, "replay", rec["video"]["id"], spec_id + ".json"), os.path.join(d, "fights", spec_id + ".json")):
        if os.path.exists(p):
            return load(p)
    return None


def flatten(rec, built=built_record):
    al = rec.get("aliases", {})
    out = []
    for k, st in enumerate(rec["steps"]):
        if st.get("fight"):
            for a in fight_actions(st["fight"], al, built(rec, st["fight"]["id"]) if built else None):
                out.append(dict(a, step=k, floor=st.get("floor"), screen="COMBAT"))
        elif isinstance(st.get("pick"), list) and st.get("screen") == "REWARDS":
            for j, p in enumerate(st["pick"]):
                out.append(dict(st, pick=p, step=k, sub=j, kind="macro"))
        else:
            out.append(dict(st, step=k, kind="gap" if "gap" in st else "macro"))
    return out


def describe(a):
    k = a["kind"]
    if k == "play":
        return f"play {a['card']}{'+' if a['upgrade'] else ''}{'*' if a['enchanted'] else ''}" + (f" -> e{a['target']}" if a.get("target") is not None else "")
    if k == "choose":
        return "choose " + ", ".join(c + ("+" if u else "") for c, u in a["cards"])
    if k == "potion":
        return f"potion {a.get('potion')} (slot {a['slot']})" + (f" -> e{a['target']}" if a.get("target") is not None else "")
    if k == "end":
        return "end turn"
    if k == "gap":
        return f"GAP: {a['gap']}"
    return f"{a.get('screen', '?')} pick {json.dumps(a.get('pick'))}"


def _find(screen, pred):
    return [int(n) for n, label in scr.options(screen) if pred(label)]


def _label(screen, i):
    return dict((int(n), lb) for n, lb in scr.options(screen)).get(i, "")


def _hand_index(act, hand):
    def ok(c, ench):
        return base(c["id"]) == base(act["card"]) and int(c.get("upgrade", 0) > 0) == act["upgrade"] and bool(c.get("enchantment")) == ench
    cands = [j for j, c in enumerate(hand) if ok(c, act["enchanted"])]
    if not cands and not act["enchanted"]:
        cands = [j for j, c in enumerate(hand) if ok(c, True)]
    if act.get("pos") is not None:
        return act["pos"] if act["pos"] in cands else None
    return cands[0] if cands else None


def _target(act, state, screen):
    t, want = act.get("target"), act.get("expect")
    es = state.get("enemies") or []
    shown = {int(m.group(1)): m.group(2) for line in screen.split("\n") for m in [re.match(r"^e(\d+) (.*)", line)] if m}
    hittable = [(e.get("index", j), e) for j, e in enumerate(es) if e.get("alive", True) and e.get("index", j) in shown
                and "[untargetable]" not in shown[e.get("index", j)]]
    if t is None:
        return (None, None) if len(hittable) <= 1 else (None, "the card needs a target and the record names none")
    at = next((e for i, e in hittable if i == t), None)
    if at is not None and (want is None or at["id"] == want):
        return t, None
    same = [i for i, e in hittable if e["id"] == want]
    if want is not None and len(same) == 1:
        return same[0], None
    return None, f"target e{t}{' (' + want + ')' if want else ''} is not a hittable enemy (shown: {sorted(shown)})"


def combat_command(act, state, screen):
    k = act["kind"]
    if k == "end":
        hit = _find(screen, lambda lb: lb == "end turn")
        return (f"a {hit[0]}", None) if hit else (None, "no `end turn` option")
    if k == "choose":
        return select_command([c for c, _ in act["cards"]], [u for _, u in act["cards"]], screen)
    if scr.kind(screen) != "COMBAT":
        return None, f"{describe(act)}, but the screen is {scr.kind(screen)}"
    if k == "play":
        hand = state.get("hand") or []
        j = _hand_index(act, hand)
        if j is None:
            have = [c["id"] + ("+" if c.get("upgrade") else "") + ("*" if c.get("enchantment") else "") for c in hand]
            return None, f"{describe(act)}: no such card in the game's hand {have}"
        label = _label(screen, j)
        if label.startswith("(x)"):
            return None, f"{describe(act)}: the game marks it unplayable: {label[:60]}"
        if label_card(label)[0] != base(act["card"]):
            return None, f"{describe(act)}: option {j} is `{label[:40]}`"
    elif k == "potion":
        name = base(act.get("potion") or "")
        hit = _find(screen, lambda lb: lb.startswith("potion ") and base(lb[7:].split(":")[0]) == name)
        same = [p_["slot"] for p_ in sorted(state.get("potions") or [], key=lambda p_: p_["slot"]) if base(p_["id"]) == name]
        if len(hit) > 1 and len(hit) == len(same) and act["slot"] in same:
            hit = [hit[same.index(act["slot"])]]  # duplicates (two Potion-Shaped Rocks): options are listed in slot order
        if len(hit) != 1:
            return None, f"{describe(act)}: no single potion option for it"
        j, label = hit[0], _label(screen, hit[0])
    else:
        return None, f"unknown combat action {k}"
    tgt, err = _target(act, state, screen) if label.endswith("->e") else (None, None)
    if err:
        return None, f"{describe(act)}: {err}"
    return f"a {j}" + (f" e{tgt}" if tgt is not None else ""), None


def to_bridge(act, cmd):
    toks = cmd.split()[1:]
    if act["kind"] == "end":
        return {"end_turn": True}
    if act["kind"] == "choose":
        return {"choose": [] if toks == ["-"] else [int(x) for x in toks]}
    tgt = next((int(x[1:]) for x in toks[1:] if x.startswith("e")), None)
    if act["kind"] == "play":
        return {"play": dict(hand_pos=int(toks[0]), **({} if tgt is None else {"target": tgt}))}
    return {"use_potion": dict(slot=act["slot"], **({} if tgt is None else {"target": tgt}))}


def select_command(names, ups, screen):
    if scr.kind(screen) != "SELECT":
        return None, f"a selection, but the screen is {scr.kind(screen)}"
    opts = [(int(n), label_card(lb)) for n, lb in scr.options(screen)]
    used, idx = set(), []
    for name, up in zip(names, ups):
        j = next((n for n, (b, u) in opts if n not in used and b == base(name) and u == up), None)
        if j is None:
            return None, f"selection: {name}{'+' if up else ''} not offered ({[b for _, (b, _) in opts]})"
        used.add(j)
        idx.append(j)
    m = re.match(r"SELECT (\d+)(?:-(\d+))?", screen)
    if m and not int(m.group(1)) <= len(idx) <= int(m.group(2) or m.group(1)):
        return None, f"selection of {len(idx)} on a `{m.group(0)}` screen"
    return ("a " + " ".join(map(str, idx))) if idx else "a -", None


def map_options(screen):
    return [(int(n), m.group(1), int(m.group(2)), int(m.group(3))) for n, label in scr.options(screen) for m in [re.match(r"^(\w+) r(\d+)c(\d+)", label)] if m]


def _seen_ok(step, screen):
    miss = [s for s in step.get("seen") or [] if s.lower() not in screen.lower()]
    return None if not miss else f"the screen does not offer {miss} (record: {step['seen']}): seed or transcription diverged"


def macro_command(step, screen):
    kind = scr.kind(screen)
    if step.get("screen") and kind != step["screen"]:
        return None, f"record expects {step['screen']}, the screen is {kind}"
    bad = None if step.get("sub") else _seen_ok(step, screen)
    if bad:
        return None, bad
    pick = step.get("pick")
    if isinstance(pick, dict) and "discard_potion" in pick:
        return f"a dp {pick['discard_potion']}", None
    if isinstance(pick, dict) and "potion" in pick:
        name = base(pick.get("id", ""))
        hit = _find(screen, lambda lb: lb.startswith("potion ") and base(lb[7:].split(":")[0]) == name)
        return (f"a {hit[0]}", None) if hit else (None, f"no `potion {pick.get('id')}` option on {kind}")
    if kind == "MAP":
        if isinstance(pick, str):
            m = re.fullmatch(r"r(\d+)c(\d+)", pick)
            pick = dict(row=int(m.group(1)), col=int(m.group(2))) if m else dict(room=pick)
        pick = pick or {}
        opts = map_options(screen)
        hit = [o for o in opts if pick.get("room") in (None, o[1]) and pick.get("row") in (None, o[2]) and pick.get("col") in (None, o[3])]
        if len(hit) != 1:
            return None, f"map pick {pick} matches {len(hit)} of {[f'{t} r{r}c{c}' for _, t, r, c in opts]}"
        return f"a {hit[0][0]}", None
    if kind == "CARD_REWARD":
        if pick is None:
            hit = _find(screen, lambda lb: lb.lower().startswith("skip"))
        else:
            hit = _find(screen, lambda lb: label_card(lb) == (base(pick), int(str(pick).endswith("+"))))
        return (f"a {hit[0]}", None) if hit else (None, f"card reward {pick!r} not offered")
    if kind == "SELECT":
        names = pick if isinstance(pick, list) else [pick]
        return select_command(names, [int(str(x).endswith("+")) for x in names], screen)
    if not isinstance(pick, str):
        return None, f"pick {pick!r}: a label substring is needed on a {kind} screen"
    labels = scr.options(screen)
    hit = [int(n) for n, lb in labels if lb.lower().startswith(pick.lower())] or [int(n) for n, lb in labels if pick.lower() in lb.lower()]
    if not hit and kind == "REWARDS":
        hit = [int(n) for n, lb in labels if lb.lower().startswith("take your stolen card")]
    if len(hit) == 1 or (hit and (len({_label(screen, h) for h in hit}) == 1 or (kind == "REWARDS" and pick.lower() == "card"))):
        return f"a {hit[0]}", None
    return None, f"pick {pick!r} matches {len(hit)} options on {kind}"


def parse_map(text):
    nodes, boss = {}, None
    for line in (text or "").split("\n"):
        m = re.match(r"^r(\d+):(.*)", line)
        if m:
            for tok in m.group(2).split():
                mm = re.match(r"^\*?(.)c(\d+)>([\d,]*)", tok)
                if mm:
                    nodes[(int(m.group(1)), int(mm.group(2)))] = (MAP_CODES.get(mm.group(1), "?"), [int(x) for x in mm.group(3).split(",") if x])
        m = re.match(r"^boss: (\d+) (.*)", line)
        if m:
            boss = (int(m.group(1)), m.group(2).replace("+", " ").split())
    return nodes, boss


def floor_rooms(rec, act=0):
    steps = [st for st in rec["steps"] if st.get("floor") is not None and st.get("act", 0) == act]
    first = min((st["floor"] for st in steps), default=1)
    rooms = {}
    for st in steps:
        room = room_of(st)
        if room:
            rooms[st["floor"] - first + 1] = room
    return rooms


def _allowed(nodes, rooms, r, c):
    want = rooms.get(r + 1)
    return (r, c) in nodes and (want is None or nodes[(r, c)][0] in ROOM_ALLOWS.get(want, {want}))


def consistent_from(nodes, rooms, r, c, memo=None):
    memo = {} if memo is None else memo
    if (r, c) not in memo:
        ok = _allowed(nodes, rooms, r, c)
        kids = [k for k in nodes.get((r, c), (None, []))[1] if (r + 1, k) in nodes]
        memo[(r, c)] = ok and (not kids or any(consistent_from(nodes, rooms, r + 1, k, memo) for k in kids))
    return memo[(r, c)]


def map_paths(map_text, rooms, limit=50):
    nodes, boss = parse_map(map_text)
    paths = []

    def walk(r, c, path):
        if len(paths) >= limit or not _allowed(nodes, rooms, r, c):
            return
        path = path + [(r, c)]
        kids = [k for k in nodes[(r, c)][1] if (r + 1, k) in nodes]
        if not kids:
            paths.append(path)
        for k in kids:
            walk(r + 1, k, path)
    if nodes:
        first = min(r for r, _ in nodes)
        for r, c in sorted(k for k in nodes if k[0] == first):
            walk(r, c, [])
    return paths, boss


def path_text(path, map_text):
    nodes, _ = parse_map(map_text)
    return " ".join(f"F{r + 1}:{nodes[(r, c)][0][0]}c{c}" for r, c in path)


def opening_diff(spec, f):
    st, sc = f.get("state") or {}, f.get("scenario") or {}
    obs = (spec["turns"][0].get("obs") or {}) if spec.get("turns") else {}
    out = []
    if spec.get("encounter") and sc.get("encounter") != spec["encounter"]:
        out.append(f"encounter {sc.get('encounter')} vs record {spec['encounter']}")
    ids = [e["id"] for e in st.get("enemies", [])]
    if spec.get("enemy_ids") and ids != spec["enemy_ids"]:
        out.append(f"enemies {ids} vs record {spec['enemy_ids']}")
    for i, (e, o) in enumerate(zip(st.get("enemies", []), obs.get("e", []))):
        if "hp" in o and e.get("hp") != o["hp"]:
            out.append(f"e{i} hp {e.get('hp')} vs record {o['hp']}")
        if o.get("intent"):
            got = [(it["type"], it.get("damage"), it.get("hits")) for it in e.get("intents", [])]
            want = [tuple(list(x) + [None] * (3 - len(x))) for x in o["intent"]]
            if [g[0] for g in got] != [w[0] for w in want] or any(w[1] is not None and g[1] != w[1] for g, w in zip(got, want)):
                out.append(f"e{i} intent {got} vs record {want}")
    if "hp" in obs and (st.get("player") or {}).get("hp") != obs["hp"]:
        out.append(f"player hp {(st.get('player') or {}).get('hp')} vs record {obs['hp']}")
    if "hand" in obs:
        al = spec.get("aliases", {})
        want = sorted(base(token(t, al)[0]) + "+" * token(t, al)[1] for t in obs["hand"])
        got = sorted(base(c["id"]) + "+" * int(c.get("upgrade", 0) > 0) for c in st.get("hand", []))
        if want != got:
            out.append(f"hand {got} vs record {want}")
    return out


class Reenactor:
    def __init__(self, harness, rec_path, custom=False):
        self.h = harness
        self.rec = load(rec_path)
        self.flat = flatten(self.rec)
        self.custom = custom
        d = os.path.join(creator_dir(self.rec), "replay")
        os.makedirs(d, exist_ok=True)
        self.log_path = os.path.join(d, self.rec["video"]["id"] + ".jsonl")
        self.fight_dir = os.path.join(d, self.rec["video"]["id"])

    def _log(self, **row):
        with open(self.log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(row) + "\n")

    def cursor(self):
        n, guessed, started = 0, False, False
        if os.path.exists(self.log_path):
            for line in open(self.log_path, encoding="utf-8"):
                r = json.loads(line)
                if r.get("event") == "start":
                    n, guessed, started = 0, False, True
                elif r.get("event") == "done":
                    n = r["k"] + 1
                elif r.get("event") == "guess":
                    guessed = True
        return n, guessed, started

    def _screen(self):
        s = call("s")
        for _ in range(20):
            if not scr.busy(s):
                break
            s = call("s")
        return s

    def _json(self, cmd):
        try:
            return json.loads(call(cmd).strip())
        except ValueError:
            return None

    def _start(self, screen):
        if any(lb.startswith("continue run") for _, lb in scr.options(screen)):
            return "a run is in progress (`continue run` on the menu): finish or abandon it by hand first"
        want = "custom run" if self.custom else "new run"
        hit = _find(screen, lambda lb: lb.startswith(want))
        if not hit:
            return f"no `{want}` option on the menu"
        r = self.rec
        cmd = f"a {hit[0]} {r['character'].lower()} {r['ascension']} {r['seed']}"
        self.h._new_run()
        reply = self.h._send(cmd)
        self._log(event="start", cmd=cmd, reply=reply[:400], run=self.h.log.run_id, custom=self.custom)
        return "the game refused the new run: " + reply.split("\n")[0] if reply.startswith("ERR") else None

    def run(self, max_steps=None, until=None, seedcheck=False):
        if not build_ok(self.rec):
            return f"REFUSED: record build {self.rec.get('build')!r} is not the pinned {PINNED_BUILD}: no re-enactment (frame-based notes only)\n"
        k, guessed, started = self.cursor()
        if guessed and not seedcheck:
            return "REFUSED: seedcheck walked this run with a guessed map node: abandon it and start a fresh run for the replay\n"
        screen, out = self._screen(), []
        if scr.kind(screen) == "MENU":
            err = self._start(screen)
            if err:
                return "REFUSED: " + err + "\n"
            k, screen = 0, self._screen()
            out.append(f"started {self.rec['character']} A{self.rec['ascension']} seed {self.rec['seed']} ({'custom' if self.custom else 'standard'} run)")
        elif k == 0 and not started:
            return f"REFUSED: the replay log has no progress and the game is on {scr.kind(screen)}, not the main menu\n"
        map_checked, opened, done, autos = set(), set(), 0, 0
        while k < len(self.flat):
            a = self.flat[k]
            if (until and a.get("floor") is not None and a["floor"] >= until) or (max_steps is not None and done >= max_steps):
                break
            refusal = self.h._skill_refusal(screen) if self.h.gate else None
            if refusal:
                out.append(f"REFUSED before step {k} ({describe(a)}): {refusal}")
                break
            kind = scr.kind(screen)
            if kind == "MAP" and a.get("act", 0) not in map_checked:
                out.append(self._map_check(a.get("act", 0)))
                map_checked.add(a.get("act", 0))
            f = self.h.sync() if kind in ("COMBAT", "SELECT") else None
            key = a.get("fight") or (a["step"] if a["kind"] == "gap" and a.get("encounter") else None)
            if f is not None and key is not None and key not in opened and a.get("i", 0) == 0:
                opened.add(key)
                out.append(self._opening(a, f))
                if seedcheck:
                    break
            if a["kind"] == "gap":
                out.append(f"STOP at step {k}, floor {a.get('floor')}: record gap: {a['gap']}")
                self._log(event="stop", k=k, why="gap", gap=a["gap"], screen=screen)
                break
            cmd, err = macro_command(a, screen) if a["kind"] == "macro" else (
                combat_command(a, f.get("state") or {}, screen) if f is not None else (None, f"record expects combat ({describe(a)}), the screen is {kind}"))
            if err and autos < 6:
                auto = self._auto(screen, a)
                if auto:
                    autos += 1
                    screen = self.h._send(auto)
                    self._log(event="auto", k=k, cmd=auto, screen=screen[:1500])
                    if scr.busy(screen):
                        screen = self._screen()
                    continue
            if err and kind == "MAP" and a["kind"] == "macro":
                cmd, err = self._map_by_rooms(a, screen, err, seedcheck)
            row = dict(event="decision", k=k, step=a["step"], floor=a.get("floor"), action=describe(a), t=a.get("t"), screen=screen)
            row.update(deck=self._json("deck.json")) if a["kind"] == "macro" else row.update(state=(f or {}).get("state"), log_len=len((f or {}).get("log", [])))
            if err:
                self._log(**dict(row, event="stop", why=err))
                out.append(f"STOP at step {k} ({describe(a)}, floor {a.get('floor')}, video {a.get('t')}): {err}")
                break
            reply = self.h._send(cmd)
            self._log(**row, cmd=cmd, reply=reply.split("\n")[0])
            if reply.startswith("ERR"):
                self._log(event="stop", k=k, why="game rejected: " + reply.split("\n")[0])
                out.append(f"STOP at step {k} ({describe(a)}): the game rejected `{cmd}`: {reply.splitlines()[0]} -> transcription error")
                break
            if a["kind"] == "macro":
                self.h.log.event("macro", screen=screen.split("\n")[0], state=screen[:1500], choice=cmd[2:],
                                 why=f"reenact {self.rec['video']['id']} step {a['step']}", result=reply.split("\n")[0])
            else:
                self.h.log.event("action", fight=getattr(self.h, "fight_id", None), text=describe(a), json=cmd,
                                 why=f"reenact {self.rec['video']['id']} step {a['step']}")
                if scr.kind(reply) not in ("COMBAT", "SELECT"):
                    self._fight_over(a, reply)
            self._log(event="done", k=k)
            done, autos, k = done + 1, 0, k + 1
            screen = self._screen() if scr.busy(reply) else reply
        if k >= len(self.flat):
            out.append("record replayed to its end")
        out.append(f"cursor {k}/{len(self.flat)}; log {os.path.relpath(self.log_path, ROOT)}")
        return "\n".join(out) + "\n" + screen

    def _auto(self, screen, a=None):
        opts = scr.options(screen)
        if len(opts) == 1 and (opts[0][1].lower() in AUTO or scr.kind(screen) == "MAP"):
            return f"a {opts[0][0]}"
        if scr.kind(screen) == "REWARDS" and (a or {}).get("screen") not in ("REWARDS", "CARD_REWARD"):
            hit = _find(screen, lambda lb: lb.lower().startswith("proceed"))
            return f"a {hit[0]}" if hit else None
        return None

    def _opening(self, a, f):
        spec = next((st["fight"] for st in self.rec["steps"] if st.get("fight") and st["fight"]["id"] == a.get("fight")), None)
        spec = dict(spec, aliases=self.rec.get("aliases", {})) if spec else dict(encounter=a.get("encounter"))
        diff = opening_diff(spec, f)
        self._log(event="opening", k=a.get("step"), encounter=(f.get("scenario") or {}).get("encounter"), diff=diff, scenario=f.get("scenario"), state=f.get("state"))
        return f"opening of {a.get('fight') or a.get('encounter')} (floor {a.get('floor')}): " + ("matches the record" if not diff else "DIFFERS: " + "; ".join(diff))

    def _map_check(self, act=0):
        text = call("m")
        paths, boss = map_paths(text, floor_rooms(self.rec, act))
        want = self.rec.get("boss") if act == 0 else (self.rec.get("bosses") or {}).get(str(act))
        lines = [f"map: boss {' + '.join(boss[1]) if boss else '?'}" + ("" if not want else f" (record {want}: {'ok' if boss and want in boss[1] else 'DIFFERS'})"),
                 f"paths consistent with the record's rooms by floor: {len(paths)}" + ("" if paths else " -> the map or the record's floor numbering differs")]
        lines += ["  " + path_text(p, text) for p in paths[:6]]
        self._log(event="map", text=text, paths=paths)
        return "\n".join(lines)

    def _map_by_rooms(self, a, screen, err, seedcheck):
        nodes, _ = parse_map(call("m"))
        rooms = floor_rooms(self.rec, a.get("act", 0))
        pick = a.get("pick") if isinstance(a.get("pick"), dict) else {}
        opts = [o for o in map_options(screen) if pick.get("room") in (None, o[1]) and consistent_from(nodes, rooms, o[2], o[3])]
        if len(opts) == 1:
            return f"a {opts[0][0]}", None
        if opts and seedcheck:
            self._log(event="guess", k=a["step"], options=[o[1:] for o in opts], took=opts[0][1:])
            return f"a {opts[0][0]}", None
        return None, err + (f"; consistent with the record's later rooms: {[f'{t} r{r}c{c}' for _, t, r, c in opts]}: read the map frame" if opts else "")

    def _fight_over(self, a, reply):
        f = getattr(self.h, "_last_f", None)
        g = self._json("fight")
        if isinstance(g, dict) and f is not None and g.get("id") == f.get("id") and len(g.get("log", [])) >= len(f.get("log", [])):
            f = g
        if f is not None:
            os.makedirs(self.fight_dir, exist_ok=True)
            sc = dict(f["scenario"], run_seed=self.rec["seed"], game_build=self.rec["build"], video=self.rec["video"]["url"])
            rec = dict(id=a["fight"], encounter=sc.get("encounter"), hp_start=[sc.get("hp"), sc.get("max_hp")], hp_end=list(scr.hp(reply) or []),
                       scenario=sc, fight=dict(f, scenario=sc), source="replay")
            with open(os.path.join(self.fight_dir, a["fight"] + ".json"), "w", encoding="utf-8") as fh:
                json.dump(rec, fh)
        try:
            self.h._fight_end(reply)
        except Exception as e:  # noqa: BLE001
            self.h._bookkeeping_error("reenact fight end", e)
