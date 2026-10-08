"""Turn-exhaustive check of a decision: enumerate every distinct line to the end of the current turn, value each end-of-turn
state (terminal utility if the fight ends, else the best searched q at the next player decision, averaged over enemy-turn /
draw determinizations), and report each first-action class's best line. Removes the rollout policy from the current turn.

usage: python turncheck.py spec.py i[,j...] [--k 64] [--dets 6] [--potions]
"""
import argparse, json, math, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, r"C:\Users\steve\sts2\sts2-solver")
import numpy as np  # noqa: E402
from evaluate import classes, decision_states, load_spec, match  # noqa: E402

WIN, HPB = 1.0, 0.5


def free_gb():
    import subprocess
    out = subprocess.run(["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory"],
                         capture_output=True, text=True).stdout
    return int(out.strip() or 0) / 1e6


def terminal(sim, max_hp):
    o = sim.outcome()
    if o == 1:
        return WIN + HPB * min(json.loads(sim.snapshot())["player"]["hp"] / max_hp, 1.0)
    if o != 0:
        return -1.0
    return None


def leaf_value(fs, sc, sim, dets, seed0, max_hp):
    vals = []
    for d in range(dets):
        s = sim.copy()
        s.determinize(seed0 + 7919 * d)
        s.apply('{"end_turn":true}')
        t = terminal(s, max_hp)
        if t is not None:
            vals.append(t)
            continue
        while s.stage() == "choice":
            s.step(s.legal()[0][0])
        r = fs.decide(sc, s, seed=seed0 + d)
        qs = [q for q, ok in zip(r["q"], r["legal"]) if ok and not np.isnan(q)]
        vals.append(max(qs) if qs else float("nan"))
    return float(np.mean(vals)), vals


CAP = 4000


def lines(sim, potions, cap=CAP):
    n, seen, stack = 0, set(), [(sim, [])]
    while stack and n < cap:
        s, path = stack.pop()
        if s.stage() == "over":
            n += 1
            yield s, path, True
            continue
        picks = []
        if s.stage() == "choice":
            for t in reversed(path):
                if not t.startswith("pick"):
                    break
                picks.append(t)
        key = hash((path[0] if path else "", s.snapshot(), tuple(sorted(picks))))
        if key in seen or len(path) > 30:
            continue
        seen.add(key)
        for idx, t in s.legal():
            if t == "end turn":
                n += 1
                yield s, path, False
                continue
            if t.startswith("discard potion") or (t.startswith("potion") and not potions):
                continue
            if t.startswith("pick") and t in picks:
                continue
            c = s.copy()
            try:
                c.step(idx)
            except Exception:  # noqa: BLE001
                continue
            stack.append((c, path + [t]))
    lines.capped = n >= cap


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("spec")
    ap.add_argument("idx")
    ap.add_argument("--k", type=int, default=64)
    ap.add_argument("--dets", type=int, default=6)
    ap.add_argument("--potions", action="store_true")
    ap.add_argument("--revalue", type=int, default=12)
    a = ap.parse_args()
    spec = load_spec(a.spec)
    sc, dec = decision_states(spec)
    from agent.engine import Engine
    eng = Engine(K=a.k)
    max_hp = sc["max_hp"]
    for i in [int(x) for x in a.idx.split(",")]:
        d = dec[i]
        sim = d["sim"]
        cls = classes(sim)
        mine = match(sim, d["action"])
        his_k = cls[mine[0]]
        t0 = time.time()
        if free_gb() < 8:
            print(f"{i}: not checked (resource: {free_gb():.1f} GB free)")
            continue
        n_lines = 0
        best = {}
        cache = {}
        for s_, path, over in lines(sim.copy(), a.potions):
            n_lines += 1
            if n_lines % 50 == 0 and free_gb() < 8:
                print(f"{i}: aborted (resource: {free_gb():.1f} GB free)")
                break
            k0 = ("end turn",) if not path else cls[next(x for x, t in sim.legal() if t == path[0])]
            if over:
                v = terminal(s_, max_hp)
            else:
                key = hash(s_.snapshot())
                if key not in cache:
                    cache[key] = leaf_value(eng.fs, sc, s_, a.dets, 4242, max_hp)[0]
                v = cache[key]
            if k0 not in best or v > best[k0][0]:
                best[k0] = (v, path, s_, over)
        reval = {}
        for k, (v, path, s_, over) in best.items():
            reval[k] = [terminal(s_, max_hp)] * a.revalue if over else leaf_value(eng.fs, sc, s_, a.revalue, 9001, max_hp)[1]
        his_vals = np.array(reval.get(his_k, [np.nan] * a.revalue), float)
        ranked = sorted(best, key=lambda k: -np.mean(reval[k]))
        capped = getattr(lines, "capped", False)
        print(f"{chr(10)}{i} {d['where']}: {n_lines} lines{' (CAP HIT: incomplete, lower bound)' if capped else ''}, {len(cache)} distinct end-of-turn states, "
              f"{time.time() - t0:.0f}s; his class {his_k}{'' if his_k in best else ' NOT ENUMERATED'}")
        out = {}
        for k in ranked:
            vals = np.array(reval[k], float)
            dif = vals - his_vals
            se = float(dif.std(ddof=1) / math.sqrt(len(dif))) if len(dif) > 1 else float("nan")
            out[str(k)] = dict(v=float(vals.mean()), v_se=float(vals.std(ddof=1) / math.sqrt(len(vals))), minus_his=float(dif.mean()), minus_his_se=se,
                               terminal=bool(best[k][3]), line=best[k][1])
            tag = " <- his" if k == his_k else ""
            print(f"  {vals.mean():+.4f} (vs his {dif.mean():+.4f} +- {se:.4f})  {' > '.join(best[k][1]) or 'end turn'}{tag}")
        rec = dict(i=i, where=d["where"], n_lines=n_lines, capped=capped, n_leaves=len(cache), his=str(his_k), potions=a.potions, classes=out)
        with open(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "out", "turncheck.jsonl"), "a") as f:
            f.write(json.dumps(dict(spec=os.path.basename(a.spec), **rec)) + chr(10))


if __name__ == "__main__":
    main()
