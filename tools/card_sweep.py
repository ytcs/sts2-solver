#!/usr/bin/env python3
"""Per-entity oracle-vs-sim sweep (A10): every card of the non-Fool pools (2 forced copies, both upgrade levels), every enchantment
on eligible cards, every affliction via its encounter; reports times played per entity. Usage: card_sweep.py OUT --kind cards|enchant|afflict --per K [--cards A,B]
Needs target/release/sts2diff (or STS2DIFF=) and the built oracle."""
import argparse, json, os, random, subprocess, sys, types, glob
from concurrent.futures import ThreadPoolExecutor
WT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(WT, "tools"))
os.environ.setdefault("STS2DIFF", os.path.join(WT, "target/release/sts2diff" + (".exe" if os.name == "nt" else "")))
import fuzz_gen_mix as F  # noqa
F.DIFF = os.environ["STS2DIFF"]
if os.environ.get("ORACLE_DLL"):  # e.g. the main checkout's build when running from a worktree
    F.ORACLE = ["dotnet", os.environ["ORACLE_DLL"]]
HERE = None  # set in main: OUT holds catalog.json / meta.json

CHARS = ["IRONCLAD", "SILENT", "DEFECT", "NECROBINDER", "REGENT"]
POOLS = CHARS + ["COLORLESS", "CURSE", "STATUS", "TOKEN", "EVENT", "QUEST"]
MODES = ["deep", "greedy", "stall", "uniform", "deep", "greedy"]
AFFLICT = {"RINGING": "CEREMONIAL_BEAST_BOSS", "GALVANIZED": "GLOBE_HEAD_NORMAL", "TAINTED": "INFESTED_PRISMS_ELITE",
           "HEXED": "KNIGHTS_ELITE", "SMOG": "LIVING_FOG_NORMAL", "BOUND": "QUEEN_BOSS", "ENTANGLED": "VINE_SHAMBLER_NORMAL"}


FOOL_TOKENS = {"JOY", "SADNESS", "AMNESIA", "DEATH", "JUDGEMENT", "JUSTICE", "STRENGTH", "TEMPERANCE", "WHEEL_OF_FORTUNE"}


def fool_only(cat):
    return ({c["id"] for c in cat["cards"].get("FOOL", [])} | FOOL_TOKENS
            | {c["id"] for c in cat["cards"].get("TOKEN", []) if c["id"].startswith("THE_")})


F.fool_only = fool_only


def args_ns(**kw):
    d = dict(character=None, encounter=None, act=None, room=None, mode=None, focus="mix", cross=0.0, cross_w=0, enchant=0.0,
             relics="2-6", force_potions=None, force_relics=None, force_cards=None, ascension="10", fool_w=0, tarot=0, each_relic=False, each_potion=False)
    d.update(kw)

    class NS(types.SimpleNamespace):
        def __getattr__(self, k):
            return None
    return NS(**d)


def cid_of(c):
    return c if isinstance(c, str) else c["id"]


def excluded(g):
    return F.multiplayer_only() | F.fool_only(g.cat) | F.fool_only(json.load(open(os.path.join(WT, "data", "catalog.json"))))


def owners(g):
    owner = {}
    for pool in POOLS:
        for c in g.cat["cards"].get(pool, []):
            owner.setdefault(c["id"], pool)
    return owner


def shrink(r, deck):
    rest = [c for c in deck if cid_of(c) != "ASCENDERS_BANE"]
    r.shuffle(rest)
    return rest[: r.randint(5, 10)] + [c for c in deck if cid_of(c) == "ASCENDERS_BANE"]


def save(out, sc, name, paths):
    sc["name"] = name
    p = os.path.join(out, name + ".scenario.json")
    json.dump(sc, open(p, "w"))
    paths.append(p)


def build(g, out, per, only, seed):
    owner, excl = owners(g), excluded(g)
    targets = sorted(cid for cid in owner if cid not in excl and (not only or cid in only))
    unimpl = [cid for cid in targets if cid not in g.have["card"]]
    paths = []
    for ci, cid in enumerate(targets):
        pool = owner[cid]
        mu = g.max_upgrade.get(cid, 0)
        for k in range(per):
            r = random.Random(f"{seed}/{cid}/{k}")
            ch = pool if pool in CHARS else CHARS[(k + ci) % 5]
            a = args_ns(character=ch, mode=MODES[k % len(MODES)], ascension="10")
            sc = g.scenario(k, f"{seed}{cid}", a)
            deck = shrink(r, sc["deck"]) if k % 2 == 0 else sc["deck"]
            if cid == "DINGY_DAGGER":
                ups = [[0, 1], [2, 5], [1, 3], [0, 9]][k % 4]
            elif mu:
                ups = [k % 2, (k // 2) % 2]
            else:
                ups = [0, 0]
            for u in ups:
                c = {"id": cid, "upgrade": u} if u else cid
                if cid == "MAD_SCIENCE":
                    ty = r.randint(1, 3)
                    c = {"id": cid, "upgrade": u, "props": {"TinkerTimeRider": 3 * (ty - 1) + r.randint(1, 3), "TinkerTimeType": ty}}
                deck.insert(r.randrange(len(deck) + 1), c)
            sc["deck"] = deck
            sc["meta"]["target"] = cid
            save(out, sc, f"c_{cid}_{k}", paths)
    return paths, unimpl


def build_enchant(g, out, per, seed):
    meta = json.load(open(os.path.join(HERE, "meta.json")))
    owner, excl = owners(g), excluded(g)
    paths = []
    for e in meta["enchantments"]:
        if e["id"] == "DEPRECATED_ENCHANTMENT":
            continue
        tgt = {c["id"]: c.get("target") for pool in g.cat["cards"].values() for c in pool}
        elig = sorted(c for c in e["cards"] if c in owner and c not in excl and c in g.have["card"] and c != "MAD_SCIENCE"
                      and (e["id"] != "INKY" or tgt.get(c) in ("AnyEnemy", "AllEnemies")))
        r0 = random.Random(f"{seed}/{e['id']}")
        picks = r0.sample(elig, min(len(elig), per)) if elig else []
        for k, cid in enumerate(picks):
            r = random.Random(f"{seed}/{e['id']}/{k}")
            pool = owner[cid]
            ch = pool if pool in CHARS else CHARS[k % 5]
            a = args_ns(character=ch, mode=MODES[k % len(MODES)], ascension="10")
            sc = g.scenario(k, f"{seed}{e['id']}", a)
            deck = shrink(r, sc["deck"]) if k % 2 == 0 else sc["deck"]
            amt = r.choice([1, 2, 3, 4])
            for u in (0, 1 if g.max_upgrade.get(cid, 0) else 0):
                deck.insert(r.randrange(len(deck) + 1), {"id": cid, "upgrade": u, "enchantment": {"id": e["id"], "amount": amt}})
            sc["deck"] = deck
            sc["meta"]["target"] = e["id"]
            sc["meta"]["card"] = cid
            save(out, sc, f"e_{e['id']}_{cid}_{k}", paths)
    return paths


def build_afflict(g, out, per, seed):
    paths = []
    for af, enc in AFFLICT.items():
        for k in range(per):
            a = args_ns(character=CHARS[k % 5], mode=MODES[k % len(MODES)], ascension="10", encounter=enc)
            sc = g.scenario(k, f"{seed}{af}", a)
            sc["meta"]["target"] = af
            save(out, sc, f"a_{af}_{k}", paths)
    return paths


def count(base):
    plays, seen = {}, set()
    prev = None
    try:
        for line in open(base + ".jsonl"):
            rec = json.loads(line)
            act = rec.get("action") or {}
            if prev is not None and "play" in act:
                hp = act["play"]["hand_pos"]
                h = prev.get("hand", [])
                if hp < len(h):
                    for key in ("affliction", "enchantment"):
                        if h[hp].get(key):
                            x = h[hp][key]["id"]
                            plays[x] = plays.get(x, 0) + 1
            for c in rec.get("hand", []):
                for key in ("affliction", "enchantment"):
                    if c.get(key):
                        seen.add(c[key]["id"])
            prev = rec
            for c in rec.get("hand", []):
                seen.add(c["id"])
            for l in rec.get("log", []):
                if l.startswith("play "):
                    x = l.split()[1]
                    plays[x] = plays.get(x, 0) + 1
    except (OSError, ValueError):
        pass
    return plays, seen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--per", type=int, default=6)
    ap.add_argument("--cards")
    ap.add_argument("--seed", default="s1")
    ap.add_argument("--kind", default="cards", choices=["cards", "enchant", "afflict"])
    ap.add_argument("--jobs", type=int, default=12)
    ap.add_argument("--rerun", action="store_true", help="reuse existing scenarios in OUT")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    global HERE
    HERE = a.out
    for cmd, f in (("catalog", "catalog.json"), ("list-meta", "meta.json")):
        if not os.path.exists(os.path.join(a.out, f)):
            subprocess.run(F.ORACLE + [cmd, "--out", os.path.join(a.out, f)], capture_output=True, check=True)
    g = F.Gen(os.path.join(HERE, "catalog.json"))
    ex = excluded(g)
    g.cards = {k: [c for c in v if c["id"] not in ex] for k, v in g.cards.items() if k != "FOOL"}
    unimpl = []
    if a.rerun:
        paths = sorted(glob.glob(os.path.join(a.out, "*.scenario.json")))
    elif a.kind == "cards":
        paths, unimpl = build(g, a.out, a.per, set(a.cards.split(",")) if a.cards else None, a.seed)
    elif a.kind == "enchant":
        paths = build_enchant(g, a.out, a.per, a.seed)
    else:
        paths = build_afflict(g, a.out, a.per, a.seed)
    chunks = [(paths[i:i + 20], True) for i in range(0, len(paths), 20)]
    res = []
    with ThreadPoolExecutor(a.jobs) as ex:
        for rr in ex.map(F.run_chunk, chunks):
            res.extend(rr)
    cov, counts, bad, allplays = {}, {}, [], {}
    for base, verdict, msg in res:
        counts[verdict] = counts.get(verdict, 0) + 1
        sc = json.load(open(base + ".scenario.json"))
        t = sc["meta"]["target"]
        plays, seen = count(base)
        e = cov.setdefault(t, {"fights": 0, "ok": 0, "played": 0, "played_ok": 0, "drawn": 0})
        e["fights"] += 1
        e["ok"] += verdict == "ok"
        e["played"] += plays.get(t, 0)
        e["drawn"] += t in seen
        if verdict == "ok":
            e["played_ok"] += plays.get(t, 0)
            for x, n in plays.items():
                allplays[x] = allplays.get(x, 0) + n
            for ext in (".scenario.json", ".jsonl"):
                try: os.remove(base + ext)
                except OSError: pass
        else:
            bad.append((base, verdict, msg))
    json.dump({"targets": cov, "all_plays_ok": allplays}, open(os.path.join(a.out, "coverage.json"), "w"), indent=0)
    with open(os.path.join(a.out, "bad.txt"), "w") as f:
        for base, verdict, msg in bad:
            sc_meta = json.load(open(base + ".scenario.json"))["meta"]
            f.write(f"--- {verdict}: {base} {sc_meta}\n{msg}\n")
    print("SUMMARY", json.dumps(counts), "unimplemented targets:", unimpl)
    never = [t for t, e in cov.items() if e["played"] == 0]
    print("never played:", len(never), " ".join(sorted(never)))


if __name__ == "__main__":
    main()
