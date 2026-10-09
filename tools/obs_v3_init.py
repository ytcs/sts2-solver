#!/usr/bin/env python3
"""Observation v3 network from a v2 checkpoint (needs the sts2 module built with --features obs_v3).

init:   v2 weights copied; the one power table becomes three side tables (player / enemy / Osty, each a copy, no parameter shared); every v3 input
        enters through a zero-initialised projection, so the v3 network computes the v2 outputs; --ancient (default on) resets the never-trained
        rows of Ancient cards (card + pile tables) and of the powers they apply (all three side tables) to the mean of the trained rows.
check:  replays trained rows of collection parts; v2 network on the v2 prefix vs v3 network on the full row: policy logits and outcome logits
        (rows holding a reset id are counted apart); --obs-dump writes the replayed rows for a cross-build byte check.
"""
import argparse, glob, json, os, re, sys

import numpy as np
import torch

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]
import sts2  # noqa: E402
import model as M  # noqa: E402

NM = sts2.names()
NEW = ("pp_player.", "pp_enemy.", "pp_osty.", "px_", "x_", "pile_x.", "pilex.", "relic_x.", "card.x.")


def ancient_rows(old, new):
    """card ids (Ancient rarity) and power ids (applied by them, per card_powers.rs) whose rows the old -> new round never moved"""
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
    anc = {c["id"] for lst in cat["cards"].values() for c in lst if c["rarity"] == "Ancient"}
    cid = {n: i for i, n in enumerate(NM["card"])}
    w5, w6 = old["card.card.weight"], new["card.card.weight"]
    cards = sorted(cid[n] for n in anc if n in cid and torch.equal(w5[cid[n] + 1], w6[cid[n] + 1]))
    pw = set()
    for line in open(os.path.join(ROOT, "crates", "sts2sim", "src", "card_powers.rs"), encoding="utf8"):
        m = re.match(r"\s*\[(.*)\], // (\w+)$", line)
        if m and m.group(2) in anc:
            pw |= {int(p) - 1 for p, *_ in re.findall(r"\((\d+), (\d+), (-?\d+), (-?\d+)\)", m.group(1)) if int(p) > 0}
    n = len(NM["power"]) + 1
    p5, p6 = old["pp.bag.emb.weight"].view(4, n, -1), new["pp.bag.emb.weight"].view(4, n, -1)
    powers = sorted(p for p in pw if torch.equal(p5[:, p + 1], p6[:, p + 1]))
    return cards, powers


def mean_reset(w, ids, channels):
    """rows 1 + id of each channel block := the mean of that block's other (trained) rows"""
    w = w.clone()
    n = w.shape[0] // channels
    v = w.view(channels, n, -1)
    keep = torch.ones(n, dtype=torch.bool)
    keep[0] = False
    keep[[i + 1 for i in ids]] = False
    for k in range(channels):
        v[k, [i + 1 for i in ids]] = v[k, keep].mean(0)
    return w


def init(a):
    ck = torch.load(a.src, map_location="cpu")
    args = dict(ck.get("args", {}))
    assert int(args.get("obs_version", 2)) == 2, "the source must be an observation-v2 checkpoint"
    sd = dict(ck["net"])
    torch.manual_seed(a.seed)
    net = M.Net(d=args.get("d", 64), rounds=args.get("rounds", 2), heads=bool(args.get("heads", False)), obs=3)
    pp = sd.pop("pp.bag.emb.weight")
    for side in ("player", "enemy", "osty"):
        sd[f"pp_{side}.bag.emb.weight"] = pp.clone()
    missing, unexpected = net.load_state_dict(sd, strict=False)
    bad = [k for k in missing if not k.startswith(NEW)] + list(unexpected)
    assert not bad, bad
    assert all(net.state_dict()[k].abs().sum() == 0 for k in missing if k.startswith(("x_", "card.x."))), "side projections must start at zero"
    info = dict(src=a.src, new_params=sorted(missing))
    if a.ancient:
        old = torch.load(a.ancient_ref, map_location="cpu")
        cards, powers = ancient_rows(old.get("net", old), ck["net"])
        with torch.no_grad():
            net.card.card.weight.copy_(mean_reset(net.card.card.weight, cards, 1))
            net.pile.emb.weight.copy_(mean_reset(net.pile.emb.weight, cards, 2))
            for side in ("player", "enemy", "osty"):
                t = getattr(net, f"pp_{side}").bag.emb.weight
                t.copy_(mean_reset(t, powers, 4))
        info.update(ancient_cards=[NM["card"][i] for i in cards], ancient_powers=[NM["power"][i] for i in powers])
        print(f"mean-reset {len(cards)} Ancient card rows, {len(powers)} power rows: {info['ancient_cards']} {info['ancient_powers']}")
    torch.save({"net": net.state_dict(), "args": args | {"obs_version": 3}, "v3init": info}, a.out)
    print(f"-> {a.out} ({M.n_params(net)} parameters, {sum(net.state_dict()[k].numel() for k in missing)} new)")


def check(a):
    from power_coverage import part_rows
    torch.set_num_threads(a.threads)
    v2, v3 = M.load(a.v2).cpu(), M.load(a.v3).cpu()
    assert v2.obs == 2 and v3.obs == 3
    info = torch.load(a.v3, map_location="cpu").get("v3init", {})
    reset = {NM["card"].index(n) + 1 for n in info.get("ancient_cards", [])}
    O, MK = [], []
    for path in sorted(glob.glob(a.parts))[:a.n_parts]:
        o, mk = part_rows(path)[:2]
        O.append(o)
        MK.append(mk)
        if sum(map(len, O)) >= a.n:
            break
    obs, mask = np.concatenate(O)[:a.n], np.concatenate(MK)[:a.n]
    if a.obs_dump:
        np.save(a.obs_dump, obs[:, :sts2.OBS_SIZE_V2])
    secs = {n: (o, s) for n, o, s in sts2.layout()["sections"]}
    has_reset = np.zeros(len(obs), bool)
    if reset:
        for nm in ("hand", "draw", "discard", "exhaust", "decision", "played"):
            o, s = secs[nm]
            stride = {"hand": 15, "draw": 2, "discard": 2, "exhaust": 2, "decision": 1, "played": 17}[nm]
            blk = obs[:, o:o + s]
            ids = blk[:, 8::16] if nm == "decision" else blk[:, ::stride]
            has_reset |= np.isin(ids.astype(np.int64), list(reset)).any(1)
    res = dict(rows=len(obs), rows_with_reset_ids=int(has_reset.sum()))
    worst = dict(policy=0.0, outcome=0.0)
    eq = dict(policy=0, outcome=0)
    with torch.no_grad():
        for b in range(0, len(obs), a.mb):
            ob, mk = torch.from_numpy(obs[b:b + a.mb]), torch.from_numpy(mask[b:b + a.mb].astype(np.int64))
            keep = torch.from_numpy(~has_reset[b:b + a.mb])
            l2, _, o2 = v2(ob, mk, outcome=True)
            l3, _, o3 = v3(ob, mk, outcome=True)
            for k, x2, x3 in (("policy", l2, l3), ("outcome", o2, o3)):
                x2, x3 = x2[keep], x3[keep]
                worst[k] = max(worst[k], float((x2 - x3).abs().max()) if len(x2) else 0.0)
                eq[k] += int((x2 == x3).all(1).sum())
    res.update(max_abs_diff=worst, rows_bitwise_equal=eq, rows_compared=int((~has_reset).sum()))
    print(json.dumps(res, indent=1))
    assert worst["policy"] <= a.tol and worst["outcome"] <= a.tol, "the v3 network does not reproduce the v2 outputs"


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    i = sp.add_parser("init")
    i.add_argument("--src", default=os.path.join(ROOT, "models", "solver_r6.pt"))
    i.add_argument("--out", default=os.path.join(ROOT, "models", "solver_r6_v3init.pt"))
    i.add_argument("--ancient", action=argparse.BooleanOptionalAction, default=True)
    i.add_argument("--ancient-ref", default=os.path.join(ROOT, "models", "solver_r5.pt"), help="the previous round: rows equal in both were never trained")
    i.add_argument("--seed", type=int, default=0)
    c = sp.add_parser("check")
    c.add_argument("--v2", default=os.path.join(ROOT, "models", "solver_r6.pt"))
    c.add_argument("--v3", default=os.path.join(ROOT, "models", "solver_r6_v3init.pt"))
    c.add_argument("--parts", required=True)
    c.add_argument("--n-parts", type=int, default=4)
    c.add_argument("--n", type=int, default=4000)
    c.add_argument("--mb", type=int, default=512)
    c.add_argument("--tol", type=float, default=0.0)
    c.add_argument("--threads", type=int, default=8)
    c.add_argument("--obs-dump")
    a = ap.parse_args()
    init(a) if a.cmd == "init" else check(a)


if __name__ == "__main__":
    main()
