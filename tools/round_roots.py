#!/usr/bin/env python3
"""Collection roots from community replays (tools/codex_replay.py): fight scenarios + `--starts` recipes for `rl/exit.py collect`.
A root is the human decision state before action k, admitted when the replay matches the log through step k + 2 (two further human actions;
a fight without a matched logged end counts as diverged at its last action). Every admitted root is rebuilt and labelled: phase (Test
Subject 1 / 2 / 3, "d" while a revive is pending; Waterfall Giant pre / killed / explode), player HP band, fight outcome (a fight with no
logged end is lost when it ended the run at that encounter), deck strength tertile per encounter (predictor, full HP, no potions), strong
cohort. Draw: weight 1 / sqrt(stratum size) x --strong-w (strong cohort) x --lost-w (lost fights of death-effect encounters), up to --cap
copies of a root (a copy is a new job seed: hidden piles and RNG resampled). Each recipe is self-contained (scenario, ops up to the root,
stop); `rl/exit.py root` rebuilds it.
usage: tools/round_roots.py --n 40000 --out-fights target/r11/roots_fights.json --out-starts target/r11/roots_starts.json"""
import argparse, collections, json, math, multiprocessing, os, random, sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import codex_replay as cr  # noqa: E402
from round_decks import _main_target  # noqa: E402

DEATH = ("TEST_SUBJECT_BOSS", "WATERFALL_GIANT_BOSS", "PHROG_PARASITE_ELITE", "GREMLIN_MERC_NORMAL", "AXEBOTS_NORMAL", "DECIMILLIPEDE_ELITE",
         "FOGMOG_NORMAL", "KAISER_CRAB_BOSS")
ENCOUNTERS = DEATH + ("QUEEN_BOSS", "AEONGLASS_BOSS", "KNOWLEDGE_DEMON_BOSS", "THE_INSATIABLE_BOSS")
HP_BANDS = (0.25, 0.5, 0.75)


def admitted(row):
    """stops k whose state the replay matches through step k + 2"""
    d = [row["divs"][0]["step"]] if row["divs"] else []
    if row["failed"]:
        d.append(row["failed"]["step"])
    end = row["end"] or {}
    if not end.get("log") or end["log"] != end["sim"]:
        d.append(row["actions"])
    return min(row["actions"], min(d) - 2) if d else row["actions"]


def outcome(row, meta, last):
    log = (row["end"] or {}).get("log")
    if log:
        return "won" if log == "victory" else "lost"
    return "lost" if row["k"] == last and meta.get("killed_by") == row["enc"] else "won"


def phase(sn, enc):
    if enc == "TEST_SUBJECT_BOSS":
        e = next(e for e in sn["enemies"] if e["id"] == "TEST_SUBJECT")
        return f"{1 if e['max_hp'] < 150 else 2 if e['max_hp'] < 260 else 3}{'d' if e['hp'] <= 0 else ''}"
    if enc == "WATERFALL_GIANT_BOSS":
        e = next(e for e in sn["enemies"] if e["id"] == "WATERFALL_GIANT")
        return "explode" if e.get("next_move") == "EXPLODE_MOVE" else "killed" if e["hp"] >= 999_999_999 else "pre"
    return "-"


def prefix(ops, stop):
    kept, n = [], 0
    for op in ops:
        if op[0] == "apply":
            if n == stop:
                break
            n += 1
        if op[0] != "expect":
            kept.append(list(op))
    return kept


def labelled(job):
    """(path, k, n) -> (scenario, ops, [(stop, phase, player HP fraction)]) for the admitted stops that rebuild at a play decision"""
    path, k, n, enc = job
    from exit import root
    try:
        _, (pre, ops) = cr.chain(path, keep=k)
    except Exception:  # noqa: BLE001
        return None
    out = []
    for stop in range(n):
        try:
            s = root(pre, 0, dict(scenario=pre, ops=prefix(ops, stop), stop=stop))
        except (ValueError, AssertionError):
            continue
        if s.stage() == "play" and s.outcome() == 0:
            sn = json.loads(s.snapshot())
            out.append((stop, phase(sn, enc), sn["player"]["hp"] / max(sn["player"]["max_hp"], 1)))
    return pre, [op for op in ops if op[0] != "expect"], out


def strength(scs, ckpt):
    from predictor import Predictor, p_win
    return p_win(Predictor(ckpt).fight_start([dict(sc, hp=sc["max_hp"], potions=[]) for sc in scs], shuffles=4))


def main():
    ap = argparse.ArgumentParser()
    tgt = _main_target()
    ap.add_argument("--fidelity", default=os.path.join(tgt, "review", "replay_fidelity.jsonl"))
    ap.add_argument("--replays", default=os.path.join(tgt, "codex", "replays"))
    ap.add_argument("--encounters", nargs="*", default=list(ENCOUNTERS))
    ap.add_argument("--n", type=int, default=40000)
    ap.add_argument("--strong-w", type=float, default=2.0)
    ap.add_argument("--lost-w", type=float, default=3.0)
    ap.add_argument("--cap", type=int, default=4, help="copies of one root at most")
    ap.add_argument("--fights", type=int, default=0, help="first N eligible fights only (a smoke test)")
    ap.add_argument("--ckpt", default=os.path.join(ROOT, "models", json.load(open(os.path.join(ROOT, "models", "current.json")))["predictor"]))
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--out-fights", required=True)
    ap.add_argument("--out-starts", required=True)
    a = ap.parse_args()
    idx = {x["run_hash"]: x for x in map(json.loads, open(os.path.join(a.replays, "index.jsonl"), encoding="utf-8"))}
    rows = [json.loads(l) for l in open(a.fidelity, encoding="utf-8")]
    last = collections.defaultdict(int)
    for r in rows:
        last[r["run"]] = max(last[r["run"]], r["k"])
    fights = []
    for r in rows:
        if r["enc"] not in a.encounters or "error" in r:
            continue
        assert not idx[r["run"]].get("base_seed"), r["run"]
        if admitted(r) > 0:
            fights.append(dict(run=r["run"], k=r["k"], enc=r["enc"], n=admitted(r), outcome=outcome(r, idx[r["run"]], last[r["run"]]),
                               strong=bool(idx[r["run"]].get("strong"))))
    fights = fights[:a.fights] if a.fights else fights
    jobs = [(os.path.join(a.replays, f["run"] + ".jsonl.gz"), f["k"], f["n"], f["enc"]) for f in fights]
    with multiprocessing.Pool(a.workers) as pool:
        got = pool.map(labelled, jobs, chunksize=4)
    fights = [dict(f, pre=g[0], ops=g[1], roots=g[2]) for f, g in zip(fights, got) if g and g[2]]
    p = strength([f["pre"] for f in fights], a.ckpt)
    for enc in a.encounters:
        ix = [i for i, f in enumerate(fights) if f["enc"] == enc]
        if ix:
            cut = np.quantile(p[ix], [1 / 3, 2 / 3])
            for i in ix:
                fights[i]["strength"] = ("weak", "mid", "strong")[int(np.searchsorted(cut, p[i], side="right"))]
                fights[i]["p0"] = round(float(p[i]), 4)
    roots = [(fi, stop, (f["enc"], ph, int(np.searchsorted(HP_BANDS, hp, side="right")), f["outcome"], f["strength"]))
             for fi, f in enumerate(fights) for stop, ph, hp in f["roots"]]
    size = collections.Counter(s for _, _, s in roots)
    w = np.array([(a.strong_w if fights[fi]["strong"] else 1.0) * (a.lost_w if s[3] == "lost" and s[0] in DEATH else 1.0) / math.sqrt(size[s])
                  for fi, _, s in roots])
    lo, hi = 0.0, 1e9
    for _ in range(100):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if np.minimum(a.cap, w * mid).sum() < a.n else (lo, mid)
    e = np.minimum(a.cap, w * lo)
    rng = np.random.default_rng(a.seed)
    copies = np.floor(e).astype(int) + (rng.random(len(e)) < e - np.floor(e))
    out_f, out_s, chosen = [], [], collections.Counter()
    for (fi, stop, s), c in zip(roots, copies):
        f = fights[fi]
        chosen[s] += c
        for j in range(c):
            out_f.append(dict(f["pre"], name=f"replay:{f['run']}:{f['k']}:{stop}:{j}", meta=dict(
                source="replay_root", run=f["run"], k=f["k"], stop=stop, copy=j, phase=s[1], hp_band=s[2], outcome=s[3], strength=s[4], p0=f["p0"],
                strong=f["strong"])))
            out_s.append(dict(scenario=f["pre"], ops=prefix(f["ops"], stop), stop=stop))
    order = list(range(len(out_f)))
    random.Random(a.seed).shuffle(order)
    os.makedirs(os.path.dirname(os.path.abspath(a.out_fights)), exist_ok=True)
    json.dump([out_f[i] for i in order], open(a.out_fights, "w"))
    json.dump([out_s[i] for i in order], open(a.out_starts, "w"))
    table = [dict(encounter=s[0], phase=s[1], hp_band=s[2], outcome=s[3], strength=s[4], roots=size[s], chosen=int(chosen[s])) for s in sorted(size)]
    json.dump(dict(fights=len(fights), roots=len(roots), chosen=len(out_f), strata=table), open(a.out_fights[:-5] + "_strata.json", "w"), indent=0)
    by = collections.defaultdict(lambda: collections.Counter())
    for f in fights:
        by[f["enc"]]["fights"] += 1
        by[f["enc"]]["lost fights"] += f["outcome"] == "lost"
    for (fi, stop, s), c in zip(roots, copies):
        by[s[0]]["roots"] += 1
        by[s[0]]["strong roots"] += fights[fi]["strong"]
        by[s[0]]["lost roots"] += s[3] == "lost"
        by[s[0]]["chosen"] += int(c)
    for enc in a.encounters:
        print(f"{enc:24s} " + " ".join(f"{k} {v}" for k, v in by[enc].items()))
    print(f"{len(out_f)} roots ({len(roots)} admitted, {len(size)} strata, {len(fights)} fights) -> {a.out_fights}, {a.out_starts}")


if __name__ == "__main__":
    main()
