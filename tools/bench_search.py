#!/usr/bin/env python3
import argparse, glob, json, os, random, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402

EXCLUDE = {("20261005-201805", "19_KNOWLEDGE_DEMON_BOSS")}


def kind_of(enc):
    return "boss" if enc.endswith("_BOSS") else "elite" if enc.endswith("_ELITE") else "hallway"


def act_key(text):
    parts = text.split()
    if parts[0] == "play" and len(parts) >= 3:
        return " ".join([parts[0], parts[1]] + parts[3:])
    return text


def distinct_actions(sim):
    keys = {}
    for a, t in sim.legal():
        if not t.startswith("discard potion"):
            keys.setdefault(act_key(t), (a, t))
    return keys


def collect_states(max_states, seed, max_cands=8):
    from agent.fight import Replayer
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
                continue
            sim = r.sim
            if sim.stage() != "play":
                continue
            keys = distinct_actions(sim)
            if len(keys) < 3:
                continue
            out.append(dict(file=f"{run}/{name}", step=i + 1, encounter=f["encounter"], kind=kind_of(f["encounter"]), scenario=fight["scenario"],
                            sim=sim.copy(), cands=list(keys.values())[:max_cands], keys=keys))
    rng.shuffle(out)
    return stratify(out, max_states)


def collect_states_scen(path, max_states, seed, max_cands=8, n_fights=None, M=3, K=8):
    from solver import Solver
    data = json.load(open(path))
    scens = [d["scenario"] if isinstance(d, dict) and "scenario" in d else d for d in data]
    rng = random.Random(seed)
    nf = min(len(scens), n_fights or max(1, -(-2 * max_states // PER_FIGHT)))
    pick = sorted(rng.sample(range(len(scens)), nf))
    S = Solver(M=M, K=K)
    S.fs.record = True
    seeds = np.array([1_000_003 * seed + i for i in pick], np.uint64)
    t0 = time.time()
    res = S.fs.run([scens[i] for i in pick], np.arange(nf, dtype=np.uint32), seeds)
    print(f"{nf} fights of {os.path.basename(path)} played ({M}x{K}, win {np.mean(res[:, 1] == 1):.2f}) in {time.time() - t0:.0f}s", flush=True)
    out, diverged = [], 0
    for j, i in enumerate(pick):
        sc, sd = scens[i], int(seeds[j])
        acts = S.fs.job_actions(j)
        sj = json.dumps(sc)
        _, masks = sts2.replay(sc, sd, np.asarray(acts, np.int32))
        sim = sts2.Sim(sj, sd)
        enc = sc.get("encounter", "?")
        for t, a in enumerate(acts):
            if sorted(x for x, _ in sim.legal()) != np.flatnonzero(masks[t]).tolist():
                diverged += 1
                break
            if sim.stage() == "play":
                keys = distinct_actions(sim)
                if len(keys) >= 3:
                    out.append(dict(file=f"{os.path.basename(path)}#{i}", step=t, encounter=enc, kind=kind_of(enc), scenario=sc, sim=sim.copy(),
                                    cands=list(keys.values())[:max_cands], keys=keys))
            if not sim.step(int(a)):
                diverged += 1
                break
    if diverged:
        print(f"  {diverged} of {nf} fights diverged on replay (cut there)", flush=True)
    del S
    rng.shuffle(out)
    return stratify(out, max_states)


def stratify(out, max_states):
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


REFEREE_LEAF = 1


class Referee:
    def __init__(self, M=3, K=8, leaf=REFEREE_LEAF):
        from solver import Solver
        self.solver = Solver(M=M, K=K)
        self.solver.fs.leaf_turns, self.solver.fs.roll_cap = leaf, 60 * leaf

    def play(self, st, cand_idx, reps, rep0):
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
        alive = [c for c in alive if means[c] + 2 * ses[c] >= means[best] - 2 * ses[best]]
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


DEPTH2 = dict(leaf=2, cap=120)
CONFIGS = {
    "greedy": dict(M=5, K=32, budget=0.05, tol=1.0, leaf=1, cap=60, greedy=True),
    "live": dict(M=5, K=32, budget=1.0, tol=0.6, leaf=1, cap=60),
    "w3": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=1, cap=60),
    "wide3": dict(M=8, K=512, budget=3.0, tol=0.0, leaf=1, cap=60),
    "leaf2": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=2, cap=120),
    "leafend": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=10_000, cap=400),
    "leaf2_1s": dict(M=5, K=32, budget=1.0, tol=0.0, leaf=2, cap=120),
    "leaf2_live": dict(M=5, K=32, budget=1.0, tol=0.6, leaf=2, cap=120),
    "leaf3_3s": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=3, cap=180),
    "mixed_3s": dict(M=5, K=32, budget=3.0, tol=0.0, leaf=2, cap=120, boss_leaf=10_000, boss_cap=400),
    "mixed_1s": dict(M=5, K=32, budget=1.0, tol=0.0, leaf=2, cap=120, boss_leaf=10_000, boss_cap=400),
    "leaf2_wide1s": dict(M=8, K=128, budget=1.0, tol=0.0, leaf=2, cap=120),
    "topm5x32": dict(M=5, K=32, budget=0.0, tol=0.0, rounds=1, **DEPTH2),
    "topm5x32x4": dict(M=5, K=32, budget=0.0, tol=0.0, rounds=4, **DEPTH2),
    "cover32": dict(M=16, K=32, cover=True, budget=0.0, tol=0.0, rounds=1, **DEPTH2),
    "cover_t160": dict(M=16, K=160, cover=True, futures=160, budget=0.0, tol=0.0, rounds=1, **DEPTH2),
}


def engine_key(c):
    return c["M"], c["K"], c.get("cover", False), c.get("futures", 0)


def choose(eng, st, cfg):
    boss = st["kind"] == "boss" and "boss_leaf" in cfg
    eng.fs.leaf_turns, eng.fs.roll_cap = (cfg["boss_leaf"], cfg["boss_cap"]) if boss else (cfg["leaf"], cfg["cap"])
    d = eng.decide(st["scenario"], st["sim"], budget=cfg["budget"], tol_hp=cfg["tol"], rounds=cfg.get("rounds"))
    if cfg.get("greedy"):
        opts = [o for o in d["options"]]
        top = max(opts, key=lambda o: o["p"]) if opts else None
        text = top["text"] if top else d["text"]
    else:
        text = d["text"]
    return text, d["seconds"], list(d.get("rows", (0, 0))), [act_key(o["text"]) for o in d["options"]]


def prior_of(net, st):
    import torch
    from model import DEV
    v = getattr(net, "obs_version", 1)
    obs, mask = np.zeros(sts2.obs_size(v), np.float32), np.zeros(sts2.ACTIONS, np.uint8)
    sim = st["sim"].copy()
    sim.observe(obs, mask, version=v)
    with torch.no_grad():
        lg = net(torch.from_numpy(obs).unsqueeze(0).to(DEV), torch.from_numpy(mask).unsqueeze(0).to(DEV), value=False)[0].float()
        p = torch.softmax(lg, 1)[0].cpu().numpy()
    out = {}
    for a, t in st["sim"].legal():
        if not t.startswith("discard potion"):
            k = act_key(t)
            out[k] = out.get(k, 0.0) + float(p[a])
    return out


def make_engines(names):
    from agent.engine import Engine
    engines = {}
    for n in names:
        k = engine_key(CONFIGS[n])
        if k not in engines:
            engines[k] = Engine(M=k[0], K=k[1], cover=k[2], futures=k[3])
    return engines


def add_cands(st, keys):
    have = {act_key(t) for _, t in st["cands"]}
    for k in keys:
        if k not in have and k in st["keys"]:
            st["cands"].append(st["keys"][k])
            have.add(k)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--states", type=int, default=150)
    ap.add_argument("--max-reps", type=int, default=256)
    ap.add_argument("--max-cands", type=int, default=8, help="distinct actions the referee plays out (first ones in legal order), plus every picked action")
    ap.add_argument("--configs", default="greedy,live,w3,wide3,leaf2,leafend")
    ap.add_argument("--strong", type=int, default=20)
    ap.add_argument("--seed", type=int, default=3)
    ap.add_argument("--from-scenarios", help="states from search-played fights of this scenario file instead of the recorded real runs")
    ap.add_argument("--fights", type=int, help="with --from-scenarios: fights played (default 2 x states / 6)")
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "bench_search.jsonl"))
    ap.add_argument("--reuse", help="an earlier output: keep its referee values, add the picks of --configs (same states: collection is deterministic)")
    a = ap.parse_args()
    t0 = time.time()
    if a.from_scenarios:
        states = collect_states_scen(a.from_scenarios, a.states, a.seed, a.max_cands, a.fights)
    else:
        states = collect_states(a.states, a.seed, a.max_cands)
    if a.reuse:
        return add_picks(a, states, t0)
    print(f"{len(states)} states ({sum(s['kind'] == 'boss' for s in states)} boss, {sum(s['kind'] == 'elite' for s in states)} elite) in {time.time() - t0:.0f}s", flush=True)
    names = a.configs.split(",")
    engines = make_engines(names)
    net = next(iter(engines.values())).fs.net
    picks, extra = [], []
    for i, st in enumerate(states):
        row, secs, rows, searched = {}, {}, {}, {}
        for n in names:
            c = CONFIGS[n]
            text, secs[n], rows[n], searched[n] = choose(engines[engine_key(c)], st, c)
            row[n] = act_key(text)
        add_cands(st, row.values())
        picks.append(row)
        extra.append(dict(secs=secs, rows=rows, searched=searched, prior=prior_of(net, st)))
        if i % 10 == 0:
            print(f"  picks {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    del engines, net
    ref = Referee()
    strong = Referee(M=5, K=32) if a.strong else None
    with open(a.out, "w") as fo:
        for i, (st, row, ex) in enumerate(zip(states, picks, extra)):
            res = referee_state(ref, st, a.max_reps)
            keys = [act_key(t) for _, t in st["cands"]]
            rec = dict(file=st["file"], step=st["step"], encounter=st["encounter"], kind=st["kind"], cands=keys, ref={keys[ci]: r for ci, r in res.items()},
                       picks=row, potion=any(k.startswith("potion") for k in keys), referee_leaf=REFEREE_LEAF, n_legal=len(st["keys"]), **ex)
            if strong and i < a.strong:
                rs = referee_state(strong, st, min(a.max_reps, 128))
                rec["ref_strong"] = {keys[ci]: r for ci, r in rs.items()}
            fo.write(json.dumps(rec) + "\n")
            fo.flush()
            if i % 5 == 0:
                print(f"  referee {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    report(a.out, names)


def add_picks(a, states, t0):
    old = {(r["file"], r["step"]): r for r in (json.loads(l) for l in open(a.reuse))}
    names = a.configs.split(",")
    engines = make_engines(names)
    net = next(iter(engines.values())).fs.net
    out, missing = [], 0
    for i, st in enumerate(states):
        rec = old.get((st["file"], st["step"]))
        if rec is None:
            missing += 1
            continue
        for n in names:
            c = CONFIGS[n]
            text, s, rw, sr = choose(engines[engine_key(c)], st, c)
            rec["picks"][n] = act_key(text)
            rec.setdefault("secs", {})[n] = s
            rec.setdefault("rows", {})[n] = rw
            rec.setdefault("searched", {})[n] = sr
        rec.setdefault("prior", prior_of(net, st))
        out.append(rec)
        if i % 10 == 0:
            print(f"  picks {i + 1}/{len(states)} ({time.time() - t0:.0f}s)", flush=True)
    with open(a.out, "w") as fo:
        for rec in out:
            fo.write(json.dumps(rec) + "\n")
    print(f"{len(out)} states matched, {missing} not in {a.reuse}")
    report(a.out, list(dict.fromkeys(list(out[0]["picks"]) if out else names)))


def regret(rec, n, field="v"):
    best = max(r[field] for r in rec["ref"].values() if r["n"])
    pick = rec["ref"].get(rec["picks"].get(n))
    return None if pick is None or not pick["n"] else best - pick[field]


def prior_rank(rec):
    pr = rec.get("prior")
    if not pr:
        return None
    best = max((k for k, r in rec["ref"].items() if r["n"]), key=lambda k: rec["ref"][k]["v"])
    p = pr.get(best)
    return None if p is None else 1 + sum(1 for x in pr.values() if x > p)


def report(path, names):
    recs = [json.loads(l) for l in open(path)]

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
    print("\nregret vs the Monte Carlo referee (value = +1 + 0.5 HP fraction / -1; 0.1 ~ 5% win or ~16 HP at 80 max HP); n states per subset")
    print(f"{'config':12s} " + " ".join(f"{k:>22s}" for k in subsets))
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
        print(f"{n:12s} " + " ".join(cells))
    cost = [(n, [r["secs"][n] for r in recs if n in r.get("secs", {})], [r["rows"][n] for r in recs if n in r.get("rows", {})]) for n in names]
    if any(s for _, s, _ in cost):
        print("\ncost per decision: seconds, policy rows, value rows (means)")
        for n, s, rw in cost:
            if s:
                rw = np.array(rw, float).reshape(-1, 2) if rw else np.zeros((1, 2))
                print(f"  {n:12s} {np.mean(s):6.2f} s  {rw[:, 0].mean():9.0f}  {rw[:, 1].mean():9.0f}")
    rk = [(r, prior_rank(r)) for r in recs]
    rk = [(r, x) for r, x in rk if x is not None]
    if rk:
        print("\nrank of the referee's best action in the policy prior (duplicates summed): rank 1 / top 5 / top 8 / beyond 8")
        for label, sub in (("all", rk), ("contested", [(r, x) for r, x in rk if contested(r)])):
            x = np.array([v for _, v in sub])
            if len(x):
                print(f"  {label:10s} {np.mean(x == 1):.0%} / {np.mean(x <= 5):.0%} / {np.mean(x <= 8):.0%} / {np.mean(x > 8):.0%}   (n {len(x)}, mean distinct legal "
                      f"{np.mean([r.get('n_legal', len(r['prior'])) for r, _ in sub]):.1f})")
        full = np.mean([len(r["cands"]) >= r.get("n_legal", len(r["prior"])) for r, _ in rk])
        print(f"  the referee played out every distinct legal action in {full:.0%} of these states (raise --max-cands if low)")
        best = lambda r: max((k for k, x in r["ref"].items() if x["n"]), key=lambda k: r["ref"][k]["v"])  # noqa: E731
        print("share of states whose referee-best action the config searched: all / referee best outside the prior's top 5")
        for n in names:
            sub = [(r, x) for r, x in rk if n in r.get("searched", {})]
            if sub:
                hit = np.array([best(r) in r["searched"][n] for r, _ in sub])
                out5 = np.array([x > 5 for _, x in sub])
                print(f"  {n:12s} {hit.mean():.0%} / {hit[out5].mean() if out5.any() else float('nan'):.0%} (n {len(sub)}, {out5.sum()} outside top 5)")
    rs = [r for r in recs if "ref_strong" in r]
    if rs:
        same = np.mean([max(r["ref"], key=lambda k: r["ref"][k]["v"]) == max(r["ref_strong"], key=lambda k: r["ref_strong"][k]["v"]) for r in rs])
        print(f"referee check: the strong referee picks the same best action in {same:.0%} of {len(rs)} states")


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--report":
        report(sys.argv[2], sys.argv[3].split(",") if len(sys.argv) > 3 else list(CONFIGS))
    else:
        main()
