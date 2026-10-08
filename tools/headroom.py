#!/usr/bin/env python3
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))

KINDS = ("live", "cv", "pi")


def parse_arm(spec, leaf_default):
    kind, _, shape = spec.partition(":")
    if kind not in KINDS or not shape:
        raise SystemExit(f"bad arm {spec!r}: kind:MxK[xL] with kind in {KINDS}")
    v = [int(x) for x in shape.split("x")]
    if len(v) not in (2, 3):
        raise SystemExit(f"bad arm shape {shape!r}: MxK or MxKxL")
    M, K = v[0], v[1]
    L = v[2] if len(v) == 3 else (100 if kind == "pi" else leaf_default)
    if kind == "pi" and K != 1:
        raise SystemExit(f"pi arm {spec!r}: one true future per option (K = 1)")
    return dict(kind=kind, M=M, K=K, L=L, clairvoyant=kind != "live", label=f"{kind} {M}x{K}x{L}")


def play(net, fights, arm, attempts, seed, roots, threads):
    from fastsearch import FastSearch
    fs = FastSearch(net, M=arm["M"], K=arm["K"], leaf_turns=arm["L"], roots=roots, amp=True, threads=threads, clairvoyant=arm["clairvoyant"])
    fs.warm()
    S = len(fights)
    js = np.tile(np.arange(S, dtype=np.uint32), attempts)
    jd = np.uint64(seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
    t = time.time()
    r = fs.run(fights, js, jd)
    dt = time.time() - t
    st = fs.stats
    ends = st.get("end_cap", 0) + st.get("end_term", 0) + st.get("end_turn", 0)
    capped = st.get("end_cap", 0) / max(ends, 1)
    print(f"  {arm['label']}: {len(js)} fights {dt:.0f}s ({len(js) / max(dt, 1e-9):.1f}/s), play-outs capped {capped:.4f} (roll_cap {fs.roll_cap})", flush=True)
    return (r[:, 1] == 1).reshape(attempts, S).T, r[:, 6].reshape(attempts, S).T, dt, capped


def paired(a, b):
    d = a.mean(1) - b.mean(1)
    return float(d.mean()), float(d.std(ddof=1) / np.sqrt(len(d))) if len(d) > 1 else float("nan")


def main():
    ap = argparse.ArgumentParser(description="Headroom of a fight set (DIAGNOSTIC ONLY: cv / pi arms see hidden information). Arms kind:MxK[xL], kind in live / cv / pi.")
    ap.add_argument("--fights", required=True, help="scenario list, or the bench format [{scenario, wins, ends}]")
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", "solver_h128.pt"))
    ap.add_argument("--arms", default="live:5x32,cv:5x32,cv:8x64,pi:8x1x100", help="comma-separated kind:MxK[xL]; the first live arm is the reference")
    ap.add_argument("--attempts", type=int, default=2)
    ap.add_argument("--seed", type=int, default=91)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--roots", type=int, default=1024)
    ap.add_argument("--threads", type=int, default=None)
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "headroom", "headroom.json"))
    a = ap.parse_args()
    from fastsearch import LEAF_TURNS
    from model import load

    raw = json.load(open(a.fights))
    if a.limit:
        raw = raw[:a.limit]
    bench = bool(raw) and isinstance(raw[0], dict) and "scenario" in raw[0]
    fights = [x["scenario"] for x in raw] if bench else raw
    labels = np.array([np.mean([w for w in x["wins"] if w is not None] or [0.0]) for x in raw]) if bench and all(x.get("wins") for x in raw) else None
    arms = [parse_arm(s.strip(), LEAF_TURNS) for s in a.arms.split(",") if s.strip()]
    ref = next((i for i, x in enumerate(arms) if x["kind"] == "live"), None)
    if ref is None:
        raise SystemExit("--arms needs a live arm (the reference)")
    print(f"headroom: {len(fights)} fights from {a.fights} x {a.attempts} attempts, seed {a.seed}, {a.ckpt}; arms: {', '.join(x['label'] for x in arms)}", flush=True)
    print("  (cv / pi arms are DIAGNOSTIC ONLY: they see hidden information)", flush=True)
    net = load(a.ckpt)
    res = []
    for arm in arms:
        w, hp, dt, capped = play(net, fights, arm, a.attempts, a.seed, a.roots, a.threads)
        res.append(dict(arm=arm, wins=w, hp=hp, seconds=dt, capped=capped))

    live = res[ref]["wins"]
    S = len(fights)
    print(f"\n{S} fights x {a.attempts} attempts" + (f"; bench labels (live-width Solver, own seeds): win {labels.mean():.3f}, any {np.mean(labels > 0):.3f}" if labels is not None else ""))
    print(f"{'arm':<18} {'win':>6} {'any':>6} {'vs live':>16} {'won, live never':>16} {'live won, arm never':>20} {'seed: arm W live L':>19} {'reverse':>8}")
    summary = []
    for r in res:
        w = r["wins"]
        d, se = paired(w, live)
        gain = w.any(1) & ~live.any(1)
        lost = live.any(1) & ~w.any(1)
        row = dict(arm=r["arm"]["label"], kind=r["arm"]["kind"], win=float(w.mean()), any=float(w.any(1).mean()), d_win=d, d_se=se,
                   fights_gain=float(gain.mean()), fights_lost=float(lost.mean()), seeds_gain=float((w & ~live).mean()), seeds_lost=float((live & ~w).mean()),
                   seconds=r["seconds"], playouts_capped=r["capped"])
        summary.append(row)
        print(f"{row['arm']:<18} {row['win']:>6.3f} {row['any']:>6.3f} {d:>+8.3f} +- {se:<5.3f} {row['fights_gain']:>16.3f} {row['fights_lost']:>20.3f} "
              f"{row['seeds_gain']:>19.3f} {row['seeds_lost']:>8.3f}")
    cv = [r["wins"] for r in res if r["arm"]["clairvoyant"]]
    head = {}
    if cv:
        any_cv = np.any([w.any(1) for w in cv], 0)
        head = dict(cv_any=float(any_cv.mean()), live_any=float(live.any(1).mean()), won_cv_never_live=float((any_cv & ~live.any(1)).mean()),
                    won_live_never_cv=float((live.any(1) & ~any_cv).mean()), never_won=float((~any_cv & ~live.any(1)).mean()))
        msg = (f"\nheadroom: some clairvoyant arm wins {head['cv_any']:.3f} of the fights (live {head['live_any']:.3f}); won by clairvoyant play but never by live "
               f"{head['won_cv_never_live']:.3f} (the reverse {head['won_live_never_cv']:.3f}); won by no arm {head['never_won']:.3f}")
        if labels is not None:
            head["won_cv_never_live_nor_labels"] = float((any_cv & ~live.any(1) & ~(labels > 0)).mean())
            msg += f"; never by live nor the bench labels {head['won_cv_never_live_nor_labels']:.3f}"
        print(msg, flush=True)
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    json.dump(dict(fights=os.path.abspath(a.fights), n=S, attempts=a.attempts, seed=a.seed, ckpt=a.ckpt, summary=summary, headroom=head,
                   per_fight={r["arm"]["label"]: dict(wins=r["wins"].astype(int).tolist(), hp=r["hp"].astype(int).tolist()) for r in res},
                   names=[f.get("name", str(i)) for i, f in enumerate(fights)], labels=None if labels is None else labels.tolist()),
              open(a.out, "w"), indent=1)
    print(f"-> {a.out}", flush=True)


if __name__ == "__main__":
    main()
