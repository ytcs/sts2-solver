#!/usr/bin/env python3
"""Randomised differential fuzzing for the orb (Defect) and pet (Necrobinder) characters, ascension 10.

  tools/fuzz_gen_orb_pet.py --n 3000 --seed 1 --out DIR [--jobs 5] [--characters DEFECT,NECROBINDER]
      generate N realistic A10 scenarios (starter deck + 5-25 random cards (+curses/status) + random upgrades + random
      relics/potions + realistic hp + random encounter of any act/room type), run the oracle's random policy over all of
      them (one `oracle batch` process per shard: no per-fight JIT cost) and diff the traces against Rust (`sts2diff run`).
  tools/fuzz_gen_orb_pet.py --gen-only ...        only write DIR/*.scenario.json
  tools/fuzz_gen_orb_pet.py --encounter X ...     restrict to one encounter id (repro / focus)

Mismatching runs keep NAME.scenario.json + NAME.jsonl in DIR; clean ones are deleted. `--keep-ok` keeps everything.
Verdicts: ok | mismatch | unimplemented | sim-error | oracle-error. Metadata (pools, encounters) comes from `oracle list-meta`.
"""
import argparse, json, os, random, re, subprocess, sys, glob
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ORACLE = os.path.join(ROOT, "oracle/combat/oracle.sh")
DIFF = os.environ.get("STS2DIFF") or os.path.join(ROOT, "target/debug/sts2diff")
DECOMP = os.environ.get("STS2_DECOMP", "/home/ytc/Projects/sts2-solver/decomp/MegaCrit/Sts2/Core/Models/Cards")

# interaction-heavy relics / potions get extra weight
ORB_RELICS = {"DATA_DISK", "EMOTION_CHIP", "GOLD_PLATED_CABLES", "POWER_CELL", "RUNIC_CAPACITOR", "SYMBIOTIC_VIRUS", "METRONOME"}
PET_RELICS = {"BIG_HAT", "BONE_FLUTE", "BOOKMARK", "BOOK_REPAIR_KNIFE", "FUNERARY_MASK", "IVORY_TILE", "UNDYING_SIGIL"}
ORB_POTIONS = {"ESSENCE_OF_DARKNESS", "FOCUS_POTION", "POTION_OF_CAPACITY"}
PET_POTIONS = {"BONE_BREW", "POTION_OF_DOOM", "POT_OF_GHOULS"}
# not ported in Rust (see tools/coverage.py): skipped so fights are not wasted on UNIMPLEMENTED
UNPORTED_POTIONS = set()
THEME_RE = {
    "DEFECT": re.compile(r"Orb|Focus|Channel|Evoke|Lightning|Frost|Plasma|Glass|Dark"),
    "NECROBINDER": re.compile(r"Osty|Doom|Soul|Summon|Pet|Sacrifice|Revive"),
}


def load_meta():
    p = os.path.join(ROOT, "target", "sts2_fuzz_meta.json")
    os.makedirs(os.path.dirname(p), exist_ok=True)
    if not os.path.exists(p) or os.path.getmtime(p) < os.path.getmtime(os.path.join(ROOT, "oracle/combat/Fuzz.cs")):
        subprocess.run([ORACLE, "list-meta", "--out", p], check=True, capture_output=True)
    m = json.load(open(p))
    # single-player runs never contain multiplayer-only cards / relics (CardPoolModel constraint, RelicModel.IsAllowed)
    for ch in m["characters"].values():
        ch["cards"] = [c for c in ch["cards"] if c["mp"] != "MultiplayerOnly"]
        ch["relics"] = [r for r in ch["relics"] if r != "MASSIVE_SCROLL"]
    for k in ("colorless", "curse", "status"):
        m[k] = [c for c in m[k] if c["mp"] != "MultiplayerOnly"]
    m["shared_relics"] = [r for r in m["shared_relics"] if r["id"] != "MASSIVE_SCROLL"]
    return m


def card_theme(meta):
    """id -> True when the decompiled card source mentions the character's orb/pet vocabulary."""
    out = {}
    for ch in ("DEFECT", "NECROBINDER"):
        for c in meta["characters"][ch]["cards"]:
            f = os.path.join(DECOMP, "".join(w.capitalize() for w in c["id"].split("_")) + ".cs")
            try:
                out[c["id"]] = bool(THEME_RE[ch].search(open(f).read()))
            except OSError:
                out[c["id"]] = False
    return out


def make(meta, theme, i, rnd, characters, encounter=None, unported=frozenset()):
    ch = rnd.choice(characters)
    cm = meta["characters"][ch]
    encs = [e for e in meta["encounters"] if encounter is None or e["id"] == encounter]
    enc = rnd.choice(encs)
    act = enc["act"] if enc["act"] >= 0 else rnd.randint(0, 2)
    floor = [rnd.randint(1, 16), rnd.randint(18, 33), rnd.randint(35, 50)][act]
    if enc["room"] == "Boss":
        floor = [17, 34, 51][act]
    # --- deck
    deck = [{"id": c} for c in cm["starting_deck"]]
    pool = [c for c in cm["cards"] if c["rarity"] in ("Common", "Uncommon", "Rare")]
    if rnd.random() < 0.1:
        pool = pool + [c for c in cm["cards"] if c["rarity"] == "Ancient"]
    weights = [(3 if theme.get(c["id"]) else 1) * {"Common": 3, "Uncommon": 2, "Rare": 1.3}.get(c["rarity"], 1) for c in pool]
    if rnd.random() < 0.35:  # "build" mode: strongly theme-focused deck
        weights = [w * (6 if theme.get(c["id"]) else 1) for w, c in zip(weights, pool)]
    n_add = rnd.randint(5, 25)
    dupe_ok = rnd.random() < 0.5
    for _ in range(n_add):
        r = rnd.random()
        if r < 0.15:
            c = rnd.choice(meta["colorless"])
            if c["rarity"] not in ("Common", "Uncommon", "Rare", "Ancient"):
                continue
        elif r < 0.19:
            c = rnd.choice([x for x in meta["curse"] if x["id"] != "ASCENDERS_BANE"])
        elif r < 0.22:
            c = rnd.choice(meta["status"])
        else:
            c = rnd.choices(pool, weights)[0]
        if not dupe_ok and c["rarity"] == "Rare" and any(d["id"] == c["id"] for d in deck):
            continue
        d = {"id": c["id"]}
        if c["max_up"] > 0 and rnd.random() < 0.4:
            d["upgrade"] = 1
        deck.append(d)
    deck.append({"id": "ASCENDERS_BANE"})
    # --- relics (starter kept first)
    relics = list(cm["starting_relics"])
    rpool = [r for r in cm["relics"] if r not in relics] + [r["id"] for r in meta["shared_relics"] if r["rarity"] in ("Common", "Uncommon", "Rare", "Shop", "Event")]
    focus = ORB_RELICS if ch == "DEFECT" else PET_RELICS
    rw = [4 if r in focus else 1 for r in rpool]
    for _ in range(rnd.choice([0, 0, 1, 2, 3, 3, 4, 5, 6])):
        r = rnd.choices(rpool, rw)[0]
        if r not in relics:
            relics.append(r)
    # --- potions (A10: 2 slots)
    potions = []
    ppool = [p for p in cm["potions"] + meta["shared_potions"] if p not in UNPORTED_POTIONS and p not in unported]
    pfocus = ORB_POTIONS if ch == "DEFECT" else PET_POTIONS
    pw = [5 if p in pfocus else 1 for p in ppool]
    for _ in range(rnd.choice([0, 1, 1, 2, 2])):
        potions.append(rnd.choices(ppool, pw)[0])
    # --- hp
    base = cm["hp"]
    max_hp = base + rnd.randint(0, 12) + act * rnd.randint(0, 14) + rnd.choice([0, 0, 0, 7, 14])
    hp = max(5, int(max_hp * rnd.uniform(0.45, 1.0)))
    if rnd.random() < 0.2:  # stress mode: survive long enough to reach deep into the monster move sets
        max_hp = rnd.randint(150, 300)
        hp = max_hp
    s = {
        "name": f"{ch.lower()}_{enc['id'].lower()}_{i}", "ascension": 10, "encounter": enc["id"], "character": ch,
        "hp": hp, "max_hp": max_hp, "max_energy": 3, "base_orb_slots": 3 if ch == "DEFECT" else 0,
        "max_potion_slots": 2, "seed": f"fz{i}x{rnd.randint(0, 10**9)}", "total_floor": floor, "act": act,
        "deck": deck, "relics": relics, "potions": potions,
    }
    return s


def run_shard(args):
    paths, shard_dir, policy_base = args
    lst = os.path.join(shard_dir, "list.txt")
    open(lst, "w").write("\n".join(paths) + "\n")
    r = subprocess.run([ORACLE, "batch", "--list", lst, "--policy-base", str(policy_base)], capture_output=True, text=True)
    return r.returncode, r.stderr[-500:]


def diff_one(base):
    if os.path.exists(base + ".err"):
        return base, "oracle-error", open(base + ".err").read()[:400]
    if not os.path.exists(base + ".jsonl"):
        return base, "oracle-error", "no trace"
    d = subprocess.run([DIFF, "run", base + ".scenario.json", base + ".jsonl", "--max", "4"], capture_output=True, text=True)
    out = d.stdout.strip()
    if d.returncode == 0:
        return base, "ok", ""
    if d.returncode == 3:
        return base, "unimplemented", out.splitlines()[-1]
    m = re.search(r"Unimplemented(\w+)\(\"(\w+)\"\)", out + d.stderr)
    if m:
        return base, "unimplemented", f"UNIMPLEMENTED {m.group(1).lower()} {m.group(2)}"
    return base, "mismatch" if d.returncode == 1 else "sim-error", (out or d.stderr)[:900]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", required=True)
    ap.add_argument("--jobs", type=int, default=5)
    ap.add_argument("--characters", default="DEFECT,NECROBINDER")
    ap.add_argument("--encounter", default=None)
    ap.add_argument("--policy-seed", type=int, default=None)
    ap.add_argument("--gen-only", action="store_true")
    ap.add_argument("--keep-ok", action="store_true")
    ap.add_argument("--skip-potions", default="")
    ap.add_argument("--recheck", action="store_true", help="only re-diff the kept NAME.scenario.json/.jsonl pairs in --out (after a fix)")
    a = ap.parse_args()
    if a.recheck:
        bases = sorted(p[: -len(".scenario.json")] for p in glob.glob(os.path.join(a.out, "*.scenario.json")))
        res = {}
        with ThreadPoolExecutor(4) as ex:
            for base, v, msg in ex.map(diff_one, bases):
                res[v] = res.get(v, 0) + 1
                if v != "ok":
                    print(f"--- {v}: {base}\n{msg}")
        print("SUMMARY", json.dumps(res))
        return
    meta = load_meta()
    theme = card_theme(meta)
    os.makedirs(a.out, exist_ok=True)
    rnd = random.Random(a.seed)
    chars = a.characters.split(",")
    bases = []
    for i in range(a.n):
        s = make(meta, theme, a.seed * 100000 + i, rnd, chars, a.encounter, set(filter(None, a.skip_potions.split(","))))
        base = os.path.join(a.out, s["name"])
        json.dump(s, open(base + ".scenario.json", "w"))
        bases.append(base)
    if a.gen_only:
        return
    pol = a.policy_seed if a.policy_seed is not None else a.seed * 1000
    shards = [bases[k::a.jobs] for k in range(a.jobs)]
    jobs = []
    for k, sh in enumerate(shards):
        d = os.path.join(a.out, f"shard{k}")
        os.makedirs(d, exist_ok=True)
        jobs.append(([b + ".scenario.json" for b in sh], d, pol + k * 100003))
    with ThreadPoolExecutor(a.jobs) as ex:
        for rc, err in ex.map(run_shard, jobs):
            if rc != 0:
                print("oracle shard failed:", err, file=sys.stderr)
    res, bad = {}, []
    with ThreadPoolExecutor(4) as ex:
        for base, v, msg in ex.map(diff_one, bases):
            res[v] = res.get(v, 0) + 1
            if v == "ok" and not a.keep_ok:
                for ext in (".scenario.json", ".jsonl"):
                    if os.path.exists(base + ext):
                        os.remove(base + ext)
            elif v == "unimplemented" and not a.keep_ok:
                bad.append((base, v, msg))
                for ext in (".jsonl",):
                    if os.path.exists(base + ext):
                        os.remove(base + ext)
            elif v != "ok":
                bad.append((base, v, msg))
    missing = {}
    for base, v, msg in bad:
        if v == "unimplemented":
            name = msg.split(" (step")[0].replace("UNIMPLEMENTED ", "")
            missing[name] = missing.get(name, 0) + 1
    nonmiss = [b for b in bad if b[1] != "unimplemented"]
    for base, v, msg in nonmiss[:40]:
        print(f"--- {v}: {base}\n{msg}")
    if missing:
        print("UNIMPLEMENTED content hit:", ", ".join(f"{k} x{v}" for k, v in sorted(missing.items(), key=lambda t: -t[1])))
    print("SUMMARY", json.dumps(res), f"(artifacts in {a.out})")
    sys.exit(0 if not nonmiss else 1)


main()
