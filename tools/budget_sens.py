#!/usr/bin/env python3
"""Search-budget sensitivity: play bench fights with the live Engine at --play rounds per decision; at each searched decision record the
best action after 4/16/64 rounds and score each against --eval independent rounds (disjoint seeds): regret(R) = q_eval[best_eval] - q_eval[a_R].
Features at 16 rounds: top-2 gap, root value, legal count, turn, player HP fraction, incoming damage. Output jsonl, then --report."""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path[:0] = [os.path.join(ROOT, "rl"), ROOT]
CHECK = (4, 16, 64)


def rounds_q(eng, scenario, sim, n, seed):
    acc, first = {}, None
    for i in range(n):
        r = eng.fs.decide(scenario, sim, seed + i)
        first = first or r
        if not r["searched"]:
            return first, None
        for a, q, ok in zip(r["opts"], r["q"], r["legal"]):
            if ok and not np.isnan(q):
                acc.setdefault(a, []).append(float(q))
    return first, acc


def best_after(acc, n):
    m = {a: np.mean(v[:n]) for a, v in acc.items() if len(v) >= n}
    return max(m, key=m.get), m


def run(a):
    import sts2
    from agent.engine import Engine
    eng = Engine()
    bench = json.load(open(os.path.join(ROOT, "data", "bench", a.bench + ".json")))
    rng = np.random.default_rng(a.seed)
    idx = rng.choice(len(bench), size=min(a.fights, len(bench)), replace=False)
    out = open(a.out, "a", encoding="utf-8")
    t0 = time.time()
    for fi, bi in enumerate(idx):
        sc = bench[bi]["scenario"]
        sim = sts2.Sim(json.dumps(sc), int(a.seed * 1000 + bi))
        step = 0
        while sim.outcome() == 0 and sim.stage() != "over" and step < 400:
            base = 10_000_000 * (fi + 1) + 1000 * step
            first, sel = rounds_q(eng, sc, sim, max(CHECK), base)
            if sel is None or len(sel) < 2:
                sim.step(first["action"])
                step += 1
                continue
            _, ev = rounds_q(eng, sc, sim, a.eval, base + 500)
            qe = {k: np.mean(v) for k, v in ev.items()}
            be = max(qe, key=qe.get)
            row = dict(bench=a.bench, i=int(bi), step=step, legal=len(sel))
            for n in CHECK:
                b, m = best_after(sel, n)
                row[f"a{n}"], row[f"reg{n}"] = int(b), float(qe[be] - qe.get(b, np.nan))
                if n == 16:
                    top = sorted(m.values(), reverse=True)
                    row.update(gap16=float(top[0] - top[1]), v16=float(top[0]))
            snap = json.loads(sim.snapshot())
            p = snap["player"]
            row.update(hp_frac=p["hp"] / max(1, p["max_hp"]), turn=snap.get("turn"), a_eval=int(be))
            out.write(json.dumps(row) + "\n")
            out.flush()
            sim.step(row[f"a{a.play}"])
            step += 1
        print(f"fight {fi + 1}/{len(idx)} bench #{bi}: outcome {sim.outcome()}, {step} steps, {time.time() - t0:.0f}s", flush=True)


def report(a):
    rows = [json.loads(l) for l in open(a.out, encoding="utf-8")]
    n = len(rows)
    print(f"{n} searched decisions with >= 2 legal options")
    for k in CHECK:
        r = np.array([x[f"reg{k}"] for x in rows])
        ch = np.mean([x[f"a{k}"] != x["a64"] for x in rows])
        print(f"R {k:3d}: regret mean {r.mean():.4f} +- {r.std() / np.sqrt(n):.4f}; differs from R64 {ch:.3f}; regret > 0.02 in {np.mean(r > 0.02):.3f}")
    d = np.array([x["reg16"] - x["reg64"] for x in rows])
    print(f"R16 - R64 regret {d.mean():.4f} +- {d.std() / np.sqrt(n):.4f}")
    gap = np.array([x["gap16"] for x in rows])
    v = np.array([x["v16"] for x in rows])
    for name, key, edges in (("gap16", gap, (0, 0.005, 0.02, 0.05, 9)), ("v16", v, (-9, -0.5, 0, 0.5, 1.0, 9))):
        for lo, hi in zip(edges, edges[1:]):
            m = (key >= lo) & (key < hi)
            if m.sum():
                print(f"  {name} [{lo:g},{hi:g}): n {m.sum():4d} ({m.mean():.2f}); R16-R64 regret {d[m].mean():.4f} +- {d[m].std() / np.sqrt(m.sum()):.4f}; "
                      f"share of total gain {d[m].sum() / max(1e-9, d.sum()):.2f}")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--bench", default="mix")
    ap.add_argument("--fights", type=int, default=30)
    ap.add_argument("--eval", type=int, default=64)
    ap.add_argument("--play", type=int, default=16, choices=CHECK)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "budget_sens.jsonl"))
    ap.add_argument("--report", action="store_true")
    a = ap.parse_args()
    report(a) if a.report else run(a)
