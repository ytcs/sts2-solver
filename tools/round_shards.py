#!/usr/bin/env python3
"""Collection shards: fight files (each with its `--starts` file or "-" for none) shuffled together and cut into equal pool files of at least
--min fights, each with its matching starts file (null where a fight has no root).
usage: tools/round_shards.py --inputs F1 S1 F2 - ... --min 12288 --out-dir target/r11/queue --prefix s"""
import argparse, collections, json, os, random


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--inputs", nargs="+", required=True, help="pairs: fights file, starts file or -")
    ap.add_argument("--min", type=int, default=12288)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--prefix", default="s")
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    rows = []
    for ff, sf in zip(a.inputs[::2], a.inputs[1::2]):
        fs = json.load(open(ff))
        ss = [None] * len(fs) if sf == "-" else json.load(open(sf))
        assert len(fs) == len(ss), (ff, len(fs), len(ss))
        rows += zip(fs, ss)
    random.Random(a.seed).shuffle(rows)
    k = max(1, len(rows) // a.min)
    os.makedirs(a.out_dir, exist_ok=True)
    for i in range(k):
        part = rows[i * len(rows) // k:(i + 1) * len(rows) // k]
        name = os.path.join(a.out_dir, f"{a.prefix}{i:03d}")
        json.dump([f for f, _ in part], open(name + ".json", "w"))
        json.dump([s for _, s in part], open(name + "_starts.json", "w"))
        c = collections.Counter(f.get("meta", {}).get("source") for f, _ in part)
        print(f"{name}.json: {len(part)} fights, {sum(s is not None for _, s in part)} roots, {dict(c)}")


if __name__ == "__main__":
    main()
