#!/usr/bin/env python3
import argparse, json, os, sys, time
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads as H
from model import Net, n_params, DEV, load_weights, HostShape, net_policy


def make_env(path, n, seed, max_steps, hp_bonus, turn_cap=H.TURN_CAP):
    scen = json.load(open(path))
    return sts2.VecEnv(n, scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=hp_bonus, turn_cap=turn_cap), scen


def rollout_graph(net, n, kw):
    """The rollout forward as one CUDA graph at the observation's full shapes (all enemy and pile slots, every row a decision row)."""
    so = torch.zeros(n, sts2.OBS_SIZE, device=DEV)
    sm = torch.zeros(n, sts2.ACTIONS, dtype=torch.uint8, device=DEV)
    sm[:, 0] = 1
    shp = dict(E=net.C["OBS_MAX_ENEMIES"], L=max(net.SEC[k][1] // 2 for k in ("draw", "discard", "exhaust")), has_dec=True)
    st = torch.cuda.Stream()
    st.wait_stream(torch.cuda.current_stream())
    with torch.cuda.stream(st), torch.no_grad():
        for _ in range(2):
            net(so, sm, **kw, **shp)
    torch.cuda.current_stream().wait_stream(st)
    g = torch.cuda.CUDAGraph()
    with torch.no_grad(), torch.cuda.graph(g):
        out = net(so, sm, **kw, **shp)

    def run(o, m):
        so.copy_(o)
        sm.copy_(m)
        g.replay()
        return out
    return run


def evaluate(policy, path, n_envs, per_env, seed, max_steps, hp_bonus, turn_cap=H.TURN_CAP):
    env, scen = make_env(path, n_envs, seed, max_steps, hp_bonus, turn_cap)
    obs, mask = env.reset()
    got = np.zeros(n_envs, np.int32)
    rec = []
    while got.min() < per_env:
        obs, mask, r, d, info = env.step(policy(obs, mask))
        idx = np.nonzero(d)[0]
        if len(idx):
            ei = env.episode_info()
            for i in idx:
                if got[i] < per_env:
                    got[i] += 1
                    rec.append((int(ei["scenario"][i]), int(info["outcome"][i]), float(ei["hp_lost"][i]), int(ei["length"][i]), float(ei["hp_end"][i])))
    return summarize(rec, scen)


def summarize(rec, scen):
    rec = np.array(rec, dtype=np.float64)
    out = rec[:, 1]
    valid = (out == 1) | (out == -1) | (out == 2)
    win = out == 1
    res = {"episodes": int(len(rec)), "win": float(win[valid].mean()) if valid.any() else 0.0,
           "hp_lost_all": float(rec[valid, 2].mean()) if valid.any() else None,
           "hp_lost_on_win": float(rec[win, 2].mean()) if win.any() else None,
           "stall": float((out == 2).mean()), "aborted": float(((out == 3) | (out == 4)).mean()), "len": float(rec[:, 3].mean())}
    by = {}
    for key in ("character", "act"):
        g = {}
        for k in sorted({str(s.get(key)) for s in scen}):
            sel = np.array([str(scen[int(i)].get(key)) == k for i in rec[:, 0]]) & valid
            if sel.any():
                g[k] = [round(float((out[sel] == 1).mean()), 3), round(float(rec[sel, 2].mean()), 3)]
        by[key] = g
    res["by"] = by
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--train", required=True)
    ap.add_argument("--eval", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--envs", type=int, default=1024)
    ap.add_argument("--horizon", type=int, default=48)
    ap.add_argument("--iters", type=int, default=500)
    ap.add_argument("--epochs", type=int, default=2)
    ap.add_argument("--mb", type=int, default=4096)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--gamma", type=float, default=0.999)
    ap.add_argument("--lam", type=float, default=0.95)
    ap.add_argument("--clip", type=float, default=0.2)
    ap.add_argument("--ent", type=float, default=0.01)
    ap.add_argument("--vf", type=float, default=0.5)
    ap.add_argument("--hp-bonus", type=float, default=0.5)
    ap.add_argument("--max-steps", type=int, default=600)
    ap.add_argument("--eval-every", type=int, default=25)
    ap.add_argument("--eval-envs", type=int, default=1024)
    ap.add_argument("--eval-per-env", type=int, default=2)
    ap.add_argument("--d", type=int, default=64)
    ap.add_argument("--rounds", type=int, default=2)
    ap.add_argument("--hold-prob", type=float, default=0.0, help="fraction of episodes that run under a random 'no potion before turn T' rule (T in 2..5, or never): states that hold a resource then show up in the data")
    ap.add_argument("--heads", action="store_true", help="value = expected worth of the fight-outcome head (rl/heads.py) instead of the scalar value head")
    ap.add_argument("--head-warmup", type=int, default=0, help="with --heads: first train the outcome head alone for this many iterations (policy and trunk frozen)")
    ap.add_argument("--lam-head", type=float, default=0.95, help="lambda of the outcome head's targets (1 = Monte Carlo endings, 0 = next state's prediction)")
    ap.add_argument("--turn-cap", type=int, default=H.TURN_CAP, help="a fight still running after this many player turns is a loss (0 = no cap)")
    ap.add_argument("--warm-prefix", default="outcome.", help="comma-separated parameter-name prefixes the --head-warmup iterations train (the rest frozen)")
    ap.add_argument("--adaptive", type=int, default=25, help="every N iterations reweight the training fights by p(1 - p), p the policy's win rate per fight "
                    "shrunk toward its (encounter, act, character) group, plus --adaptive-anchor of the draws uniform")
    ap.add_argument("--adaptive-anchor", type=float, default=0.15, help="share of the draws spread uniformly over every fight")
    ap.add_argument("--adaptive-decay", type=float, default=0.8, help="per reweight, the per-fight counts are multiplied by this (< 1: recent "
                    "episodes count more, so the estimates follow the improving policy)")
    ap.add_argument("--lr-floor", type=float, default=0.05, help="the lr decays linearly to this fraction of --lr")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--resume", help="checkpoint to continue from (iteration count and lr schedule continue; --iters is the total)")
    ap.add_argument("--warm", action="store_true", help="with --resume: take the weights only (fresh optimizer, iteration 0)")
    ap.add_argument("--threads", type=int, default=12)
    ap.add_argument("--graph-rollout", action=argparse.BooleanOptionalAction, default=True, help="CUDA: the rollout forward as one CUDA graph at full shapes")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    torch.set_num_threads(a.threads)
    torch.manual_seed(a.seed)
    net = Net(d=a.d, rounds=a.rounds, heads=a.heads).to(DEV)
    opt = torch.optim.Adam(net.parameters(), lr=a.lr, eps=1e-5)
    allow = ("outcome.",) if a.heads else ()
    it0, steps = 0, 0
    if a.resume:
        ck = torch.load(a.resume, map_location="cpu")
        if "net" in ck:
            load_weights(net, ck["net"], allow)
            if not a.warm:
                opt.load_state_dict(ck["opt"])
                it0, steps = ck["it"], ck["steps"]
        else:
            load_weights(net, ck, allow)
    print("params", n_params(net), "resuming at iteration", it0, flush=True)
    env, scen = make_env(a.train, a.envs, a.seed + 1000, a.max_steps, a.hp_bonus, a.turn_cap)
    adapt_rec = None
    if a.adaptive:
        gkey = {}
        grp = np.array([gkey.setdefault((s.get("encounter"), s.get("act"), s.get("character")), len(gkey)) for s in scen])
        s_n, s_w = np.zeros(len(scen)), np.zeros(len(scen))
    prefixes = tuple(p for p in a.warm_prefix.split(",") if p)
    warm_params = [p for n, p in net.named_parameters() if n.startswith(prefixes)]
    opt_w = torch.optim.Adam(warm_params, lr=a.lr, eps=1e-5) if a.heads and a.head_warmup > 0 else None
    if opt_w is not None and not warm_params:
        raise SystemExit(f"--warm-prefix {a.warm_prefix}: no parameters")
    it_warm = it0 + (a.head_warmup if opt_w is not None else 0)
    N, T = a.envs, a.horizon
    A = sts2.ACTIONS
    b_obs = torch.zeros(T, N, sts2.OBS_SIZE, device=DEV)
    b_mask = torch.zeros(T, N, A, dtype=torch.uint8, device=DEV)
    b_act = torch.zeros(T, N, dtype=torch.long, device=DEV)
    b_lp = torch.zeros(T, N, device=DEV)
    b_val_d = torch.zeros(T + 1, N, device=DEV)
    b_rew = torch.zeros(T, N)
    b_done = torch.zeros(T, N)
    if a.heads:
        b_pout_d = torch.zeros(T + 1, N, H.NC, device=DEV)
        b_term = torch.full((T, N), -1, dtype=torch.long)
    hs = HostShape()
    h_e = np.zeros((T, N), np.int16)
    h_l = np.zeros((T, N, 3), np.int16)
    h_d = np.zeros((T, N), bool)
    obs, mask = env.reset()
    if DEV.type == "cuda":
        for arr in (env.obs, env.mask):
            err = torch.cuda.cudart().cudaHostRegister(arr.ctypes.data, arr.nbytes, 0)
            if int(err) != 0:
                raise SystemExit(f"cudaHostRegister failed ({arr.nbytes} bytes): {err}")
    kw = dict(outcome=a.heads)
    graph = rollout_graph(net, N, kw) if a.graph_rollout and DEV.type == "cuda" else None

    def roll_out(t, o, m, rows):
        out = graph(o, m) if graph else net(o, m, **kw, **hs.of(*rows, DEV))
        if a.heads:
            b_pout_d[t] = torch.softmax(out[2], 1)
        return out[0], out[1]
    rng = np.random.default_rng(a.seed + 7)
    POT = slice(net.C["OFF_POTION"], net.C["OFF_DISCARD"])
    hold_until = np.zeros(N, np.int32)
    def draw_rules(idx):
        use = rng.random(len(idx)) < a.hold_prob
        t = rng.choice([2, 3, 4, 5, 99], size=len(idx))
        hold_until[idx] = np.where(use, t, 0)
    draw_rules(np.arange(N))
    log = open(os.path.join(a.out, "log.jsonl"), "a")
    t0 = time.time()
    steps0 = steps
    ep_stats = []
    for it in range(it0 + 1, a.iters + 1 + (it_warm - it0)):
        warm = it <= it_warm
        lr = a.lr * max(a.lr_floor, 1 - (max(it - (it_warm - it0), 1) - 1) / max(a.iters, 1))
        if a.adaptive and it % a.adaptive == 0 and s_n.sum() > 0:
            g_n, g_w = np.bincount(grp, s_n, len(gkey)), np.bincount(grp, s_w, len(gkey))
            pg = (g_w + 1) / (g_n + 2)
            ps = (s_w + 4 * pg[grp]) / (s_n + 4)
            v = ps * (1 - ps)
            wts = (1 - a.adaptive_anchor) * v / v.sum() + a.adaptive_anchor / len(v)
            env.set_weights(wts)
            q = wts / wts.sum()
            adapt_rec = dict(share_mid=round(float(((ps >= 0.2) & (ps <= 0.8)).mean()), 3), seen=round(float((s_n > 0).mean()), 3),
                             sat=round(float((ps > 0.95).mean()), 3), lost=round(float((ps < 0.05).mean()), 3),
                             drawn_pq=round(float((q * ps * (1 - ps)).sum()), 4), uniform_pq=round(float((ps * (1 - ps)).mean()), 4))
            s_n *= a.adaptive_decay
            s_w *= a.adaptive_decay
        for g in (opt_w if warm else opt).param_groups:
            g["lr"] = lr
        net.eval()
        t_roll = time.time()
        t_env = t_net = 0.0
        with torch.inference_mode():
            for t in range(T):
                t_a = time.perf_counter()
                m_eff = mask
                if a.hold_prob > 0:
                    m_eff = mask.copy()
                    bad = (hold_until > 0) & (obs[:, 1] < hold_until)
                    m_eff[bad, POT] = 0
                    empty = m_eff.sum(1) == 0
                    m_eff[empty] = mask[empty]
                b_obs[t].copy_(torch.from_numpy(obs), non_blocking=True)
                b_mask[t].copy_(torch.from_numpy(m_eff), non_blocking=m_eff is mask)
                h_e[t], h_l[t], h_d[t] = hs.rows_info(obs)
                lg, v = roll_out(t, b_obs[t], b_mask[t], (h_e[t], h_l[t], h_d[t]))
                logp = F.log_softmax(lg, 1)
                act = torch.multinomial(logp.exp(), 1).squeeze(1)
                b_act[t] = act
                b_lp[t] = logp.gather(1, act[:, None]).squeeze(1)
                b_val_d[t] = v
                act_h = act.cpu().numpy().astype(np.int32)
                t_b = time.perf_counter()
                obs, mask, rew, done, info = env.step(act_h)
                t_c = time.perf_counter()
                t_net += t_b - t_a
                t_env += t_c - t_b
                r = rew.copy()
                oc = info["outcome"]
                r[oc == 2] = -1.0
                if done.any():
                    ei = env.episode_info()
                b_rew[t] = torch.from_numpy(r)
                b_done[t] = torch.from_numpy(done.astype(np.float32))
                if a.heads:
                    b_term[t] = -1
                    if done.any():
                        cls = H.end_class(oc == 1, ei["hp_end_abs"])
                        cls = np.where((oc == 3) | (oc == 4), -2, cls)
                        b_term[t] = torch.from_numpy(np.where(done, cls, -1).astype(np.int64))
                if done.any():
                    if a.hold_prob > 0:
                        draw_rules(np.nonzero(done)[0])
                    for i in np.nonzero(done)[0]:
                        ep_stats.append((int(oc[i]), float(ei["hp_lost"][i]), int(ei["length"][i])))
                        if a.adaptive and oc[i] in (1, -1, 2):
                            si = int(ei["scenario"][i])
                            s_n[si] += 1
                            s_w[si] += oc[i] == 1
                steps += N
            _, b_val_d[T] = roll_out(T, torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask).to(DEV), hs.rows_info(obs))
            b_val = b_val_d.cpu()
            if a.heads:
                b_pout = b_pout_d.cpu()
        t_roll = time.time() - t_roll
        adv = torch.zeros(T, N)
        last = torch.zeros(N)
        for t in reversed(range(T)):
            nd = 1.0 - b_done[t]
            delta = b_rew[t] + a.gamma * b_val[t + 1] * nd - b_val[t]
            last = delta + a.gamma * a.lam * nd * last
            adv[t] = last
        ret = adv + b_val[:T]
        if a.heads:
            tgt = torch.zeros(T, N, H.NC)
            wt = torch.ones(T, N)
            y = b_pout[T].clone()
            eye = torch.eye(H.NC)
            for t in reversed(range(T)):
                term = b_term[t]
                boot = (1 - a.lam_head) * b_pout[t + 1] + a.lam_head * y
                ended = term >= 0
                aborted = term == -2
                y = torch.where(ended.unsqueeze(1), eye[term.clamp(min=0)], boot)
                y = torch.where(aborted.unsqueeze(1), b_pout[t], y)
                wt[t] = (~aborted).float()
                tgt[t] = y
        net.train()
        t_upd = time.time()
        fo, fm = b_obs.view(T * N, -1), b_mask.view(T * N, -1)
        fa, flp, fadv, fret = b_act.view(-1), b_lp.view(-1), adv.view(-1).to(DEV), ret.view(-1).to(DEV)
        if a.heads:
            ftgt, fwt = tgt.view(T * N, -1).to(DEV), wt.view(-1).to(DEV)
        zero = lambda: torch.zeros((), dtype=torch.float64, device=DEV)  # noqa: E731
        stats = {"pl": zero(), "vl": zero(), "ent": zero(), "kl": zero(), "clip": zero()}
        fe, fl, fd = h_e.reshape(-1), h_l.reshape(-1, 3), h_d.reshape(-1)
        nb = 0
        for ep in range(a.epochs):
            perm = torch.randperm(T * N)
            perm_d = perm.to(DEV)
            perm_h = perm.numpy()
            for s in range(0, T * N, a.mb):
                ix = perm_d[s:s + a.mb]
                ih = perm_h[s:s + a.mb]
                shp = hs.of(fe[ih], fl[ih], fd[ih], DEV)
                mk = fm[ix]
                out = net(fo[ix], mk, **kw, **shp)
                lg, v = out[:2]
                if a.heads:
                    ol = out[2]
                logp = F.log_softmax(lg, 1)
                nlp = logp.gather(1, fa[ix, None]).squeeze(1)
                ratio = (nlp - flp[ix]).exp()
                ad = fadv[ix]
                ad = (ad - ad.mean()) / (ad.std() + 1e-8)
                pl = -torch.min(ratio * ad, ratio.clamp(1 - a.clip, 1 + a.clip) * ad).mean()
                if a.heads:
                    w_ = fwt[ix]
                    vl = (-(ftgt[ix] * F.log_softmax(ol, 1)).sum(1) * w_).sum() / w_.sum().clamp(min=1)
                else:
                    vl = F.smooth_l1_loss(v, fret[ix])
                p = logp.exp()
                ent = -(p * logp.clamp(min=-30) * (mk > 0)).sum(1).mean()
                o_ = opt_w if warm else opt
                loss = a.vf * vl if warm else pl + a.vf * vl - a.ent * ent
                o_.zero_grad(set_to_none=True)
                net.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_([q for g in o_.param_groups for q in g["params"]], 0.5)
                o_.step()
                with torch.no_grad():
                    stats["pl"] += pl.detach(); stats["vl"] += vl.detach(); stats["ent"] += ent.detach()
                    stats["kl"] += ((ratio - 1) - (nlp - flp[ix])).mean()
                    stats["clip"] += ((ratio - 1).abs() > a.clip).float().mean()
                nb += 1
        stats = {k: float(v) for k, v in stats.items()}
        t_upd = time.time() - t_upd
        rec = {"it": it, "warm": warm, "steps": steps, "sps": int((steps - steps0) / (time.time() - t0)), "t_roll": round(t_roll, 2), "t_env": round(t_env, 2), "t_net": round(t_net, 2), "t_upd": round(t_upd, 2), "lr": lr}
        rec.update({k: round(v / nb, 4) for k, v in stats.items()})
        if adapt_rec:
            rec["adaptive"] = adapt_rec
        if ep_stats:
            e = np.array(ep_stats)
            wins = e[:, 0] == 1
            rec.update(win=round(float(wins.mean()), 3), hp_lost=round(float(e[wins, 1].mean()), 3) if wins.any() else None,
                       ep_len=round(float(e[:, 2].mean()), 1), eps=len(e), aborted=int(((e[:, 0] == 3) | (e[:, 0] == 4)).sum()))
            ep_stats = []
        last_it = a.iters + (it_warm - it0)
        if it % a.eval_every == 0 or it == last_it:
            net.eval()
            ev = evaluate(net_policy(net), a.eval, a.eval_envs, a.eval_per_env, 777, a.max_steps, a.hp_bonus, a.turn_cap)
            net.train()
            rec["eval"] = ev
            ck = {"net": net.state_dict(), "opt": opt.state_dict(), "it": it, "steps": steps, "args": vars(a)}
            torch.save(ck, os.path.join(a.out, "ckpt.pt"))
            if it % (4 * a.eval_every) == 0 or it == last_it:
                torch.save(ck, os.path.join(a.out, f"ckpt_{it}.pt"))
        print(json.dumps(rec), flush=True)
        log.write(json.dumps(rec) + "\n"); log.flush()


if __name__ == "__main__":
    main()
