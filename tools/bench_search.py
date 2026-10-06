#!/usr/bin/env python3
"""Search-quality benchmark: which search setting picks the best action, judged by a Monte Carlo referee instead of another search.

  STS2_DEVICE=cuda python tools/bench_search.py [--states 150] [--out evals/bench_search.jsonl] [--configs live,w3,wide3,leaf2,leafend] [--strong 20]

States: decisions of recorded fights (`runs/*/fights/*.json`) replayed in the simulator (`agent.fight.Replayer`); only states whose replay is clean
(no errors, no residual divergence at that point) with 3+ distinct legal actions. Fights recorded under a since-fixed simulator bug are excluded.

Referee: every distinct legal action (two copies of the same card on the same target count once; up to 8) is played out to the end of the fight by the
batch solver (`rl/solver.py`, 3 options x 8 futures per decision), from futures that resample the hidden information (draw order, RNG; the visible state
is kept). Common random numbers: repetition k of every action uses the same resampled future and the same continuation seed. Repetitions are added
(32 at a time, up to `--max-reps`) while the top two actions are not yet separated; actions clearly behind are dropped. Value of an action = mean of
(+1 + 0.5 x HP fraction left) for a win, -1 for a loss, the search's own scale. `--strong N` re-referees N states with a stronger continuation
(5 x 32) to check the ranking does not depend on the referee.

Configurations (each picks one action per state): greedy (the policy's top option), live (5x32, 1 s, early stop at 0.6 HP), w3 (5x32, 3 s, no early
stop), wide3 (8x512, 3 s), leaf2 (5x32, 3 s, play-outs 2 turns before the value net), leafend (5x32, 3 s, play-outs to the fight's end).
Score per state and configuration: regret = referee value of the best action - referee value of the chosen action (also in win rate and HP).
"""
import argparse, glob, json, os, random, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402
from agent.fight import Replayer  # noqa: E402
from agent.engine import Engine  # noqa: E402

EXCLUDE = {("20261005-201805", "19_KNOWLEDGE_DEMON_BOSS")}  # recorded while the simulator let the curse be skipped


def kind_of(enc):
    return "boss" if enc.endswith("_BOSS") else "elite" if enc.endswith("_ELITE") else "hallway"


def act_key(text):
    """Distinct actions: `play STRIKE_IRONCLAD #2 -> e0` and `#3 -> e0` are the same move."""
    parts = text.split()
    if parts[0] == "play" and len(parts) >= 3:
        return " ".join([parts[0], parts[1]] + parts[3:])
    return text


def collect_states(max_states, seed):
    rng = random.Random(seed)
    out = []
    for path in sorted(glob.glob(os.path.join(ROOT, "runs", "*", "fights", "*.json"))):
        run, name = path.split(os.sep)[-3], os.path.splitext(os.path.basename(path))[0]
        if (run, name) in EXCLUDE:
            continue
        f = json.load(open(path))
        fight = f["fight"]
        log, states = fight["log"], fight.get("states") or []
        if not states or len(states) < len(log) + 1:
            continue
        try:
            r = Replayer(dict(fight, log=[], states=states[:1], state=states[0]), seed=1)
        except Exception:  # noqa: BLE001
            continue
        for i in range(len(log)):
            view = dict(fight, log=log[:i + 1], states=states[:i + 2], state=states[i + 1])
            before = sum(v for k, v in r.stats.items() if k.startswith("residual"))
            if r.advance(view) is False or r.errors:
                break
            if sum(v for k, v in r.stats.items() if k.startswith("residual")) > before:
                continue  # misaligned here: the referee would score another fight
            sim = r.sim
            if sim.stage() != "play":
                continue
            legal = [(a, t) for a, t in sim.legal() if not t.startswith("discard potion")]
            keys = {}
            for a, t in legal:
                keys.setdefault(act_key(t), (a, t))
            if len(keys) < 3:
                continue
            out.append(dict(file=f"{run}/{name}", step=i + 1, encounter=f["encounter"], kind=kind_of(f["encounter"]), scenario=fight["scenario"],
                            sim=sim.copy(), cands=list(keys.values())[:8]))
    rng.shuffle(out)
    # stratified: at most PER_FIGHT states per fight (decisions of one fight are correlated), then quotas per kind (40 % boss, 33 % elite, 27 % hallway)
    per, kept = {}, []
    for s in out:
        if per.get(s["file"], 0) < PER_FIGHT:
            per[s["file"]] = per.get(s["file"], 0) + 1
            kept.append(s)
    quota = {"boss": round(0.40 * max_states), "elite": round(0.33 * max_states)}
    quota["hallway"] = max_states - quota["boss"] - quota["elite"]
    picked = []
    for k in ("boss", "elite", "hallway"):
        picked += [s for s in kept if s["kind"] == k][:quota[k]]
    rest = [s for s in kept if s not in picked]
    return (picked + rest)[:max_states]


PER_FIGHT = 6


REFEREE_LEAF = 1  # the referee's continuation depth, pinned: rounds 1-2 (evals/bench_search*.jsonl) were refereed at depth 1, whatever the search default is


class Referee:
    def __init__(self, M=3, K=8, leaf=REFEREE_LEAF):
        from solver import Solver
        self.solver = Solver(M=M, K=K)
        self.solver.fs.leaf_turns, self.solver.fs.roll_cap = leaf, 60 * leaf

    def play(self, st, cand_idx, reps, rep0):
        """Values (and wins, HP fractions) of candidates `cand_idx` over repetitions rep0..rep0+reps-1 (common random numbers across candidates)."""
        scen, starts, jobs, done = [], [], [], {}
        for r in range(rep0, rep0 + reps):
            base = st["sim"].copy()
            base.determinize(1_000_003 * (r + 1) + 17)
            for ci in cand_idx:
                c = base.copy()
                c.step(st["cands"][ci][0])
                if c.stage() == "over":
                    snap = json.loads(c.snapshot())
                    hp, mx = snap["player"]["hp"], snap["player"]["max_hp"]
                    done[(ci, r)] = (1.0 + 0.5 * hp / mx, 1.0, hp / mx) if hp > 0 else (-1.0, 0.0, 0.0)
                    continue
                jobs.append((ci, r))
                scen.append(st["scenario"])
                starts.append(c)
        if jobs:
            fs = self.solver.fs
            seeds = np.array([7_919 * (r + 1) for _, r in jobs], np.uint64)
            res = fs.run(scen, np.arange(len(jobs), dtype=np.uint32), seeds, starts=starts)
            for (ci, r), row in zip(jobs, res):
                oc, hp_end = int(row[1]), float(row[3])
                done[(ci, r)] = (1.0 + 0.5 * hp_end, 1.0, hp_end) if oc == 1 else (-1.0, 0.0, 0.0)
        return done


def referee_state(ref, st, max_reps, step=32):
    n = len(st["cands"])
    vals = {ci: [] for ci in range(n)}
    alive = list(range(n))
    rep = 0
    while rep < max_reps:
        got = ref.play(st, alive, step, rep)
        for (ci, r), v in got.items():
            vals[ci].append(v)
        rep += step
        means = {ci: np.mean([v[0] for v in vals[ci]]) for ci in alive}
        ses = {ci: np.std([v[0] for v in vals[ci]], ddof=1) / len(vals[ci]) ** 0.5 for ci in alive}
        order = sorted(alive, key=lambda c: -means[c])
        best = order[0]
        alive = [c for c in alive if means[c] + 2 * ses[c] >= means[best] - 2 * ses[best]]  # drop the ones clearly behind
        if len(alive) <= 1:
            break
        second = sorted(alive, key=lambda c: -means[c])[1]
        if means[best] - means[second] > 2 * (ses[best] ** 2 + ses[second] ** 2) ** 0.5:
            break
    out = {}
    for ci in range(n):
        v = np.array(vals[ci]) if vals[ci] else np.zeros((0, 3))
        out[ci] = dict(v=float(v[:, 0].mean()), se=float(v[:, 0].std(ddof=1) / len(v) ** 0.5) if len(v) > 1 else None,
                       win=float(v[:, 1].mean()), hp=float(v[:, 2].mean()), n=len(v))
    return out


CONFIGS = {
    "greedy": dict(M=5, K=32, budget=0.05, tol=1.0, leaf=1, cap=60, greedy=True),
    "live": dict(M=5, K=32, budget=1.0, tol=0.6, leaf=1, cap=60),
    "w3": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=1, cap=60),
    "wide3": dict(M=8, K=512, budget=3.0, tol=0.0, leaf=1, cap=60),
    "leaf2": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=2, cap=120),
    "leafend": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=10_000, cap=400),
    # round 2: depth at the live budget, depth per fight kind, depth plus width
    "leaf2_1s": dict(M=5, K=32, budget=1.0, tol=0.0, leaf=2, cap=120),
    "leaf2_live": dict(M=5, K=32, budget=1.0, tol=0.6, leaf=2, cap=120),
    "leaf3_3s": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=3, cap=180),
    "mixed_3s": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=2, cap=120, boss_leaf=10_000, boss_cap=400),
    "mixed_1s": dict(M=5, K=32, budget=1.0, tol=0.0, leaf=2, cap=120, boss_leaf=10_000, boss_cap=400),
    "leaf2_wide1s": dict(M=8, K=128, budget=1.0, tol=0.0, leaf=2, cap=120),
}


def choose(eng, st, cfg):
    boss = st["kind"] == "boss" and "boss_leaf" in cfg
    eng.fs.leaf_turns, eng.fs.roll_cap = (cfg["boss_leaf"], cfg["boss_cap"]) if boss else (cfg["leaf"], cfg["cap"])
    d = eng.decide(st["scenario"], st["sim"], budget=cfg["budget"], tol_hp=cfg["tol"])
    if cfg.get("greedy"):
        opts = [o for o in d["options"]]
        top = max(opts, key=lambda o: o["p"]) if opts else None
        text = top["text"] if top else d["text"]
    else:
        text = d["text"]
    return text


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--states", type=int, default=150)
    ap.add_argument("--max-reps", type=int, default=256)
    ap.add_argument("--configs", default="greedy,live,w3,wide3,leaf2,leafend")
    ap.add_argument("--strong", type=int, default=20)
    ap.add_argument("--seed", type=int, default=3)
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "bench_search.jsonl"))
    ap.add_argument("--reuse", help="an earlier output: keep its referee values, add the picks of --configs (same states: collection is deterministic)")
    a = ap.parse_args()
    t0 = time.time()
    states = collect_states(a.states, a.seed)
    if a.reuse:
        return add_picks(a, states, t0)
    print(f"{len(states)} states ({sum(s['kind'] == 'boss' for s in states)} boss, {sum(s['kind'] == 'elite' for s in states)} elite) in {time.time() - t0:.0f}s", flush=True)
    names = a.configs.split(",")
    engines = {}
    for n in names:  # one engine per (M, K): the networks are loaded once per shape
        c = CONFIGS[n]
        if (c["M"], c["K"]) not in engines:
            engines[(c["M"], c["K"])] = Engine(M=c["M"], K=c["K"])
    picks = []
    for i, st in enumerate(states):
        row = {}
        for n in names:
            c = CONFIGS[n]
            row[n] = act_key(choose(engines[(c["M"], c["K"])], st, c))
        picks.append(row)
        if i % 10 == 0:
            print(f"  picks {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    del engines
    ref = Referee()
    strong = Referee(M=5, K=32) if a.strong else None
    with open(a.out, "w") as fo:
        for i, (st, row) in enumerate(zip(states, picks)):
            res = referee_state(ref, st, a.max_reps)
            keys = [act_key(t) for _, t in st["cands"]]
            rec = dict(file=st["file"], step=st["step"], encounter=st["encounter"], kind=st["kind"], cands=keys, ref={keys[ci]: r for ci, r in res.items()},
                       picks=row, potion=any(k.startswith("potion") for k in keys), referee_leaf=REFEREE_LEAF)
            if strong and i < a.strong:
                rs = referee_state(strong, st, min(a.max_reps, 128))
                rec["ref_strong"] = {keys[ci]: r for ci, r in rs.items()}
            fo.write(json.dumps(rec) + "\n")
            fo.flush()
            if i % 5 == 0:
                print(f"  referee {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    report(a.out, names)


def add_picks(a, states, t0):
    """The picks of new configurations on the states an earlier run refereed (matched by fight file and step)."""
    old = {(r["file"], r["step"]): r for r in (json.loads(l) for l in open(a.reuse))}
    names = a.configs.split(",")
    engines = {}
    for n in names:
        c = CONFIGS[n]
        if (c["M"], c["K"]) not in engines:
            engines[(c["M"], c["K"])] = Engine(M=c["M"], K=c["K"])
    out, missing = [], 0
    for i, st in enumerate(states):
        rec = old.get((st["file"], st["step"]))
        if rec is None:
            missing += 1
            continue
        for n in names:
            c = CONFIGS[n]
            rec["picks"][n] = act_key(choose(engines[(c["M"], c["K"])], st, c))
        out.append(rec)
        if i % 10 == 0:
            print(f"  picks {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    with open(a.out, "w") as fo:
        for rec in out:
            fo.write(json.dumps(rec) + "\n")
    print(f"{len(out)} states matched, {missing} not in {a.reuse}")
    report(a.out, list(dict.fromkeys(list(out[0]["picks"]) if out else names)))


def report(path, names):
    recs = [json.loads(l) for l in open(path)]
    def regret(rec, n, field="v"):
        best = max(r[field] for r in rec["ref"].values() if r["n"])
        pick = rec["ref"].get(rec["picks"][n])
        return None if pick is None or not pick["n"] else best - pick[field]
    def contested(rec):
        vs = [(r["v"], r["se"] or 0) for r in rec["ref"].values() if r["n"] > 1]
        if len(vs) < 2:
            return False
        vs.sort(key=lambda x: -x[0])
        return vs[0][0] - vs[-1][0] > 2 * (vs[0][1] ** 2 + vs[-1][1] ** 2) ** 0.5
    rng = np.random.default_rng(0)
    subsets = {"all": recs, "contested": [r for r in recs if contested(r)], "boss": [r for r in recs if r["kind"] == "boss"],
               "elite": [r for r in recs if r["kind"] == "elite"], "hallway": [r for r in recs if r["kind"] == "hallway"],
               "potion states": [r for r in recs if r["potion"]]}
    print(f"\nregret vs the Monte Carlo referee (value = +1 + 0.5 HP fraction / -1; 0.1 ~ 5% win or ~16 HP at 80 max HP); n states per subset")
    print(f"{'config':10s} " + " ".join(f"{k:>22s}" for k in subsets))
    for n in names:
        cells = []
        for k, rs in subsets.items():
            x = np.array([v for v in (regret(r, n) for r in rs) if v is not None])
            if len(x) == 0:
                cells.append(f"{'-':>22s}")
                continue
            boot = [rng.choice(x, len(x)).mean() for _ in range(500)]
            agree = np.mean([regret(r, n) == 0 for r in rs if regret(r, n) is not None])
            cells.append(f"{x.mean():.4f}+-{np.std(boot):.4f} {agree:.0%} n{len(x)}".rjust(22))
        print(f"{n:10s} " + " ".join(cells))
    rs = [r for r in recs if "ref_strong" in r]
    if rs:
        same = np.mean([max(r["ref"], key=lambda k: r["ref"][k]["v"]) == max(r["ref_strong"], key=lambda k: r["ref_strong"][k]["v"]) for r in rs])
        print(f"referee check: the strong referee picks the same best action in {same:.0%} of {len(rs)} states")


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--report":
        report(sys.argv[2], sys.argv[3].split(",") if len(sys.argv) > 3 else list(CONFIGS))
    else:
        main()
