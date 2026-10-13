#!/usr/bin/env python3
"""Community prior LUT: per character x key x card / relic, win rate with vs without the item in the deck at that fight (spire-codex A10 export,
target/codex/a10.jsonl.gz; other players' seeds, never the current run's). Keys: each act's elites and bosses (outcome: the fight won; every meeting
counts), ACT<N> and ACT<N>:<BOSS> (deck at act N's first boss floor, that boss first; outcome: the run won). Deck at a fight: `codex.walk`; relics
owned before the floor. Counts only: {char: {key: {"n": [w, n], "c": {card: [w, n]}, "r": {relic: [w, n]}}}}, items with n >= MIN_N.
usage: tools/community_lut.py build [--out data/community_lut.json.gz]
       tools/community_lut.py query <char> <KEY> [ITEM ...]   (no game, no skill gate; the harness's `prior` prints the same)"""
import argparse, collections, functools, gzip, json, math, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LUT = os.path.join(ROOT, "data", "community_lut.json.gz")
CHARS = ("ironclad", "silent", "defect", "necrobinder", "regent")
TAG = "[community]: correlational (player and deck confounded), a prior, never evidence for promotion"
MIN_N = 10


def build(out):
    sys.path[:0] = [ROOT, os.path.join(ROOT, "tools")]
    import codex, sts2
    from agent import pools
    from reward_values import _codex_path
    fights = {e for n in pools.ACTS for k in ("elite", "boss") for e in pools.pool(n, k)}
    boss_act = {e: d["act"] for n, d in pools.ACTS.items() for e in pools.pool(n, "boss")}
    relics = set(sts2.names()["relic"])
    T = collections.defaultdict(lambda: collections.defaultdict(lambda: {"n": [0, 0], "c": collections.defaultdict(lambda: [0, 0]), "r": collections.defaultdict(lambda: [0, 0])}))

    def add(char, key, won, deck, rel):
        row = T[char][key]
        row["n"][0] += won
        row["n"][1] += 1
        for k, items in (("c", deck), ("r", rel)):
            for x in items:
                row[k][x][0] += won
                row[k][x][1] += 1
    runs = 0
    for r in codex.rows(_codex_path()):
        if r["char"] not in CHARS:
            continue
        runs += 1
        last, met = sum(len(a) for a in r["floors"]), set()
        for _a, _i, g, f, now, rl, _up in codex.walk(r):
            enc = [e for e in f["enc"] if e in fights]
            if not enc or f["t"] not in ("elite", "boss"):
                continue
            deck, rel = set(now), rl & relics
            add(r["char"], enc[0], int(not (g == last and not r["win"])), deck, rel)
            a = boss_act.get(enc[0]) if f["t"] == "boss" else None
            if a is not None and a not in met:
                met.add(a)
                for key in (f"ACT{a + 1}", f"ACT{a + 1}:{enc[0]}"):
                    add(r["char"], key, int(r["win"]), deck, rel)
    lut = {c: {k: {"n": v["n"], **{t: {x: wn for x, wn in sorted(v[t].items()) if wn[1] >= MIN_N} for t in ("c", "r")}} for k, v in sorted(keys.items())}
           for c, keys in sorted(T.items())}
    with gzip.open(out, "wt", encoding="utf-8") as g:
        json.dump(dict(source=os.path.basename(_codex_path()), runs=runs, min_n=MIN_N, chars=lut), g, separators=(",", ":"))
    print(f"{runs} runs -> {out} ({os.path.getsize(out) // 1024} KB): " + ", ".join(f"{c} {len(k)} keys" for c, k in lut.items()))


@functools.cache
def load(path=LUT):
    with gzip.open(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def _norm(s):
    return re.sub(r"[^A-Z0-9:]+", "_", s.upper().rstrip("+")).strip("_")


def _rate(w, n):
    p = w / n if n else float("nan")
    return p, math.sqrt(p * (1 - p) / n) if n else float("nan")


def _row(name, with_, tot):
    (w1, n1), (w0, n0) = with_, (tot[0] - with_[0], tot[1] - with_[1])
    (p1, s1), (p0, s0) = _rate(w1, n1), _rate(w0, n0)
    return f"  {name[:28]:28s} {p1:5.2f} {n1:6d}   {p0:5.2f} {n0:6d}   {p1 - p0:+.2f} {math.sqrt(s1 * s1 + s0 * s0):.2f}"


def resolve(keys, arg):
    a = _norm(arg)
    hit = [a] if a in keys else [k for k in keys if k.startswith(a)]
    return hit[0] if len(hit) == 1 else None, hit


def lines(char, key, items=(), top=8, min_n=30, lut=None, tag=True):
    keys = (lut or load())["chars"].get(char.lower())
    if not keys:
        return f"prior: no community runs for {char}\n"
    k, hit = resolve(keys, key)
    if k is None:
        return f"prior: key {key} is " + (f"ambiguous: {', '.join(hit)}" if hit else f"unknown; keys: {', '.join(sorted(keys))}") + "\n"
    row = keys[k]
    w, n = row["n"]
    what = "the run won, deck at the act's first boss" if k.startswith("ACT") else "the fight won, deck at the fight"
    out = [TAG] * tag + [f"{char.upper()} {k}: {w / max(n, 1):.2f} of {n} ({what})", f"  {'item':28s} {'with':>5s} {'n':>6s}   {'w/o':>5s} {'n':>6s}   {'diff':>5s} {'se':>4s}"]
    if items:
        for it in items:
            x = _norm(it)
            got = [(t, row[t][x]) for t in ("c", "r") if x in row[t]]
            out += [_row(("relic " if t == "r" else "") + x, wn, row["n"]) for t, wn in got] or [f"  {x[:28]:28s} not in the table (fewer than {MIN_N} decks with it, or not a card / relic id)"]
    else:
        ok = [(("relic " if t == "r" else "") + x, wn) for t in ("c", "r") for x, wn in row[t].items() if wn[1] >= min_n and n - wn[1] >= min_n]
        d = sorted(ok, key=lambda it: it[1][0] / it[1][1] - (w - it[1][0]) / (n - it[1][1]))
        out += [f"  top / bottom {top} by diff (n >= {min_n} each side):"] + [_row(x, wn, row["n"]) for x, wn in d[::-1][:top]] + ["  ..."] + [_row(x, wn, row["n"]) for x, wn in d[:top][::-1]]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--out", default=LUT)
    q = sub.add_parser("query")
    q.add_argument("char")
    q.add_argument("key")
    q.add_argument("items", nargs="*")
    a = ap.parse_args()
    if a.cmd == "build":
        build(a.out)
    else:
        sys.stdout.write(lines(a.char, a.key, a.items))


if __name__ == "__main__":
    main()
