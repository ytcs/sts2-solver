#!/usr/bin/env python3
"""Randomized differential fuzzing (generator + runner): realistic random A10 runs -> oracle traces -> Rust replay.

  tools/fuzz_gen_mix.py --n 500 --seed 1 --out DIR [--jobs 6] [--character REGENT] [--act 2] [--encounter X] [--focus colorless]
                        [--mode deep|stall|uniform|greedy] [--relics 3-8] [--keep-ok]

Each scenario is a random A10 run state: character (Regent-heavy), deck = starter + act-scaled picks from the character pool,
colorless / event / curse / status cards, random upgrades; 3-8 random relics (own pool + shared + a few event/foreign ones, counters
injected through `relics[].props`), 0-2 potions, any implemented encounter, random policy (weights end_turn / attacks / potions,
`deep` = huge HP + never end turn early so fights reach deep turns). The oracle runs scenarios in `batch` mode (one process per
chunk) and `sts2diff run` compares. Mismatching / oracle-error scenarios stay in DIR as NAME.scenario.json (+ .jsonl, .error.txt).
Needs the catalog: `oracle/combat/oracle.sh catalog --out DIR/catalog.json` (done automatically when missing).
"""
import argparse, glob, json, os, random, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")
SRC = os.path.join(ROOT, "crates/sts2sim/src")

STARTERS = {  # character -> (starter deck, starter relic, hp, energy, orb slots)
    "IRONCLAD": (["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH"], "BURNING_BLOOD", 80, 3, 0),
    "SILENT": (["STRIKE_SILENT"] * 5 + ["DEFEND_SILENT"] * 5 + ["NEUTRALIZE", "SURVIVOR"], "RING_OF_THE_SNAKE", 70, 3, 0),
    "DEFECT": (["STRIKE_DEFECT"] * 4 + ["DEFEND_DEFECT"] * 4 + ["ZAP", "DUALCAST"], "CRACKED_CORE", 75, 3, 3),
    "NECROBINDER": (["STRIKE_NECROBINDER"] * 4 + ["DEFEND_NECROBINDER"] * 4 + ["BODYGUARD", "UNLEASH"], "BOUND_PHYLACTERY", 66, 3, 0),
    "REGENT": (["STRIKE_REGENT"] * 4 + ["DEFEND_REGENT"] * 4 + ["FALLING_STAR", "VENERATE"], "DIVINE_RIGHT", 75, 3, 0),
}
ATTACK_ENCH = ["SHARP", "VIGOROUS", "INSTINCT", "MOMENTUM", "SWIFT", "STEADY", "ADROIT", "GLAM", "PERFECT_FIT", "SOWN", "TEZCATARAS_EMBER", "SLUMBERING_ESSENCE", "CLONE"]
SKILL_ENCH = ["IMBUED", "IMBUED", "SWIFT", "STEADY", "ADROIT", "GLAM", "PERFECT_FIT", "SOWN", "TEZCATARAS_EMBER", "SLUMBERING_ESSENCE", "CLONE"]
CHAR_W = {"REGENT": 40, "IRONCLAD": 15, "SILENT": 15, "DEFECT": 15, "NECROBINDER": 15}


def slugify(name):  # same as tools/coverage.py
    out, i, last = [], 0, -1
    while i < len(name):
        if i + 1 < len(name) and name[i].isascii() and name[i].isalnum() and "A" <= name[i + 1] <= "Z":
            out.append(name[i] + "_" + name[i + 1]); i += 2; last = i; continue
        if i == last and i != 0 and "A" <= name[i] <= "Z":
            out.append("_" + name[i]); i += 1; last = i; continue
        out.append(name[i]); i += 1
    return "".join(out).upper()


def implemented():
    """(set of implemented ids per category, relic -> [(prop, kind)] from relic_props!)."""
    def scan(cat, pat):
        found = set()
        for f in glob.glob(os.path.join(SRC, "content", cat, "*.rs")):
            found |= set(re.findall(pat, open(f).read()))
        return found
    have = {
        "card": {slugify(n) for n in scan("cards", r"listener!\(\s*(\w+)\s*\{")},
        "relic": {slugify(n) for n in scan("relics", r"listener!\(\s*(\w+)\s*\{")},
        "potion": {slugify(n) for n in scan("potions", r"listener!\(\s*(\w+)\s*\{")},
        "encounter": {n.upper() for n in scan("encounters", r"pub fn spawn_(\w+)\(")},
    }
    props = {}
    for f in glob.glob(os.path.join(SRC, "content", "relics", "*.rs")):
        txt = open(f).read()
        parts = re.split(r"listener!\(\s*(\w+)\s*\{", txt)
        for name, body in zip(parts[1::2], parts[2::2]):
            for kind, pname in re.findall(r"PropDef::(int|flag)\(\"(\w+)\"", body):
                props.setdefault(slugify(name), []).append((pname, kind))
    return have, props


class Gen:
    def __init__(self, catalog_path):
        self.cat = json.load(open(catalog_path))
        self.have, self.relic_props = implemented()
        self.cards = {k: [c for c in v if c["id"] in self.have["card"]] for k, v in self.cat["cards"].items()}
        self.relics = {k: [r for r in v if r["id"] in self.have["relic"]] for k, v in self.cat["relics"].items()}
        self.potions = {k: [p for p in v if p["id"] in self.have["potion"]] for k, v in self.cat["potions"].items()}
        self.encs = [e for e in self.cat["encounters"] if e["id"] in self.have["encounter"]]
        self.ctypes = {c["id"]: c["type"] for pool in self.cat["cards"].values() for c in pool}

    # ---- deck -----------------------------------------------------------------------------------------------------
    def pick_card(self, r, pool, rarity_w=(("Common", 5), ("Uncommon", 4), ("Rare", 2))):
        cards = [c for c in pool if c["rarity"] in dict(rarity_w)]
        if not cards:
            cards = pool
        w = [dict(rarity_w).get(c["rarity"], 1) for c in cards]
        return r.choices(cards, w)[0]

    def make_deck(self, r, ch, act, focus, upg_p, enchant_p=0.04):
        starter = list(STARTERS[ch][0])
        n_add = {0: r.randint(4, 14), 1: r.randint(10, 22), 2: r.randint(14, 30)}[act]
        # composition weights: own pool, colorless, event, curse, status/token (focus shifts them)
        w = {"own": 60, "colorless": 20, "event": 8, "curse": 6, "status": 4, "dupe": 8}
        if focus == "colorless":
            w = {"own": 20, "colorless": 50, "event": 12, "curse": 8, "status": 6, "dupe": 4}
        elif focus == "junk":
            w = {"own": 25, "colorless": 20, "event": 20, "curse": 20, "status": 15, "dupe": 0}
        elif focus == "gen":  # card generation / auto-play heavy
            w = {"own": 40, "colorless": 45, "event": 5, "curse": 2, "status": 2, "dupe": 6}
        elif focus == "turn":  # turn-start auto-play / decisions: Mayhem, Stratagem, auto-play cards, choice cards (+ Imbued)
            w = {"own": 40, "colorless": 25, "event": 5, "curse": 3, "status": 3, "dupe": 4, "autoplay": 20}
        gen_ids = ["DISCOVERY", "ENTROPY", "MAYHEM", "STRATAGEM", "BEAT_DOWN", "JACK_OF_ALL_TRADES", "MASTER_OF_STRATEGY", "ALCHEMIZE",
                   "HAND_OF_GREED", "TRANSFORM", "CATASTROPHE", "DRAMATIC_ENTRANCE", "NOSTALGIA", "MIMIC", "PRODUCTION", "BELIEVE_IN_YOU",
                   "SCRAWL", "BEGONE", "HIDDEN_GEM", "SPLASH", "WHITE_NOISE", "QUASAR", "SEEKING_EDGE", "THE_BOMB", "COORDINATE",
                   "PRIMAL_FORCE", "HEGEMONY", "CONVERGENCE", "PARRY", "FOREGONE_CONCLUSION"]
        autoplay_ids = ["MAYHEM", "STRATAGEM", "BEAT_DOWN", "BOMBARDMENT", "CATASTROPHE", "DECISIONS_DECISIONS", "EIDOLON", "HOWL_FROM_BEYOND",
                        "KNIFE_TRAP", "UPROAR", "HAVOC", "CASCADE", "STAMPEDE", "IMITATION_LEARNING", "HELLRAISER", "DISCOVERY", "ENTROPY",
                        "JACK_OF_ALL_TRADES", "MASTER_OF_STRATEGY", "ALCHEMIZE", "ARMAMENTS", "HEADBUTT", "BURNING_PACT", "TRUE_GRIT", "TOOLBOX",
                        "SURVIVOR", "PREPARED", "ACROBATICS", "CALCULATED_GAMBLE", "DAGGER_THROW", "TACTICIAN", "MASTER_PLANNER", "TOOLS_OF_THE_TRADE"]
        added = []
        kinds = list(w)
        for _ in range(n_add):
            k = r.choices(kinds, [w[k] for k in kinds])[0]
            if k == "own":
                pool = [c for c in self.cards[ch] if c["rarity"] in ("Common", "Uncommon", "Rare")]
                added.append(self.pick_card(r, pool))
            elif k == "colorless":
                pool = [c for c in self.cards["COLORLESS"]]
                if focus == "gen" and r.random() < 0.5:
                    g = [c for c in pool + self.cards[ch] if c["id"] in gen_ids]
                    pool = g or pool
                added.append(self.pick_card(r, pool, (("Common", 2), ("Uncommon", 3), ("Rare", 3), ("Uncommon", 0))))
            elif k == "event":
                pool = [c for c in self.cards["EVENT"] if c["type"] != "Quest"]
                if pool:
                    added.append(r.choice(pool))
            elif k == "curse":
                pool = self.cards["CURSE"]
                if pool:
                    added.append(r.choice(pool))
            elif k == "status":
                pool = self.cards["STATUS"] + self.cards["TOKEN"]
                if pool:
                    added.append(r.choice(pool))
            elif k == "dupe" and added:
                added.append(r.choice(added))
            elif k == "autoplay":
                pool = [c for c in self.cards[ch] + self.cards["COLORLESS"] if c["id"] in autoplay_ids]
                if pool:
                    added.append(r.choice(pool))
        deck = []
        for cid in starter:
            deck.append((cid, 1 if r.random() < upg_p * 0.5 else 0))
        for c in added:
            deck.append((c["id"], 1 if (c["max_upgrade"] > 0 and r.random() < upg_p) else 0))
        if r.random() < 0.9:
            deck.append(("ASCENDERS_BANE", 0))
        out = []
        for cid, up in deck:
            c = {"id": cid, "upgrade": up} if up else cid
            ctype = self.ctypes.get(cid)
            if ctype in ("Attack", "Skill") and r.random() < enchant_p and cid != "MAD_SCIENCE":
                pool = ATTACK_ENCH if ctype == "Attack" else SKILL_ENCH
                c = {"id": cid, "upgrade": up, "enchantment": {"id": r.choice(pool), "amount": 1}}
            if cid == "MAD_SCIENCE":  # per-instance type (1 attack / 2 skill / 3 power) + rider (1-9), saved props
                ty = r.randint(1, 3)  # riders: attack 1-3, skill 4-6, power 7-9 (TinkerTime.ChooseRiderEffect)
                c = {"id": cid, "upgrade": up, "props": {"TinkerTimeRider": 3 * (ty - 1) + r.randint(1, 3), "TinkerTimeType": ty}}
            out.append(c)
        # the game removes/transforms: occasionally drop a random starter strike/defend
        for _ in range(r.randint(0, 2)):
            i = r.randrange(len(out))
            if (out[i] if isinstance(out[i], str) else out[i]["id"]).startswith(("STRIKE_", "DEFEND_")):
                out.pop(i)
        return out

    # ---- relics ---------------------------------------------------------------------------------------------------
    def make_relics(self, r, ch, lo, hi):
        starter_id = STARTERS[ch][1]
        pools = self.relics[ch] + self.relics["SHARED"]
        pools = [x for x in pools if x["rarity"] not in ("Starter",)]
        ev = [x for x in self.relics["EVENT"] if x["rarity"] != "Starter"]
        foreign = [x for k in STARTERS if k != ch for x in self.relics[k] if x["rarity"] != "Starter"]
        n = r.randint(lo, hi)
        chosen = []
        seen = {starter_id}
        tries = 0
        while len(chosen) < n and tries < 100:
            tries += 1
            u = r.random()
            pool = pools if u < 0.8 else ev if u < 0.93 else foreign
            if not pool:
                continue
            x = r.choice(pool)
            if x["id"] in seen:
                continue
            seen.add(x["id"])
            chosen.append(x["id"])
        out = []
        ids = ([starter_id] if (starter_id in self.have["relic"] and r.random() < 0.85) else []) + chosen
        for rid in ids:
            props = self.relic_props.get(rid)
            if props and r.random() < 0.6:
                p = {}
                for name, kind in props:
                    p[name] = (r.random() < 0.5) if kind == "flag" else r.choice([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9])  # < 10: some setters take `value % 10`
                out.append({"id": rid, "props": p})
            else:
                out.append(rid)
        r.shuffle(out)  # hook order = list order; the starter relic is usually first in a real run, keep it so 70% of the time
        if ids and ids[0] == starter_id and r.random() < 0.7:
            i = next(i for i, x in enumerate(out) if (x if isinstance(x, str) else x["id"]) == starter_id)
            out.insert(0, out.pop(i))
        return out

    def make_potions(self, r, ch):
        pool = [p for p in self.potions[ch] + self.potions["SHARED"] if p["usage"] in ("CombatOnly", "AnyTime")]
        n = r.choices([0, 1, 2], [1, 4, 5])[0]
        return [r.choice(pool)["id"] for _ in range(n)] if pool else []

    # ---- scenario ---------------------------------------------------------------------------------------------------
    def scenario(self, idx, seed, a):
        r = random.Random(f"{seed}/{idx}")
        ch = a.character or r.choices(list(CHAR_W), list(CHAR_W.values()))[0]
        encs = self.encs
        if a.encounter:
            encs = [e for e in encs if e["id"] in a.encounter.split(",")]
        if a.act is not None:
            ai = {0: (0, 1), 1: (2,), 2: (3,)}[a.act]
            encs = [e for e in encs if e["act"] in ai or (e["event"] and a.act == 0)]
        if a.room:
            encs = [e for e in encs if e["room"].lower() in a.room.split(",")]
        # elites/bosses/multi-enemy are the interesting ones: weight non-weak normal 3, elite 4, boss 4, weak 1
        w = [1 if e["weak"] else 4 if e["room"] in ("Elite", "Boss") else 3 for e in encs]
        enc = r.choices(encs, w)[0]
        act = {0: 0, 1: 0, 2: 1, 3: 2, -1: r.randint(0, 2)}[enc["act"]]
        starter, _, hp0, energy, orbs = STARTERS[ch]
        floor = {0: r.randint(1, 16), 1: r.randint(18, 33), 2: r.randint(35, 50)}[act]
        max_hp = hp0 + act * r.randint(5, 25) + r.randint(0, 15)
        mode = a.mode or r.choices(["uniform", "greedy", "stall", "deep"], [30, 35, 10, 25])[0]
        focus = a.focus or r.choices(["mix", "colorless", "junk", "gen", "turn"], [30, 20, 8, 20, 22])[0]
        deck = self.make_deck(r, ch, act, focus, upg_p=[0.15, 0.35, 0.5][act], enchant_p=a.enchant)
        relics = self.make_relics(r, ch, *[int(x) for x in a.relics.split("-")])
        policy = {"seed": r.randrange(1 << 30), "endw": 1.0, "atkw": 1.0, "potw": r.choice([0.3, 1.0, 2.0]), "max_steps": 500, "max_rounds": 40}
        hp = int(max_hp * r.uniform(0.5, 1.0))
        if mode == "greedy":
            policy.update(endw=0.0)
        elif mode == "stall":
            policy.update(endw=0.4, atkw=0.15)
        elif mode == "deep":
            policy.update(endw=0.0 if r.random() < 0.6 else 0.15, atkw=r.choice([0.3, 1.0]), max_steps=1200, max_rounds=60)
            max_hp = hp = r.randint(400, 999)
        potions = self.make_potions(r, ch)
        if a.force_potions:
            potions = a.force_potions.split(",")[:2]
        if focus == "turn" and not a.force_relics:
            tsr = [x for x in ("GAMBLING_CHIP", "TOASTY_MITTENS", "TOOLBOX", "CHOICES_PARADOX", "WHISPERING_EARRING", "HISTORY_COURSE", "UNCEASING_TOP", "BAG_OF_PREPARATION") if x in self.have["relic"]]
            have = {x if isinstance(x, str) else x["id"] for x in relics}
            relics = [x for x in r.sample(tsr, r.randint(1, 2)) if x not in have] + relics
        if a.force_relics:
            have = {x if isinstance(x, str) else x["id"] for x in relics}
            relics = [x for x in a.force_relics.split(",") if x not in have] + relics
        return {
            "name": f"fm_{idx}", "ascension": 10, "encounter": enc["id"], "character": ch, "hp": hp, "max_hp": max_hp,
            "max_energy": energy, "base_orb_slots": orbs, "max_potion_slots": 2, "gold": r.choice([0, 50, 99, 150, 300, 800]),
            "seed": f"fm{seed}-{idx}", "total_floor": floor, "act": act, "deck": deck, "relics": relics, "potions": potions,
            "policy": policy, "meta": {"mode": mode, "focus": focus},
        }


def run_chunk(args):
    """One oracle process for a chunk of scenario paths, then replay each in Rust."""
    paths, keep_ok = args
    listf = paths[0].replace(".scenario.json", ".list")
    open(listf, "w").write("\n".join(paths) + "\n")
    crashed = {}
    pending = list(paths)
    while pending:  # a process crash (stack overflow, OOM kill...) leaves no .done marker: isolate the culprit, rerun the rest
        open(listf, "w").write("\n".join(pending) + "\n")
        subprocess.run([ORACLE, "batch", listf], capture_output=True, text=True)
        pending = [p for p in pending if not os.path.exists(p[: -len(".scenario.json")] + ".done")]
        if pending:
            open(listf, "w").write(pending[0] + "\n")
            r = subprocess.run([ORACLE, "batch", listf], capture_output=True, text=True)
            if not os.path.exists(pending[0][: -len(".scenario.json")] + ".done"):
                crashed[pending[0]] = (r.stderr or r.stdout)[-600:] + f" (exit {r.returncode})"
                open(pending[0][: -len(".scenario.json")] + ".done", "w").write("crash")
            pending = [p for p in pending if not os.path.exists(p[: -len(".scenario.json")] + ".done")]
    res = []
    for p in paths:
        base = p[: -len(".scenario.json")]
        if p in crashed:
            res.append((base, "oracle-crash", crashed[p]))
            continue
        if os.path.exists(base + ".error.txt"):
            res.append((base, "oracle-error", open(base + ".error.txt").read()[:500]))
            continue
        if not os.path.exists(base + ".jsonl"):
            res.append((base, "oracle-error", "no trace (oracle crashed?)"))
            continue
        d = subprocess.run([DIFF, "run", p, base + ".jsonl", "--max", "4"], capture_output=True, text=True)
        out = d.stdout.strip()
        if d.returncode in (0, 3):
            verdict = "ok" if d.returncode == 0 else "unimplemented"
            if not keep_ok:
                for ext in (".scenario.json", ".jsonl"):
                    try: os.remove(base + ext)
                    except OSError: pass
            res.append((base, verdict, out.splitlines()[-1] if d.returncode == 3 else ""))
        else:
            res.append((base, "mismatch" if d.returncode == 1 else "sim-error", (out or d.stderr)[-700:]))
    for p in paths:
        try: os.remove(p[: -len(".scenario.json")] + ".done")
        except OSError: pass
    try: os.remove(listf)
    except OSError: pass
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--seed", default="1")
    ap.add_argument("--out", required=True)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--chunk", type=int, default=20)
    ap.add_argument("--character")
    ap.add_argument("--encounter")
    ap.add_argument("--act", type=int)
    ap.add_argument("--room", help="comma list of monster,elite,boss")
    ap.add_argument("--focus", choices=["mix", "colorless", "junk", "gen", "turn"])
    ap.add_argument("--enchant", type=float, default=0.04, help="probability that an Attack/Skill card carries an enchantment")
    ap.add_argument("--mode", choices=["uniform", "greedy", "stall", "deep"])
    ap.add_argument("--relics", default="3-8")
    ap.add_argument("--force-potions", help="comma list (max 2) used instead of random potions")
    ap.add_argument("--force-relics", help="comma list prepended to the random relics")
    ap.add_argument("--keep-ok", action="store_true")
    ap.add_argument("--gen-only", action="store_true")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    cat = os.path.join(a.out, "catalog.json")
    if not os.path.exists(cat):
        subprocess.run([ORACLE, "catalog", "--out", cat], capture_output=True, check=True)
    g = Gen(cat)
    paths = []
    for i in range(a.n):
        sc = g.scenario(i, a.seed, a)
        p = os.path.join(a.out, f"fm_{a.seed}_{i}.scenario.json")
        sc["name"] = f"fm_{a.seed}_{i}"
        json.dump(sc, open(p, "w"))
        paths.append(p)
    if a.gen_only:
        return
    chunks = [(paths[i:i + a.chunk], a.keep_ok) for i in range(0, len(paths), a.chunk)]
    counts, bad, missing = {}, [], {}
    with ThreadPoolExecutor(a.jobs) as ex:
        for res in ex.map(run_chunk, chunks):
            for base, verdict, msg in res:
                counts[verdict] = counts.get(verdict, 0) + 1
                if verdict == "unimplemented":
                    name = msg.split(" (step")[0].replace("UNIMPLEMENTED ", "")
                    missing[name] = missing.get(name, 0) + 1
                elif verdict != "ok":
                    bad.append((base, verdict, msg))
    for base, verdict, msg in bad[:25]:
        sc = json.load(open(base + ".scenario.json"))
        print(f"--- {verdict}: {base}  [{sc['character']} {sc['encounter']} {sc['meta']}]\n{msg}")
    if missing:
        print("UNIMPLEMENTED content hit:", ", ".join(f"{k} x{v}" for k, v in sorted(missing.items(), key=lambda t: -t[1])))
    print("SUMMARY", json.dumps(counts), f"(artifacts in {a.out})")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
