#!/usr/bin/env python3
"""Observation v4 network from a v3 checkpoint, and the check that it computes the v3 outputs.

init  (v4 build): v3 weights copied; the death block (`enemy_death_x`) enters through a zero-initialised projection added to the enemy side input,
      so the v4 network computes the v3 outputs on any row whose 9th potion slot is empty.
dump  (v3 build): the v3 network's policy and outcome logits on searched rows of the original parts, for the fights kept in the remapped parts
      (tools/remap_parts.py), matched by (scenario, seed).
check (v4 build): the v4 network on the same rows replayed from the remapped parts; reference policy logits moved to the v4 action indices
      (the 9th potion's actions stay masked); prints max |diff| and bitwise-equal row counts, fails above --tol.
"""
import argparse, glob, json, os, sys

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import sts2  # noqa: E402
import model as M  # noqa: E402
from exit import Data  # noqa: E402

NEW = ("death_kind.", "x_death.")


def init(a):
    ck = torch.load(a.src, map_location="cpu")
    args = dict(ck.get("args", {}))
    assert int(args.get("obs_version", 2)) == 3, "the source must be an observation-v3 checkpoint"
    torch.manual_seed(a.seed)
    net = M.Net(d=args.get("d", 64), rounds=args.get("rounds", 2), heads=bool(args.get("heads", False)), obs=4)
    missing, unexpected = net.load_state_dict(ck["net"], strict=False)
    bad = [k for k in missing if not k.startswith(NEW)] + list(unexpected)
    assert not bad, bad
    assert net.x_death.weight.abs().sum() == 0, "the death projection must start at zero"
    torch.save({"net": net.state_dict(), "args": args | {"obs_version": 4}, "v4init": dict(src=a.src, new_params=sorted(missing))}, a.out)
    print(f"-> {a.out} ({sum(net.state_dict()[k].numel() for k in missing)} new parameters: {sorted(missing)})")


def pairs(a):
    """(remapped part path, original part path, kept fight indices in the remapped part, their indices in the original): --n rows spread over
    --n-parts parts, each part's first fights"""
    out = []
    for rp in sorted(glob.glob(os.path.join(a.remapped, "*.npz")))[:a.n_parts]:
        rows = 0
        op = os.path.join(a.orig, os.path.basename(rp))
        zr, zo = np.load(rp), np.load(op)
        key = {(int(s), int(d)): f for f, (s, d) in enumerate(zip(zo["f_scen"], zo["f_seed"]))}
        n_rows = np.bincount(zr["d_fight"], minlength=len(zr["f_cls"]))
        fr, fo = [], []
        for f, (s, d) in enumerate(zip(zr["f_scen"], zr["f_seed"])):
            if n_rows[f] == 0:
                continue
            fr.append(f)
            fo.append(key[(int(s), int(d))])
            rows += int(n_rows[f])
            if rows >= a.n // a.n_parts:
                break
        out.append((rp, op, fr, fo))
    return out


@torch.no_grad()
def logits(net, path, fights, mb):
    d = Data([path])
    r = d.rows([(0, f) for f in fights])
    assert not d.bad, f"{path}: fights that do not replay: {sorted(d.bad)[:5]}"
    pl, ol = [], []
    for b in range(0, len(r[0]), mb):
        lg, _, oc = net(torch.from_numpy(r[0][b:b + mb]), torch.from_numpy(r[1][b:b + mb].astype(np.int64)), outcome=True)
        pl.append(lg.float().numpy())
        ol.append(oc.float().numpy())
    return np.concatenate(pl), np.concatenate(ol)


def dump(a):
    torch.set_num_threads(a.threads)
    assert sts2.OBS_VERSION == 3, "dump runs on the v3 build"
    net = M.load(a.v3).cpu()
    P, O = [], []
    for rp, op, fr, fo in pairs(a):
        p, o = logits(net, op, fo, a.mb)
        P.append(p)
        O.append(o)
    np.savez_compressed(a.dump, policy=np.concatenate(P), outcome=np.concatenate(O), actions=np.array(sts2.ACTIONS))
    print(f"-> {a.dump}: {sum(map(len, P))} rows")


def check(a):
    from remap_parts import remap
    torch.set_num_threads(a.threads)
    assert sts2.OBS_VERSION == 4, "check runs on the v4 build"
    net = M.load(a.v4).cpu()
    assert net.obs == 4
    ref = np.load(a.dump)
    P, O = [], []
    for rp, op, fr, fo in pairs(a):
        p, o = logits(net, rp, fr, a.mb)
        P.append(p)
        O.append(o)
    P, O = np.concatenate(P), np.concatenate(O)
    old = int(ref["actions"])
    rp_ = np.full((len(ref["policy"]), sts2.ACTIONS), -1e9, np.float32)
    rp_[:, remap(np.arange(old), 8)] = ref["policy"]
    assert P.shape == rp_.shape and O.shape == ref["outcome"].shape, (P.shape, rp_.shape, O.shape, ref["outcome"].shape)
    res = dict(rows=len(P), max_abs_diff=dict(policy=float(np.abs(P - rp_).max()), outcome=float(np.abs(O - ref["outcome"]).max())),
               rows_bitwise_equal=dict(policy=int((P == rp_).all(1).sum()), outcome=int((O == ref["outcome"]).all(1).sum())))
    print(json.dumps(res, indent=1))
    assert max(res["max_abs_diff"].values()) <= a.tol, "the v4 network does not reproduce the v3 outputs"


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    i = sp.add_parser("init")
    i.add_argument("--src", default=os.path.join(ROOT, "models", "solver_r9.pt"))
    i.add_argument("--out", default=os.path.join(ROOT, "models", "solver_r9_v4init.pt"))
    i.add_argument("--seed", type=int, default=0)
    for name in ("dump", "check"):
        c = sp.add_parser(name)
        c.add_argument("--v3", default=os.path.join(ROOT, "models", "solver_r9.pt"))
        c.add_argument("--v4", default=os.path.join(ROOT, "models", "solver_r9_v4init.pt"))
        c.add_argument("--orig", required=True, help="directory of the original (v3, 8-potion) parts")
        c.add_argument("--remapped", required=True, help="directory of the remapped parts (tools/remap_parts.py)")
        c.add_argument("--dump", required=True, help="the v3 logits (.npz)")
        c.add_argument("--n", type=int, default=12000, help="searched rows compared")
        c.add_argument("--n-parts", type=int, default=6)
        c.add_argument("--mb", type=int, default=512)
        c.add_argument("--tol", type=float, default=0.0)
        c.add_argument("--threads", type=int, default=8)
    a = ap.parse_args()
    {"init": init, "dump": dump, "check": check}[a.cmd](a)


if __name__ == "__main__":
    main()
