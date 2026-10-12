#!/usr/bin/env python3
"""Collection parts recorded with an older potion count (MAX_POTIONS 8: action space 308) -> this build's action space (v4: 9 potions, 322).

Potion uses and discards are per slot, so the indices from the discard block on move: a < OFF_DISCARD(old) unchanged, the discards by
(new - old) x targets, picks / confirm by (new - old) x (targets + 1); recorded options (d_opts, -1 = none) the same. Each copy is then
replayed under this build (every recorded action legal to its fight's end, tools/prune_divergent.py) and the fights that no longer replay are
dropped, at most --max-share per part.
"""
import argparse, glob, json, os, sys, time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import sts2  # noqa: E402
from prune_divergent import divergent, pruned  # noqa: E402

C = sts2.layout()["consts"]


def remap(a, old_potions):
    t = C["MAX_CREATURES"] + 1
    new = C["MAX_POTIONS"]
    disc = 1 + C["MAX_HAND"] * t + old_potions * t
    a = np.asarray(a)
    out = a.astype(np.int32)
    out = np.where((a >= disc) & (a < disc + old_potions), out + (new - old_potions) * t, out)
    out = np.where(a >= disc + old_potions, out + (new - old_potions) * (t + 1), out)
    return np.where(a < 0, a, out).astype(a.dtype)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("parts", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--old-potions", type=int, default=8)
    ap.add_argument("--max-share", type=float, default=0.01)
    a = ap.parse_args()
    assert C["MAX_POTIONS"] > a.old_potions, f"this build holds {C['MAX_POTIONS']} potions: nothing to remap (needs --features obs_v4)"
    assert C["OFF_DISCARD"] == 1 + C["MAX_HAND"] * (C["MAX_CREATURES"] + 1) + C["MAX_POTIONS"] * (C["MAX_CREATURES"] + 1)
    os.makedirs(a.out, exist_ok=True)
    files = sorted({f for p in a.parts for f in glob.glob(p)})
    t0, tot, dropped, report = time.time(), 0, 0, []
    for fn in files:
        z = np.load(fn, allow_pickle=False)
        d = {k: z[k] for k in z.files}
        z.close()
        d["acts"] = remap(d["acts"], a.old_potions)
        d["d_opts"] = remap(d["d_opts"], a.old_potions)
        scen = json.loads(str(d["scenarios"]))
        bad = divergent(d, scen)
        n = len(d["f_cls"])
        tot += n
        dropped += len(bad)
        report.append(dict(part=os.path.basename(fn), fights=n, dropped=len(bad), first=bad[:5]))
        print(f"{os.path.basename(fn)}: {n} fights, {len(bad)} no longer replay{' ' + str(bad[:8]) if bad else ''}", flush=True)
        if len(bad) > a.max_share * n:
            raise SystemExit(f"{fn}: {len(bad) / n:.2%} > --max-share {a.max_share:.0%}")
        out = pruned(d, bad) if bad else d
        np.savez_compressed(os.path.join(a.out, os.path.basename(fn)), **out)
    summary = dict(parts=len(files), fights=tot, dropped=dropped, share=dropped / max(tot, 1), seconds=round(time.time() - t0),
                   action_space=sts2.ACTIONS, old_potions=a.old_potions, per_part=report)
    json.dump(summary, open(os.path.join(a.out, "remap.json"), "w"), indent=1)
    print(f"{len(files)} parts, {tot} fights, {dropped} dropped ({dropped / max(tot, 1):.3%}) in {time.time() - t0:.0f}s -> {a.out}")


if __name__ == "__main__":
    main()
