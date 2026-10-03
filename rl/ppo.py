#!/usr/bin/env python3
"""PPO for the STS2 combat environment (CPU).

  .venv/bin/python rl/ppo.py --train target/train/train.json --eval target/train/eval.json --out target/runs/a --iters 500

Reward: +1 for a win (+ `--hp-bonus` x HP fraction left), -1 for a loss or for stalling past `--max-steps`; aborted episodes
(unported content / capacity overflow) end with reward 0 and are counted separately.
"""
import argparse, json, os, sys, time
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import Net, n_params


def make_env(path, n, seed, max_steps, hp_bonus):
    scen = json.load(open(path))
    return sts2.VecEnv(n, scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=hp_bonus), scen


@torch.no_grad()
def evaluate(net, path, n_envs, per_env, seed, max_steps, hp_bonus, greedy=True):
    """First `per_env` finished episodes of every env (no short-fight bias), greedy policy. Returns a stats dict."""
    env, scen = make_env(path, n_envs, seed, max_steps, hp_bonus)
    obs, mask = env.reset()
    got = np.zeros(n_envs, np.int32)
    rec = []
    net.eval()
    while got.min() < per_env:
        o = torch.from_numpy(obs.copy())
        m = torch.from_numpy(mask.astype(np.int64))
        lg, _ = net(o, m)
        a = lg.argmax(1) if greedy else torch.distributions.Categorical(logits=lg).sample()
        obs, mask, r, d, info = env.step(a.numpy().astype(np.int32))
        idx = np.nonzero(d)[0]
        if len(idx):
            ei = env.episode_info()
            for i in idx:
                if got[i] < per_env:
                    got[i] += 1
                    rec.append((int(ei["scenario"][i]), int(info["outcome"][i]), float(ei["hp_lost"][i]), int(ei["length"][i])))
    net.train()
    return summarize(rec, scen)


def summarize(rec, scen):
    rec = np.array(rec, dtype=np.float64)
    out = rec[:, 1]
    valid = (out == 1) | (out == -1) | (out == 2)
    win = out == 1
    res = {"episodes": int(len(rec)), "win": float(win[valid].mean()) if valid.any() else 0.0,
           "hp_lost_on_win": float(rec[win, 2].mean()) if win.any() else None,
           "stall": float((out == 2).mean()), "aborted": float(((out == 3) | (out == 4)).mean()), "len": float(rec[:, 3].mean())}
    by = {}
    for key in ("character", "act"):
        g = {}
        for k in sorted({str(s.get(key)) for s in scen}):
            sel = np.array([str(scen[int(i)].get(key)) == k for i in rec[:, 0]]) & valid
            if sel.any():
                g[k] = round(float((out[sel] == 1).mean()), 3)
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
    ap.add_argument("--layers", type=int, default=1)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--resume")
    ap.add_argument("--threads", type=int, default=12)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    torch.set_num_threads(a.threads)
    torch.manual_seed(a.seed)
    net = Net(d=a.d, layers=a.layers)
    if a.resume:
        net.load_state_dict(torch.load(a.resume))
    print("params", n_params(net), flush=True)
    opt = torch.optim.Adam(net.parameters(), lr=a.lr, eps=1e-5)
    env, scen = make_env(a.train, a.envs, a.seed + 1000, a.max_steps, a.hp_bonus)
    N, T = a.envs, a.horizon
    A = sts2.ACTIONS
    b_obs = torch.zeros(T, N, sts2.OBS_SIZE)
    b_mask = torch.zeros(T, N, A, dtype=torch.uint8)
    b_act = torch.zeros(T, N, dtype=torch.long)
    b_lp = torch.zeros(T, N)
    b_val = torch.zeros(T + 1, N)
    b_rew = torch.zeros(T, N)
    b_done = torch.zeros(T, N)
    obs, mask = env.reset()
    log = open(os.path.join(a.out, "log.jsonl"), "a")
    t0 = time.time()
    steps = 0
    ep_stats = []
    for it in range(1, a.iters + 1):
        lr = a.lr * max(0.05, 1 - (it - 1) / a.iters)
        for g in opt.param_groups:
            g["lr"] = lr
        # ---- rollout ----
        net.eval()
        t_roll = time.time()
        with torch.inference_mode():
            for t in range(T):
                b_obs[t].numpy()[:] = obs
                b_mask[t].numpy()[:] = mask
                lg, v = net(b_obs[t], b_mask[t].long())
                logp = F.log_softmax(lg, 1)
                act = torch.multinomial(logp.exp(), 1).squeeze(1)
                b_act[t] = act
                b_lp[t] = logp.gather(1, act[:, None]).squeeze(1)
                b_val[t] = v
                obs, mask, rew, done, info = env.step(act.numpy().astype(np.int32))
                r = rew.copy()
                oc = info["outcome"]
                r[oc == 2] = -1.0  # stalled out
                b_rew[t] = torch.from_numpy(r)
                b_done[t] = torch.from_numpy(done.astype(np.float32))
                if done.any():
                    ei = env.episode_info()
                    for i in np.nonzero(done)[0]:
                        ep_stats.append((int(oc[i]), float(ei["hp_lost"][i]), int(ei["length"][i])))
                steps += N
            _, last_v = net(torch.from_numpy(obs.copy()), torch.from_numpy(mask.astype(np.int64)))
            b_val[T] = last_v
        t_roll = time.time() - t_roll
        # ---- GAE ----
        adv = torch.zeros(T, N)
        last = torch.zeros(N)
        for t in reversed(range(T)):
            nd = 1.0 - b_done[t]
            delta = b_rew[t] + a.gamma * b_val[t + 1] * nd - b_val[t]
            last = delta + a.gamma * a.lam * nd * last
            adv[t] = last
        ret = adv + b_val[:T]
        # ---- update ----
        net.train()
        t_upd = time.time()
        fo, fm = b_obs.view(T * N, -1), b_mask.view(T * N, -1)
        fa, flp, fadv, fret = b_act.view(-1), b_lp.view(-1), adv.view(-1), ret.view(-1)
        stats = {"pl": 0.0, "vl": 0.0, "ent": 0.0, "kl": 0.0, "clip": 0.0}
        nb = 0
        for ep in range(a.epochs):
            perm = torch.randperm(T * N)
            for s in range(0, T * N, a.mb):
                ix = perm[s:s + a.mb]
                lg, v = net(fo[ix], fm[ix].long())
                logp = F.log_softmax(lg, 1)
                nlp = logp.gather(1, fa[ix, None]).squeeze(1)
                ratio = (nlp - flp[ix]).exp()
                ad = fadv[ix]
                ad = (ad - ad.mean()) / (ad.std() + 1e-8)
                pl = -torch.min(ratio * ad, ratio.clamp(1 - a.clip, 1 + a.clip) * ad).mean()
                vl = 0.5 * (v - fret[ix]).pow(2).mean()
                p = logp.exp()
                ent = -(p * logp.clamp(min=-30) * (fm[ix] > 0)).sum(1).mean()
                loss = pl + a.vf * vl - a.ent * ent
                opt.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), 0.5)
                opt.step()
                stats["pl"] += pl.item(); stats["vl"] += vl.item(); stats["ent"] += ent.item()
                stats["kl"] += ((ratio - 1) - (nlp - flp[ix])).mean().item()
                stats["clip"] += ((ratio - 1).abs() > a.clip).float().mean().item()
                nb += 1
        t_upd = time.time() - t_upd
        rec = {"it": it, "steps": steps, "sps": int(steps / (time.time() - t0)), "t_roll": round(t_roll, 1), "t_upd": round(t_upd, 1), "lr": lr}
        rec.update({k: round(v / nb, 4) for k, v in stats.items()})
        if ep_stats:
            e = np.array(ep_stats)
            wins = e[:, 0] == 1
            rec.update(win=round(float(wins.mean()), 3), hp_lost=round(float(e[wins, 1].mean()), 3) if wins.any() else None,
                       ep_len=round(float(e[:, 2].mean()), 1), eps=len(e), aborted=int(((e[:, 0] == 3) | (e[:, 0] == 4)).sum()))
            ep_stats = []
        if it % a.eval_every == 0 or it == a.iters:
            ev = evaluate(net, a.eval, a.eval_envs, a.eval_per_env, 777, a.max_steps, a.hp_bonus)
            rec["eval"] = ev
            torch.save(net.state_dict(), os.path.join(a.out, "ckpt.pt"))
            torch.save(net.state_dict(), os.path.join(a.out, f"ckpt_{it}.pt"))
        print(json.dumps(rec), flush=True)
        log.write(json.dumps(rec) + "\n"); log.flush()


if __name__ == "__main__":
    main()
