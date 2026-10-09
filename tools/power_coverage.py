#!/usr/bin/env python3
"""Powers in the observation and in the training data (E45). Read-only, CPU.

probe:  observation vs simulator powers on replayed fights; scripted power states; encoding channels; unported power ids
embed:  PowerPool / card embedding rows the r5 -> r6 training never moved
stats:  power coverage of decision states: collection parts (trained rows), greedy play on scenario files, expert replay logs
"""
import argparse, glob, json, os, sys
from collections import Counter, defaultdict

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [os.path.join(ROOT, "rl")]
import sts2  # noqa: E402

LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}
NM = sts2.names()
PNAME, CNAME = NM["power"], NM["card"]
PID = {n: i for i, n in enumerate(PNAME)}
P, PF, EF, CF, E = C["OBS_POWERS"], C["POWER_F"], C["ENEMY_F"], C["CARD_F"], C["OBS_MAX_ENEMIES"]
CAT = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
CTYPE = {c["id"]: c["type"] for pool in CAT["cards"].values() for c in pool}
IS_POWER_CARD = np.array([CTYPE.get(n) == "Power" for n in CNAME] + [False])
DEBUFF = {"WEAK_POWER", "VULNERABLE_POWER", "FRAIL_POWER", "POISON_POWER", "DOOM_POWER", "CONSTRICT_POWER", "TANGLED_POWER", "NO_DRAW_POWER",
          "NO_BLOCK_POWER", "CONFUSED_POWER", "HEX_POWER", "SHRINK_POWER", "CHAINS_OF_BINDING_POWER", "NO_ENERGY_GAIN_POWER", "DEBILITATE_POWER",
          "SMOGGY_POWER", "STRANGLE_POWER", "DISINTEGRATION_POWER", "MIND_ROT_POWER", "WASTE_AWAY_POWER", "SLOTH_POWER", "RINGING_POWER", "KNOCKDOWN_POWER"}
BUFF_ID = np.array([n not in DEBUFF for n in PNAME])


def cid(c):
    return c if isinstance(c, str) else c["id"]


def S(x):
    return np.sign(x) * np.log1p(np.abs(x))


# ---------------------------------------------------------------- observation decoding
def dec_powers(blk):
    """(..., P*PF) -> ids (-1 empty), amount, display"""
    b = blk.reshape(*blk.shape[:-1], P, PF)
    return b[..., 0].astype(np.int64) - 1, b[..., 1], b[..., 2]


def decode(obs):
    o, s = SEC["player"]
    pid, pam, _ = dec_powers(obs[:, o + 8:o + 8 + P * PF])
    o, s = SEC["enemies"]
    en = obs[:, o:o + s].reshape(len(obs), E, EF)
    alive = (en[..., 0] > 0.5) & (en[..., 6] > 0.5)
    eid, eam, _ = dec_powers(en[..., 8:8 + P * PF])
    o, s = SEC["hand"]
    hand = obs[:, o:o + s].reshape(len(obs), -1, CF)[..., 0].astype(np.int64) - 1
    return dict(pid=pid, pam=pam, eid=np.where(alive[..., None], eid, -1), eam=eam, hand=hand, rnd=obs[:, SEC["global"][0]].astype(np.int64))


# ---------------------------------------------------------------- statistics
STACKS = [("player", "STRENGTH_POWER", 5), ("player", "STRENGTH_POWER", 10), ("player", "DEXTERITY_POWER", 3), ("player", "FOCUS_POWER", 1),
          ("player", "FOCUS_POWER", 3), ("player", "VIGOR_POWER", 1), ("player", "PLATING_POWER", 1), ("player", "THORNS_POWER", 1),
          ("player", "NOXIOUS_FUMES_POWER", 1), ("player", "ACCELERANT_POWER", 1), ("player", "AFTERIMAGE_POWER", 1), ("player", "SERPENT_FORM_POWER", 1),
          ("player", "DEMON_FORM_POWER", 1), ("player", "FEEL_NO_PAIN_POWER", 1), ("player", "RITUAL_POWER", 1),
          ("enemy", "POISON_POWER", 1), ("enemy", "POISON_POWER", 10), ("enemy", "POISON_POWER", 20), ("enemy", "POISON_POWER", 40),
          ("enemy", "DOOM_POWER", 1), ("enemy", "DOOM_POWER", 20), ("enemy", "WEAK_POWER", 1), ("enemy", "VULNERABLE_POWER", 1)]


class Acc:
    def __init__(self):
        self.n = 0
        self.f = Counter()
        self.pp = Counter()   # states with power on the player
        self.ep = Counter()   # states with power on a living enemy
        self.hc = Counter()   # states with power card in hand
        self.dc = Counter()   # states whose fight deck holds the card
        self.fights = 0
        self.fdeck = Counter()  # fights whose deck holds the power card
        self.fdeck_n = []
        self.conv = Counter()  # states with the power card in the deck and its same-name power on the player
        self.rounds = Counter()

    def add(self, d, deck_pc):
        """d: decoded block of one fight's states; deck_pc: the fight deck's power-card ids (list, with repeats)"""
        n = len(d["pid"])
        if n == 0:
            return
        self.n += n
        self.fights += 1
        self.fdeck_n.append(len(deck_pc))
        for c in set(deck_pc):
            self.fdeck[c] += 1
            self.dc[c] += n
        npc = len(deck_pc)
        self.f["deck_pc>=1"] += n * (npc >= 1)
        self.f["deck_pc>=3"] += n * (npc >= 3)
        self.f["deck_pc>=5"] += n * (npc >= 5)
        hand = d["hand"]
        hp = IS_POWER_CARD[np.where(hand >= 0, hand, -1)] & (hand >= 0)
        self.f["hand_pc>=1"] += int(hp.any(1).sum())
        for c, k in Counter(CNAME[c] for r in range(n) for c in set(hand[r][hp[r]].tolist())).items():
            self.hc[c] += k
        pid, pam, eid, eam = d["pid"], d["pam"], d["eid"], d["eam"]
        for r in d["rnd"].tolist():
            self.rounds[min(r, 12)] += 1
        for c in set(deck_pc):
            i = PID.get(c + "_POWER")
            if i is not None:
                self.conv[c] += int((pid == i).any(1).sum())
        has = pid >= 0
        self.f["player_any"] += int(has.any(1).sum())
        buff = has & BUFF_ID[np.where(has, pid, 0)]
        self.f["player_buff"] += int(buff.any(1).sum())
        self.f["player_buff>=3"] += int((buff.sum(1) >= 3).sum())
        self.f["enemy_debuff_any"] += int(((eid >= 0) & ~BUFF_ID[np.where(eid >= 0, eid, 0)]).any((1, 2)).sum())
        for side, name, th in STACKS:
            i = PID[name]
            if side == "player":
                v = np.where(pid == i, pam, 0).sum(1)
            else:
                v = np.where(eid == i, eam, 0).max((1, 2))
            self.f[f"{side}:{name[:-6]}>={th}"] += int((v >= th).sum())
        for ids, ctr in ((pid, self.pp), (eid.reshape(n, -1), self.ep)):
            u = [np.unique(r[r >= 0]) for r in ids]
            for k, cnt in Counter(np.concatenate(u).tolist() if u else []).items():
                ctr[PNAME[k]] += cnt

    def summary(self):
        n = max(self.n, 1)
        return dict(states=self.n, fights=self.fights, deck_pc_mean=float(np.mean(self.fdeck_n)) if self.fdeck_n else 0.0,
                    share={k: v / n for k, v in sorted(self.f.items())},
                    player_power={k: v / n for k, v in self.pp.most_common()}, enemy_power={k: v / n for k, v in self.ep.most_common()},
                    hand_card={k: v / n for k, v in self.hc.most_common()}, deck_card_states={k: v / n for k, v in self.dc.most_common()},
                    deck_card_fights={k: v / max(self.fights, 1) for k, v in self.fdeck.most_common()},
                    active_given_deck={k: (self.conv[k] / self.dc[k], self.dc[k]) for k in self.conv if self.dc[k]},
                    round_mean=sum(k * v for k, v in self.rounds.items()) / max(sum(self.rounds.values()), 1),
                    round_ge5=sum(v for k, v in self.rounds.items() if k >= 5) / max(sum(self.rounds.values()), 1))


def deck_pc(deck):
    return [cid(c) for c in deck if CTYPE.get(cid(c)) == "Power"]


def acc_for(groups, key):
    return groups.setdefault(key, Acc())


# ---------------------------------------------------------------- sources
def stats_parts(paths, groups, every):
    for pi, path in enumerate(paths):
        if pi % every:
            continue
        try:
            o, _, dfi, scen, z = part_rows(path)
        except Exception as ex:  # noqa: BLE001
            print(f"{os.path.basename(path)}: replay failed {str(ex)[:80]}", file=sys.stderr)
            continue
        lo = np.searchsorted(dfi, np.arange(len(z["f_scen"]) + 1))
        d = decode(o)
        del o
        for f in range(len(z["f_scen"])):
            sc = scen[z["f_scen"][f]]
            a, b = lo[f], lo[f + 1]
            blk = {k: v[a:b] for k, v in d.items()}
            dpc = deck_pc(sc["deck"])
            src = sc.get("meta", {}).get("source", "?")
            for key in ("ALL", sc["character"], f"src:{src}", f"{sc['character']}:{'loss' if z['f_cls'][f] == 0 else 'win'}"):
                acc_for(groups, key).add(blk, dpc)
        print(f"{os.path.basename(path)} {len(dfi)} rows", file=sys.stderr, flush=True)


def stats_play(path, groups, ckpt, n_max, seeds, label, full_only=False):
    import torch
    import model as M
    data = json.load(open(path))
    scs = [x["scenario"] if "scenario" in x else x for x in data]
    if full_only:
        scs = [sc for sc in scs if sc.get("meta", {}).get("base") is None and sc.get("meta", {}).get("enabler") is None]
    if n_max and len(scs) > n_max:
        r = np.random.default_rng(0)
        scs = [scs[i] for i in sorted(r.choice(len(scs), n_max, replace=False))]
    net = M.load(ckpt)
    act = M.net_policy(net, greedy=True)
    for seed in range(seeds):
        env = sts2.VecEnv(len(scs), scs, seed=1000 + seed, round_robin=True, max_steps=600)
        obs, mask = env.reset()
        live = np.ones(len(scs), bool)
        rows = [[] for _ in scs]
        while live.any():
            for i in np.flatnonzero(live):
                rows[i].append(obs[i].copy())
            with torch.no_grad():
                a = act(obs, mask)
            obs, mask, _, done, _ = env.step(a)
            live &= ~done.astype(bool)
        for i, sc in enumerate(scs):
            d = decode(np.stack(rows[i]))
            for key in (f"{label}:ALL", f"{label}:{sc['character']}"):
                acc_for(groups, key).add(d, deck_pc(sc["deck"]))
        print(f"{label} seed {seed}: {sum(map(len, rows))} states", file=sys.stderr, flush=True)


def json_state(st, deck):
    """an expert replay-log state -> the decoded layout (one row)"""
    pid = np.full((1, P), -1)
    pam = np.zeros((1, P))
    for k, p in enumerate(st["player"].get("powers", [])[:P]):
        pid[0, k], pam[0, k] = PID.get(p["id"], -1), p.get("amount", 0)
    eid = np.full((1, E, P), -1)
    eam = np.zeros((1, E, P))
    for j, e in enumerate([e for e in st.get("enemies", []) if e.get("alive", True)][:E]):
        for k, p in enumerate(e.get("powers", [])[:P]):
            eid[0, j, k], eam[0, j, k] = PID.get(p["id"], -1), p.get("amount", 0)
    ci = {n: i for i, n in enumerate(CNAME)}
    hand = np.full((1, 10), -1)
    for k, c in enumerate(st.get("hand", [])[:10]):
        hand[0, k] = ci.get(c["id"], -1)
    return dict(pid=pid, pam=pam, eid=eid, eam=eam, hand=hand, rnd=np.array([st.get("round", 0)]))


def stats_expert(paths, groups, label):
    unknown = Counter()
    for path in paths:
        x = json.load(open(path))
        sc = x["scenario"]
        sts = [s for s in x["fight"].get("states", []) if s.get("combat_in_progress", True) and s.get("enemies")]
        if not sts:
            continue
        for s in sts:
            for p in s["player"].get("powers", []) + [p for e in s["enemies"] for p in e.get("powers", [])]:
                if p["id"] not in PID:
                    unknown[p["id"]] += 1
        ds = [json_state(s, sc["deck"]) for s in sts]
        d = {k: np.concatenate([q[k] for q in ds]) for k in ds[0]}
        for key in (f"{label}:ALL", f"{label}:{sc['character']}"):
            acc_for(groups, key).add(d, deck_pc(sc["deck"]))
    if unknown:
        print(f"{label}: power ids outside the vocabulary {dict(unknown)}", file=sys.stderr)


def deck_only(path, groups, label, by_source=True):
    """fight-level deck stats of a scenario list (no states)"""
    data = json.load(open(path))
    for x in data:
        sc = x["scenario"] if "scenario" in x else x
        src = sc.get("meta", {}).get("source", "?")
        dpc = deck_pc(sc["deck"])
        keys = [f"{label}:ALL", f"{label}:{sc['character']}"] + ([f"{label}:src:{src}"] if by_source else [])
        for key in keys:
            a = groups.setdefault(key, Acc())
            a.fights += 1
            a.fdeck_n.append(len(dpc))
            a.f["deck_size"] += len(sc["deck"])
            for c in set(dpc):
                a.fdeck[c] += 1
            a.f["deck_pc>=1"] += len(dpc) >= 1
            a.f["deck_pc>=3"] += len(dpc) >= 3
            a.f["deck_pc>=5"] += len(dpc) >= 5
    del data
    for k, a in groups.items():
        if k.startswith(label + ":") and a.n == 0:
            a.n = a.fights  # shares per fight


# ---------------------------------------------------------------- probe
def probe(a):
    out = {}
    # 1. unported power ids: listener classes vs the id list
    import re
    cls = set()
    for f in glob.glob(os.path.join(ROOT, "crates/sts2sim/src/content/**/*.rs"), recursive=True):
        cls |= set(re.findall(r"^listener!\((\w+)", open(f, encoding="utf8").read(), flags=re.M))
    snake = lambda s: re.sub(r"(?<!^)(?=[A-Z])", "_", s).upper()  # noqa: E731
    have = {snake(c) for c in cls}
    out["power_ids_without_listener"] = [n for n in PNAME if n not in have]
    # 2. encoding channels
    out["channels"] = {f"{n}={v}": [1.0, round(float(S(v) / 2), 3), round(float(np.clip(v, -10, 10) / 10), 3)]
                       for n, v in [("Str", 3), ("Str", 10), ("Str", 30), ("Str", -2), ("Poison", 10), ("Poison", 30), ("Poison", 99), ("Doom", 60), ("Doom", 150)]}
    # 3. observation vs simulator on replayed collection fights
    mism, checked, states = Counter(), Counter(), 0
    examples = []
    for path in sorted(glob.glob(a.parts))[:a.probe_parts]:
        z = np.load(path)
        scen = json.loads(str(z["scenarios"]))
        for f in range(0, len(z["f_scen"]), a.probe_every):
            sc = scen[z["f_scen"][f]]
            sim = sts2.Sim(json.dumps(sc), int(z["f_seed"][f]))
            ob = np.zeros(sts2.OBS_SIZE, np.float32)
            mk = np.zeros(sts2.ACTIONS, np.uint8)
            for t in range(z["f_off"][f], z["f_off"][f + 1] + 1):
                sim.observe(ob, mk)
                snap = json.loads(sim.snapshot())
                states += 1
                o, _ = SEC["player"]
                pid, pam, pdisp = dec_powers(ob[o + 8:o + 8 + P * PF])
                obs_p = [(PNAME[i], int(x)) for i, x in zip(pid, pam) if i >= 0]
                sim_p = [(p["id"], p["amount"]) for p in snap["player"]["powers"]]
                checked["player"] += 1
                if obs_p != sim_p:
                    mism["player"] += 1
                    examples.append(("player", sc["name"], obs_p, sim_p))
                eo, _ = SEC["enemies"]
                en = ob[eo:eo + E * EF].reshape(E, EF)
                for j, e in enumerate(snap["enemies"][:E]):
                    eid, eam, _ = dec_powers(en[j, 8:8 + P * PF])
                    obs_e = [(PNAME[i], int(x)) for i, x in zip(eid, eam) if i >= 0]
                    sim_e = [(p["id"], p["amount"]) for p in e["powers"]]
                    checked["enemy"] += 1
                    if NM["monster"][int(en[j, 2]) - 1] != e["id"] or obs_e != sim_e:
                        mism["enemy"] += 1
                        examples.append(("enemy", sc["name"], obs_e, sim_e))
                for pet in [q for q in snap.get("pets", []) if q["id"] == "OSTY"][:1]:
                    oo, _ = SEC["osty"]
                    oid, oam, _ = dec_powers(ob[oo + 4:oo + 4 + P * PF])
                    obs_o = [(PNAME[i], int(x)) for i, x in zip(oid, oam) if i >= 0]
                    checked["osty"] += 1
                    if obs_o != [(p["id"], p["amount"]) for p in pet.get("powers", [])]:
                        mism["osty"] += 1
                        examples.append(("osty", sc["name"], obs_o, pet.get("powers")))
                if t < z["f_off"][f + 1]:
                    sim.step(int(z["acts"][t]))
    out["obs_vs_sim"] = dict(states=states, checked=dict(checked), mismatches=dict(mism), examples=examples[:8])
    # 4. scripted: play every power card of a deck, show the resulting slots and the card's features in hand
    scripted = []
    for ch, deck, enc in [("SILENT", ["NOXIOUS_FUMES", "ACCELERANT", "AFTERIMAGE", "DEADLY_POISON", "DEADLY_POISON", "SERPENT_FORM", "FOOTWORK"], "SKULKING_COLONY_ELITE"),
                          ("IRONCLAD", ["INFLAME", "DEMON_FORM", "FEEL_NO_PAIN", "DISARM", "BARRICADE"], "SKULKING_COLONY_ELITE"),
                          ("DEFECT", ["DEFRAGMENT", "CAPACITOR", "BIASED_COGNITION", "ZAP", "COOLHEADED"], "SKULKING_COLONY_ELITE"),
                          ("NECROBINDER", ["DEATHS_DOOR", "NEGATIVE_PULSE", "BLIGHT_STRIKE", "REAPER_FORM", "DEMISE"], "SKULKING_COLONY_ELITE")]:
        deck = [d for d in deck if d in CTYPE]
        sc = dict(name="probe", ascension=10, encounter=enc, character=ch, hp=70, max_hp=70, max_energy=9, gold=0, max_potion_slots=2,
                  base_orb_slots=3 if ch == "DEFECT" else 0, seed="probe", total_floor=5, act=0, deck=deck, relics=[], potions=[])
        try:
            sim = sts2.Sim(json.dumps(sc), 1)
        except Exception as ex:  # noqa: BLE001
            scripted.append((ch, f"cannot build: {str(ex)[:100]}"))
            continue
        ob = np.zeros(sts2.OBS_SIZE, np.float32)
        mk = np.zeros(sts2.ACTIONS, np.uint8)
        sim.observe(ob, mk)
        ho, _ = SEC["hand"]
        hand_f = {CNAME[int(r[0]) - 1]: r[1:].astype(int).tolist() for r in ob[ho:ho + 150].reshape(10, CF) if r[0] > 0}
        for _ in range(12):
            leg = sorted([(i, s) for i, s in sim.legal() if s.startswith("play")], key=lambda x: CTYPE.get(x[1].split()[1]) != "Power")
            if not leg:
                break
            sim.step(leg[0][0])
            while sim.stage() == "choice":
                sim.step(sim.legal()[0][0])
        sim.observe(ob, mk)
        snap = json.loads(sim.snapshot())
        o, _ = SEC["player"]
        pid, pam, pdisp = dec_powers(ob[o + 8:o + 8 + P * PF])
        eo, _ = SEC["enemies"]
        eid, eam, _ = dec_powers(ob[eo + 8:eo + 8 + P * PF])
        scripted.append(dict(character=ch, hand_features_at_start=hand_f,
                             obs_player=[(PNAME[i], int(x), int(y)) for i, x, y in zip(pid, pam, pdisp) if i >= 0],
                             sim_player=[(p["id"], p["amount"]) for p in snap["player"]["powers"]],
                             obs_enemy0=[(PNAME[i], int(x)) for i, x in zip(eid, eam) if i >= 0],
                             sim_enemy0=[(p["id"], p["amount"]) for p in (snap["enemies"][0]["powers"] if snap["enemies"] else [])]))
    out["scripted"] = scripted
    print(json.dumps(out, indent=1, default=str))


def embed(a):
    import torch
    r5 = torch.load(a.old, map_location="cpu")
    r6 = torch.load(a.new, map_location="cpu")
    r5, r6 = r5.get("net", r5), r6.get("net", r6)
    res = {}
    for key, names, n in (("pp.bag.emb.weight", PNAME, len(PNAME) + 1), ("card.card.weight", CNAME, None)):
        w5, w6 = r5[key].float(), r6[key].float()
        moved = (w5 - w6).abs().amax(1) > 0
        if n:  # 4 channels x (N+1) rows
            moved = moved.view(-1, n)[:, 1:].any(0)
            norm = w6.view(-1, n, w6.shape[1])[:, 1:].norm(dim=2).mean(0)
        else:
            moved = moved[1:]
            norm = w6[1:].norm(dim=1)
        res[key] = dict(rows=len(names), unmoved=int((~moved).sum()), unmoved_ids=[names[i] for i in np.flatnonzero(~moved.numpy())],
                        norm_moved=float(norm[moved].mean()) if moved.any() else 0.0, norm_unmoved=float(norm[~moved].mean()) if (~moved).any() else 0.0)
    print(json.dumps(res, indent=1))


def part_rows(path):
    """a collection part's trained decision rows: obs, the row's fight, the part's arrays"""
    z = np.load(path)
    scen = json.loads(str(z["scenarios"]))
    f_scen, f_seed, f_off, acts = z["f_scen"], z["f_seed"], z["f_off"], z["acts"].astype(np.int32)
    order = np.argsort(z["d_fight"], kind="stable")
    dfi, dst = z["d_fight"][order], z["d_step"][order]
    lo = np.searchsorted(dfi, np.arange(len(f_scen) + 1))
    uniq, inv = np.unique(f_scen, return_inverse=True)
    off = np.concatenate([[0], np.cumsum(f_off[1:] - f_off[:-1])]).tolist()
    o, m = sts2.replay_rows([scen[u] for u in uniq], inv, f_seed, acts, off, dst, lo.tolist())
    return o, m, dfi, scen, z


def p_win(net, obs, bs=4096):
    import torch
    out = []
    with torch.no_grad():
        for i in range(0, len(obs), bs):
            ol = net.heads_out(torch.from_numpy(np.ascontiguousarray(obs[i:i + bs])))
            out.append(1.0 - torch.softmax(ol.float(), 1)[:, 0].numpy())
    return np.concatenate(out) if out else np.zeros(0)


CALIB_NAMED = ["NOXIOUS_FUMES_POWER", "ACCELERANT_POWER", "AFTERIMAGE_POWER", "SERPENT_FORM_POWER", "DEMON_FORM_POWER", "FEEL_NO_PAIN_POWER",
               "BARRICADE_POWER", "DARK_EMBRACE_POWER", "RUPTURE_POWER", "INFERNO_POWER", "CRIMSON_MANTLE_POWER", "COUNTDOWN_POWER", "HAUNT_POWER",
               "CALCIFY_POWER", "SHROUD_POWER", "HAILSTORM_POWER", "THUNDER_POWER", "INFINITE_BLADES_POWER", "ACCURACY_POWER", "TOOLS_OF_THE_TRADE_POWER",
               "FURNACE_POWER", "CHILD_OF_THE_STARS_POWER", "BLACK_HOLE_POWER", "PILLAR_OF_CREATION_POWER", "ARSENAL_POWER", "PLATING_POWER"]


def row_features(obs, d):
    o, _ = SEC["enemies"]
    en = obs[:, o:o + E * EF].reshape(len(obs), E, EF)
    alive = (en[..., 0] > 0.5) & (en[..., 6] > 0.5)
    hp = np.where(alive, en[..., 3], 0)
    pid, pam, eid, eam = d["pid"], d["pam"], d["eid"], d["eam"]
    amt = lambda i: np.where(pid == i, pam, 0).sum(1)  # noqa: E731
    eamt = lambda i: np.where(eid == i, eam, 0).sum(2)  # noqa: E731
    poison, doom = eamt(PID["POISON_POWER"]), eamt(PID["DOOM_POWER"])
    has = pid >= 0
    hand = d["hand"]
    return dict(poison=poison.max(1), poison_lethal=((poison * (poison + 1) / 2 >= hp) & alive & (poison > 0)).any(1),
                doom=doom.max(1), doom_lethal=((doom >= hp) & alive & (doom > 0)).any(1), strength=amt(PID["STRENGTH_POWER"]),
                focus=amt(PID["FOCUS_POWER"]), buff=(has & BUFF_ID[np.where(has, pid, 0)]).sum(1),
                hand_pc=(IS_POWER_CARD[np.where(hand >= 0, hand, -1)] & (hand >= 0)).sum(1),
                named={n: amt(PID[n]) > 0 for n in CALIB_NAMED}, n_alive=alive.sum(1))


def calib(a):
    import model as M
    net = M.load(a.ckpt)
    rows = defaultdict(lambda: [0, 0.0, 0.0, 0.0])

    def put(key, mask, p, y):
        if mask.any():
            r = rows[key]
            r[0] += int(mask.sum())
            r[1] += float(p[mask].sum())
            r[2] += float(y[mask].sum())
            r[3] += float(((p[mask] - y[mask]) ** 2).sum())

    for pi, path in enumerate(sorted(glob.glob(a.parts))):
        if pi % a.every:
            continue
        o, _, dfi, scen, z = part_rows(path)
        d = decode(o)
        F = row_features(o, d)
        p = p_win(net, o)
        del o
        y = (z["f_cls"][dfi] > 0).astype(np.float64)
        fsc = [scen[z["f_scen"][f]] for f in range(len(z["f_scen"]))]
        src = np.array([sc.get("meta", {}).get("source", "?") for sc in fsc])[dfi]
        ch = np.array([sc["character"] for sc in fsc])[dfi]
        npc = np.array([len(deck_pc(sc["deck"])) for sc in fsc])[dfi]
        put("all", np.ones(len(p), bool), p, y)
        for v in np.unique(src):
            put(f"src={v}", src == v, p, y)
        for v in np.unique(ch):
            put(f"char={v}", ch == v, p, y)
        for lo_, hi_ in ((0, 1), (1, 10), (10, 20), (20, 40), (40, 10 ** 6)):
            put(f"enemy poison {lo_}-{hi_ - 1}", (F["poison"] >= lo_) & (F["poison"] < hi_), p, y)
        put("poison p(p+1)/2 >= hp on some enemy", F["poison_lethal"], p, y)
        put("doom > 0, below hp", (F["doom"] > 0) & ~F["doom_lethal"], p, y)
        put("doom >= hp on some enemy", F["doom_lethal"], p, y)
        for lo_, hi_ in ((-99, 1), (1, 5), (5, 10), (10, 999)):
            put(f"player strength {lo_}..{hi_ - 1}", (F["strength"] >= lo_) & (F["strength"] < hi_), p, y)
        put("player focus >= 1", F["focus"] >= 1, p, y)
        put("player focus >= 3", F["focus"] >= 3, p, y)
        for k in range(5):
            put(f"player buffs {k}{'+' if k == 4 else ''}", (F["buff"] >= 4) if k == 4 else (F["buff"] == k), p, y)
        for lo_, hi_ in ((0, 1), (1, 3), (3, 5), (5, 99)):
            put(f"deck power cards {lo_}-{hi_ - 1}", (npc >= lo_) & (npc < hi_), p, y)
        put("power card in hand", F["hand_pc"] >= 1, p, y)
        for n, msk in F["named"].items():
            put(f"has {n[:-6]}", msk, p, y)
        print(f"{os.path.basename(path)} {len(p)} rows", file=sys.stderr, flush=True)
    out = {k: dict(n=v[0], p=v[1] / v[0], won=v[2] / v[0], bias=(v[1] - v[2]) / v[0], brier=v[3] / v[0]) for k, v in rows.items()}
    json.dump(out, open(a.out, "w"), indent=1)
    for k, v in out.items():
        print(f"{k:48s} n {v['n']:8d}  p {v['p']:.3f}  won {v['won']:.3f}  bias {v['bias']:+.3f}  brier {v['brier']:.4f}")


def set_power(obs, base, pid, amount):
    """write power `pid` x `amount` into the power block at `base` (replace it or take the first empty slot); amount 0 removes it"""
    blk = obs[:, base:base + P * PF].reshape(len(obs), P, PF).copy()
    ids = blk[..., 0].astype(np.int64) - 1
    at = np.where((ids == pid).any(1), np.argmax(ids == pid, 1), np.argmax(ids < 0, 1))
    r = np.arange(len(obs))
    if amount == 0:
        keep = ids[r, at] == pid
        blk[r[keep], at[keep]] = 0
    else:
        blk[r, at, 0], blk[r, at, 1], blk[r, at, 2] = pid + 1, amount, 0
    obs[:, base:base + P * PF] = blk.reshape(len(obs), -1)


def sens(a):
    import model as M
    net = M.load(a.ckpt)
    eo = SEC["enemies"][0]
    po = SEC["player"][0] + 8
    got, paths = [], sorted(glob.glob(a.parts))
    for path in paths:
        if a.end_only:  # every state of every fight: forced end-turn states are not trained rows
            z = np.load(path)
            scen = json.loads(str(z["scenarios"]))
            om = [sts2.replay(scen[z["f_scen"][f]], int(z["f_seed"][f]), z["acts"][z["f_off"][f]:z["f_off"][f + 1]].astype(np.int32))
                  for f in range(len(z["f_scen"]))]
            o, mk = np.concatenate([x[0] for x in om]), np.concatenate([x[1] for x in om])
            del om
        else:
            o, mk, dfi, scen, z = part_rows(path)
        d = decode(o)
        F = row_features(o, d)
        play = o[:, SEC["decision"][0]] < 0.5
        single = (F["n_alive"] == 1) & play & (o[:, eo] > 0.5) & (o[:, eo + 6] > 0.5) & (o[:, eo + 3] <= 400)
        if a.end_only:  # nothing left to play: doom = hp vs hp - 1 is a kill vs no kill
            single &= mk[:, C["OFF_PLAY"]:C["OFF_DISCARD"]].sum(1) == 0
        clean = ~(d["eid"][:, 0] >= 0).any(1) & ~(d["pid"] >= 0).any(1)
        got.append(o[np.flatnonzero(single & clean)].copy())
        del o
        if sum(map(len, got)) >= a.n:
            break
    X = np.concatenate(got)
    X = X[np.random.default_rng(0).choice(len(X), min(a.n, len(X)), replace=False)]
    path = f"{len(got)} parts"
    hp = X[:, eo + 3].copy()
    res = {}

    def run(name, edits):
        Y = X.copy()
        for base, pid, amt in edits:
            amt = np.broadcast_to(np.asarray(amt), (len(Y),))
            for v in np.unique(amt):
                m = amt == v
                Z = Y[m]
                set_power(Z, base, pid, int(v))
                Y[m] = Z
        pw = p_win(net, Y)
        res[name] = float(pw.mean())
        return pw

    run("baseline: one enemy, no powers anywhere", [])
    d9 = run("enemy doom hp-1", [(eo + 8, PID["DOOM_POWER"], np.maximum(1, hp - 1).astype(int))])
    d0 = run("enemy doom = hp (dies at its turn end)", [(eo + 8, PID["DOOM_POWER"], hp.astype(int))])
    res["doom hp vs hp-1: mean delta"] = float((d0 - d9).mean())
    res["doom hp vs hp-1: share of rows delta > 0.05"] = float(((d0 - d9) > 0.05).mean())
    run("player accelerant 1 alone", [(po, PID["ACCELERANT_POWER"], 1)])
    for v in (5, 10, 20, 40, 80):
        run(f"enemy poison {v}", [(eo + 8, PID["POISON_POWER"], v)])
    lethal = np.ceil((np.sqrt(8 * hp + 1) - 1) / 2).astype(int)
    for f in (0.5, 0.9, 1.0, 1.5):
        run(f"enemy poison {f}x the lethal stack", [(eo + 8, PID["POISON_POWER"], np.maximum(1, (lethal * f).astype(int)))])
    for f in (0.5, 0.9, 1.0, 1.1, 2.0):
        run(f"enemy doom {f}x hp", [(eo + 8, PID["DOOM_POWER"], np.maximum(1, (hp * f).astype(int)))])
    for v in (-3, 3, 6, 10, 20):
        run(f"player strength {v} (card previews unchanged)", [(po, PID["STRENGTH_POWER"], v)])
    for v in (1, 2, 4):
        run(f"player noxious fumes {v}", [(po, PID["NOXIOUS_FUMES_POWER"], v)])
    for v in (1, 2, 4):
        run(f"enemy poison 10 + player accelerant {v}", [(eo + 8, PID["POISON_POWER"], 10), (po, PID["ACCELERANT_POWER"], v)])
    for v in (1, 2, 3):
        run(f"player demon form {v}", [(po, PID["DEMON_FORM_POWER"], v)])
        run(f"player serpent form {v}", [(po, PID["SERPENT_FORM_POWER"], v)])
        run(f"player afterimage {v}", [(po, PID["AFTERIMAGE_POWER"], v)])
    for v in (5, 10):
        run(f"player plating {v}", [(po, PID["PLATING_POWER"], v)])
    run("player intangible 1", [(po, PID["INTANGIBLE_POWER"], 1)])
    run("player barricade 1", [(po, PID["BARRICADE_POWER"], 1)])
    for v in (1, 3):
        run(f"player focus {v}", [(po, PID["FOCUS_POWER"], v)])
    run("player wraith form 2 (embedding never trained)", [(po, PID["WRAITH_FORM_POWER"], 2)])
    run("player biased cognition 4 (embedding never trained)", [(po, PID["BIASED_COGNITION_POWER"], 4)])
    print(json.dumps(dict(rows=len(X), part=path, enemy_hp_mean=float(hp.mean()), p_win=res), indent=1))
    json.dump(dict(rows=len(X), enemy_hp_mean=float(hp.mean()), p_win=res), open(a.out, "w"), indent=1)


def sim_states(paths, n, end_only, seed=0):
    """simulator states (copies) of single-enemy play rows without powers: trained rows, or with end_only every state where only end turn is legal"""
    out = []
    for path in paths:
        z = np.load(path)
        scen = json.loads(str(z["scenarios"]))
        dfi, dst = z["d_fight"], z["d_step"]
        for f in range(len(z["f_scen"])):
            acts = z["acts"][z["f_off"][f]:z["f_off"][f + 1]].astype(int)
            want = set(range(len(acts) + 1)) if end_only else set(dst[dfi == f].tolist())
            if not want:
                continue
            sim = sts2.Sim(json.dumps(scen[z["f_scen"][f]]), int(z["f_seed"][f]))
            ob, mk = np.zeros(sts2.OBS_SIZE, np.float32), np.zeros(sts2.ACTIONS, np.uint8)
            for t in range(len(acts) + 1):
                if t in want and sim.stage() == "play":
                    sim.observe(ob, mk)
                    d = decode(ob[None])
                    eo = SEC["enemies"][0]
                    en = ob[eo:eo + E * EF].reshape(E, EF)
                    alive = (en[:, 0] > 0.5) & (en[:, 6] > 0.5)
                    ok = alive.sum() == 1 and alive[0] and en[0, 3] <= 400 and not (d["eid"] >= 0).any() and not (d["pid"] >= 0).any()
                    if ok and end_only:
                        ok = mk[C["OFF_PLAY"]:C["OFF_DISCARD"]].sum() == 0
                    if ok:
                        out.append(sim.copy())
                if t < len(acts) and not sim.step(int(acts[t])):
                    break
            if len(out) >= 2 * n:
                break
        if len(out) >= 2 * n:
            break
    r = np.random.default_rng(seed)
    return [out[i] for i in sorted(r.choice(len(out), min(n, len(out)), replace=False))]


GATE = [("player Intangible 1 raises p_win", "player intangible 1", ">", "baseline"),
        ("enemy Intangible 1 lowers p_win", "enemy intangible 1", "<", "baseline"),
        ("enemy poison 10: player Accelerant 0 -> 1 does not lower p_win", "enemy poison 10 + player accelerant 1", ">=", "enemy poison 10"),
        ("enemy poison 10: player Accelerant 1 -> 2 does not lower p_win", "enemy poison 10 + player accelerant 2", ">=", "enemy poison 10 + player accelerant 1"),
        ("enemy poison 10: player Accelerant 2 -> 4 does not lower p_win", "enemy poison 10 + player accelerant 4", ">=", "enemy poison 10 + player accelerant 2"),
        ("player Wraith Form (card: Intangible 2 + Wraith Form 1) raises p_win", "player wraith form card", ">", "baseline"),
        ("player Buffer 1 raises p_win", "player buffer 1", ">", "baseline"),
        ("player Plating 5 raises p_win", "player plating 5", ">", "baseline")]


def sens_state(a):
    """value response to power edits made on the simulator state (the observation, v3 derived features included, is recomputed): the sign gate"""
    import model as M
    net = M.load(a.ckpt).cpu()
    paths = sorted(glob.glob(a.parts))
    res, rows = {}, {}

    def run(states, name, edits):
        obs = np.zeros((len(states), sts2.OBS_SIZE), np.float32)
        mk = np.zeros(sts2.ACTIONS, np.uint8)
        refused = 0
        for i, s in enumerate(states):
            s = s.copy()
            for side, pw, amt in edits:
                v = amt(s) if callable(amt) else amt
                refused += not s.set_power(side, 0, pw, int(v))
            s.observe(obs[i], mk)
        pw_ = p_win(net, obs)
        res[name] = dict(p_win=float(pw_.mean()), refused=refused)
        rows[name] = pw_
        return pw_

    hp = lambda s: json.loads(s.snapshot())["enemies"][0]["hp"]  # noqa: E731
    X = sim_states(paths, a.n, False)
    run(X, "baseline", [])
    run(X, "player intangible 1", [("player", "INTANGIBLE_POWER", 1)])
    run(X, "enemy intangible 1", [("enemy", "INTANGIBLE_POWER", 1)])
    run(X, "enemy poison 10", [("enemy", "POISON_POWER", 10)])
    for v in (1, 2, 4):
        run(X, f"enemy poison 10 + player accelerant {v}", [("enemy", "POISON_POWER", 10), ("player", "ACCELERANT_POWER", v)])
    run(X, "player wraith form card", [("player", "INTANGIBLE_POWER", 2), ("player", "WRAITH_FORM_POWER", 1)])
    run(X, "player wraith form power 1 alone", [("player", "WRAITH_FORM_POWER", 1)])
    run(X, "player buffer 1", [("player", "BUFFER_POWER", 1)])
    run(X, "player plating 5", [("player", "PLATING_POWER", 5)])
    gate = []
    for label, x, op, y in GATE:
        dlt = rows[x] - rows[y]
        mean, se = float(dlt.mean()), float(dlt.std() / np.sqrt(len(dlt)))
        ok = mean > 0 if op == ">" else mean < 0 if op == "<" else mean >= 0
        gate.append(dict(check=label, delta=round(mean, 4), se=round(se, 4), passed=bool(ok)))
    Y = sim_states(paths, a.n, True, seed=1)
    if Y:
        d9 = run(Y, "end-turn-only: enemy doom hp-1", [("enemy", "DOOM_POWER", lambda s: max(1, hp(s) - 1))])
        d0 = run(Y, "end-turn-only: enemy doom = hp", [("enemy", "DOOM_POWER", hp)])
        dlt = d0 - d9
        gate.append(dict(check="end-turn-only: doom = hp vs hp-1 separates (mean > 0.05, > half the rows > 0.05)", delta=round(float(dlt.mean()), 4),
                         se=round(float(dlt.std() / np.sqrt(len(dlt))), 4), share_gt_005=round(float((dlt > 0.05).mean()), 3),
                         passed=bool(dlt.mean() > 0.05 and (dlt > 0.05).mean() > 0.5)))
    out = dict(ckpt=a.ckpt, obs_version=net.obs, rows=len(X), end_turn_rows=len(Y), p_win=res, gate=gate, gate_pass=all(g["passed"] for g in gate))
    for g in gate:
        print(f"{'PASS' if g['passed'] else 'FAIL'}  {g['check']:72s} {g['delta']:+.4f} +- {g['se']:.4f}")
    print("SIGN GATE " + ("PASS" if out["gate_pass"] else "FAIL"))
    json.dump(out, open(a.out, "w"), indent=1)


class Audit:
    """observation audit counters over simulator states (Sim.diag + the observation)"""

    def __init__(self):
        self.n = self.play = self.dec = 0
        self.c = Counter()
        self.src = Counter()
        self.pow_max = Counter()
        self.counter_cards = Counter()
        self.relic_hidden = Counter()
        self.relic_masked = Counter()

    def add(self, sim, ob):
        g = json.loads(sim.diag())
        self.n += 1
        if g["decision"]:
            self.dec += 1
            kind, i = g["decision"][:2]
            nm = {1: NM["card"], 2: NM["potion"], 3: NM["relic"], 4: NM["monster"]}.get(kind)
            self.src[f"{['', 'card', 'potion', 'relic', 'monster'][kind]}:{nm[i] if nm and 0 <= i < len(nm) else i}"] += 1
        m = max(g["powers"])
        self.pow_max[min(m, 16)] += 1
        self.c["powers>=16"] += m >= 16
        self.c["powers>=12"] += m >= 12
        pile = [c for p in ("draw", "discard", "exhaust") for c in g[p]]
        has = dict(counter=any(c[2] or c[3] for c in pile), enchant=any(c[4] for c in pile), affliction=any(c[6] for c in pile), upgrade2=any(c[1] > 1 for c in pile))
        for k, v in has.items():
            self.c["pile_" + k] += v
        self.c["pile_any_extra"] += any(has.values())
        for c in pile:
            if c[2] or c[3]:
                self.counter_cards[CNAME[c[0]]] += 1
        for rid, observed, disp, props in g["relics"]:
            used = [p for p in props if p[1]]
            if observed and disp is None and props:
                self.c["relic_state_hidden_rows"] += 0  # counted per relic below
                self.relic_hidden[NM["relic"][rid]] += 1
                if used:
                    self.c["relic_state_nonzero_hidden"] += 1
            if not observed and props and used:
                self.relic_masked[NM["relic"][rid]] += 1
        if not g["decision"] and sts2.OBS_VERSION == 3:
            self.play += 1
            hs, ho = SEC["hand_tgt"][0], SEC["hand"][0]
            tg = ob[hs:hs + 10 * E].reshape(10, E)
            pre = ob[ho:ho + 10 * CF].reshape(10, CF)[:, 6]
            eo = SEC["enemies"][0]
            en = ob[eo:eo + E * EF].reshape(E, EF)
            alive = (en[:, 0] > 0.5) & (en[:, 6] > 0.5)
            dmg = (pre != 0)[:, None] & alive[None]
            if dmg.any():
                self.c["play_rows_with_damage_card"] += 1
                self.c["target_changes_damage"] += bool((tg != pre[:, None])[dmg].any())
                if alive.sum() >= 2:
                    self.c["multi_enemy_damage_rows"] += 1
                    t2 = np.where(dmg, tg, np.nan)
                    self.c["targets_differ"] += bool((np.nanmax(t2, 1) != np.nanmin(t2, 1))[dmg.any(1)].any())

    def summary(self):
        n = max(self.n, 1)
        dec = max(self.dec, 1)
        rare = {k: v for k, v in self.src.items() if v / dec < 0.001}
        out = dict(rows=self.n, decision_rows=self.dec, decision_share=self.dec / n, decision_sources=len(self.src),
                   top_sources=[(k, round(v / dec, 4)) for k, v in self.src.most_common(12)],
                   rare_sources_share_of_decisions=sum(rare.values()) / dec, rare_sources=sorted(rare),
                   powers_max_hist={k: v / n for k, v in sorted(self.pow_max.items())},
                   counter_cards=self.counter_cards.most_common(10), relic_state_hidden=self.relic_hidden.most_common(15),
                   relic_masked_with_state=self.relic_masked.most_common(10))
        out.update({k: v / n for k, v in self.c.items() if not k.startswith(("play_", "target", "multi", "targets"))})
        p = max(self.c["play_rows_with_damage_card"], 1)
        out.update(play_rows=self.play, play_rows_with_damage_card=self.c["play_rows_with_damage_card"],
                   target_changes_damage_share=self.c["target_changes_damage"] / p,
                   multi_enemy_damage_rows=self.c["multi_enemy_damage_rows"],
                   targets_differ_share_of_multi=self.c["targets_differ"] / max(self.c["multi_enemy_damage_rows"], 1))
        return out


def audit(a):
    res = {}
    acc = Audit()
    ob, mk = np.zeros(sts2.OBS_SIZE, np.float32), np.zeros(sts2.ACTIONS, np.uint8)
    paths = sorted(glob.glob(a.parts))[::a.every]
    for path in paths:
        z = np.load(path)
        scen = json.loads(str(z["scenarios"]))
        dfi, dst = z["d_fight"], z["d_step"]
        for f in range(0, len(z["f_scen"]), a.fight_every):
            want = set(dst[dfi == f].tolist())
            if not want:
                continue
            acts = z["acts"][z["f_off"][f]:z["f_off"][f + 1]].astype(int)
            sim = sts2.Sim(json.dumps(scen[z["f_scen"][f]]), int(z["f_seed"][f]))
            for t in range(len(acts) + 1):
                if t in want:
                    sim.observe(ob, mk)
                    acc.add(sim, ob)
                if t < len(acts) and not sim.step(int(acts[t])):
                    break
        print(f"{os.path.basename(path)}: {acc.n} rows", file=sys.stderr, flush=True)
    res["trained"] = acc.summary()
    if a.plans:
        import torch
        import model as M
        net = M.load(a.ckpt).cpu()
        data = [x["scenario"] for x in json.load(open(a.plans)) if x["scenario"].get("meta", {}).get("source") == "plan"
                and x["scenario"]["meta"].get("base") is None and x["scenario"]["meta"].get("enabler") is None]
        data = [data[i] for i in sorted(np.random.default_rng(0).choice(len(data), min(a.plans_n, len(data)), replace=False))]
        acc = Audit()
        sims = [sts2.Sim(json.dumps(sc), 1000 + i) for i, sc in enumerate(data)]
        for _ in range(400):
            live = [s for s in sims if s.stage() != "over"]
            if not live:
                break
            O = np.zeros((len(live), sts2.OBS_SIZE), np.float32)
            K = np.zeros((len(live), sts2.ACTIONS), np.uint8)
            for i, s in enumerate(live):
                s.observe(O[i], K[i])
                acc.add(s, O[i])
            with torch.no_grad():
                act = net(torch.from_numpy(O), torch.from_numpy(K.astype(np.int64)), value=False)[0].argmax(1).numpy()
            for s, x in zip(live, act):
                s.step(int(x))
        res["plans_greedy"] = acc.summary()
    if a.expert:
        n = over = enchant = aff = 0
        mx = Counter()
        hidden = Counter()
        for path in sorted(glob.glob(os.path.join(a.expert, "*.json"))):
            for s in json.load(open(path))["fight"].get("states", []):
                if not s.get("enemies"):
                    continue
                n += 1
                m = max([len(s["player"].get("powers", []))] + [len(e.get("powers", [])) for e in s["enemies"]])
                mx[min(m, 17)] += 1
                over += m > 16
                pile = [c for p in ("draw", "discard", "exhaust") for c in s.get(p, [])]
                enchant += any("enchantment" in c for c in pile)
                aff += any("affliction" in c for c in pile)
                for r in s.get("relics", []):
                    if r.get("props") and "counter" not in r:
                        hidden[r["id"]] += 1
        res["expert"] = dict(states=n, powers_max_hist={k: v / max(n, 1) for k, v in sorted(mx.items())}, creature_over_16=over / max(n, 1),
                             pile_enchant=enchant / max(n, 1), pile_affliction=aff / max(n, 1), relic_props_without_counter=hidden.most_common(15))
    print(json.dumps(res, indent=1, default=str))
    json.dump(res, open(a.out, "w"), indent=1, default=str)


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    p = sp.add_parser("probe")
    p.add_argument("--parts", required=True, help="glob of collection parts (npz)")
    p.add_argument("--probe-parts", type=int, default=2)
    p.add_argument("--probe-every", type=int, default=10)
    e = sp.add_parser("embed")
    e.add_argument("--old", required=True)
    e.add_argument("--new", required=True)
    s = sp.add_parser("stats")
    s.add_argument("--parts", help="glob of collection parts (npz): the trained decision rows")
    s.add_argument("--every", type=int, default=1, help="use every k-th part")
    s.add_argument("--pools", nargs="*", default=[], help="label=path scenario lists: deck-level only")
    s.add_argument("--play", nargs="*", default=[], help="label=path scenario lists played greedily by --ckpt")
    s.add_argument("--play-max", type=int, default=4096)
    s.add_argument("--seeds", type=int, default=2)
    s.add_argument("--ckpt")
    s.add_argument("--expert", nargs="*", default=[], help="label=dir of replay-log fight JSONs")
    s.add_argument("--out", required=True)
    s.add_argument("--full-only", action="store_true", help="--play: only scenarios whose meta has no base/enabler (the plans slice)")
    c = sp.add_parser("calib")
    c.add_argument("--parts", required=True)
    c.add_argument("--every", type=int, default=10)
    c.add_argument("--ckpt", required=True)
    c.add_argument("--out", required=True)
    v = sp.add_parser("sens")
    v.add_argument("--parts", required=True)
    v.add_argument("--end-only", action="store_true", help="only states where end turn is the one remaining play (no card or potion use legal)")
    v.add_argument("--ckpt", required=True)
    v.add_argument("--n", type=int, default=4000)
    v.add_argument("--out", required=True)
    g = sp.add_parser("gate", help="value-response sign gate on simulator-state edits (promotion acceptance for observation-v3 checkpoints)")
    g.add_argument("--parts", required=True)
    g.add_argument("--ckpt", required=True)
    g.add_argument("--n", type=int, default=2000)
    g.add_argument("--out", required=True)
    u = sp.add_parser("audit", help="observation audit: decision sources, power-slot use, pile-card extras, relic state, per-target damage")
    u.add_argument("--parts", required=True)
    u.add_argument("--every", type=int, default=15, help="every k-th part")
    u.add_argument("--fight-every", type=int, default=4)
    u.add_argument("--plans", help="bench scenario file: full plan decks played greedily by --ckpt")
    u.add_argument("--plans-n", type=int, default=200)
    u.add_argument("--ckpt")
    u.add_argument("--expert", help="dir of expert replay-log fight JSONs")
    u.add_argument("--out", required=True)
    a = ap.parse_args()
    if a.cmd == "gate":
        return sens_state(a)
    if a.cmd == "audit":
        return audit(a)
    if a.cmd == "calib":
        return calib(a)
    if a.cmd == "sens":
        return sens(a)
    if a.cmd == "probe":
        return probe(a)
    if a.cmd == "embed":
        return embed(a)
    groups = {}
    for spec in a.expert:
        lab, d = spec.split("=", 1)
        stats_expert(sorted(glob.glob(os.path.join(d, "*.json"))), groups, lab)
    for spec in a.pools:
        lab, path = spec.split("=", 1)
        deck_only(path, groups, lab)
    for spec in a.play:
        lab, path = spec.split("=", 1)
        stats_play(path, groups, a.ckpt, a.play_max, a.seeds, lab, a.full_only)
    if a.parts:
        stats_parts(sorted(glob.glob(a.parts)), groups, a.every)
    json.dump({k: v.summary() for k, v in groups.items()}, open(a.out, "w"), indent=1)
    print(f"-> {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
