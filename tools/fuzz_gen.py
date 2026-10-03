#!/usr/bin/env python3
"""Randomized differential fuzzing (oracle vs Rust) over realistic A10 Ironclad / Silent runs.

  tools/fuzz_gen.py run --n 3000 --seed 1 --out DIR [--jobs 6] [--characters IRONCLAD,SILENT] [--encounters ID,ID]
  tools/fuzz_gen.py gen --n 200 --seed 1 --out DIR            # only write scenarios (DIR/jobK/*.scenario.json)

Pipeline: (1) generate scenarios (random deck / relics / potions / hp / encounter / policy); (2) run each through the
oracle in ONE process per job (`oracle.sh batch DIR`, ~tens of ms per fight; policies random | playall | stall);
(3) diff the trace against the Rust sim (`sts2diff run`). Passing runs are deleted; failing ones keep
NAME.scenario.json + NAME.jsonl (+ NAME.err for oracle errors) so `target/debug/sts2diff run` can reproduce them.
Pool data comes from `oracle.sh dump-pools` (cached in tools/fuzz_pools.json).
"""
import argparse, collections, glob, json, os, random, re, subprocess, sys, threading
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")
POOLS = os.path.join(ROOT, "tools/fuzz_pools.json")

STARTERS = {  # deck, relic, base max hp
    "IRONCLAD": (["STRIKE_IRONCLAD"] * 5 + ["DEFEND_IRONCLAD"] * 4 + ["BASH"], "BURNING_BLOOD", 80),
    "SILENT": (["STRIKE_SILENT"] * 5 + ["DEFEND_SILENT"] * 5 + ["NEUTRALIZE", "SURVIVOR"], "RING_OF_THE_SNAKE", 70),
}
POOL_OF = {"IRONCLAD": "IroncladCardPool", "SILENT": "SilentCardPool"}
ACT_IDX = {"Overgrowth": 0, "Underdocks": 0, "Hive": 1, "Glory": 2}
NOT_PORTED_POTIONS = {"BONE_BREW", "DISTILLED_CHAOS", "KINGS_COURAGE"}  # tools/coverage.py --missing potions
STRESS_RELICS = ["WHISPERING_EARRING", "GAMBLING_CHIP", "TOOLBOX", "TOASTY_MITTENS", "CHOICES_PARADOX", "HISTORY_COURSE", "PAELS_EYE",
                 "PAELS_LEGION", "BONE_FLUTE", "BYRDPIP", "FENCING_MANUAL", "ICE_CREAM", "CHEMICAL_X", "LIZARD_TAIL", "PAPER_PHROG",
                 "PAPER_KRANE", "RUNIC_PYRAMID", "UNCEASING_TOP", "PEN_NIB", "KUNAI", "SHURIKEN", "ORNAMENTAL_FAN", "DEMON_TONGUE",
                 "BAG_OF_PREPARATION", "ART_OF_WAR", "CENTENNIAL_PUZZLE", "TOUGH_BANDAGES", "TINGSHA", "SNECKO_SKULL", "RED_SKULL",
                 "MUSIC_BOX", "SIGNET_RING", "GAME_PIECE", "BLOOD_SOAKED_ROSE", "TANXS_WHISTLE", "BRILLIANT_SCARF"]
OTHER_CHAR_RELIC_POOLS = ["RegentRelicPool", "NecrobinderRelicPool", "DefectRelicPool"]


def slugify(name):  # same as tools/coverage.py
    out, i, last = [], 0, -1
    while i < len(name):
        if i + 1 < len(name) and name[i].isascii() and name[i].isalnum() and "A" <= name[i + 1] <= "Z":
            out.append(name[i] + "_" + name[i + 1]); i += 2; last = i; continue
        if i == last and i != 0 and "A" <= name[i] <= "Z":
            out.append("_" + name[i]); i += 1; last = i; continue
        out.append(name[i]); i += 1
    return "".join(out).upper()


def load_pools():
    if not os.path.exists(POOLS):
        subprocess.run([ORACLE, "dump-pools", "--out", POOLS], check=True, capture_output=True)
    return json.load(open(POOLS))


def runlevel_relics():
    src = open(os.path.join(ROOT, "crates/sts2sim/src/content/relics/runlevel.rs")).read()
    return {slugify(n) for n in re.findall(r"listener!\(\s*(\w+)\s*\{", src)}


class Gen:
    def __init__(self, characters, encounters=None):
        P = self.P = load_pools()
        self.chars = characters
        self.cards = {}
        colorless = [c for c in P["cards"]["ColorlessCardPool"]]
        for ch in characters:
            pool = [c for c in P["cards"][POOL_OF[ch]] if c["rarity"] in ("Common", "Uncommon", "Rare", "Ancient")]
            self.cards[ch] = pool
        self.colorless = colorless
        # Inky is only created by Blade of Ink on attacks; on a non-targeted card the real game NREs in Inky.OnPlay
        self.ench = [e for e in P.get("enchantments", []) if e["cards"] and e["id"] != "INKY"]
        self.curses = P["cards"]["CurseCardPool"]
        self.status = P["cards"]["StatusCardPool"]
        self.event_cards = P["cards"]["EventCardPool"]
        other = {r["id"] for p in OTHER_CHAR_RELIC_POOLS for r in P["relics"][p]}
        char_relics = {r["id"] for p in ("IroncladRelicPool", "SilentRelicPool") for r in P["relics"][p]}
        rl = runlevel_relics()
        allr = {}
        for p in ("IroncladRelicPool", "SilentRelicPool", "SharedRelicPool", "EventRelicPool"):
            for r in P["relics"][p]:
                allr.setdefault(r["id"], r)
        self.relics = {}
        for ch, (_, start, _) in STARTERS.items():
            mine = {r["id"] for r in P["relics"]["IroncladRelicPool" if ch == "IRONCLAD" else "SilentRelicPool"]}
            foreign = (char_relics - mine) | other
            self.relics[ch] = [r["id"] for r in allr.values() if r["rarity"] not in ("Starter", "None") and r["id"] not in foreign
                               and r["id"] not in ("DEPRECATED_RELIC",) and r["id"] not in rl]
        self.relics_rl = sorted(rl & set(allr))
        self.potions = {}
        for ch, pn in (("IRONCLAD", "IroncladPotionPool"), ("SILENT", "SilentPotionPool")):
            ps = P["potions"][pn] + P["potions"]["SharedPotionPool"] + P["potions"]["EventPotionPool"]
            self.potions[ch] = [p["id"] for p in ps if p["usage"] in ("CombatOnly", "AnyTime", "Automatic") and p["id"] not in NOT_PORTED_POTIONS]
        encs = [e for e in P["encounters"] if e["id"] != "DEPRECATED_ENCOUNTER"]
        if encounters:
            encs = [e for e in encs if e["id"] in encounters]
        self.encs = encs

    def card(self, rng, c, up_p, ench_p=0.0):
        up = 1 if c["max_upgrade"] > 0 and rng.random() < up_p else 0
        d = {"id": c["id"], "upgrade": up}
        if ench_p and rng.random() < ench_p:
            opts = [e for e in self.ench if c["id"] in e["cards"]]
            if opts:
                e = rng.choice(opts)
                d["enchantment"] = {"id": e["id"], "amount": rng.randint(1, 3) if e["show_amount"] else 1}
        return d if (up or "enchantment" in d) else c["id"]

    def make(self, rng, name, seed_str, ch=None, enc=None, policy=None):
        ch = ch or rng.choice(self.chars)
        e = enc or rng.choice(self.encs)
        act = ACT_IDX.get(e["act"], rng.randrange(3))
        floor_lo, floor_hi = [(2, 16), (19, 33), (36, 50)][act]
        if e["room"] == "Boss":
            floor = [17, 34, 51][act]
        elif e["room"] == "Elite":
            floor = rng.randint(floor_lo + 4, floor_hi)
        else:
            floor = rng.randint(floor_lo, floor_hi)
        deck_s, relic, base = STARTERS[ch]
        deck = []
        ench_p = 0.12 if rng.random() < 0.35 else 0.0   # a third of the runs carry a few enchanted cards
        for cid in deck_s:
            deck.append(self.card(rng, {"id": cid, "max_upgrade": 1}, 0.12, ench_p))
        nadd = rng.randint(5, 12) if act == 0 else rng.randint(8, 18) if act == 1 else rng.randint(11, 25)
        nadd = min(nadd, 25)
        pool = self.cards[ch]
        byr = collections.defaultdict(list)
        for c in pool:
            byr[c["rarity"]].append(c)
        for _ in range(nadd):
            x = rng.random()
            if x < 0.22:
                c = rng.choice(self.colorless)
            else:
                y = rng.random()
                r = "Common" if y < 0.45 else "Uncommon" if y < 0.85 else "Rare" if y < 0.985 else "Ancient"
                c = rng.choice(byr[r] or pool)
            deck.append(self.card(rng, c, 0.4, ench_p))
        if rng.random() < 0.5:   # themed deck: a few cards with extra copies, so card x card interactions occur
            for c in [rng.choice(pool + self.colorless) for _ in range(rng.randint(2, 3))]:
                for _ in range(rng.randint(1, 2)):
                    deck.append(self.card(rng, c, 0.4, ench_p))
        if rng.random() < 0.2:
            for _ in range(rng.randint(1, 2)):
                deck.append(self.card(rng, rng.choice(self.curses), 0.0))
        if rng.random() < 0.05:
            deck.append(self.card(rng, rng.choice(self.status), 0.0))
        # a few removals of starter cards (card removal at shops / events)
        for _ in range(rng.choice([0, 0, 1, 1, 2, 3])):
            i = rng.randrange(len(deck_s))
            if i < len(deck) and rng.random() < 0.8:
                deck[i] = None
        deck = [c for c in deck if c is not None]
        deck.append("ASCENDERS_BANE")
        relics = [relic]
        nrel = rng.choice([0, 0, 1, 2, 2, 3, 3, 4, 5, 6])
        for _ in range(nrel):
            r = rng.choice(self.relics_rl) if rng.random() < 0.08 else rng.choice(self.relics[ch])
            if r not in relics:
                relics.append(r)
        if rng.random() < 0.3:   # relics that raise decisions / auto-play / reshape the turn
            for r in rng.sample(STRESS_RELICS, rng.randint(1, 2)):
                if r in self.relics[ch] and r not in relics:
                    relics.append(r)
        npot = rng.choice([0, 1, 1, 2, 2])
        pots = [rng.choice(self.potions[ch]) for _ in range(npot)]
        tank = rng.random() < 0.12   # not a realistic HP pool: survives long enough to reach deep turns of the fight
        maxhp = rng.randint(150, 400) if tank else rng.randint(50, 90)
        hp = maxhp if (tank or rng.random() < 0.5) else max(1, int(maxhp * rng.uniform(0.3, 1.0)))
        pk = policy or (rng.choices(["random", "playall", "stall"], [1, 3, 2])[0] if tank else rng.choices(["random", "playall", "stall"], [5, 3, 1.2])[0])
        pol = {"kind": pk, "seed": rng.randrange(1, 1 << 30)}
        if pk != "random":
            pol["max_steps"] = 1500
            pol["max_rounds"] = 80
        return {"name": name, "ascension": 10, "encounter": e["id"], "character": ch, "hp": hp, "max_hp": maxhp,
                "max_energy": 3, "base_orb_slots": 0, "max_potion_slots": 2, "gold": rng.choice([0, 99, 99, 250]),
                "seed": seed_str, "total_floor": floor, "act": act, "deck": deck, "relics": relics, "potions": pots, "policy": pol}


def gen(a):
    g = Gen(a.characters.split(","), set(a.encounters.split(",")) if a.encounters else None)
    os.makedirs(a.out, exist_ok=True)
    names = []
    for i in range(a.n):
        k = i % a.jobs
        d = os.path.join(a.out, f"job{k}")
        os.makedirs(d, exist_ok=True)
        name = f"f{a.seed}_{i}"
        rng = random.Random(f"{a.seed}/{i}")
        sc = g.make(rng, name, f"fz{a.seed}x{i}", policy=a.policy)
        json.dump(sc, open(os.path.join(d, name + ".scenario.json"), "w"))
        names.append((k, name))
    return names


def oracle_job(d):
    r = subprocess.run([ORACLE, "batch", d, "--max-steps", "600"], capture_output=True, text=True)
    return d, r.returncode, (r.stderr or "")[-300:]


def diff_one(base):
    if os.path.exists(base + ".err"):
        return base, "oracle-error", open(base + ".err").read()[:400].strip()
    if not os.path.exists(base + ".res"):
        return base, "oracle-missing", ""
    d = subprocess.run([DIFF, "run", base + ".scenario.json", base + ".jsonl", "--max", "4"], capture_output=True, text=True)
    out = (d.stdout or "").strip()
    if "not implemented in the simulator" in (d.stderr or ""):
        return base, "unimplemented", "UNIMPLEMENTED " + d.stderr.split("UnimplementedPotion(")[-1].strip()
    if d.returncode == 0:
        return base, "ok", open(base + ".res").read().strip()
    if d.returncode == 3:
        return base, "unimplemented", out.splitlines()[-1] if out else ""
    return base, ("mismatch" if d.returncode == 1 else "sim-error"), (out or d.stderr)[-700:]


def run(a):
    names = gen(a)
    dirs = sorted({os.path.join(a.out, f"job{k}") for k, _ in names})
    print(f"generated {len(names)} scenarios in {len(dirs)} job dirs", flush=True)
    import time; t0 = time.time()
    with ThreadPoolExecutor(a.jobs) as ex:
        for d, rc, err in ex.map(oracle_job, dirs):
            if rc != 0:
                print("oracle batch failed", d, err)
    print(f"oracle done in {time.time()-t0:.0f}s; diffing", flush=True)
    bases = [os.path.join(a.out, f"job{k}", n) for k, n in names]
    res = collections.Counter()
    bad = []
    lengths = collections.Counter()
    with ThreadPoolExecutor(a.jobs) as ex:
        for base, verdict, msg in ex.map(diff_one, bases):
            res[verdict] += 1
            if verdict == "ok":
                lengths[msg.split()[0]] += 1
                for ext in (".scenario.json", ".jsonl", ".res"):
                    try: os.remove(base + ext)
                    except OSError: pass
            else:
                bad.append((base, verdict, msg))
    miss = collections.Counter()
    for base, v, msg in bad:
        if v == "unimplemented":
            miss[msg.split(" (step")[0].replace("UNIMPLEMENTED ", "")] += 1
    # group mismatches by the first differing path (indices stripped)
    groups = collections.defaultdict(list)
    for base, v, msg in bad:
        if v == "unimplemented":
            continue
        if v == "oracle-error":
            key = v + " " + msg[:100].replace("\n", " ")
        else:
            dl = [l.strip() for l in msg.splitlines() if l.startswith("    ")]
            key = v + " " + (re.sub(r"\[\d+\]", "[]", dl[0].split(":")[0])[:90] if dl else msg[:80].replace("\n", " "))
        groups[key].append(base)
    print("RESULTS", dict(res), "fight results", dict(lengths))
    if miss:
        print("UNIMPLEMENTED:", ", ".join(f"{k} x{v}" for k, v in miss.most_common()))
    for k, v in sorted(groups.items(), key=lambda t: -len(t[1])):
        print(f"{len(v):4d}  {k}   e.g. {v[0]}")
    json.dump({"results": res, "groups": groups, "unimplemented": miss}, open(os.path.join(a.out, "summary.json"), "w"), indent=1)


def triage(a):
    """Print the first differences of every failing scenario under DIR (from a previous run)."""
    for d in sorted(glob.glob(os.path.join(a.out, "job*"))):
        for f in sorted(glob.glob(os.path.join(d, "*.scenario.json"))):
            base = f[:-len(".scenario.json")]
            sc = json.load(open(f))
            _, v, msg = diff_one(base)
            print(f"== {base} [{v}] {sc['encounter']} {sc['character']} policy={sc['policy']['kind']} relics={sc['relics'][1:]} potions={sc['potions']}")
            print("   " + msg.replace("\n", "\n   ")[:600])


def freeze(a):
    """Turn a failing fuzz scenario into a committed, policy-independent regression scenario (oracle/regression/NAME.scenario.json):
    the oracle's recorded script is replayed up to the first mismatching step (inclusive)."""
    base = a.base
    sc = json.load(open(base + ".scenario.json"))
    d = subprocess.run([DIFF, "run", base + ".scenario.json", base + ".jsonl", "--max", "1"], capture_output=True, text=True)
    m = re.search(r"step (\d+)", d.stdout)
    step = a.step if a.step is not None else (int(m.group(1)) if m else 10**9)
    pol = sc.pop("policy")
    tmp = base + ".frozen.json"
    r = subprocess.run([ORACLE, "run", base + ".scenario.json", "--random", str(pol["seed"]), "--policy", pol["kind"], "--max-steps", "1500",
                        "--max-rounds", "80", "--out", base + ".frozen.jsonl", "--record", tmp], capture_output=True, text=True)
    if not os.path.exists(tmp):
        sys.exit("oracle failed: " + r.stderr[-300:])
    full = json.load(open(tmp))
    os.remove(tmp); os.remove(base + ".frozen.jsonl")
    script, n = [], 0
    for e in full["script"]:
        if "choose" in e:
            if n <= step:
                script.append(e)
            continue
        n += 1
        if n > step:
            break
        script.append(e)
    full["script"] = script
    full.pop("result", None)
    full["name"] = a.name
    full["note"] = a.note or f"fuzz regression {a.name}"
    os.makedirs(os.path.join(ROOT, "oracle/regression"), exist_ok=True)
    out = os.path.join(ROOT, "oracle/regression", a.name + ".scenario.json")
    json.dump(full, open(out, "w"), separators=(",", ":"))
    print("wrote", out, f"({len(script)} script entries, step {step})")


def regress(a):
    """Replay every oracle/regression/*.scenario.json through the oracle (scripted) and the Rust diff."""
    import shutil, tempfile
    d = tempfile.mkdtemp(prefix="sts2reg_")
    files = sorted(glob.glob(os.path.join(ROOT, "oracle/regression/*.scenario.json")))
    for f in files:
        shutil.copy(f, d)
    subprocess.run([ORACLE, "batch", d], capture_output=True, text=True)
    bad = 0
    for f in files:
        base = os.path.join(d, os.path.basename(f)[:-len(".scenario.json")])
        _, v, msg = diff_one(base)
        if v not in ("ok",):
            bad += 1
            print(f"FAIL {os.path.basename(f)} [{v}] {msg[:300]}")
    print(f"regression scenarios: {len(files)}, failing: {bad}")
    sys.exit(1 if bad else 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["gen", "run", "triage", "freeze", "regress"])
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", default=None)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--characters", default="IRONCLAD,SILENT")
    ap.add_argument("--encounters", default=None)
    ap.add_argument("--base", help="freeze: failing scenario path without .scenario.json")
    ap.add_argument("--name", help="freeze: regression name")
    ap.add_argument("--note", default=None)
    ap.add_argument("--step", type=int, default=None)
    ap.add_argument("--policy", default=None, help="force one policy kind (random|playall|stall)")
    a = ap.parse_args()
    {"gen": gen, "run": run, "triage": triage, "freeze": freeze, "regress": regress}[a.cmd](a)


main()
