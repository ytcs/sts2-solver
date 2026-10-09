"""Macro-only expert extraction (skill `expert-reenact`, "Macro corpus"); entry points via `tools/expert.py macro*`.

macro URL [--creator X]           fetch (1 fps frames, transcript; video deleted) + scan
macro-scan ID --creator X         screen class per frame -> segments + one contact sheet per decision segment (git-ignored work/)
macro-build WORK                  agent transcription (work/<id>.macro.work.json) -> <id>.macro.jsonl; tracks deck/relics/gold, validates
macro-replay LOG --compact C      game-truth macro record from a live replay log (`python -m agent replay`)
macro-compare RECORD              `price` per decision -> <id>.macro_verdicts.json (verdicts never enter the record)
"""
import difflib
import json
import os
import re
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))

FORMAT = ("macro record: header, then one decision per line in game order. decision: k, floor, act (0-2), t, type "
          "(ancient|event|map|card|rest|shop|potion), screen (harness screen text at the decision, pre-decision values), pick "
          "(card name|null; 'r<row>c<col>'; 'Rest'|'Smith'; option head; shop: bought labels in order; potion: {take, discard_slot}|'leave'), "
          "then (follow-up selections: smith target, removal target, event card choices), state {hp, max_hp, gold, deck[{id, upgrade, enchantment?}], "
          "relics[{id, props?}], potions[name|null by slot], slots, max_energy, act_name, bosses, pos ('r<row>c<col>' last node visited this act, null before "
          "the first), seen {weak, regular, elite: encounter ids this act}, monsters, removals, potion_p, offset}, checks (validation notes). "
          "header.maps[act] = the act map in `routes.parse_map` text (absent when not transcribed)")
TYPES = ("ancient", "event", "map", "card", "rest", "shop", "potion")
CHARS = ("IRONCLAD", "SILENT", "DEFECT", "NECROBINDER", "REGENT")


# ---------------------------------------------------------------- names -> ids

def _cat():
    from agent import runmodel as R
    return R.CAT


def ident(name):
    return re.sub(r"[^A-Z0-9]+", "_", str(name).strip().upper().replace("'", "").replace("’", "")).strip("_")


def _ids(kind):
    return {r["id"] for rows in _cat()[kind].values() for r in rows}


def card_ids():
    from agent import macro
    return macro.card_id_set()


def card(name, character):
    """'Dagger Spray+*GLAM' -> {id, upgrade, enchantment?}; raises on an unknown name"""
    s = str(name).strip()
    ench = None
    if "*" in s:
        s, e = s.split("*", 1)
        ench = {"id": ident(e or "UNKNOWN"), "amount": 1}
    up = 0
    m = re.search(r"\+(\d*)$", s)
    if m:
        up = int(m.group(1) or 1)
        s = s[:m.start()]
    cid = ident(s)
    ids = card_ids()
    for c in (cid, f"{cid}_{character}"):
        if c in ids:
            cid = c
            break
    else:
        close = difflib.get_close_matches(cid, ids, 1, 0.88)
        if not close:
            raise ValueError(f"unknown card {name!r}")
        cid = close[0]
    out = {"id": cid, "upgrade": up}
    if ench:
        out["enchantment"] = ench
    return out


def thing(kind, name):
    """relic / potion name -> id (exact ident, else a close catalog match, else ValueError)"""
    rid = ident(name)
    ids = _ids(kind)
    if kind == "relics":
        from agent import events as EV
        ids = ids | set(EV.ancient_relics())
    if rid in ids:
        return rid
    close = difflib.get_close_matches(rid, ids, 1, 0.85)
    if close:
        return close[0]
    raise ValueError(f"unknown {kind[:-1]} {name!r}")


def card_name(c):
    cid = re.sub(r"_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$", "", c["id"])
    return cid.replace("_", " ").title() + ("+" if c.get("upgrade") else "")


def cost_of(cid):
    for pool in _cat()["cards"].values():
        for c in pool:
            if c["id"] == cid:
                return c.get("cost", "?")
    return "?"


def rarity_of(cid):
    for pool in _cat()["cards"].values():
        for c in pool:
            if c["id"] == cid:
                return c.get("rarity")
    return None


def act_name_of(boss_or_encounter, act):
    from agent import pools
    names = pools.act_names(act)
    for n in names:
        if any(boss_or_encounter in pools.pool(n, k) for k in ("weak", "regular", "elite", "boss")):
            return n
    return names[0]


def kind_of(act_name, enc):
    from agent import pools
    for k in ("weak", "regular", "elite", "boss"):
        if enc in pools.pool(act_name, k):
            return k
    return None


# ---------------------------------------------------------------- counters (mirrors agent/tracker.py)

class Counters:
    def __init__(self):
        from agent import tracker as T
        self.T = T
        self.potion_p, self.offset, self.removals = T.POTION_START, T.OFFSET_START, 0

    def combat_rewards(self, kind, potion_dropped, card_ids_shown):
        T = self.T
        if kind in ("hallway", "elite") and potion_dropped is not None:
            self.potion_p += -T.POTION_STEP if potion_dropped else T.POTION_STEP
        for cid in card_ids_shown or []:
            if kind == "boss":
                continue
            self.offset = T.OFFSET_START if rarity_of(cid) == "Rare" else min(T.OFFSET_CAP, self.offset + T.OFFSET_STEP)

    def snap(self):
        return dict(removals=self.removals, potion_p=round(self.potion_p, 4), offset=round(self.offset, 4))


def fight_kind(enc):
    return "elite" if enc.endswith("_ELITE") else "boss" if enc.endswith("_BOSS") else "hallway"


# ---------------------------------------------------------------- record I/O

def write_record(path, head, decisions):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(json.dumps(head, ensure_ascii=False, separators=(",", ":")) + "\n")
        for d in decisions:
            fh.write(json.dumps(d, ensure_ascii=False, separators=(",", ":")) + "\n")
    from collections import Counter
    print(f"-> {os.path.relpath(path, ROOT)}: {len(decisions)} decisions {dict(Counter(d['type'] for d in decisions))}, "
          f"{os.path.getsize(path) / 1024:.0f} KB")


def load_record(path):
    rows = [json.loads(x) for x in open(path, encoding="utf-8") if x.strip()]
    return rows[0], rows[1:]


def header_line(act, floor, character, asc, hp, max_hp, gold, belt):
    return f"A{act + 1} F{floor} {character} A{asc} HP {hp}/{max_hp} G{gold} pots[{', '.join(b or '-' for b in belt) or '-'}]"


# ---------------------------------------------------------------- from a live replay log (game truth)

def _pick_of(action):
    m = re.match(r"^\S+ pick (.*)$", action)
    if not m:
        return None
    try:
        return json.loads(m.group(1))
    except json.JSONDecodeError:
        return m.group(1)


def _opt_index(cmd):
    m = re.match(r"^a (\d+)", cmd or "")
    return int(m.group(1)) if m else None


def _potion_name(pid):
    return pid.replace("_", " ").title()


def from_replay(a):
    from agent import screen as scr
    log = a.log if os.path.isabs(a.log) else os.path.join(ROOT, a.log)
    comp = a.compact if os.path.isabs(a.compact) else os.path.join(ROOT, a.compact)
    head0 = json.loads(open(comp, encoding="utf-8").readline())
    rows = [json.loads(x) for x in open(log, encoding="utf-8") if x.strip()]
    character, asc = head0["character"], head0["ascension"]
    maps, bosses, act_names, decisions = {}, {}, {}, []
    seen, counters, pos, monsters = {}, Counters(), {}, {}
    last_fight = fight_floor = None
    reward_open = False
    base = None
    i = 0
    while i < len(rows):
        r = rows[i]
        ev = r.get("event")
        if ev == "map":
            text = r["text"]
            # the act of a map event = the act of the next decision header
            nxt = next((x for x in rows[i + 1:] if x.get("event") == "decision"), None)
            act = scr.act_index(nxt["screen"]) if nxt else 0
            body = "\n".join(line for line in text.splitlines() if re.match(r"^(r\d+:|boss:)", line))
            maps[str(act)] = body
            b = re.search(r"^boss: \d+ (.*)$", body, re.M)
            if b:
                bosses[act] = [x for x in re.split(r"[ +,]+", b.group(1).strip()) if x.endswith("_BOSS")]
                act_names[act] = act_name_of(bosses[act][0], act)
            i += 1
            continue
        if ev == "opening":
            enc = r["encounter"]
            act = r["scenario"].get("act", 0)
            name = act_names.get(act) or act_name_of(enc, act)
            act_names.setdefault(act, name)
            k = kind_of(name, enc)
            if k in ("weak", "regular", "elite"):
                seen.setdefault(act, {}).setdefault(k, []).append(enc)
                if k in ("weak", "regular"):
                    monsters[act] = monsters.get(act, 0) + 1
            last_fight, reward_open, fight_floor = enc, True, r["scenario"].get("total_floor")
            base = base or {k: r["scenario"][k] for k in ("max_energy", "base_orb_slots", "max_potion_slots") if k in r["scenario"]}
            i += 1
            continue
        if ev != "decision":
            i += 1
            continue
        screen = r["screen"]
        kind = scr.kind(screen)
        lines = screen.split("\n")
        act = scr.act_index(screen)
        floor = r["floor"]
        pick = _pick_of(r["action"])
        hm = re.search(r"HP (\d+)/(\d+) G(\d+)", lines[1])
        deck = r.get("deck") or {}
        belt = [None if b == "-" else b for b in scr.belt(screen)]
        if kind == "REWARDS" and reward_open and last_fight:
            labels = [lb for _, lb in scr.options(screen)]
            cards = []
            for lb in labels:
                m = re.match(r"^card: (.*)$", lb)
                if m:
                    for n in m.group(1).split(" | "):
                        try:
                            cards.append(card(n, character)["id"])
                        except ValueError:
                            pass
            counters.combat_rewards(fight_kind(last_fight), any(lb.startswith("potion ") for lb in labels), cards)
            reward_open = False
        state = dict(hp=int(hm.group(1)), max_hp=int(hm.group(2)), gold=int(hm.group(3)),
                     deck=[{k: v for k, v in c.items() if k in ("id", "upgrade", "enchantment")} for c in deck.get("deck", [])],
                     relics=[{k: v for k, v in x.items() if k in ("id", "props")} for x in deck.get("relics", [])],
                     potions=belt, slots=deck.get("max_potion_slots", len(belt)), max_energy=deck.get("max_energy", 3),
                     act_name=act_names.get(act), bosses=bosses.get(act, []), pos=pos.get(act), seen=seen.get(act, {}), monsters=monsters.get(act, 0),
                     **counters.snap())
        if last_fight and last_fight.endswith("_BOSS") and floor == fight_floor:
            state["post_boss"] = True
        d = None
        opts = [lb for _, lb in scr.options(screen)]
        if kind == "MAP":
            idx = _opt_index(r.get("cmd"))
            lab = opts[idx] if idx is not None and idx < len(opts) else None
            m = re.search(r"r(\d+)c(\d+)", lab or "")
            if m:
                pos[act] = f"r{m.group(1)}c{m.group(2)}"
            if len([o for o in opts if re.search(r"r\d+c\d+", o)]) >= 2 and m:
                d = dict(type="map", pick=f"r{m.group(1)}c{m.group(2)}")
        elif kind == "EVENT":
            typ = "ancient" if r.get("room") == "Ancient" or floor == 1 or (len(lines) > 2 and lines[2].split(":")[0].strip() in ANCIENTS) else "event"
            then = _follow(rows, i, ("SELECT", "CHOOSE_CARD", "CARD_REWARD"), stop=("MAP", "EVENT", "COMBAT", "REWARDS", "SHOP", "RESTSITE"))
            d = dict(type=typ, pick=pick, then=then or None)
        elif kind == "CARD_REWARD":
            d = dict(type="card", pick=pick)
        elif kind == "RESTSITE":
            then = _follow(rows, i, ("SELECT",), stop=("MAP",)) if str(pick).lower().startswith("smith") else []
            d = dict(type="rest", pick=pick, then=then or None)
        elif kind == "SHOP":
            buys, then, j = [], [], i
            while j < len(rows):
                x = rows[j]
                if x.get("event") == "decision":
                    xk = scr.kind(x["screen"])
                    if xk == "SHOP":
                        p = _pick_of(x["action"])
                        if p in ("leave shop", None):
                            break
                        buys.append(p)
                    elif xk == "SELECT":
                        then += _pick_of(x["action"]) or []
                    elif xk == "MAP":
                        break
                j += 1
            d = dict(type="shop", pick=buys, then=then or None)
            i = j
        elif kind == "REWARDS" and isinstance(pick, dict) and "discard_potion" in pick:
            nxt = next((x for x in rows[i + 1:] if x.get("event") == "decision"), None)
            take = _pick_of(nxt["action"]) if nxt else None
            take = take[0] if isinstance(take, list) and take else take
            d = dict(type="potion", pick=dict(take=take, discard_slot=pick["discard_potion"]))
        elif kind == "REWARDS" and all(b for b in belt) and belt and any(o.startswith("potion ") for o in opts) and isinstance(pick, list) \
                and not any(str(p).lower().startswith("potion") or str(p) in [o.split(":")[0][7:] for o in opts if o.startswith("potion ")] for p in pick):
            offer = next(o for o in opts if o.startswith("potion "))
            d = dict(type="potion", pick="leave", offer=offer.split(":")[0][7:])
        if d is not None:
            d = dict(k=len(decisions), floor=floor, act=act, t=r.get("t"), type=d.pop("type"), screen=screen, **d, state=state)
            d = {k: v for k, v in d.items() if v is not None or k == "pick"}
            decisions.append(d)
        if kind == "SHOP":
            counters.removals += sum(1 for b in (decisions[-1]["pick"] if d else []) if str(b).startswith("remove"))
        i += 1
    for d in decisions:
        d["state"]["act_name"] = d["state"]["act_name"] or act_names.get(d["act"])
        d["state"]["bosses"] = d["state"]["bosses"] or bosses.get(d["act"], [])
    head = dict(video=head0["video"], build=head0.get("build"), build_hash=head0.get("build_hash"), modded=head0.get("modded"), seed=head0.get("seed"),
                character=character, ascension=asc, result=head0.get("result"), source=f"replay log {os.path.basename(log)} (game states)",
                base=dict(base or {}, max_potion_slots=(base or {}).get("max_potion_slots", 2)), maps=maps, format=FORMAT)
    out = a.out or os.path.join(os.path.dirname(comp), f"{head0['video']['id']}.macro.jsonl")
    write_record(out, head, decisions)


ANCIENTS = ("Neow", "Darv", "Nonupeipe", "Orobas", "Pael", "Tanx", "Tezcatara", "Vakuu")


def _follow(rows, i, kinds, stop):
    from agent import screen as scr
    out = []
    for x in rows[i + 1:]:
        if x.get("event") != "decision":
            continue
        k = scr.kind(x["screen"])
        if k in kinds:
            p = _pick_of(x["action"])
            out += p if isinstance(p, list) else [p]
        elif k in stop:
            break
    return out


# ---------------------------------------------------------------- price on a decision

def run_state(head, d):
    from agent import routes
    from agent import runmodel as R
    s = d["state"]
    act = d["act"]
    character = head["character"]
    base = dict(name="macro", ascension=head["ascension"], character=character, max_energy=s.get("max_energy", head["base"].get("max_energy", 3)),
                max_potion_slots=s["slots"], base_orb_slots=head["base"].get("base_orb_slots", 0), seed="placeholder", total_floor=d["floor"], act=act,
                hp=s["hp"], max_hp=s["max_hp"], gold=s["gold"], deck=s["deck"], relics=[r for r in s["relics"] if r.get("id")], potions=[])
    from agent import tracker as T
    counters = (s.get("potion_p", T.POTION_START), s.get("offset", T.OFFSET_START), dict(T.UNKNOWN_BASE), s.get("removals", 0))
    pots = [thing("potions", p) if not re.fullmatch(r"[A-Z0-9_]+", p) else p for p in s["potions"] if p and not p.startswith("?")]
    relics = [r["id"] for r in s["relics"] if r.get("id")]
    if s.get("post_boss") and act < R.LAST_ACT:
        # boss rewards: the act is over; the rollout starts the next act (template map, unknown bosses) after the ancient heal
        hp = s["hp"] + int(R.HEAL_ANCIENT * (s["max_hp"] - s["hp"]))
        return R.RunState(dict(base, act=act + 1), act + 1, R.ACT_NAMES[act + 1][0], hp, s["max_hp"], s["gold"], s["deck"], relics, pots, s["slots"],
                          counters[:2] + (dict(T.UNKNOWN_BASE), counters[3]), {}, [], None, None, 0)
    text = (head.get("maps") or {}).get(str(act))
    nodes = frontier = None
    if text:
        nodes = pad_map(routes.parse_map(text)[0], act)
        first = sorted(k for k in nodes if k[0] == min(r for r, _ in nodes))
        if d["type"] == "map":
            frontier = routes.offered(d["screen"])
        elif s.get("pos"):
            m = re.match(r"r(\d+)c(\d+)", s["pos"])
            key = (int(m.group(1)), int(m.group(2)))
            frontier = [k for k in nodes[key]["children"] if k in nodes] if key in nodes else first
        else:
            frontier = first
    act_name = s.get("act_name") or act_name_of((s.get("bosses") or [""])[0], act)
    return R.RunState(base, act, act_name, s["hp"], s["max_hp"], s["gold"], s["deck"], relics, pots, s["slots"], counters, s.get("seen") or {},
                      s.get("bosses") or [], frontier, nodes, s.get("monsters", 0))


ACT_ROWS = (15, 14, 13)


def pad_map(nodes, act):
    """rows past the transcribed part of the map: one template room per row (same for every option), joined to every node of the last row"""
    from agent import runmodel as R
    last = max(r for r, _ in nodes)
    rows = R.TEMPLATE.get(act, R.TEMPLATE[1]).split()
    if last >= ACT_ROWS[act]:
        return nodes
    for r, _c in [k for k in nodes if k[0] == last]:
        nodes[(r, _c)]["children"] = [(last + 1, 9)]
    for r in range(last + 1, ACT_ROWS[act] + 1):
        t = rows[min(len(rows) - 1, r - 1 - (ACT_ROWS[act] - len(rows)))] if r > ACT_ROWS[act] - len(rows) else "M"
        nodes[(r, 9)] = dict(type="R" if r == ACT_ROWS[act] else t, children=[(r + 1, 9)] if r < ACT_ROWS[act] else [], visited=False)
    return nodes


def options_for(head, d, st):
    """price options for the screen + the label his pick maps to (his bundle is added when it is not a single option)"""
    from agent import price as PR
    from agent import screen as scr
    screen, pick, typ = d["screen"], d.get("pick"), d["type"]
    opts = PR.options(st, screen)
    labels = [lb for lb, _ in opts]
    low = {lb.lower(): lb for lb in labels}
    mine = None
    if typ == "card":
        pick = str(pick).split("*")[0] if pick else pick
        mine = "skip" if not pick else low.get(str(pick).lower().strip()) or low.get(re.sub(r"\+$", "", str(pick)).lower().strip())
        if mine is None and pick:
            mine = next((lb for lb in labels if lb.lower().startswith(str(pick).lower().rstrip("+"))), None)
    elif typ == "rest":
        if str(pick).lower().startswith("rest"):
            mine = "rest"
        elif str(pick).lower().startswith("smith") and d.get("then"):
            c = card(d["then"][0], head["character"])
            mine = f"smith {c['id']}" if f"smith {c['id']}" in labels else None
    elif typ == "map":
        mine = next((lb for lb in labels if lb.endswith(" " + str(pick))), None)
    elif typ in ("event", "ancient"):
        p = str(pick).lower().strip()
        mine = next((lb for lb in labels if lb.lower().split(":")[0].strip() == p), None) or \
            next((lb for lb in labels if lb.lower().startswith(p[:20])), None)
    elif typ == "potion":
        if pick == "leave":
            mine = next((lb for lb in labels if lb.startswith("leave ")), None)
        else:
            slot = pick.get("discard_slot")
            belt = [b for b in scr.belt(screen)]
            held = ident(belt[slot]) if slot is not None and slot < len(belt) else None
            mine = next((lb for lb in labels if not lb.startswith("leave ") and held and lb.split(" for ")[-1].split(" (")[0] == held), None)
    elif typ == "shop":
        buys = [b for b in (pick or [])]
        if not buys:
            mine = "nothing"
        else:
            parts, fns = [], []
            rm_targets = list(d.get("then") or [])
            fn_of = dict(opts)
            swap = None
            for b in buys:
                if isinstance(b, dict) and "discard_potion" in b:
                    swap = b["discard_potion"]
                    continue
                if str(b).startswith("?"):
                    continue
                if str(b).startswith("remove"):
                    tgt = rm_targets.pop(0) if rm_targets else None
                    c = card(tgt, head["character"]) if tgt else None
                    name = f"remove {re.sub(r'_(IRONCLAD|SILENT|DEFECT|REGENT|NECROBINDER)$', '', c['id'])}".lower() if c else None
                else:
                    name = str(b).split("(")[0].strip().lower()
                lb = next((x for x in labels if name and x.lower().split(" (")[0] == name), None)
                if lb is None and swap is not None:
                    m = next((m for _, t in scr.options(screen) for m in [re.match(r"^(\d+)g potion (.+?):", t)] if m and m.group(2).strip().lower() == name), None)
                    if m:
                        pid, price, slot = thing("potions", m.group(2)), int(m.group(1)), swap
                        lb = f"{m.group(2).strip()} for slot {slot} ({price}g)"

                        def fn(s, _d, pid=pid, price=price, slot=slot):
                            if slot < len(s.potions):
                                s.potions[slot] = pid
                            else:
                                s.potions.append(pid)
                            s.gold -= price
                        fn_of[lb] = fn
                        swap = None
                if lb is None:
                    return opts, None, f"bought item not priceable: {b}"
                parts.append(lb)
                fns.append(fn_of[lb])
            mine = " + ".join(parts)
            if mine not in labels:
                opts = opts + [(mine, lambda s, dr, fs=tuple(fns): [f(s, dr) for f in fs if f is not None])]
    return opts, mine, None


def nvidia_busy():
    import subprocess
    try:
        out = subprocess.run(["nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"], capture_output=True, text=True, timeout=20).stdout
        return max(int(x) for x in out.split())
    except Exception:  # noqa: BLE001
        return 100


def compare(a):
    import expert as X
    X.watchdog()
    path = a.record if os.path.isabs(a.record) else os.path.join(ROOT, a.record)
    head, decs = load_record(path)
    if a.device == "auto":
        busy = nvidia_busy()
        os.environ["STS2_DEVICE"] = "cuda" if busy < 50 else "cpu"
        print(f"GPU utilization {busy}% -> STS2_DEVICE={os.environ['STS2_DEVICE']}")
    else:
        os.environ["STS2_DEVICE"] = a.device
    from agent import price as PR
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    pred = Predictor(PREDICTOR_CKPT, batch=1024)
    out = a.out or path.replace(".macro.jsonl", ".macro_verdicts.json")
    prev = {}
    if os.path.exists(out) and not a.fresh:
        prev = {r["k"]: r for r in json.load(open(out, encoding="utf-8"))["rows"]}
    rows = []
    types = set(a.types.split(",")) if a.types else None
    t_all = time.time()
    redo = {int(x) for x in (a.redo or "").split(",") if x}
    for d in decs:
        if types and d["type"] not in types:
            if d["k"] in prev:
                rows.append(prev[d["k"]])
            continue
        if d["k"] in prev and prev[d["k"]].get("n") == a.n and prev[d["k"]]["verdict"] != "unpriced" and d["k"] not in redo:
            rows.append(prev[d["k"]])
            continue
        t0 = time.time()
        row = dict(k=d["k"], floor=d["floor"], type=d["type"], played=d.get("pick"), then=d.get("then"), n=a.n)
        try:
            if d["state"].get("post_boss") and d["act"] >= 2:
                raise ValueError("between the final two bosses: not modelled by the run model")
            st = run_state(head, d)
            opts, mine, err = options_for(head, d, st)
        except ValueError as e:
            opts, mine, err = [], None, str(e)
        row["n_options"] = len(opts)
        if err or len(opts) < 2 or mine is None:
            row["verdict"] = "unpriced"
            row["why"] = err or ("fewer than 2 priced options" if len(opts) < 2 else f"pick {d.get('pick')!r} matches no priced option")
        else:
            res = PR.price(st, opts, pred, n=a.n, seed=d["floor"] + 1000 * a.seed)
            best, horizon = PR.best(res)
            row.update(priced_as=mine, price_best=best, horizon=horizon,
                       means={lb: round(float(r[horizon].mean()), 4) for lb, r in sorted(res.items(), key=lambda kv: -kv[1][horizon].mean())[:6]})
            if mine == best:
                row["verdict"] = "agree"
            else:
                dd, se = X.paired(res[mine][horizon], res[best][horizon])
                row.update(d=round(dd, 4), se=round(se, 4), verdict="disagree" if dd < -2 * se else "tie")
        row["secs"] = round(time.time() - t0, 1)
        rows.append(row)
        print(f"k{d['k']:<4d} F{d['floor']:<3d} {d['type']:8s} played {str(row.get('priced_as') or d.get('pick'))[:28]:28s} price {str(row.get('price_best', ''))[:28]:28s} "
              f"[{row.get('horizon', '')}] {row['verdict']}" + (f" {row['d']:+.4f} ({row['se']:.4f})" if "d" in row else "") +
              (f" ({row.get('why')})" if row["verdict"] == "unpriced" else "") + f" {row['secs']}s", flush=True)
        dump_verdicts(out, path, head, rows, a)
    dump_verdicts(out, path, head, rows, a)
    print(f"-> {os.path.relpath(out, ROOT)} ({time.time() - t_all:.0f} s)")
    summarize(rows)


def dump_verdicts(out, path, head, rows, a):
    model = json.load(open(os.path.join(ROOT, "models", "current.json"), encoding="utf-8")).get("predictor")
    json.dump(dict(record=os.path.relpath(path, ROOT).replace("\\", "/"), video=head["video"]["id"], character=head["character"],
                   settings=dict(n=a.n, seed=f"floor + 1000 * {a.seed}", predictor=model, device=os.environ.get("STS2_DEVICE"), rule="agree = price best is his option; "
                                 "else disagree if his - best < -2 paired se on price's ladder horizon, else tie"),
                   rows=rows), open(out, "w", encoding="utf-8"), indent=1)


def summarize(rows):
    from collections import Counter, defaultdict
    by = defaultdict(Counter)
    for r in rows:
        by[r["type"]][r["verdict"]] += 1
        by["all"][r["verdict"]] += 1
    for t in list(TYPES) + ["all"]:
        if t in by:
            print(f"{t:8s} " + ", ".join(f"{k} {v}" for k, v in sorted(by[t].items())))
    for r in rows:
        if r["verdict"] == "disagree":
            print(f"  k{r['k']} F{r['floor']} {r['type']}: his {r['priced_as']} | price {r['price_best']} [{r['horizon']}] {r['d']:+.4f} ({r['se']:.4f})")


# ---------------------------------------------------------------- CLI (dispatched from tools/expert.py)

def add_parsers(sub):
    p = sub.add_parser("macro", help="fetch + scan a video for macro extraction")
    p.add_argument("url")
    p.add_argument("--creator")
    p.add_argument("--fps", type=float, default=1.0)
    p = sub.add_parser("macro-scan")
    p.add_argument("id")
    p.add_argument("--creator", required=True)
    p.add_argument("--train", help="compact record of this video: label its frames and (re)write the screen exemplars")
    p = sub.add_parser("macro-crop", help="compose crops of frames for reading (1080p frame coordinates)")
    p.add_argument("id")
    p.add_argument("--creator", required=True)
    p.add_argument("--at", required=True, help="comma-separated m:ss times (nearest frame), or a-b for every frame in the range")
    p.add_argument("--region", default="full", help=f"one of {', '.join(REGIONS)} or x0,y0,x1,y1")
    p.add_argument("--top", action="store_true", help="prepend the top bar (HP, gold, potions, floor, deck count) of each frame")
    p.add_argument("--scale", type=float, default=1.0)
    p.add_argument("--out", required=True)
    p = sub.add_parser("macro-build")
    p.add_argument("work")
    p.add_argument("--out")
    p.add_argument("--allow", action="store_true", help="write the record despite validation errors (each lands in the decision's checks)")
    p = sub.add_parser("macro-replay")
    p.add_argument("log")
    p.add_argument("--compact", required=True)
    p.add_argument("--out")
    p = sub.add_parser("macro-summary", help="agree/tie/disagree by decision type over verdict files")
    p.add_argument("verdicts", nargs="+")
    p = sub.add_parser("macro-diff", help="transcription accuracy of record A against reference record B of the same run")
    p.add_argument("a")
    p.add_argument("b")
    p = sub.add_parser("macro-compare")
    p.add_argument("record")
    p.add_argument("--n", type=int, default=32)
    p.add_argument("--types", help="comma-separated decision types only")
    p.add_argument("--device", default="auto", help="auto = cuda only while nvidia-smi shows < 50%% use")
    p.add_argument("--fresh", action="store_true", help="ignore rows already in the verdict file")
    p.add_argument("--redo", help="comma-separated k to recompute")
    p.add_argument("--seed", type=int, default=0, help="rollout seed block (0 = the default; another value measures Monte Carlo verdict noise)")
    p.add_argument("--out")


def run(a):
    return {"macro": macro, "macro-scan": scan, "macro-crop": crop, "macro-build": build, "macro-replay": from_replay, "macro-compare": compare,
            "macro-diff": diff, "macro-summary": lambda a: summary_all(a.verdicts)}[a.cmd](a)


def summary_all(paths):
    rows = []
    for p in paths:
        v = json.load(open(record_path_(p), encoding="utf-8"))
        print(f"{v['video']} {v['character']}: {len(v['rows'])} decisions, n {v['settings']['n']}")
        rows += [dict(r, video=v["video"]) for r in v["rows"]]
    summarize(rows)


def _opts(d):
    from agent import screen as scr
    out = []
    for _, lb in scr.options(d["screen"]):
        if lb.startswith(("Skip", "leave shop", "proceed", "full map")):
            continue
        lb = re.sub(r"\*\w+", "", re.sub(r"^\d+g (card|relic|potion) ", "", lb))
        out.append(ident(re.split(r"[(:]| -> |\+", lb)[0]).removesuffix("_IRONCLAD"))
    return sorted(out)


def diff(a):
    """transcription accuracy: a frames record (A) against a reference record of the same run (B, e.g. from the replay log)"""
    from collections import Counter
    _, da = load_record(record_path_(a.a))
    _, db = load_record(record_path_(a.b))
    key = lambda d: (d["floor"], d["type"])  # noqa: E731
    pool = {}
    for d in db:
        pool.setdefault(key(d), []).append(d)
    c, rows = Counter(), []
    def norm(p):
        if isinstance(p, list):
            p = [x for x in p if not isinstance(x, dict)]
        t = re.sub(r"\*\w+", "", json.dumps(p, sort_keys=True).lower().replace("+", ""))
        return t.replace('"remove a card"', '"remove"')
    for d in da:
        cands = pool.get(key(d)) or []
        ref = next((x for x in cands if norm(x.get("pick")) == norm(d.get("pick"))), cands[0] if cands else None)
        if ref is not None:
            cands.remove(ref)
        if ref is None:
            c["extra (not in reference)"] += 1
            rows.append((d["floor"], d["type"], "extra", d.get("pick"), None))
            continue
        c["matched"] += 1
        sa, sb = d["state"], ref["state"]
        checks = dict(pick=norm(d.get("pick")) == norm(ref.get("pick")) or (d["type"] == "rest" and norm(d.get("then")) == norm(ref.get("then"))),
                      options=_opts(d) == _opts(ref) if d["type"] != "map" else True,
                      hp=sa["hp"] == sb["hp"], gold=sa["gold"] == sb["gold"],
                      deck=Counter((x["id"], x.get("upgrade", 0)) for x in sa["deck"]) == Counter((x["id"], x.get("upgrade", 0)) for x in sb["deck"]),
                      relics=len(sa["relics"]) == len(sb["relics"]))
        for k, ok in checks.items():
            c[f"{k} ok"] += ok
        bad = [k for k, ok in checks.items() if not ok]
        if bad:
            rows.append((d["floor"], d["type"], ",".join(bad), d.get("pick"), ref.get("pick")))
        if not checks["deck"] or not checks["relics"]:
            ca = Counter((x["id"], x.get("upgrade", 0)) for x in sa["deck"])
            cb = Counter((x["id"], x.get("upgrade", 0)) for x in sb["deck"])
            ra = Counter(r.get("id") or r.get("name") for r in sa["relics"])
            rb = Counter(r["id"] for r in sb["relics"])
            rows.append((d["floor"], d["type"], "  detail", f"deck A-B {dict(ca - cb)} relics A-B {dict(ra - rb)}", f"deck B-A {dict(cb - ca)} relics B-A {dict(rb - ra)}"))
    from agent import routes
    ha, hb = load_record(record_path_(a.a))[0], load_record(record_path_(a.b))[0]
    for k, tb in sorted((hb.get("maps") or {}).items()):
        na, _ = routes.parse_map((ha.get("maps") or {}).get(k, ""))
        nb, _ = routes.parse_map(tb)
        ea = {(n, ch) for n, v in na.items() for ch in v["children"]}
        eb = {(n, ch) for n, v in nb.items() for ch in v["children"]}
        print(f"map act {k}: nodes {len(nb)}, A has {sum(n in na for n in nb)} at the right place, {sum(n in na and na[n]['type'] == nb[n]['type'] for n in nb)} "
              f"with the right type, {len(set(na) - set(nb))} spurious; edges {len(eb)}, A has {len(ea & eb)}, {len(ea - eb)} spurious")
    missed = [d for v in pool.values() for d in v]
    c["missed (in reference only)"] = len(missed)
    print(f"A {len(da)} decisions, B {len(db)}: " + ", ".join(f"{k} {v}" for k, v in c.items()))
    for r in rows:
        print("  F{} {} {}: A {} | B {}".format(*r))
    for d in missed:
        print(f"  missed F{d['floor']} {d['type']} pick {d.get('pick')}")


def record_path_(p):
    return p if os.path.isabs(p) else os.path.join(ROOT, p)


REGIONS = dict(full=(0, 0, 1920, 1080), top=(0, 0, 1920, 64), cards=(440, 380, 1480, 820), center=(240, 100, 1680, 1000), relics=(0, 60, 1920, 110),
               map=(300, 40, 1620, 1080), left=(0, 60, 960, 1080), right=(960, 60, 1920, 1080))


def crop(a):
    from PIL import Image, ImageDraw
    fd = frames_dir(a.creator, a.id)
    have = {secs(f): f for f in os.listdir(fd) if f.endswith(".jpg")}
    ts = []
    for tok in a.at.split(","):
        if "-" in tok:
            lo, hi = (int(x.split(":")[0]) * 60 + int(x.split(":")[1]) for x in tok.split("-"))
            ts += [s for s in sorted(have) if lo <= s <= hi]
        else:
            s = int(tok.split(":")[0]) * 60 + int(tok.split(":")[1])
            ts.append(min(have, key=lambda k: abs(k - s)))
    box = REGIONS.get(a.region) or tuple(int(v) for v in a.region.split(","))
    tiles = []
    for s in ts:
        im = Image.open(os.path.join(fd, have[s])).convert("RGB")
        parts = ([im.crop(REGIONS["top"])] if a.top and a.region != "top" else []) + [im.crop(box)]
        w, h = max(p.width for p in parts), sum(p.height for p in parts)
        t = Image.new("RGB", (w, h))
        y = 0
        for p in parts:
            t.paste(p, (0, y))
            y += p.height
        ImageDraw.Draw(t).rectangle((0, 0, 64, 16), fill="black")
        ImageDraw.Draw(t).text((3, 2), mmss(s), fill="yellow")
        if a.scale != 1.0:
            t = t.resize((int(t.width * a.scale), int(t.height * a.scale)))
        tiles.append(t)
    cols = max(1, min(len(tiles), 1920 // tiles[0].width))
    rows = (len(tiles) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * tiles[0].width, rows * tiles[0].height))
    for i, t in enumerate(tiles):
        sheet.paste(t, ((i % cols) * tiles[0].width, (i // cols) * tiles[0].height))
    sheet.save(a.out, quality=88)
    print(a.out, sheet.size, len(tiles), "frames")


def macro(a):
    import expert as X
    t0 = time.time()
    X.fetch(argparse_ns(url=a.url, creator=a.creator, fps=a.fps, transcript_only=False, dry_run=False))
    vid = re.search(r"(?:v=|youtu\.be/)([\w-]{11})", a.url).group(1)
    creator = a.creator or next(c for c in os.listdir(os.path.join(ROOT, "data", "expert")) if os.path.isdir(frames_dir(c, vid)))
    print(f"fetch {time.time() - t0:.0f} s")
    scan(argparse_ns(id=vid, creator=creator))


def argparse_ns(**kw):
    import argparse
    return argparse.Namespace(**kw)


# ---------------------------------------------------------------- frame scan: screen class per frame -> decision segments

DECISION = ("MAP", "CARD_REWARD", "REWARDS", "SHOP", "RESTSITE", "EVENT", "TREASURE", "DECK")
TH = (32, 18)


def frames_dir(creator, vid):
    return os.path.join(ROOT, "data", "expert", creator, "frames", vid)


def work_dir(creator, vid):
    d = os.path.join(ROOT, "data", "expert", creator, "work", vid)
    os.makedirs(d, exist_ok=True)
    return d


def secs(name):
    s = os.path.splitext(os.path.basename(name))[0].split("_")[0]
    return int(s[:-2]) * 60 + int(s[-2:])


def mmss(s):
    s = int(s)
    return f"{s // 60}:{s % 60:02d}"


def thumbs(creator, vid):
    """per-frame 32x18 RGB thumbnails (cached, git-ignored); webcam corner and top bar kept: the classifier learns around them"""
    import numpy as np
    from PIL import Image
    cache = os.path.join(work_dir(creator, vid), "thumbs.npz")
    fd = frames_dir(creator, vid)
    files = sorted((f for f in os.listdir(fd) if f.endswith(".jpg")), key=secs)
    if os.path.exists(cache):
        z = np.load(cache)
        if len(z["t"]) == len(files):
            return z["t"], z["x"]
    xs = []
    for f in files:
        im = Image.open(os.path.join(fd, f))
        im.draft("RGB", (240, 135))
        xs.append(np.asarray(im.convert("RGB").resize(TH, Image.BILINEAR), np.float32).ravel() / 255.0)
    t, x = np.array([secs(f) for f in files]), np.stack(xs)
    np.savez_compressed(cache, t=t, x=x)
    return t, x


def labels_from_compact(path, t):
    """screen label per frame time from a compact record (decision screens at their t; COMBAT inside each fight)"""
    rows = [json.loads(x) for x in open(path, encoding="utf-8")][1:]
    lab = {}
    tsec = lambda s: int(s.split(":")[0]) * 60 + int(s.split(":")[1]) if isinstance(s, str) and ":" in s else None  # noqa: E731
    last_t = 0
    for i, st in enumerate(rows):
        ts = tsec(st.get("t"))
        if st.get("fight"):
            nxt = next((tsec(x.get("t")) for x in rows[i + 1:] if tsec(x.get("t"))), None)
            if nxt:
                for s in range(last_t + 6, nxt - 6):
                    lab[s] = "COMBAT"
        elif ts is not None:
            scr_ = "MAP" if st["screen"] == "MAP" else st["screen"]
            if scr_ in DECISION or scr_ == "SELECT":
                for s in (ts - 1, ts):
                    lab[s] = "DECK" if scr_ == "SELECT" else scr_
        if ts is not None:
            last_t = ts
    return {s: v for s, v in lab.items() if s in set(t.tolist())}


def exemplars_path():
    return os.path.join(ROOT, "data", "expert", "work", "screen_exemplars.npz")


def load_exemplars():
    import numpy as np
    p = exemplars_path()
    if not os.path.exists(p):
        raise SystemExit(f"no screen exemplars at {p}: run `macro-scan hMrQSndDvPc --creator baalorlord --train data/expert/baalorlord/hMrQSndDvPc.compact.jsonl` "
                         "(frames of a video with a compact record) first")
    z = np.load(p, allow_pickle=True)
    return z["x"], z["y"]


def knn(x, ex, ey, k=5):
    import numpy as np
    from collections import Counter
    out = []
    exn = (ex ** 2).sum(1)
    for i in range(0, len(x), 512):
        b = x[i:i + 512]
        dist = (b ** 2).sum(1)[:, None] + exn[None, :] - 2 * b @ ex.T
        idx = np.argsort(dist, 1)[:, :k]
        for row, dr in zip(idx, np.take_along_axis(dist, idx, 1)):
            c = Counter(ey[row])
            lab, n = c.most_common(1)[0]
            out.append((lab, n / k, float(np.sqrt(max(dr[0], 0)))))
    return out


def segments(t, pred, min_len=2):
    segs, cur = [], None
    for s, (lab, conf, dist) in zip(t.tolist(), pred):
        if cur and lab == cur["cls"] and s - cur["end"] <= 2:
            cur["end"] = s
            cur["n"] += 1
        else:
            if cur:
                segs.append(cur)
            cur = dict(cls=lab, start=s, end=s, n=1)
    if cur:
        segs.append(cur)
    return [g for g in segs if g["cls"] in DECISION and (g["n"] >= min_len or g["cls"] != "MAP")]


def scan(a):
    import numpy as np
    t0 = time.time()
    t, x = thumbs(a.creator, a.id)
    if getattr(a, "train", None):
        lab = labels_from_compact(os.path.join(ROOT, a.train) if not os.path.isabs(a.train) else a.train, t)
        idx = {s: i for i, s in enumerate(t.tolist())}
        keys = sorted(lab)
        X_, Y_ = x[[idx[s] for s in keys]], np.array([lab[s] for s in keys])
        half = len(keys) // 2
        pred = knn(X_[half:], X_[:half], Y_[:half])
        acc = np.mean([p[0] == y for p, y in zip(pred, Y_[half:])])
        from collections import Counter
        print(f"labelled frames {len(keys)} {dict(Counter(Y_))}; train first half -> second half accuracy {acc:.3f}")
        conf = Counter((y, p[0]) for p, y in zip(pred, Y_[half:]) if p[0] != y)
        print("confusions:", dict(conf.most_common(10)))
        os.makedirs(os.path.dirname(exemplars_path()), exist_ok=True)
        np.savez_compressed(exemplars_path(), x=X_, y=Y_)
        print(f"-> {exemplars_path()} ({len(keys)} exemplars)")
    ex, ey = load_exemplars()
    pred = knn(x, ex, ey)
    segs = segments(t, pred)
    wd = work_dir(a.creator, a.id)
    json.dump(dict(video=a.id, frames=len(t), segments=segs), open(os.path.join(wd, "scan.json"), "w"), indent=0)
    with open(os.path.join(wd, "index.txt"), "w", encoding="utf-8") as fh:
        for i, g in enumerate(segs):
            fh.write(f"{i:3d} {mmss(g['start'])}-{mmss(g['end'])} {g['cls']}\n")
    overview(a.creator, a.id, segs)
    from collections import Counter
    print(f"{len(t)} frames, {len(segs)} decision segments {dict(Counter(g['cls'] for g in segs))}; scan {time.time() - t0:.0f} s -> {os.path.relpath(wd, ROOT)}")


def overview(creator, vid, segs, per=16):
    """grids of 16 segments (4x4, 480x270 each: the segment's last frame) in game order: the transcriber's first pass"""
    from PIL import Image, ImageDraw
    fd, out = frames_dir(creator, vid), os.path.join(work_dir(creator, vid), "overview")
    os.makedirs(out, exist_ok=True)
    for f in os.listdir(out):
        os.remove(os.path.join(out, f))
    have = {secs(f): f for f in os.listdir(fd) if f.endswith(".jpg")}
    for p0 in range(0, len(segs), per):
        sheet = Image.new("RGB", (1920, 1080), "black")
        dr = ImageDraw.Draw(sheet)
        for j, g in enumerate(segs[p0:p0 + per]):
            s = min(have, key=lambda k: abs(k - g["end"]))
            x, y = (j % 4) * 480, (j // 4) * 270
            sheet.paste(Image.open(os.path.join(fd, have[s])).convert("RGB").resize((480, 270)), (x, y))
            dr.rectangle((x, y, x + 200, y + 14), fill="black")
            dr.text((x + 3, y + 1), f"#{p0 + j} {mmss(g['start'])}-{mmss(g['end'])} {g['cls']}", fill="yellow")
        sheet.save(os.path.join(out, f"{p0 // per:02d}.jpg"), quality=85)


def sheets(creator, vid, segs):
    """one 2x2 sheet per segment (first, middle, last frame, first frame after) at 960x540 each, for the transcriber"""
    from PIL import Image, ImageDraw
    fd, out = frames_dir(creator, vid), os.path.join(work_dir(creator, vid), "sheets")
    os.makedirs(out, exist_ok=True)
    have = {secs(f): f for f in os.listdir(fd) if f.endswith(".jpg")}
    for i, g in enumerate(segs):
        ts = [g["start"], (g["start"] + g["end"]) // 2, g["end"], g["end"] + 1]
        sheet = Image.new("RGB", (1920, 1080), "black")
        dr = ImageDraw.Draw(sheet)
        for j, s in enumerate(ts):
            s = min(have, key=lambda k: abs(k - s))
            im = Image.open(os.path.join(fd, have[s])).convert("RGB").resize((960, 540))
            sheet.paste(im, ((j % 2) * 960, (j // 2) * 540))
            dr.rectangle(((j % 2) * 960, (j // 2) * 540, (j % 2) * 960 + 70, (j // 2) * 540 + 16), fill="black")
            dr.text(((j % 2) * 960 + 4, (j // 2) * 540 + 2), mmss(s), fill="yellow")
        sheet.save(os.path.join(out, f"{i:03d}_{g['cls']}_{mmss(g['start']).replace(':', '')}.jpg"), quality=85)


# ---------------------------------------------------------------- build: agent transcription -> record

ROOM = {"M": "Monster", "E": "Elite", "R": "RestSite", "$": "Shop", "T": "Treasure", "?": "Unknown"}
ACT_START = (1, 18, 34)
OBS = re.compile(r"(?:(?P<hp>\d+)/(?P<max>\d+))?\s*(?:G(?P<g>\d+))?\s*(?:D(?P<d>\d+))?\s*(?:R(?P<r>\d+))?\s*(?:P\[(?P<p>[^\]]*)\])?")


def parse_obs(s):
    m = OBS.fullmatch(str(s).strip())
    if not m:
        raise ValueError(f"bad obs {s!r} (want e.g. '56/70 G0 D13 R2 P[Dexterity Potion,-]')")
    o = {}
    if m["hp"]:
        o["hp"], o["max_hp"] = int(m["hp"]), int(m["max"])
    for k, g in (("gold", "g"), ("deck_n", "d"), ("relics_n", "r")):
        if m[g]:
            o[k] = int(m[g])
    if m["p"] is not None:
        o["potions"] = [None if x.strip() in ("-", "") else x.strip() for x in m["p"].split(",")]
    return o


def expand(names):
    out = []
    for n in names:
        m = re.match(r"^(.*?)\s+x(\d+)$", str(n))
        out += [m.group(1)] * int(m.group(2)) if m else [n]
    return out


class Track:
    def __init__(self, w):
        self.ch = w["character"]
        st = w["start"]
        o = parse_obs(st.get("obs", ""))
        self.hp, self.max_hp = o.get("hp", st.get("hp")), o.get("max_hp", st.get("max_hp"))
        self.gold = o.get("gold", st.get("gold", 99))
        self.deck = [card(n, self.ch) for n in expand(st["deck"])]
        self.relics = [{"id": thing("relics", r)} for r in st.get("relics", [])]
        self.slots = w["base"].get("max_potion_slots", 2)
        self.potions = (o.get("potions") or [None] * self.slots)[:self.slots]
        self.counters = Counters()
        self.pos, self.monsters, self.seen = {}, {}, {}
        self.errors = []

    def find(self, name, upgraded=None):
        c = card(name, self.ch)
        hits = [x for x in self.deck if x["id"] == c["id"] and (upgraded is None or bool(x.get("upgrade")) == upgraded)]
        if not hits:
            raise ValueError(f"{name!r} not in the tracked deck")
        return hits[0]

    def apply(self, fx, where):
        for n in expand(fx.get("cards+", [])):
            self.deck.append(card(n, self.ch))
        for n in expand(fx.get("cards-", [])):
            c = card(n, self.ch)
            self.deck.remove(self.find(n, bool(c["upgrade"]) if str(n).rstrip().endswith("+") else None))
        for n in fx.get("up", []):
            self.find(n, False)["upgrade"] = 1
        for n, e in fx.get("ench", []):
            x = next((x for x in self.deck if x["id"] == card(n, self.ch)["id"] and not x.get("enchantment")), None)
            if x is None:
                raise ValueError(f"enchant: {n!r} not in the deck un-enchanted")
            x["enchantment"] = {"id": ident(e), "amount": 1}
        for a_, b_ in fx.get("transform", []):
            self.deck.remove(self.find(a_))
            self.deck.append(card(b_, self.ch))
        for r in fx.get("relics+", []):
            self.relics.append({"id": None, "name": r} if str(r).startswith("?") else {"id": thing("relics", r)})
        for r in fx.get("relics-", []):
            rid = thing("relics", r)
            self.relics = [x for x in self.relics if x["id"] != rid]
        for p in fx.get("potions+", []):
            if None in self.potions:
                self.potions[self.potions.index(None)] = p
            else:
                self.errors.append(f"{where}: potion {p} gained with a full belt")
        for p in fx.get("potions-", []):
            if p in self.potions:
                self.potions[self.potions.index(p)] = None
        self.gold += fx.get("gold", 0)
        if "hp" in fx and self.hp is not None:
            self.hp = max(0, min(self.max_hp, self.hp + fx["hp"]))
        if "max_hp" in fx:
            self.max_hp += fx["max_hp"]
            if self.hp is not None:
                self.hp += max(0, fx["max_hp"])
        if "slots" in fx:
            self.slots += fx["slots"]
            self.potions += [None] * fx["slots"]

    def observe(self, o, where):
        if "gold" in o and o["gold"] != self.gold:
            self.errors.append(f"{where}: gold observed {o['gold']} != tracked {self.gold}")
            self.gold = o["gold"]
        if "deck_n" in o and o["deck_n"] != len(self.deck):
            self.errors.append(f"{where}: deck size observed {o['deck_n']} != tracked {len(self.deck)}")
        if "relics_n" in o and o["relics_n"] != len(self.relics):
            self.errors.append(f"{where}: relics observed {o['relics_n']} != tracked {len(self.relics)}")
        if "hp" in o:
            self.hp, self.max_hp = o["hp"], o["max_hp"]
        if "potions" in o:
            self.potions = (list(o["potions"]) + [None] * self.slots)[:max(self.slots, len(o["potions"]))]
            self.slots = len(self.potions)

    def deckview(self, names, where):
        want = sorted((c["id"], c["upgrade"]) for c in (card(n, self.ch) for n in expand(names)))
        have = sorted((c["id"], c["upgrade"]) for c in self.deck)
        if want != have:
            from collections import Counter
            miss, extra = Counter(want) - Counter(have), Counter(have) - Counter(want)
            self.errors.append(f"{where}: deck view differs: seen not tracked {dict(miss)}, tracked not seen {dict(extra)}")


def build(a):
    path = a.work if os.path.isabs(a.work) else os.path.join(ROOT, a.work)
    w = json.load(open(path, encoding="utf-8"))
    tr = Track(w)
    ch, asc = w["character"], w["ascension"]
    acts = {int(k): v for k, v in (w.get("acts") or {}).items()}
    maps = {str(k): v["map"] for k, v in acts.items() if v.get("map")}
    decisions = []
    act = 0
    last_kind = None
    for i, s in enumerate(w["steps"]):
        f = s.get("f")
        act = s.get("act", act if f is None else max(j for j, f0 in enumerate(ACT_START) if f >= f0))
        where = f"step {i} F{f}"
        try:
            if "obs" in s:
                tr.observe(parse_obs(s["obs"]), where)
            if "deckview" in s:
                tr.deckview(s["deckview"], where)
            typ = next((k for k in ("ancient", "event", "map", "card", "rest", "shop", "potion") if k in s), None)
            if typ is not None:
                d = decision(s, typ, tr, act, f, ch, asc, acts, w)
                if d["type"] != "forced":
                    d["k"] = len(decisions)
                    decisions.append(d)
            if "fight" in s:
                k = {"M": "hallway", "E": "elite", "B": "boss"}.get(s["fight"], fight_kind(s["fight"]))
                last_kind = k
                if k == "hallway":
                    tr.monsters[act] = tr.monsters.get(act, 0) + 1
                enc = s.get("enc") or (s["fight"] if len(s["fight"]) > 1 else None)
                if enc:
                    name = (acts.get(act) or {}).get("name") or act_name_of(enc, act)
                    kk = kind_of(name, enc)
                    if kk in ("weak", "regular", "elite"):
                        tr.seen.setdefault(act, {}).setdefault(kk, []).append(enc)
                if k == "boss":
                    tr.post_boss = f
            if "rewards" in s:
                r = s["rewards"]
                room = r.get("potion") and None in tr.potions
                tr.apply(dict(gold=r.get("gold", 0), **({"relics+": [r["relic"]]} if r.get("relic") else {}),
                              **({"potions+": [r["potion"]]} if room else {})), where)
                tr.counters.combat_rewards(last_kind or "hallway", bool(r.get("potion")), [])
                tr.reward_kind = last_kind or "hallway"
            if "gain" in s:
                tr.apply(s["gain"], where)
            if typ is None and "fx" in s:
                tr.apply(s["fx"], where)
        except ValueError as e:
            tr.errors.append(f"{where}: {e}")
    head = dict(video=w["video"], build=w.get("build"), modded=w.get("modded"), seed=w.get("seed"), character=ch, ascension=asc, result=w.get("result"),
                source="frames (agent transcription; deck/relics/gold tracked floor to floor, HP/potions read at the decision)", base=w["base"], maps=maps,
                format=FORMAT)
    for e in tr.errors:
        print("CHECK", e)
    out = a.out or os.path.join(ROOT, "data", "expert", w["video"]["creator"], f"{w['video']['id']}.macro.jsonl")
    if tr.errors and not a.allow:
        raise SystemExit(f"{len(tr.errors)} validation errors: fix the work file (or --allow to record them in each decision's checks)")
    if tr.errors or w.get("checks"):
        head["checks"] = list(w.get("checks", [])) + tr.errors
    write_record(out, head, decisions)


def decision(s, typ, tr, act, f, ch, asc, acts, w):
    o = parse_obs(s["obs"]) if "obs" in s else {}
    info = acts.get(act) or {}
    state = dict(hp=tr.hp, max_hp=tr.max_hp, gold=tr.gold, deck=[dict(c) for c in tr.deck], relics=[dict(r) for r in tr.relics],
                 potions=list(tr.potions), slots=tr.slots, max_energy=w["base"].get("max_energy", 3), act_name=info.get("name"),
                 bosses=info.get("bosses", []), pos=tr.pos.get(act), seen=tr.seen.get(act, {}), monsters=tr.monsters.get(act, 0), **tr.counters.snap())
    if getattr(tr, "post_boss", None) == f:
        state["post_boss"] = True
    checks = [] if "hp" in o or typ == "map" else [f"HP not read at this decision: last observed value used"]
    belt = [p for p in state["potions"]]
    header = header_line(act, f, ch, asc, state["hp"], state["max_hp"], state["gold"], belt)
    pick, then, lines = s.get("pick"), s.get("then"), []
    if typ == "card" and getattr(tr, "reward_kind", None):
        tr.counters.combat_rewards(tr.reward_kind, None, [card(n, ch)["id"] for n in s["card"]])
    if typ == "ancient":
        title = s["ancient"]
        lines = [f"{title}: "] + [f"{i} {x}" for i, x in enumerate(s["opts"])]
        screen = "EVENT"
    elif typ == "event":
        lines = [f"{s['event']}: "] + [f"{i} {x}" for i, x in enumerate(s["opts"])]
        screen = "EVENT"
    elif typ == "card":
        names = s["card"]
        lines = []
        for i, n in enumerate(names):
            c = card(n, ch)
            lines.append(f"{i} {str(n).split('*')[0]}({cost_of(c['id'])}) .")
        lines.append(f"{len(names)} Skip")
        screen = "CARD_REWARD"
    elif typ == "rest":
        lines = ["0 Rest: Heal for 30% of your Max HP.", "1 Smith: Upgrade a card in your Deck."] + [f"{i + 2} {x}" for i, x in enumerate(s.get("extra", []))]
        pick = {"rest": "Rest", "smith": "Smith"}.get(str(s["rest"]).lower(), s["rest"])
        screen = "RESTSITE"
    elif typ == "shop":
        sh = s["shop"]
        i = 0
        for kind in ("cards", "relics", "potions"):
            for name, price in sh.get(kind, []):
                if str(name).startswith("?"):
                    continue
                tail = f"({cost_of(card(name, ch)['id'])}) ." if kind == "cards" else ": ."
                aff = " (can't afford)" if price > tr.gold else ""
                lines.append(f"{i} {price}g {kind[:-1]} {name}{tail}{aff}")
                i += 1
        if sh.get("remove"):
            lines.append(f"{i} {sh['remove']}g remove a card" + (" (can't afford)" if sh["remove"] > tr.gold else ""))
            i += 1
        lines.append(f"{i} leave shop")
        pick = [b[0] if isinstance(b, list) else b for b in s.get("buy", [])]
        screen = "SHOP"
    elif typ == "potion":
        offer = s["potion"]
        lines = [f"0 potion {offer}: . (potion slots full: a dp <slot> first)", "1 proceed (skip the rest)"]
        drop = s.get("drop")
        pick = "leave" if not drop else dict(take=offer, discard_slot=belt.index(drop) if drop in belt else None)
        screen = "REWARDS"
    elif typ == "map":
        opts = s.get("opts")
        key = re.match(r"r(\d+)c(\d+)", s["map"])
        if not opts:
            from agent import routes
            nodes, _ = routes.parse_map(info.get("map", ""))
            here = tr.pos.get(act)
            if here:
                m = re.match(r"r(\d+)c(\d+)", here)
                kids = nodes.get((int(m.group(1)), int(m.group(2))), {}).get("children", [])
            else:
                kids = sorted(k for k in nodes if k[0] == 1)
            opts = [f"{nodes[k]['type']} r{k[0]}c{k[1]}" for k in kids if k in nodes]
        lines = ["full map: m"] + [f"{i} {ROOM.get(o.split()[0], o.split()[0])} {o.split()[1]} -> " for i, o in enumerate(opts)]
        pick = s["map"]
        screen = "MAP"
        if len(opts) < 2:
            tr.pos[act] = s["map"]
            return dict(type="forced")
    d = dict(floor=f, act=act, t=s.get("t"), type=typ, screen=f"{screen}\n{header}\n" + "\n".join(lines) + "\n", pick=pick)
    if then:
        d["then"] = then
    if s.get("note"):
        d["note"] = s["note"]
    d["state"] = state
    if checks:
        d["checks"] = checks
    # effects of the decision on the tracked state
    if typ == "card" and pick:
        tr.deck.append(card(pick, ch))
    elif typ == "rest" and pick == "Rest":
        tr.hp = min(tr.max_hp, tr.hp + int(0.3 * tr.max_hp)) if tr.hp is not None else None
    elif typ == "rest" and pick == "Smith" and then:
        tr.find(then[0], False)["upgrade"] = 1
    elif typ == "shop":
        sh = s["shop"]
        prices = {n.lower(): p for kind in ("cards", "relics", "potions") for n, p in sh.get(kind, [])}
        rm = list(then or [])
        for b in s.get("buy", []):
            b, paid = (b[0], b[1]) if isinstance(b, list) else (b, None)
            if isinstance(b, dict):
                continue
            if str(b).lower().startswith("remove"):
                tr.deck.remove(tr.find(rm.pop(0)))
                tr.gold -= sh["remove"] if paid is None else paid
                tr.counters.removals += 1
                continue
            tr.gold -= prices[str(b).lower()] if paid is None else paid
            kind = next(k for k in ("cards", "relics", "potions") if any(n.lower() == str(b).lower() for n, _ in sh.get(k, [])))
            if not (kind == "potions" and str(b).startswith("?")):
                tr.apply({f"{kind}+": [b]}, f"F{f} shop")
    elif typ == "potion" and isinstance(pick, dict) and pick.get("discard_slot") is not None:
        tr.potions[pick["discard_slot"]] = s["potion"]
    elif typ == "map":
        tr.pos[act] = s["map"]
    elif typ == "ancient" and pick:
        tr.apply({"relics+": [pick]}, f"F{f} ancient")
    if "fx" in s:
        tr.apply(s["fx"], f"F{f} {typ}")
    return d
