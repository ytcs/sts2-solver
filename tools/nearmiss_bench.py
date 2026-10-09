#!/usr/bin/env python3
import argparse, glob, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402


def build(a):
    rng = np.random.default_rng(a.seed)
    near = json.load(open(a.near))["near"]
    loss = [dict(kind="near_loss", part=r["part"], fight=r["fight"]) for r in near]
    files = sorted(f for p in a.parts for f in glob.glob(p))
    close = []
    for fn in files:
        z = np.load(fn, allow_pickle=True)
        scen = json.loads(str(z["scenarios"]))
        for i in np.nonzero(z["f_cls"] > 0)[0]:
            si = int(z["f_scen"][i])
            s = sts2.Sim(json.dumps(scen[si]), int(z["f_seed"][i]))
            for x in z["acts"][z["f_off"][i]:z["f_off"][i + 1]]:
                s.step(int(x))
            p = json.loads(s.snapshot())["player"]
            if s.outcome() == 1 and p["hp"] <= a.close * p["max_hp"]:
                close.append(dict(kind="close_win", part=fn, fight=int(i)))
    pick = lambda xs: [xs[i] for i in sorted(rng.choice(len(xs), min(a.n, len(xs)), replace=False))]  # noqa: E731
    rows = []
    cache = {}
    for r in pick(loss) + pick(close):
        if r["part"] not in cache:
            z = np.load(r["part"], allow_pickle=True)
            cache = {r["part"]: (z, json.loads(str(z["scenarios"])))}
        z, scen = cache[r["part"]]
        rows.append(dict(kind=r["kind"], scenario=scen[int(z["f_scen"][r["fight"]])], seed=int(z["f_seed"][r["fight"]]), source=os.path.basename(r["part"])))
    json.dump(rows, open(a.out, "w"))
    print(f"{sum(r['kind'] == 'near_loss' for r in rows)} near-miss losses (of {len(loss)}), {sum(r['kind'] == 'close_win' for r in rows)} close wins "
          f"(of {len(close)}) -> {a.out}")


def arm(spec, roots):
    from fastsearch import FastSearch
    from model import load
    parts = spec.split("@")
    pr, _, roll = parts[0].partition("+")
    M, K, cv, cover, futures, exact, cap = 5, 32, False, False, 0, None, False
    for t in parts[1:]:
        if t == "cv":
            cv = True
        elif t == "x":
            exact = True
        elif t == "cover":
            cover = True
        elif t == "cap":
            cap = True
        elif t.startswith("t"):
            futures = K = int(t[1:])
        else:
            M, K = (int(x) for x in t.split("x"))
    fs = FastSearch(load(pr), M=M, K=K, roots=roots, amp=True, roll_net=load(roll) if roll else None, clairvoyant=cv, cover=cover, futures=futures,
                    exact_turn=exact, hp_cap=cap)
    fs.warm()
    return fs


def spec_name(ck):
    return os.path.basename(ck.split("@")[0]) + ck[len(ck.split("@")[0]):]


def play(ck, rows, attempts, roots):
    fs = arm(ck, roots)
    F = len(rows)
    out = np.zeros((F, attempts))
    for att in range(attempts):
        sims = [sts2.Sim(json.dumps(r["scenario"]), r["seed"]) for r in rows]
        print(f"  {spec_name(ck)}: attempt {att + 1}/{attempts}, {F} fights", flush=True)
        res = fs.run([r["scenario"] for r in rows], np.arange(F, dtype=np.uint32), np.uint64(att + 1) * np.uint64(7_919_993) + np.arange(F, dtype=np.uint64), starts=sims)
        out[:, att] = res[:, 1] == 1
        if fs.exact:
            st = fs.stats
            print(f"    exact turn: {st['ex_triggered']} of {st['searched']} searched decisions, {st['ex_capped']} capped, {st['ex_changed']} changed", flush=True)
    release(fs)
    return out


def release(fs):
    import gc
    import torch
    fs._runs = []
    del fs
    gc.collect()
    torch.cuda.empty_cache()


def evaluate(a):
    rows = json.load(open(a.bench))
    kind = np.array([r["kind"] for r in rows])
    base = None
    for k, ck in enumerate(a.ckpts):
        t = time.time()
        w = play(ck, rows, a.attempts, a.roots)
        line = []
        for kd, name, flip in (("near_loss", "near-miss losses won", lambda x: x), ("close_win", "close wins lost", lambda x: 1 - x)):
            sel = kind == kd
            f = flip(w[sel]).mean(1)
            if base is None:
                line.append(f"{name} {f.mean():.3f}")
            else:
                d = f - flip(base[sel]).mean(1)
                line.append(f"{name} {f.mean():.3f} ({d.mean():+.3f} +- {d.std(ddof=1) / len(d) ** 0.5:.3f})")
        if base is None:
            base = w
        print(f"{os.path.basename(ck):24s} ({time.time() - t:.0f}s) " + " | ".join(line), flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build"); b.add_argument("parts", nargs="+"); b.add_argument("--near", required=True); b.add_argument("--out", required=True)
    b.add_argument("--n", type=int, default=1500); b.add_argument("--close", type=float, default=0.10); b.add_argument("--seed", type=int, default=0)
    e = sub.add_parser("eval"); e.add_argument("ckpts", nargs="+", help="the first is the player that collected the set (the luck baseline); "
                                               "PRIOR+ROLL plays PRIOR's search with ROLL's policy in the play-outs; suffixes @MxK, @cv (clairvoyant), "
                                               "@cover (every distinct legal action), @tN (N futures per decision split over the candidates), "
                                               "@x (exact turn search when the search is blind), "
                                               "@cap (leaf values capped at the win-now value)")
    e.add_argument("--bench", default=os.path.join(ROOT, "data", "bench", "nearmiss.json")); e.add_argument("--attempts", type=int, default=2)
    e.add_argument("--roots", type=int, default=1024)
    a = ap.parse_args()
    {"build": build, "eval": evaluate}[a.cmd](a)


if __name__ == "__main__":
    main()
