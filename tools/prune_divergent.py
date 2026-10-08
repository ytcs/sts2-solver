#!/usr/bin/env python3
import argparse, glob, json, os, shutil, sys, time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rl"))
import sts2  # noqa: E402

D_KEYS = ("d_fight", "d_step", "d_opts", "d_q", "d_n", "d_pi", "d_adv", "d_v")


def divergent(z, scen):
    A, O, FS, SD = z["acts"], z["f_off"], z["f_scen"], z["f_seed"]

    def ok(g):
        acts = [A[O[f]:O[f + 1]] for f in g]
        off = np.concatenate([[0], np.cumsum([len(a) for a in acts])])
        try:
            sts2.replay_rows(scen, FS[g], SD[g], np.concatenate(acts), off, [max(len(a) - 1, 0) for a in acts], np.arange(len(g) + 1))
            return True
        except ValueError:
            return False
    bad, stack = [], [np.arange(len(FS))]
    while stack:
        g = stack.pop()
        if ok(g):
            continue
        if len(g) == 1:
            bad.append(int(g[0]))
        else:
            stack += [g[:len(g) // 2], g[len(g) // 2:]]
    return sorted(bad)


def pruned(z, drop):
    out = {k: z[k] for k in z.files}
    n = len(out["f_cls"])
    keep = np.setdiff1d(np.arange(n), drop)
    new_idx = np.full(n, -1, np.int64)
    new_idx[keep] = np.arange(len(keep))
    A, O = out["acts"], out["f_off"]
    out["acts"] = np.concatenate([A[O[f]:O[f + 1]] for f in keep]).astype(A.dtype)
    out["f_off"] = np.concatenate([[0], np.cumsum([O[f + 1] - O[f] for f in keep])]).astype(O.dtype)
    for k in ("f_scen", "f_seed", "f_cls"):
        out[k] = out[k][keep]
    df = out["d_fight"]
    dsel = np.isin(df, keep)
    for k in D_KEYS:
        if k in out:
            out[k] = out[k][dsel]
    out["d_fight"] = new_idx[df[dsel]].astype(df.dtype)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("parts", nargs="+")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--backup", default=None, help="where originals go (default: <dir>/pruned_backup)")
    ap.add_argument("--max-share", type=float, default=0.01, help="stop if a part would lose more than this share of its fights: regenerate it")
    a = ap.parse_args()
    files = sorted({f for p in a.parts for f in glob.glob(p)})
    t0, total = time.time(), 0
    for fn in files:
        z = np.load(fn, allow_pickle=True)
        if "f_cls" not in z.files:
            continue
        scen = json.loads(str(z["scenarios"]))
        bad = divergent(z, scen)
        if not bad:
            continue
        n = len(z["f_cls"])
        print(f"{fn}: {len(bad)} of {n} fights no longer replay: {bad[:10]}{' ...' if len(bad) > 10 else ''}", flush=True)
        if len(bad) > a.max_share * n:
            raise SystemExit(f"{fn}: {len(bad) / n:.1%} > --max-share {a.max_share:.1%}: regenerate this collection instead of pruning it")
        total += len(bad)
        if a.dry_run:
            continue
        out = pruned(z, bad)
        z.close()
        bdir = a.backup or os.path.join(os.path.dirname(os.path.abspath(fn)), "pruned_backup")
        os.makedirs(bdir, exist_ok=True)
        shutil.copy2(fn, os.path.join(bdir, os.path.basename(fn)))
        tmp = fn[:-4] + ".pruning.npz"
        np.savez_compressed(tmp, **out)
        os.replace(tmp, fn)
        print(f"  rewritten without them (original in {bdir})", flush=True)
    print(f"{len(files)} parts checked in {time.time() - t0:.0f}s: {total} fights {'would be ' if a.dry_run else ''}dropped")


if __name__ == "__main__":
    main()
