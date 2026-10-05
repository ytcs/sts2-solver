"""Simulator fidelity sweep (a harness test, never a scored run; uses the dev console).

Every case starts a FRESH run (so no relic cooldown, counter or custom card carries over), gives it a loadout through the dev console, jumps to an encounter and
plays the fight with the harness solver, ignoring the desync stop so that play continues. The harness records every place where the simulator's prediction of the
next state differed from the game's (`Replayer` stats and examples, with the card played). Bugs found here are bugs in the networks' training data: fix them first.

Modes (`--mode`):
  cards     each card of each character (`--chars`), three copies in an otherwise starting deck
  relics    each relic that has no pickup effect, alone
  potions   each potion, alone
  fuzz      random decks (14-22 cards of the character's pool and colorless), 3-8 relics, 0-2 potions, random encounters
  recorded  the relic / deck sets recorded in earlier runs (Ironclad)

    python -m agent.fidelity_sweep --mode cards --chars ironclad,silent --out evals/fid_cards.jsonl [--limit N] [--resume]

One JSON line per case: label, encounter, non-benign divergence categories with counts and examples. `evals/fidelity_fights/` keeps the game's own log for cases with
state divergences (replay them with `python -m agent.fidelity_trace`).
"""
import argparse
import collections
import glob
import json
import os
import random
import re
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from agent import pools  # noqa: E402
from agent.bridge import call  # noqa: E402
from agent.harness import Harness  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
def _decomp_relics():
    """The decompiled relic sources (a relic with an AfterObtained effect cannot be added through the console). `STS2_DECOMP` or ./decomp, either layout."""
    base = os.environ.get("STS2_DECOMP") or os.path.join(ROOT, "decomp")
    for sub in ("MegaCrit.Sts2.Core.Models.Relics", os.path.join("MegaCrit", "Sts2", "Core", "Models", "Relics")):
        if os.path.isdir(os.path.join(base, sub)):
            return os.path.join(base, sub)
    raise SystemExit(f"decompiled relics not found under {base}: set STS2_DECOMP to the decompile directory")
ENCOUNTERS = ["SLIMES_NORMAL", "FLYCONID_NORMAL", "INKLETS_NORMAL", "CUBEX_CONSTRUCT_NORMAL", "KNIGHTS_ELITE", "DECIMILLIPEDE_ELITE", "BYGONE_EFFIGY_ELITE",
              "CORPSE_SLUGS_NORMAL", "HAUNTED_SHIP_NORMAL", "EXOSKELETONS_NORMAL", "ENTOMANCER_ELITE", "SOUL_NEXUS_ELITE"]
BENIGN_PREFIX = ("random", "residual .phase", "start_tries", "end_turn", "sync from_discard", "sync from_exhaust", "sync created", "sync powers")


def pool_ids(name):
    src = open(os.path.join(ROOT, "crates", "sts2sim", "src", "content", "gen_pools.rs"), encoding="utf-8").read()
    body = src.split(f"pub static {name}:")[1].split("];")[0]
    return re.findall(r"ids::card::([A-Z0-9_]+)", body)


def camel(snake):
    return "".join(p.capitalize() for p in snake.lower().split("_"))


def relic_ids():
    """Implemented relics without an AfterObtained effect (a pickup effect opens a selection screen when the console adds the relic)."""
    src = open(os.path.join(ROOT, "crates", "sts2sim", "src", "content", "gen_relics.rs"), encoding="utf-8").read()
    out = []
    decomp = _decomp_relics()
    for m in re.findall(r"pub mod ([a-z0-9_]+)\s*\{", src):
        f = os.path.join(decomp, camel(m) + ".cs")
        if os.path.exists(f) and "AfterObtained" in open(f, encoding="utf-8").read():
            continue
        out.append(m.upper())
    return out


def potion_ids():
    src = open(os.path.join(ROOT, "crates", "sts2sim", "src", "content", "gen_potions.rs"), encoding="utf-8").read()
    return re.findall(r"PotionDef::new\(ids::potion::([A-Z0-9_]+)", src)


def state_kind(txt):
    return txt.split("\n")[0].split(" ")[0] if txt else ""


def fresh_run(h, character, tries=14):
    """A brand-new run of the character at its first screen (the current run abandoned with the dev console `die`)."""
    for _ in range(tries):
        k = state_kind(call("s"))
        if k == "MENU":
            h.handle(f"a 0 {character} 10")
            return state_kind(call("s")) in ("EVENT", "MAP")
        if k == "GAME_OVER":
            h.handle("a 0")
            continue
        if k == "SELECT":
            h.handle("a 0")  # a selection that needs a pick (Decisions, Decisions ...); `a -` is refused then
            if state_kind(call("s")) == "SELECT":
                h.handle("a -")
            continue
        call("x die")
        time.sleep(0.8)
    return False


STARTER = collections.Counter({"STRIKE_IRONCLAD": 5, "DEFEND_IRONCLAD": 4, "BASH": 1, "ASCENDERS_BANE": 1})
PICKUP = {"HEFTY_TABLET", "ARCANE_SCROLL", "YUMMY_COOKIE", "CLAWS", "WAR_PAINT", "CURSED_PEARL", "MAD_SCIENCE"}  # open a selection screen when the console adds them


def console_loadout(sc, allowed_relics=None):
    """Dev-console commands that rebuild a recorded scenario's relics and added cards on a fresh Ironclad run. Returns (commands, relic ids applied, added cards as a Counter)."""
    relics = [r["id"] for r in sc["relics"] if r["id"] != "BURNING_BLOOD" and r["id"] not in PICKUP and (allowed_relics is None or r["id"] in allowed_relics)]
    need = collections.Counter(c["id"] for c in sc["deck"] if c["id"] not in PICKUP) - STARTER
    return [f"x relic add {r}" for r in relics] + [f"x card {c} Deck" for c, k in need.items() for _ in range(k)], relics, need


def console_fight(h, character, cmds, enc, rounds=25):
    """Fresh run, loadout, full heal, console fight, then the solver plays it. Returns None when the fight was played, else why not."""
    if not fresh_run(h, character):
        return "no fresh run"
    for c in cmds:
        call(c)
    call("x heal 999")
    r = call("x fight " + enc)
    if r.startswith(("fail", "ERR")):
        return "fight: " + r.strip()[:80]
    for _ in range(rounds):
        h.handle("combat")
        if state_kind(call("s")) not in ("COMBAT", "SELECT"):
            break
    return None


def cases(mode, chars, rng, limit):
    out = []
    if mode == "cards":
        for ch in chars:
            ids = pool_ids(ch.upper()) + (pool_ids("COLORLESS") if ch == chars[0] else [])
            for i, cid in enumerate(ids):
                out.append(dict(label=f"card:{ch}:{cid}", character=ch, cmds=[f"x card {cid} Deck"] * 3, enc=ENCOUNTERS[i % len(ENCOUNTERS)]))
    elif mode == "relics":
        for i, rid in enumerate(relic_ids()):
            out.append(dict(label=f"relic:{rid}", character="ironclad", cmds=[f"x relic add {rid}"], enc=ENCOUNTERS[i % len(ENCOUNTERS)]))
    elif mode == "potions":
        for i, pid in enumerate(potion_ids()):
            out.append(dict(label=f"potion:{pid}", character="ironclad", cmds=[f"x potion {pid}"], enc=ENCOUNTERS[i % len(ENCOUNTERS)]))
    elif mode == "fuzz":
        rel, pots = relic_ids(), potion_ids()
        allenc = [e for n in pools.ACTS for k in ("weak", "regular", "elite", "boss") for e in pools.pool(n, k)]
        for i in range(limit or 60):
            ch = rng.choice(chars)
            ids = pool_ids(ch.upper()) + pool_ids("COLORLESS")
            cm = [f"x card {c} Deck" for c in rng.sample(ids, rng.randint(14, 22))] + [f"x relic add {r}" for r in rng.sample(rel, rng.randint(3, 8))]
            cm += [f"x potion {p}" for p in rng.sample(pots, rng.randint(0, 2))]
            out.append(dict(label=f"fuzz:{i}:{ch}", character=ch, cmds=cm, enc=rng.choice(allenc)))
    elif mode == "recorded":
        best = {}
        for f in sorted(glob.glob(os.path.join(ROOT, "runs", "*", "events.jsonl"))):
            for line in open(f, encoding="utf-8"):
                e = json.loads(line)
                if e["kind"] == "fight_start":
                    best[frozenset(r["id"] for r in e["scenario"]["relics"])] = e["scenario"]
        for li, sc in enumerate(sorted(best.values(), key=lambda s: -len(s["relics"]))[: (limit or 8)]):
            cm = console_loadout(sc)[0]
            for j in range(6):
                out.append(dict(label=f"recorded:{li}:{j}", character="ironclad", cmds=cm, enc=rng.choice(ENCOUNTERS)))
    return out[:limit] if limit and mode != "fuzz" else out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", default="cards")
    ap.add_argument("--chars", default="ironclad")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", default="evals/fid.jsonl")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--only", default="", help="comma-separated labels (or substrings) to run, whatever is done already")
    a = ap.parse_args()
    rng = random.Random(a.seed)
    chars = [c for c in a.chars.split(",") if c]
    out_path = os.path.join(ROOT, a.out)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    done = set()
    if a.resume and os.path.exists(out_path):
        done = {json.loads(l)["label"] for l in open(out_path, encoding="utf-8")}
    os.makedirs(os.path.join(ROOT, "evals", "fidelity_fights"), exist_ok=True)
    h = Harness()
    h._sync_problem = lambda f: None
    h.handle("budget 0.25")  # validation needs plausible play, not strong play
    keep = {}
    orig_sync = h.sync

    def sync_and_keep():
        f = orig_sync()
        if f is not None and h.rp is not None:
            keep.update(f=f, scenario=h.rp.scenario, stats=dict(h.rp.stats), examples={k: list(v) for k, v in h.rp.examples.items()}, errors=list(h.rp.errors))
        return f
    h.sync = sync_and_keep
    log = open(os.path.join(ROOT, "evals", "fidelity.log"), "a", encoding="utf-8")
    t_start = time.time()
    n = 0
    for case in cases(a.mode, chars, rng, a.limit):
        if a.only:
            if not any(o in case["label"] for o in a.only.split(",")):
                continue
        elif case["label"] in done:
            continue
        keep.clear()
        res = dict(label=case["label"], enc=case["enc"], ok=False)
        try:
            err = console_fight(h, case["character"], case["cmds"], case["enc"])
            if err:
                res["error"] = err
            else:
                stats = keep.get("stats", {})
                ex = keep.get("examples", {})
                res.update(ok=True, stats={k: v for k, v in stats.items() if not k.startswith(BENIGN_PREFIX)},
                           examples={k: v[:3] for k, v in ex.items() if not k.startswith(BENIGN_PREFIX) and not (k.endswith(".relics") and all("props.Skin" in t for t in v))},
                           created=stats.get("sync created", 0), errors=keep.get("errors", [])[:2])
                bad = any(k.startswith(("diff .energy", "diff .player", "action failed")) or "intents" in k for k in res["stats"]) or res["stats"].get("residual .draw", 0) >= 5
                if bad and keep.get("f"):  # keep the game's own log for `agent.fidelity_trace`
                    name = re.sub(r"[^A-Za-z0-9_.-]", "_", case["label"]) + ".json"
                    json.dump(dict(label=case["label"], enc=case["enc"], scenario=keep["scenario"], fight=keep["f"]), open(os.path.join(ROOT, "evals", "fidelity_fights", name), "w"))
        except Exception as ex:  # noqa: BLE001
            res["error"] = repr(ex)[:120]
        with open(out_path, "a", encoding="utf-8") as fo:
            fo.write(json.dumps(res) + "\n")
        n += 1
        print(f"{n} {case['label']} {case['enc']} {time.time() - t_start:.0f}s {'ok' if res['ok'] else res.get('error')}", file=log, flush=True)
    print("done", n, "cases")


if __name__ == "__main__":
    main()
