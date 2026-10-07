#!/usr/bin/env python3
"""Expert iteration (`docs/rebuild.md` S3): the search plays fights, the network learns to predict how the SEARCH's fights end and to choose like it.

  rl/exit.py collect --ckpt models/solver_h128.pt --fights F.json [F2.json ...] --out target/exit/r1.npz [--M 3 --K 8] [--attempts 2]
  rl/exit.py train   --init models/solver_h128.pt --data target/exit/r1.npz [more.npz ...] --out target/exit/r1.pt [--epochs 4]

`collect` stores fights compactly (scenario, seed, the action sequence, the outcome class, and per searched decision the options tried with their
search estimates); `train` replays the actions to observations (`sts2.replay`, deterministic) chunk by chunk, so millions of rows need no disk or RAM.
Targets per searched decision:
  policy   softmax of the options' search estimates at temperature `--tau` (the search's improved policy over the options it tried)
  outcome  the class of how that fight really ended (loss, or the 2-HP end bin) under search play, HL-Gauss-smoothed over neighbouring win bins
           (Farebrother et al. 2024); realized outcomes, never the max of the search's estimates (winner's curse)
"""
import argparse, json, os, sys, time

import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2  # noqa: E402
import heads as H  # noqa: E402


def collect(a):
    from fastsearch import FastSearch
    from model import load
    scen = [s for f in a.fights for s in json.load(open(f))]
    fs = FastSearch(load(a.ckpt), M=a.M, K=a.K, record=True, roots=a.roots, amp=True)
    fs.warm()
    S = len(scen)
    js = np.tile(np.arange(S, dtype=np.uint32), a.attempts)
    jd = np.uint64(a.seed) * np.uint64(1_000_003) + np.arange(len(js), dtype=np.uint64)
    t0 = time.time()
    res = fs.run(scen, js, jd)
    F_ = dict(scen=[], seed=[], cls=[], off=[0], acts=[])
    D = dict(fight=[], step=[], opts=[], q=[])
    for idx, eng in fs._runs:
        for jl, j in enumerate(idx):
            oc, hp_end = res[j, 1], res[j, 6]
            if oc not in (1, -1, 2):
                continue
            acts, searched, opts, _p, q, legal = eng.moves(jl)
            f = len(F_["scen"])
            F_["scen"].append(int(js[j])); F_["seed"].append(int(jd[j])); F_["cls"].append(int(H.end_class(oc == 1, hp_end)))
            F_["acts"].extend(int(x) for x in acts); F_["off"].append(len(F_["acts"]))
            for t in np.nonzero(searched)[0]:
                ok = legal[t, :a.M].astype(bool) & np.isfinite(q[t, :a.M])
                if ok.sum() < 2:
                    continue
                D["fight"].append(f); D["step"].append(int(t))
                D["opts"].append(np.where(ok, opts[t, :a.M], -1).astype(np.int16)); D["q"].append(np.where(ok, q[t, :a.M], np.nan).astype(np.float32))
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    np.savez_compressed(a.out, scenarios=np.array(json.dumps(scen)), f_scen=np.array(F_["scen"], np.int32), f_seed=np.array(F_["seed"], np.uint64),
                        f_cls=np.array(F_["cls"], np.int16), f_off=np.array(F_["off"], np.int64), acts=np.array(F_["acts"], np.int16),
                        d_fight=np.array(D["fight"], np.int32), d_step=np.array(D["step"], np.int32), d_opts=np.array(D["opts"], np.int16), d_q=np.array(D["q"], np.float32))
    print(f"{S} fights x {a.attempts}: search win {np.mean(res[:, 1] == 1):.3f}; {len(F_['cls'])} fights, {len(D['fight'])} decisions -> {a.out} ({time.time() - t0:.0f}s)", flush=True)


class Data:
    """One or more `collect` files; `chunks` yields replayed rows (obs, mask, opts, policy target, outcome class) a chunk of fights at a time."""

    def __init__(self, paths, tau):
        self.parts = []
        for p in paths:
            z = np.load(p)
            scen = json.loads(str(z["scenarios"]))
            q = z["d_q"].astype(np.float64)
            ok = np.isfinite(q)
            e = np.where(ok, np.exp((np.where(ok, q, -np.inf) - np.nanmax(q, 1, keepdims=True)) / tau), 0.0)
            tgt = (e / e.sum(1, keepdims=True)).astype(np.float32)
            order = np.argsort(z["d_fight"], kind="stable")
            self.parts.append(dict(scen=scen, f_scen=z["f_scen"], f_seed=z["f_seed"], f_cls=z["f_cls"], f_off=z["f_off"], acts=z["acts"].astype(np.int32),
                                   d_fight=z["d_fight"][order], d_step=z["d_step"][order], d_opts=z["d_opts"][order], tgt=tgt[order]))
        self.index = [(pi, f) for pi, p in enumerate(self.parts) for f in range(len(p["f_cls"]))]

    def __len__(self):
        return len(self.index)

    def rows(self, fights):
        obs, mask, opts, tgt, cls = [], [], [], [], []
        for pi, f in fights:
            p = self.parts[pi]
            lo, hi = np.searchsorted(p["d_fight"], [f, f + 1])
            if lo == hi:
                continue
            o, m = sts2.replay(p["scen"][p["f_scen"][f]], int(p["f_seed"][f]), p["acts"][p["f_off"][f]:p["f_off"][f + 1]])
            st = p["d_step"][lo:hi]
            obs.append(o[st]); mask.append(m[st]); opts.append(p["d_opts"][lo:hi]); tgt.append(p["tgt"][lo:hi]); cls.append(np.full(hi - lo, p["f_cls"][f]))
        return [np.concatenate(x) for x in (obs, mask, opts, tgt, cls)]


def hl_gauss(cls, sigma):
    """[B, NC] targets: a loss stays one-hot; a win spreads a Gaussian of width `sigma` bins over the win bins around its bin."""
    t = torch.zeros(len(cls), H.NC, device=cls.device)
    win = cls > 0
    t[~win, 0] = 1.0
    if win.any():
        b = torch.arange(1, H.NC, device=cls.device, dtype=torch.float32)
        g = torch.exp(-0.5 * ((b.unsqueeze(0) - cls[win].float().unsqueeze(1)) / sigma) ** 2)
        t[win, 1:] = g / g.sum(1, keepdim=True)
    return t


def train(a):
    from model import DEV, load
    torch.manual_seed(a.seed)
    rng = np.random.default_rng(a.seed)
    net = load(a.init).train()
    assert net.heads, "an outcome-head network is needed (models/solver_h128.pt)"
    data = Data(a.data, a.tau)
    idx = np.array(data.index, dtype=object)
    perm = rng.permutation(len(idx))
    n_hold = max(1, int(len(idx) * a.holdout))
    hold, tr = [tuple(x) for x in idx[perm[:n_hold]]], [tuple(x) for x in idx[perm[n_hold:]]]
    opt = torch.optim.AdamW(net.parameters(), lr=a.lr, weight_decay=1e-4)
    print(f"{len(data)} fights ({len(tr)} train, {len(hold)} holdout), init {a.init}", flush=True)

    def batch_loss(o, m, op, tg, cl):
        o, m = torch.from_numpy(o).to(DEV), torch.from_numpy(m.astype(np.int64)).to(DEV)
        op, tg, cl = torch.from_numpy(op.astype(np.int64)).to(DEV), torch.from_numpy(tg).to(DEV), torch.from_numpy(cl.astype(np.int64)).to(DEV)
        lg, _, ol = net(o, m, outcome=True)[:3]
        lp = F.log_softmax(lg.float(), 1).gather(1, op.clamp(min=0))
        pl = -(tg * torch.where(op >= 0, lp, torch.zeros_like(lp))).sum(1).mean()
        vl = -(hl_gauss(cl, a.sigma) * F.log_softmax(ol.float(), 1)).sum(1).mean()
        return pl, vl

    @torch.no_grad()
    def evaluate():
        net.eval()
        tot = np.zeros(3)
        for c in range(0, len(hold), a.chunk):
            r = data.rows(hold[c:c + a.chunk])
            for b in range(0, len(r[0]), a.mb):
                pl, vl = batch_loss(*(x[b:b + a.mb] for x in r))
                n = len(r[0][b:b + a.mb])
                tot += [pl.item() * n, vl.item() * n, n]
        net.train()
        return tot[0] / tot[2], tot[1] / tot[2]

    print("holdout before: policy %.4f outcome %.4f" % evaluate(), flush=True)
    it, n_chunks = 0, a.epochs * ((len(tr) + a.chunk - 1) // a.chunk)
    for ep in range(a.epochs):
        order = rng.permutation(len(tr))
        for c in range(0, len(tr), a.chunk):
            t0 = time.time()
            r = data.rows([tr[i] for i in order[c:c + a.chunk]])
            sh = rng.permutation(len(r[0]))
            r = [x[sh] for x in r]
            lr = a.lr * max(a.lr_floor, 1 - it / n_chunks)
            for g in opt.param_groups:
                g["lr"] = lr
            st = np.zeros(2)
            nb = 0
            for b in range(0, len(r[0]), a.mb):
                pl, vl = batch_loss(*(x[b:b + a.mb] for x in r))
                loss = a.pol * pl + vl
                opt.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
                opt.step()
                st += [pl.item(), vl.item()]
                nb += 1
            it += 1
            print(f"ep {ep} chunk {it}/{n_chunks} rows {len(r[0])} policy {st[0] / nb:.4f} outcome {st[1] / nb:.4f} lr {lr:.2e} ({time.time() - t0:.0f}s)", flush=True)
        print(f"holdout after epoch {ep}: policy %.4f outcome %.4f" % evaluate(), flush=True)
        ck = torch.load(a.init, map_location="cpu")
        torch.save({"net": net.state_dict(), "args": ck.get("args", {}), "exit": vars(a) | {"epoch": ep}}, a.out)
    print(f"-> {a.out}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("collect")
    c.add_argument("--ckpt", required=True); c.add_argument("--fights", nargs="+", required=True); c.add_argument("--out", required=True)
    c.add_argument("--M", type=int, default=3); c.add_argument("--K", type=int, default=8); c.add_argument("--attempts", type=int, default=2)
    c.add_argument("--roots", type=int, default=2048); c.add_argument("--seed", type=int, default=101)
    t = sub.add_parser("train")
    t.add_argument("--init", required=True); t.add_argument("--data", nargs="+", required=True); t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=4); t.add_argument("--chunk", type=int, default=2048); t.add_argument("--mb", type=int, default=2048)
    t.add_argument("--lr", type=float, default=1e-4); t.add_argument("--lr-floor", type=float, default=0.1)
    t.add_argument("--tau", type=float, default=0.02, help="temperature over the options' search estimates (linear return units)")
    t.add_argument("--sigma", type=float, default=0.75, help="HL-Gauss width of the win classes, in bins")
    t.add_argument("--pol", type=float, default=1.0, help="weight of the policy loss")
    t.add_argument("--holdout", type=float, default=0.05); t.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    collect(a) if a.cmd == "collect" else train(a)


if __name__ == "__main__":
    main()
