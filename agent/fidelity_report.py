"""Summarize the fidelity sweep (`evals/fid_*.jsonl`): per divergence category, how many cases, which cards / relics / potions, and example lines.

    python -m agent.fidelity_report [evals/fid_cards.jsonl ...]
"""
import collections
import glob
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def main():
    paths = sys.argv[1:] or sorted(glob.glob(os.path.join(ROOT, "evals", "fid_*.jsonl")))
    rows = [json.loads(l) for p in paths for l in open(p, encoding="utf-8")]
    ok = [r for r in rows if r.get("ok")]
    print(f"{len(rows)} cases, {len(ok)} played, {len(rows) - len(ok)} failed to run")
    by = collections.defaultdict(list)
    for r in ok:
        for k, v in (r.get("stats") or {}).items():
            by[k].append((r["label"], v))
    for k, items in sorted(by.items(), key=lambda kv: -len(kv[1])):
        print(f"\n## {k}: {len(items)} cases, {sum(v for _, v in items)} occurrences")
        print("   in:", ", ".join(f"{l.split(':', 1)[1]}({v})" for l, v in items[:12]))
        ex = [e for r in ok if r["label"] in {l for l, _ in items} for e in (r.get("examples") or {}).get(k, [])]
        for t in ex[:3]:
            print("     ", t[:230])
    bad = [r for r in rows if not r.get("ok")]
    if bad:
        print("\nfailed:", [(r["label"], r.get("error")) for r in bad][:10])


if __name__ == "__main__":
    main()
