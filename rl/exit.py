#!/usr/bin/env python3
"""Expert iteration (`docs/rebuild.md` S3): the search plays fights, the network learns to predict how the SEARCH's fights end and to choose like it.

  rl/exit.py collect --ckpt models/solver_h128.pt --fights F.json [F2.json ...] --out target/exit/r1.npz [--M 3 --K 8] [--attempts 2] [--max-minutes 120]
                     [--root gumbel --gumbel-m 16 --gumbel-n 160]
  rl/exit.py train   --init models/solver_h128.pt --data target/exit/r1_*.npz --out target/exit/r1.pt [--epochs 4] [--target gumbel]

`collect` stores fights compactly (scenario, seed, the action sequence, the outcome class, and per searched decision the options tried with their
search estimates); `train` replays the actions to observations (`sts2.replay_rows`, deterministic, in parallel) chunk by chunk, so millions of rows need no disk or RAM.
Targets per searched decision:
  policy   `--target anchored` (default): the init network's prior shifted by c x the decision's centred min-max-normalised estimates on the options tried;
           `soft`: softmax of the options' search estimates at temperature `--tau`; `gumbel` (parts collected with `--root gumbel`): Gumbel MuZero's
           improved policy pi' = softmax(prior logits + sigma(completed Q)) over every legal action, rebuilt from the recorded shift adv = sigma(q) -
           sigma(v) of each candidate (0 for the actions not sampled, whose completed Q is v) on the init network's logits
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


def _save(path, scen, F_, D, policy_only=False):
    """Format 1: the arrays below. Format 2 (`version` = 2, a Gumbel root) adds per decision the candidates' futures `d_n`, improved policy `d_pi`, shift
    `d_adv` (pi' = softmax(logits + adv)) and the completed Q of the actions not sampled `d_v`; readers of format 1 ignore them. `policy_only` (restart
    parts, `--restarts`): `train` leaves their fights out of the outcome loss."""
    extra = {"policy_only": np.array(1)} if policy_only else {}
    if "adv" in D:
        extra |= dict(version=np.array(2), root=np.array("gumbel"), d_n=np.array(D["n"], np.int16), d_pi=np.array(D["pi"], np.float32),
                     d_adv=np.array(D["adv"], np.float32), d_v=np.array(D["v"], np.float32))
    np.savez_compressed(path, scenarios=np.array(json.dumps(scen)), f_scen=np.array(F_["scen"], np.int32), f_seed=np.array(F_["seed"], np.uint64),
                        f_cls=np.array(F_["cls"], np.int16), f_off=np.array(F_["off"], np.int64), acts=np.array(F_["acts"], np.int16),
                        d_fight=np.array(D["fight"], np.int32), d_step=np.array(D["step"], np.int32), d_opts=np.array(D["opts"], np.int16), d_q=np.array(D["q"], np.float32),
                        **extra)


def collect(a):
    """Search-played fights, a chunk at a time: each chunk is saved as its own part (`<out>_NNN.npz`, train takes them all), reports its rate, an ETA
    and its longest fights; nothing starts after `--max-minutes`; a watchdog ends the process if a chunk runs longer than `--chunk-timeout` x the
    median chunk (the chunk's scenarios are dumped next to the output for diagnosis)."""
    import threading
    from fastsearch import FastSearch
    from model import load
    # restarts (`tools/nearmiss.py`): search from the true state the prefix reaches; a fight is stored as the original seed + prefix + new actions
    rs = json.load(open(a.restarts))["restarts"] if a.restarts else None
    scen = [r["scenario"] for r in rs] if rs else [s for f in a.fights for s in json.load(open(f))]
    sts2.set_look_legacy(a.look_legacy)
    gumbel = a.root == "gumbel"
    fs = FastSearch(load(a.ckpt), M=a.M, K=a.K, record=True, roots=a.roots, amp=True, root=a.root, gumbel_m=a.gumbel_m, gumbel_n=a.gumbel_n)
    W = a.gumbel_m if gumbel else a.M  # options recorded per decision
    fs.warm()
    jobs = [(i, att) for att in range(a.attempts) for i in range(len(scen))]
    stem = a.out[:-4] if a.out.endswith(".npz") else a.out
    os.makedirs(os.path.dirname(os.path.abspath(stem)), exist_ok=True)
    t_all, times, state = time.time(), [], {"start": None, "chunk": None}

    def watchdog():
        while True:
            time.sleep(10)
            st = state["start"]
            if st is None or len(times) < 2:
                limit = a.first_timeout * 60
            else:
                limit = a.chunk_timeout * float(np.median(times))
            if st is not None and time.time() - st > limit:
                bad = stem + f"_stuck_{state['chunk']:03d}.json"
                json.dump(state["scen"], open(bad, "w"))
                print(f"WATCHDOG: chunk {state['chunk']} ran {time.time() - st:.0f}s > limit {limit:.0f}s; its scenarios -> {bad}; exiting", flush=True)
                os._exit(2)
    threading.Thread(target=watchdog, daemon=True).start()
    n_chunks = (len(jobs) + a.chunk - 1) // a.chunk
    ran = 0
    for k in range(n_chunks):
        if os.path.exists(f"{stem}_{k:03d}.npz"):  # done by an earlier process (a long-lived process slows down: fresh ones resume here)
            continue
        if ran >= a.chunks_per_process:
            print(f"{n_chunks - k} chunks left: exiting for a fresh process (exit code 3)", flush=True)
            sys.exit(3)
        ran += 1
        if time.time() - t_all > a.max_minutes * 60:
            print(f"deadline: {a.max_minutes} min reached after {k} of {n_chunks} chunks; stopping (parts saved so far are complete)", flush=True)
            break
        part = jobs[k * a.chunk:(k + 1) * a.chunk]
        cs = [scen[i] for i, _ in part]
        js = np.arange(len(cs), dtype=np.uint32)
        jd = np.array([np.uint64(a.seed) * np.uint64(1_000_003) + np.uint64(att * len(scen) + i) for i, att in part], dtype=np.uint64)
        state.update(start=time.time(), chunk=k, scen=cs)
        starts = None
        if rs:
            starts = []
            for i, _ in part:
                sim = sts2.Sim(json.dumps(rs[i]["scenario"]), int(rs[i]["seed"]))
                for x in rs[i]["prefix"]:
                    sim.step(int(x))
                starts.append(sim)
        res = fs.run(cs, js, jd, starts=starts)
        dt = time.time() - state["start"]
        state["start"] = None
        times.append(dt)
        F_ = dict(scen=[], seed=[], cls=[], off=[0], acts=[])
        D = dict(fight=[], step=[], opts=[], q=[]) | (dict(n=[], pi=[], adv=[], v=[]) if gumbel else {})
        for idx, eng in fs._runs:
            for jl, j in enumerate(idx):
                oc, hp_end = res[j, 1], res[j, 6]
                if oc not in (1, -1, 2):  # (a fight the loop guard ended is already -1: a real-game soft-lock is a loss, `sts2env::looped`)
                    continue
                acts, searched, opts, _p, q, legal = eng.moves(jl)
                if gumbel:
                    _g, gn, gpi, gadv, gv = eng.moves_gumbel(jl)
                f = len(F_["scen"])
                pre = rs[part[j][0]]["prefix"] if rs else []
                F_["scen"].append(int(js[j])); F_["seed"].append(int(rs[part[j][0]]["seed"]) if rs else int(jd[j])); F_["cls"].append(int(H.end_class(oc == 1, hp_end)))
                F_["acts"].extend(int(x) for x in pre); F_["acts"].extend(int(x) for x in acts); F_["off"].append(len(F_["acts"]))
                for t in np.nonzero(searched)[0]:
                    ok = legal[t, :W].astype(bool) & np.isfinite(q[t, :W])
                    if ok.sum() < 2:
                        continue
                    D["fight"].append(f); D["step"].append(len(pre) + int(t))
                    D["opts"].append(np.where(ok, opts[t, :W], -1).astype(np.int16)); D["q"].append(np.where(ok, q[t, :W], np.nan).astype(np.float32))
                    if gumbel:
                        D["n"].append(np.where(ok, gn[t, :W], 0).astype(np.int16)); D["pi"].append(np.where(ok, gpi[t, :W], 0.0).astype(np.float32))
                        D["adv"].append(np.where(ok, gadv[t, :W], 0.0).astype(np.float32)); D["v"].append(float(gv[t]))
        # the engines hold every block's play-out combats (~4 GB per group at 2048 roots x 5x32): a loop variable still naming one kept it alive
        # through the next chunk's search
        eng = None
        fs._runs = []
        _save(f"{stem}_{k:03d}.npz", cs, F_, D, policy_only=bool(rs))
        lens = res[:, 4]
        top = np.argsort(-lens)[:3]
        rate = len(cs) / dt
        eta = (len(jobs) - (k + 1) * a.chunk) / max(rate, 1e-9) / 60
        print(f"chunk {k + 1}/{n_chunks}: {len(cs)} fights in {dt:.0f}s ({rate:.1f}/s), win {np.mean(res[:, 1] == 1):.3f}, {len(D['fight'])} decisions; "
              f"length mean {lens.mean():.0f} max {lens.max():.0f} ({', '.join(cs[i]['encounter'] for i in top)}); outcomes {dict(zip(*np.unique(res[:, 1], return_counts=True)))}; "
              f"ETA {eta:.0f} min", flush=True)
    print(f"done in {(time.time() - t_all) / 60:.1f} min -> {stem}_NNN.npz", flush=True)


POLICY_HEADS = {"u_card", "b_card", "v_tgt", "v_none", "u_pot", "b_pot", "disc_pot", "pick", "confirm", "end"}  # rl/model.py pointer heads


class Data:
    """One or more `collect` files; `chunks` yields replayed rows (obs, mask, opts, policy target, outcome class) a chunk of fights at a time."""

    def __init__(self, paths, tau, keep_mp=False, qnorm="minmax", qse=0.078, hard=False, obs_version=1):
        self.obs_version = obs_version  # rows are replayed as observations of this version (the trained network's)
        self.parts = []
        for p in paths:
            z = np.load(p)
            scen = json.loads(str(z["scenarios"]))
            q = z["d_q"].astype(np.float64)
            ok = np.isfinite(q)
            e = np.where(ok, np.exp((np.where(ok, q, -np.inf) - np.nanmax(q, 1, keepdims=True)) / tau), 0.0)
            tgt = (e / e.sum(1, keepdims=True)).astype(np.float32)
            lo, hi = np.nanmin(np.where(ok, q, np.nan), 1, keepdims=True), np.nanmax(np.where(ok, q, np.nan), 1, keepdims=True)
            qn = np.where(ok, (q - lo) / np.maximum(hi - lo, 1e-6), np.nan)  # each decision's tried options scaled to [0, 1] (Gumbel MuZero's normalisation)
            qn = np.where(ok, qn - np.nanmean(qn, 1, keepdims=True), 0.0).astype(np.float32)  # centred: untried actions keep their prior
            if qnorm == "abs":
                # the estimates in return units, centred: min-max stretches a noise-level gap to the full scale (most decisions are near-ties: the
                # best of 5 options repeats across search seeds in 66% of states, a gap beyond 2 se in 8.5%; tools/target_noise.py), units keep it small
                qn = np.where(ok, np.where(ok, q, 0.0) - np.nanmean(np.where(ok, q, np.nan), 1, keepdims=True), 0.0).astype(np.float32)
            elif qnorm == "cmpo":
                # Muesli's CMPO: the advantage over the tried options' mean in units of one estimate's noise (`qse`: se at 5x32), clipped to [-1, 1]
                qa = np.where(ok, q, 0.0) - np.nanmean(np.where(ok, q, np.nan), 1, keepdims=True)
                qn = np.where(ok, np.clip(qa / qse, -1.0, 1.0), 0.0).astype(np.float32)
            if hard:
                # the move the search played (imitation of the stronger player): the option equal to the recorded action, else the best estimate
                played = z["acts"][z["f_off"][z["d_fight"]] + z["d_step"]]
                h = (z["d_opts"] == played[:, None]) & ok
                none = ~h.any(1)
                h[none] = np.eye(q.shape[1], dtype=bool)[np.nanargmax(np.where(ok, q, -np.inf)[none], 1)] if none.any() else h[none]
                tgt = h.astype(np.float32)
            order = np.argsort(z["d_fight"], kind="stable")
            # format 2 (a Gumbel root): the shift of pi' per candidate; None for format 1 (`--target gumbel` needs it)
            adv = np.nan_to_num(z["d_adv"].astype(np.float32))[order] if "d_adv" in z.files else None
            self.parts.append(dict(scen=scen, f_scen=z["f_scen"], f_seed=z["f_seed"], f_cls=z["f_cls"], f_off=z["f_off"], acts=z["acts"].astype(np.int32),
                                   d_fight=z["d_fight"][order], d_step=z["d_step"][order], d_opts=z["d_opts"][order], tgt=tgt[order], qn=qn[order], adv=adv))
            self.parts[-1]["policy_only"] = bool(z["policy_only"]) if "policy_only" in z.files else False
            self.parts[-1]["d_lo"] = np.searchsorted(self.parts[-1]["d_fight"], np.arange(len(z["f_cls"]) + 1))
        # parts searched with different widths (3x8 vs 5x32) carry different option counts: pad to the widest, a padded option counts as not tried
        m = max(p["d_opts"].shape[1] for p in self.parts)
        for p in self.parts:
            k = m - p["d_opts"].shape[1]
            if k:
                p["d_opts"] = np.pad(p["d_opts"], ((0, 0), (0, k)), constant_values=-1)
                p["tgt"] = np.pad(p["tgt"], ((0, 0), (0, k)))
                p["qn"] = np.pad(p["qn"], ((0, 0), (0, k)))
                if p["adv"] is not None:
                    p["adv"] = np.pad(p["adv"], ((0, 0), (0, k)))
        # fights whose deck holds a multiplayer-only card are left out (single-player runs never offer those cards; data/catalog.json flags them)
        cat = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "catalog.json")))
        mp = {c["id"] for pool in cat["cards"].values() for c in pool if c.get("multiplayer_only")}
        has_mp = lambda sc: any((c if isinstance(c, str) else c["id"]) in mp for c in sc["deck"])  # noqa: E731
        self.index = [(pi, f) for pi, p in enumerate(self.parts) for bad in [[has_mp(sc) for sc in p["scen"]] if not keep_mp else None]
                      for f in range(len(p["f_cls"])) if keep_mp or not bad[p["f_scen"][f]]]

    def __len__(self):
        return len(self.index)

    def has_gumbel(self):
        """Every part carries the Gumbel improved policy (format 2)."""
        return all(p["adv"] is not None for p in self.parts)

    def rows(self, fights):
        """The training rows of these fights, replayed in parallel (`sts2.replay_rows`): obs, mask, options, soft target, outcome class, normalised
        estimates, Gumbel shift (zeros for format 1), outcome weight (0 for the rows of `policy_only` parts: restarts selected on a lost future)."""
        out = [[], [], [], [], [], [], [], []]
        by = {}
        for pi, f in fights:
            by.setdefault(pi, []).append(f)
        for pi, fs in by.items():
            p = self.parts[pi]
            fs = [f for f in fs if p["d_lo"][f] < p["d_lo"][f + 1]]
            if not fs:
                continue
            uniq, inv = np.unique(p["f_scen"][fs], return_inverse=True)
            acts, off, steps, soff, sel = [], [0], [], [0], []
            for f in fs:
                acts.append(p["acts"][p["f_off"][f]:p["f_off"][f + 1]]); off.append(off[-1] + len(acts[-1]))
                lo, hi = p["d_lo"][f], p["d_lo"][f + 1]
                steps.append(p["d_step"][lo:hi]); soff.append(soff[-1] + hi - lo); sel.append(np.arange(lo, hi))
            o, m = sts2.replay_rows([p["scen"][u] for u in uniq], inv, p["f_seed"][fs], np.concatenate(acts), off, np.concatenate(steps), soff,
                                   obs_version=self.obs_version)
            sel = np.concatenate(sel)
            out[0].append(o); out[1].append(m); out[2].append(p["d_opts"][sel]); out[3].append(p["tgt"][sel])
            out[4].append(np.repeat(p["f_cls"][fs], np.diff(soff)))
            out[5].append(p["qn"][sel])
            out[6].append(p["adv"][sel] if p["adv"] is not None else np.zeros_like(p["qn"][sel]))
            out[7].append(np.full(len(sel), 0.0 if p["policy_only"] else 1.0, np.float32))
        return [np.concatenate(x) for x in out]


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
    data = Data(a.data, a.tau, qnorm=a.qnorm, qse=a.qse, hard=a.target == "hard", obs_version=net.obs_version)
    idx = np.array(data.index, dtype=object)
    perm = rng.permutation(len(idx))
    n_hold = max(1, int(len(idx) * a.holdout))
    hold, tr = [tuple(x) for x in idx[perm[:n_hold]]], [tuple(x) for x in idx[perm[n_hold:]]]
    if a.freeze_policy:  # the value-only arm: the pointer heads keep the init network's weights (the shared trunk still trains)
        for k, prm in net.named_parameters():
            if k.split(".")[0] in POLICY_HEADS:
                prm.requires_grad_(False)
    if a.target == "gumbel" and not data.has_gumbel():
        raise SystemExit("--target gumbel needs parts collected with --root gumbel (format 2: d_adv)")
    prior = load(a.init).eval() if a.target in ("anchored", "gumbel") else None
    opt = torch.optim.AdamW([q for q in net.parameters() if q.requires_grad], lr=a.lr, weight_decay=1e-4)
    print(f"{len(data)} fights ({len(tr)} train, {len(hold)} holdout), init {a.init}, policy target {a.target}"
          f"{' (c=%g, %s)' % (a.c, a.qnorm) if a.target == 'anchored' else ''}{', policy heads frozen' if a.freeze_policy else ''}", flush=True)

    def batch_loss(o, m, op, tg, cl, qn, adv, ow):
        o, m = torch.from_numpy(o).to(DEV), torch.from_numpy(m.astype(np.int64)).to(DEV)
        op, tg, cl = torch.from_numpy(op.astype(np.int64)).to(DEV), torch.from_numpy(tg).to(DEV), torch.from_numpy(cl.astype(np.int64)).to(DEV)
        lg, _, ol = net(o, m, outcome=True)[:3]
        if prior is not None:
            # anchored target (Gumbel MuZero's improved policy): the init network's prior, shifted by c x the centred normalised search estimate
            # on the options the search tried; untried actions keep the prior. Where the estimates are within noise the prior's ranking survives.
            # gumbel: pi' = softmax(logits + sigma(completed Q)) = softmax(logits + adv), adv = sigma(q) - sigma(v) on the candidates, 0 elsewhere
            with torch.no_grad():
                pl0 = prior(o, m, value=False)[0].float()
                sh = a.c * torch.from_numpy(qn).to(DEV) if a.target == "anchored" else torch.from_numpy(adv).to(DEV)
                shift = torch.zeros_like(pl0).scatter_(1, op.clamp(min=0), torch.where(op >= 0, sh, 0.0))
                t = torch.softmax(pl0 + shift, 1)
            pl = -(t * F.log_softmax(lg.float(), 1).clamp(min=-30)).sum(1).mean()
        else:
            lp = F.log_softmax(lg.float(), 1).gather(1, op.clamp(min=0))
            pl = -(tg * torch.where(op >= 0, lp, torch.zeros_like(lp))).sum(1).mean()
        ow = torch.from_numpy(ow).to(DEV)
        vl = (-(hl_gauss(cl, a.sigma) * F.log_softmax(ol.float(), 1)).sum(1) * ow).sum() / ow.sum().clamp(min=1.0)
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
                loss = a.pol * pl + a.vw * vl
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
        torch.save({"net": net.state_dict(), "args": ck.get("args", {}) | {"obs_version": net.obs_version}, "exit": vars(a) | {"epoch": ep}}, a.out)
    print(f"-> {a.out}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("collect")
    c.add_argument("--ckpt", required=True); c.add_argument("--fights", nargs="+"); c.add_argument("--out", required=True)
    c.add_argument("--M", type=int, default=3); c.add_argument("--K", type=int, default=8); c.add_argument("--attempts", type=int, default=2)
    c.add_argument("--roots", type=int, default=2048); c.add_argument("--seed", type=int, default=101)
    c.add_argument("--chunk", type=int, default=2048, help="fights per saved part")
    c.add_argument("--chunks-per-process", type=int, default=3, help="chunks before exiting with code 3 for a fresh process: a long-lived search process "
                   "slows down chunk after chunk (round 2: chunk 9 took 3x the median; the same fights in a fresh process ran at full speed)")
    c.add_argument("--look-legacy", action="store_true", help="the enemy look-ahead from before S1 (per-monster pattern walk)")
    c.add_argument("--root", choices=["topm", "gumbel"], default="topm", help="search root: top-M x K, or Gumbel candidates + sequential halving (records pi')")
    c.add_argument("--gumbel-m", type=int, default=16, help="--root gumbel: candidates sampled per decision")
    c.add_argument("--gumbel-n", type=int, default=160, help="--root gumbel: futures per decision")
    c.add_argument("--max-minutes", type=float, default=120, help="no new chunk starts after this")
    c.add_argument("--restarts", help="search from the restart states of `tools/nearmiss.py` (true states inside near-miss losses) instead of --fights; "
                   "parts are marked policy_only")
    c.add_argument("--chunk-timeout", type=float, default=5.0, help="watchdog: a chunk longer than this x the median chunk ends the process")
    c.add_argument("--first-timeout", type=float, default=30.0, help="watchdog limit in minutes for the first two chunks")
    t = sub.add_parser("train")
    t.add_argument("--init", required=True); t.add_argument("--data", nargs="+", required=True); t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=4); t.add_argument("--chunk", type=int, default=2048); t.add_argument("--mb", type=int, default=2048)
    t.add_argument("--lr", type=float, default=1e-4); t.add_argument("--lr-floor", type=float, default=0.1)
    t.add_argument("--tau", type=float, default=0.02, help="temperature over the options' search estimates (linear return units)")
    t.add_argument("--sigma", type=float, default=0.75, help="HL-Gauss width of the win classes, in bins")
    t.add_argument("--pol", type=float, default=1.0, help="weight of the policy loss")
    t.add_argument("--target", choices=["soft", "anchored", "gumbel", "hard"], default="anchored",
                   help="policy target: soft = softmax(q / tau) over the tried options (made the player worse, E9); anchored = prior + c x normalised q; "
                        "gumbel = Gumbel MuZero's pi' recorded by a --root gumbel collection; hard = the move the search played (one-hot)")
    t.add_argument("--c", type=float, default=2.0, help="anchored target: weight of the normalised search estimate (logits per unit of --qnorm)")
    t.add_argument("--qnorm", choices=["minmax", "abs", "cmpo"], default="minmax", help="anchored target: each decision's estimates scaled to [0, 1] (minmax) or "
                   "in return units (abs: a near-tie shifts the prior by almost nothing; c 4 turns a 2-se gap at 5x32, ~0.22, into ~0.9 logits)")
    t.add_argument("--qse", type=float, default=0.078, help="--qnorm cmpo: noise se of one option's estimate (0.078 at 5x32, tools/target_noise.py)")
    t.add_argument("--vw", type=float, default=1.0, help="weight of the outcome loss (0: policy only, no interference through the shared trunk)")
    t.add_argument("--freeze-policy", action="store_true", help="train the value side only (policy heads frozen)")
    t.add_argument("--holdout", type=float, default=0.05); t.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()
    collect(a) if a.cmd == "collect" else train(a)


if __name__ == "__main__":
    main()
