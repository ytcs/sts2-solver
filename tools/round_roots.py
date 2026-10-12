#!/usr/bin/env python3
"""Collection roots from strong players' replays (tools/codex_replay.py): fight scenarios + `--starts` recipes for `rl/exit.py collect`.
Only encounters whose replays reach --min-exact (exact / ended in the fidelity rows) give roots; a root is a human decision state before the
fight's first divergence, later states weighted up (the states before a kill). Each recipe is self-contained (scenario, ops up to the root,
stop); `rl/exit.py root` rebuilds it and resamples the hidden piles and RNG from the fight's seed.
usage: tools/round_roots.py --fidelity target/review/replay_fidelity.jsonl --replays target/codex/replays --n 10000 \
         --out-fights target/r10/roots_fights.json --out-starts target/r10/roots_starts.json"""
import argparse, collections, json, multiprocessing, os, random, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import codex_replay as cr  # noqa: E402


def exact_share(rows):
    """encounter -> (exact, ended), as the fidelity report counts them"""
    out = collections.defaultdict(lambda: [0, 0])
    for r in rows:
        if "error" in r or not (r.get("end") or {}).get("log"):
            continue
        out[r["enc"]][1] += 1
        out[r["enc"]][0] += not r["fields"] and not r["failed"] and r["end"]["log"] == r["end"]["sim"]
    return out


def recipes(job):
    """(path, k, steps) -> [(scenario, start)] for the roots that rebuild at a play decision"""
    path, k, steps = job
    import sts2
    from exit import root
    try:
        _, (pre, ops) = cr.chain(path, keep=k)
    except Exception:  # noqa: BLE001
        return []
    out = []
    for stop in steps:
        kept, n = [], 0
        for op in ops:
            if op[0] == "apply":
                if n == stop:
                    break
                n += 1
            if op[0] != "expect":
                kept.append(list(op))
        start = dict(scenario=pre, ops=kept, stop=stop)
        try:
            s = root(pre, 0, start)
        except (ValueError, AssertionError):
            continue
        if s.stage() == "play" and s.outcome() == 0:
            out.append((dict(pre, name=f"replay:{os.path.basename(path)[:16]}:{k}:{stop}", meta=dict(source="replay_root", run=os.path.basename(path)[:16], k=k,
                                                                                                 stop=stop)), start))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fidelity", default=os.path.join(ROOT, "target", "review", "replay_fidelity.jsonl"))
    ap.add_argument("--replays", default=os.path.join(ROOT, "target", "codex", "replays"))
    ap.add_argument("--encounters", nargs="*", default=list(cr.ROOTS))
    ap.add_argument("--min-exact", type=float, default=0.95)
    ap.add_argument("--n", type=int, default=10000)
    ap.add_argument("--per-fight", type=int, default=2)
    ap.add_argument("--all-players", action="store_true", help="not only the strong cohort")
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--out-fights", required=True)
    ap.add_argument("--out-starts", required=True)
    a = ap.parse_args()
    idx = {x["run_hash"]: x for x in map(json.loads, open(os.path.join(a.replays, "index.jsonl"), encoding="utf-8"))}
    rows = [json.loads(l) for l in open(a.fidelity, encoding="utf-8")]
    share = exact_share(rows)
    ok = {e for e in a.encounters if share[e][1] and share[e][0] / share[e][1] >= a.min_exact}
    for e in a.encounters:
        x, n = share[e]
        print(f"{e:28s} exact {x}/{n} = {x / max(n, 1):.3f} {'roots' if e in ok else 'generated fights instead'}")
    r = random.Random(a.seed)
    jobs = []
    for row in rows:
        meta = idx.get(row["run"], {})
        if row["enc"] not in ok or "error" in row or not (a.all_players or meta.get("strong")):
            continue
        assert not meta.get("base_seed"), row["run"]
        prefix = row["divs"][0]["step"] if row["divs"] else row["actions"]
        if prefix < 1:
            continue
        w = [(s + 1) ** 2 for s in range(prefix)]
        steps = sorted(set(r.choices(range(prefix), w, k=min(a.per_fight, prefix))))
        jobs.append((os.path.join(a.replays, row["run"] + ".jsonl.gz"), row["k"], steps))
    r.shuffle(jobs)
    fights, starts = [], []
    with multiprocessing.Pool(a.workers) as pool:
        for got in pool.imap_unordered(recipes, jobs, chunksize=2):
            for sc, st in got:
                fights.append(sc)
                starts.append(st)
            if len(fights) >= a.n:
                pool.terminate()
                break
    fights, starts = fights[:a.n], starts[:a.n]
    json.dump(fights, open(a.out_fights, "w"))
    json.dump(starts, open(a.out_starts, "w"))
    print(f"{len(fights)} roots from {len(jobs)} fights in {len(ok)} encounters: {dict(collections.Counter(s['encounter'] for s in fights))} -> {a.out_fights}, {a.out_starts}")


if __name__ == "__main__":
    main()
