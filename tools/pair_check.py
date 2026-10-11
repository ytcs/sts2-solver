#!/usr/bin/env python3
"""Plan stage C gate: the first final boss played from each recorded start under the current objective vs a previous engine.py
(`--old`, e.g. `git show <rev>:agent/engine.py`), paired seeds; scored by P(win the pair) = win x P(win the second boss | end HP,
potions left) by search.
usage: tools/pair_check.py <events dir> <arm rows .jsonl> [more dir rows ...] --old engine_old.py [--seeds 6] [--rounds 16] [--out ...]"""
import argparse, glob, importlib.util, json, os, sys, types

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl")]


def starts(pairs):
    out = []
    for ev_dir, rows_path in pairs:
        rows = {json.loads(l)["seed"]: json.loads(l) for l in open(rows_path)}
        for f in sorted(glob.glob(os.path.join(ev_dir, "*", "*", "events.jsonl"))):
            seed = os.path.basename(os.path.dirname(f))
            pair = list(((rows.get(seed) or {}).get("deck_strength") or {}).get("final deck") or {})
            e = next((json.loads(l) for l in open(f, encoding="utf-8") if '"fight_start"' in l and '_BOSS"' in l and '"act": 2' in l), None)
            if e and len(pair) == 2 and e["encounter"] in pair:
                out.append((seed, e["scenario"], next(b for b in pair if b != e["encounter"])))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pairs", nargs="+")
    ap.add_argument("--old", required=True)
    ap.add_argument("--seeds", type=int, default=6)
    ap.add_argument("--rounds", type=int, default=16)
    ap.add_argument("--out", default="target/review/pair_check.jsonl")
    a = ap.parse_args()
    import numpy as np
    import sts2
    from agent import proposal
    from agent.engine import Engine
    spec = importlib.util.spec_from_file_location("engine_old", a.old)
    old = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(old)
    eng = Engine()
    new_decide, old_decide = eng.decide, types.MethodType(old.Engine.decide, eng)
    solve = lambda s, attempts: eng.solve(s, attempts=attempts)  # noqa: E731
    cache = {}

    def p2(sc, second, hp, pots):
        key = (id(sc), hp, tuple(sorted((p["id"], p["slot"]) for p in pots)))
        if key not in cache:
            cache[key] = eng.solve([dict(sc, encounter=second, hp=hp, potions=pots)], attempts=64, seed=104729)[0]["win"] if hp > 0 else 0.0
        return cache[key]

    def play(sc, seed, decide, worth, pot_cost):
        sim = sts2.Sim(json.dumps(sc), seed)
        steps = 0
        while sim.outcome() == 0 and steps < 400 and sim.stage() != "over":
            kw = dict(pot_cost=pot_cost) if pot_cost is not None else {}
            d = decide(sc, sim, 0.3, keep_potions=set(), worth=worth, rounds=None if sim.stage() == "choice" else a.rounds, **kw)
            sim.step(d["action"])
            steps += 1
        snap = json.loads(sim.snapshot())
        pots = [dict(id=p["id"], slot=p["slot"]) for p in snap.get("potions") or []]
        return sim.outcome() == 1, max(int(snap["player"]["hp"]), 0), pots

    S = starts(list(zip(a.pairs[::2], a.pairs[1::2])))
    print(f"{len(S)} recorded first-of-pair starts, {a.seeds} seeds per arm", flush=True)
    res = {"old": [], "new": []}
    with open(a.out, "w") as fh:
        for seed_name, sc, second in S:
            pw = proposal.pair_worth(sc, second, solve)
            ok = bool(max(pw["curve"]) >= proposal.PAIR_INFORMATIVE)
            arms = {"old": (old_decide, {k: v for k, v in pw.items() if k not in ("pot", "pot_gain")} if ok else None, None),
                    "new": (new_decide, pw if ok else None, None if ok else 2.0 * max(pw["pot_gain"].values(), default=0.0))}
            row = dict(seed=seed_name, enc=sc["encounter"], second=second, hp=sc["hp"], potions=[p["id"] for p in sc["potions"]], informative=ok)
            for arm, (dec, worth, pc) in arms.items():
                out = []
                for s in range(a.seeds):
                    won, hp, pots = play(sc, 7919 * (s + 1), dec, worth, pc)
                    out.append(dict(won=bool(won), hp=hp, pots=len(pots), pair=float(won * p2(sc, second, hp, pots))))
                row[arm] = out
                res[arm].append([o["pair"] for o in out])
            fh.write(json.dumps(row) + "\n")
            fh.flush()
            print(seed_name, sc["encounter"], {k: (round(float(np.mean([o["won"] for o in row[k]])), 2), round(float(np.mean([o["pots"] for o in row[k]])), 2),
                                                   round(float(np.mean([o["pair"] for o in row[k]])), 3)) for k in ("old", "new")}, flush=True)
    d = np.array(res["new"]).mean(1) - np.array(res["old"]).mean(1)
    print(f"P(win the pair): old {np.mean(res['old']):.3f}, new {np.mean(res['new']):.3f}, paired diff {d.mean():+.3f} +- {d.std(ddof=1) / np.sqrt(len(d)):.3f} (n {len(d)})")


if __name__ == "__main__":
    main()
