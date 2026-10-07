#!/usr/bin/env python3
"""PPO for the STS2 combat environment (CPU).

  .venv/bin/python rl/ppo.py --train target/train/train.json --eval target/train/eval.json --out target/runs/a --iters 500

Reward: +1 for a win (+ `--hp-bonus` x HP fraction left), -1 for a loss, for stalling past `--max-steps` or for still fighting after `--turn-cap`
player turns; aborted episodes (unported content / capacity overflow) end with reward 0 and are counted separately.

`--heads` (`docs/rl_redesign.md` M1): the value is the expected worth of the fight-outcome distribution (`rl/heads.py`), trained by cross-entropy on
lambda-returns of the distribution itself (`--lam-head`); `--head-warmup K` first trains that head alone for K iterations (policy and trunk frozen).
`--pot-head` (M1b): the potion-use head, P(the potion in belt slot k is used before the fight ends), binary cross-entropy against lambda-mixed targets
(1 when the env reports the slot's potion used at this step, 0 when the fight ends with it, else the next state's prediction); empty slots masked.
`--warm-prefix pot_use.` with `--head-warmup K` trains that head alone on a frozen network.
"""
import argparse, json, os, sys, time
import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import utility
import heads as H
from model import Net, n_params, DEV, load_weights


def make_env(path, n, seed, max_steps, hp_bonus, turn_cap=H.TURN_CAP):
    scen = json.load(open(path))
    return sts2.VecEnv(n, scen, seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=hp_bonus, turn_cap=turn_cap), scen


def net_policy(net, greedy=True):
    @torch.no_grad()
    def act(obs, mask):
        lg, _ = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV))
        a = lg.argmax(1) if greedy else torch.distributions.Categorical(logits=lg).sample()
        return a.cpu().numpy().astype(np.int32)
    return act


def evaluate(policy, path, n_envs, per_env, seed, max_steps, hp_bonus, turn_cap=H.TURN_CAP):
    """First `per_env` finished episodes of every env (no short-fight bias). `policy(obs, mask) -> int32 actions`. Returns a stats dict."""
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
    """`hp_lost_all` (primary with `win`): mean fraction of max HP lost over every valid fight; a loss (or a stall) is charged all the HP the
    player had at the start, which is what a run-level optimizer pays for it."""
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
    res["by"] = by  # [win rate, mean HP lost]
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
    ap.add_argument("--util-prob", type=float, default=0.0, help="share of episodes whose win reward follows a random HP-worth curve (rl/utility.py) instead of the linear return; the network reads the curve as an input")
    ap.add_argument("--heads", action="store_true", help="value = expected worth of the fight-outcome head (rl/heads.py) instead of the scalar value head")
    ap.add_argument("--head-warmup", type=int, default=0, help="with --heads: first train the outcome head alone for this many iterations (policy and trunk frozen)")
    ap.add_argument("--lam-head", type=float, default=0.95, help="lambda of the outcome head's targets (1 = Monte Carlo endings, 0 = next state's prediction)")
    ap.add_argument("--turn-cap", type=int, default=H.TURN_CAP, help="a fight still running after this many player turns is a loss (0 = no cap)")
    ap.add_argument("--pot-head", action="store_true", help="with --heads: the potion-use head (per belt slot)")
    ap.add_argument("--pot-coef", type=float, default=0.5, help="weight of the potion-use head's loss")
    ap.add_argument("--warm-prefix", default="outcome.", help="comma-separated parameter-name prefixes the --head-warmup iterations train (the rest frozen)")
    ap.add_argument("--adaptive", type=int, default=0, help="every N iterations reweight the training fights (M3): fights the policy wins 20-80 %% weigh 1, the "
                    "others --adaptive-floor; win estimated per fight, shrunk toward its (encounter, act, character) group")
    ap.add_argument("--adaptive-floor", type=float, default=0.3)
    ap.add_argument("--adaptive-mode", choices=["band", "signal"], default="band",
                    help="band: the 20-80 %% rule above; signal: weight p(1 - p) (the variance of the fight's outcome: saturated and hopeless fights fade, "
                    "contested ones dominate) plus --adaptive-anchor of the draws uniform (docs/rebuild.md S3, curriculum by signal)")
    ap.add_argument("--adaptive-anchor", type=float, default=0.15, help="signal mode: share of the draws spread uniformly over every fight")
    ap.add_argument("--adaptive-decay", type=float, default=1.0, help="per reweight, the per-fight counts are multiplied by this (< 1: recent "
                    "episodes count more, so the estimates follow the improving policy)")
    ap.add_argument("--curriculum", default="", help="comma list of difficulty edges, e.g. 0.1,0.3,0.5,0.7,0.9,1: stage 0 = fights with meta.stage == easy, stage k = "
                    "certified fights (meta.cert.diff, tools/certify_fights.py) with difficulty up to edge k; every fight of the bands up to the stage weighs 1, the "
                    "stage's own band --cur-boost (cumulative: the last stage is nearly uniform over the certified pool); a stage advances when its band's mean "
                    "return (win + 0.5 x HP fraction / -1) stops improving (M3)")
    ap.add_argument("--cur-boost", type=float, default=2.0)
    ap.add_argument("--distill", default="", help="comma list of rl/distill.py files: search-played decisions mixed into every update (policy: cross-entropy to the "
                    "search's preference over its options; outcome head: the fight's real ending)")
    ap.add_argument("--distill-coef", type=float, default=1.0)
    ap.add_argument("--distill-mb", type=int, default=1024, help="distillation rows per minibatch")
    ap.add_argument("--cur-caps", default="", help="comma list: the player-turn cap of each stage (short fights first, relaxed later); default --turn-cap throughout")
    ap.add_argument("--cur-window", type=int, default=25, help="iterations per plateau window")
    ap.add_argument("--cur-min", type=int, default=50)
    ap.add_argument("--cur-max", type=int, default=400)
    ap.add_argument("--cur-eps", type=float, default=0.005, help="a window improving the stage's win by less than this ends the stage")
    ap.add_argument("--lr-floor", type=float, default=0.05, help="the lr decays linearly to this fraction of --lr")
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--resume", help="checkpoint to continue from (iteration count and lr schedule continue; --iters is the total)")
    ap.add_argument("--warm", action="store_true", help="with --resume: take the weights only (fresh optimizer, iteration 0)")
    ap.add_argument("--threads", type=int, default=12)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    torch.set_num_threads(a.threads)
    torch.manual_seed(a.seed)
    if a.heads and a.util_prob > 0:
        raise SystemExit("--heads uses today's linear worth of the ending (rl/heads.py): no --util-prob")
    if a.pot_head and not a.heads:
        raise SystemExit("--pot-head needs --heads")
    net = Net(d=a.d, rounds=a.rounds, heads=a.heads, pot=a.pot_head).to(DEV)
    opt = torch.optim.Adam(net.parameters(), lr=a.lr, eps=1e-5)
    allow = ("ucond.", "outcome.") if a.heads else ("ucond.",)
    if a.pot_head:
        allow = allow + ("pot_use.",)
    it0, steps = 0, 0
    if a.resume:  # a full checkpoint (net + optimizer + progress) or a bare state dict (weights only: warm start)
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
    cur = None
    env_cap = [a.turn_cap]
    if a.curriculum:  # band of every fight: 0 = easy stage, k = difficulty in (edge k-1, edge k]
        edges = [float(x) for x in a.curriculum.split(",")]
        band = np.array([0 if s.get("meta", {}).get("stage") == "easy" else 1 + int(np.searchsorted(edges, s["meta"]["cert"]["diff"] - 1e-9)) for s in scen])
        band = np.minimum(band, len(edges))
        cur = dict(stage=0, since=0, wins=[], last=None, n_stages=len(edges) + 1, counts=np.bincount(band, minlength=len(edges) + 1).tolist())

        def cur_weights(k):
            return np.where(band == k, a.cur_boost, np.where(band < k, 1.0, 0.0))

        for k in range(len(edges) + 1):  # sampling share of every band at every stage
            w = cur_weights(k)
            print(f"stage {k}: share by band", [round(float(w[band == b].sum() / w.sum()), 3) for b in range(len(edges) + 1)], flush=True)
        caps = [int(x) for x in a.cur_caps.split(",")] if a.cur_caps else []
        cap_of = lambda k: caps[min(k, len(caps) - 1)] if caps else a.turn_cap  # noqa: E731
        env.set_weights(cur_weights(0))
        env.set_turn_cap(cap_of(0))
        env_cap[0] = cap_of(0)
        print("curriculum bands", cur["counts"], "caps", [cap_of(k) for k in range(cur["n_stages"])], flush=True)
    if a.adaptive:  # per-fight and per-group win counts from the training episodes
        gkey = {}
        grp = np.array([gkey.setdefault((s.get("encounter"), s.get("act"), s.get("character")), len(gkey)) for s in scen])
        s_n, s_w = np.zeros(len(scen)), np.zeros(len(scen))
    # the outcome head alone first: its own optimizer, the rest of the network untouched
    dist_d = None
    if a.distill:
        parts = [np.load(p) for p in a.distill.split(",")]
        dist_d = {k: torch.from_numpy(np.concatenate([q[k] for q in parts])) for k in ("obs", "mask", "opts", "tgt", "cls")}
        print("distillation rows", len(dist_d["cls"]), flush=True)
    prefixes = tuple(p for p in a.warm_prefix.split(",") if p)
    warm_params = [p for n, p in net.named_parameters() if n.startswith(prefixes)]
    opt_w = torch.optim.Adam(warm_params, lr=a.lr, eps=1e-5) if a.heads and a.head_warmup > 0 else None
    if opt_w is not None and not warm_params:
        raise SystemExit(f"--warm-prefix {a.warm_prefix}: no parameters")
    it_warm = it0 + (a.head_warmup if opt_w is not None else 0)  # iterations up to this one only train the outcome head
    N, T = a.envs, a.horizon
    A = sts2.ACTIONS
    b_obs = torch.zeros(T, N, sts2.OBS_SIZE)
    b_mask = torch.zeros(T, N, A, dtype=torch.uint8)
    b_act = torch.zeros(T, N, dtype=torch.long)
    b_lp = torch.zeros(T, N)
    b_val = torch.zeros(T + 1, N)
    b_rew = torch.zeros(T, N)
    b_done = torch.zeros(T, N)
    b_feat = torch.zeros(T, N, 8)
    if a.heads:
        b_pout = torch.zeros(T + 1, N, H.NC)  # the outcome head's distribution at every observation (and the one after the horizon)
        b_term = torch.full((T, N), -1, dtype=torch.long)  # at an episode's last step: its ending class; -2 = aborted (no target); -1 = not done
    KP = sts2.layout()["consts"]["MAX_POTIONS"]
    _po = {n: o for n, o, s in sts2.layout()["sections"]}["potions"]
    if a.pot_head:
        b_ppot = torch.zeros(T + 1, N, KP)  # the potion-use head's probabilities at every observation
        b_pused = torch.zeros(T, N, KP)  # slot k's potion used up at this step (env)
    obs, mask = env.reset()
    rng = np.random.default_rng(a.seed + 7)
    # one HP-worth curve per running episode (redrawn when it ends): the win reward and the network's input
    curves = np.tile(utility.linear(), (N, 1))
    is_lin = np.ones(N, bool)
    feat = np.tile(utility.LINEAR_FEATS, (N, 1)).astype(np.float32)
    def draw_curves(idx):
        for i in idx:
            u = utility.sample(rng, p_linear=1.0 - a.util_prob)
            curves[i] = u
            is_lin[i] = bool(np.allclose(u, utility.linear()))
            feat[i] = utility.feats(u)
    draw_curves(np.arange(N))
    inv_err = [0.0, 0]  # max |overridden - env reward| on linear-curve wins, and how many were checked
    POT = slice(sts2.layout()["consts"]["OFF_POTION"], sts2.layout()["consts"]["OFF_DISCARD"])
    hold_until = np.zeros(N, np.int32)  # 0 = free, k = no potion before turn k, 99 = never
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
        lr = a.lr * max(a.lr_floor, 1 - (max(it - (it_warm - it0), 1) - 1) / max(a.iters, 1))  # warm-up iterations do not advance the schedule
        if a.adaptive and it % a.adaptive == 0 and s_n.sum() > 0:
            g_n, g_w = np.bincount(grp, s_n, len(gkey)), np.bincount(grp, s_w, len(gkey))
            pg = (g_w + 1) / (g_n + 2)
            ps = (s_w + 4 * pg[grp]) / (s_n + 4)
            if a.adaptive_mode == "signal":
                v = ps * (1 - ps)
                wts = (1 - a.adaptive_anchor) * v / v.sum() + a.adaptive_anchor / len(v)
            else:
                wts = np.where((ps >= 0.2) & (ps <= 0.8), 1.0, a.adaptive_floor)
            env.set_weights(wts)
            q = wts / wts.sum()
            adapt_rec = dict(share_mid=round(float(((ps >= 0.2) & (ps <= 0.8)).mean()), 3), seen=round(float((s_n > 0).mean()), 3),
                             sat=round(float((ps > 0.95).mean()), 3), lost=round(float((ps < 0.05).mean()), 3),
                             drawn_pq=round(float((q * ps * (1 - ps)).sum()), 4), uniform_pq=round(float((ps * (1 - ps)).mean()), 4))
            s_n *= a.adaptive_decay
            s_w *= a.adaptive_decay
        for g in (opt_w if warm else opt).param_groups:
            g["lr"] = lr
        # ---- rollout ----
        net.eval()
        t_roll = time.time()
        with torch.inference_mode():
            for t in range(T):
                m_eff = mask
                if a.hold_prob > 0:
                    m_eff = mask.copy()
                    bad = (hold_until > 0) & (obs[:, 1] < hold_until)
                    m_eff[bad, POT] = 0
                    empty = m_eff.sum(1) == 0
                    m_eff[empty] = mask[empty]
                b_obs[t].numpy()[:] = obs
                b_mask[t].numpy()[:] = m_eff
                b_feat[t].numpy()[:] = feat
                if a.pot_head:
                    lg, v, ol, pl_ = net(b_obs[t].to(DEV), b_mask[t].long().to(DEV), ufeat=b_feat[t].to(DEV), outcome=True, potuse=True)
                    b_pout[t] = torch.softmax(ol, 1).cpu()
                    b_ppot[t] = torch.sigmoid(pl_).cpu()
                elif a.heads:
                    lg, v, ol = net(b_obs[t].to(DEV), b_mask[t].long().to(DEV), ufeat=b_feat[t].to(DEV), outcome=True)
                    b_pout[t] = torch.softmax(ol, 1).cpu()
                else:
                    lg, v = net(b_obs[t].to(DEV), b_mask[t].long().to(DEV), ufeat=b_feat[t].to(DEV))
                logp = F.log_softmax(lg, 1)
                act = torch.multinomial(logp.exp(), 1).squeeze(1)
                b_act[t] = act.cpu()
                b_lp[t] = logp.gather(1, act[:, None]).squeeze(1).cpu()
                b_val[t] = v.cpu()
                obs, mask, rew, done, info = env.step(act.cpu().numpy().astype(np.int32))
                if a.pot_head:
                    pu = info["pot_used"]
                    b_pused[t] = torch.from_numpy(((pu[:, None] >> np.arange(KP)) & 1).astype(np.float32))
                r = rew.copy()
                oc = info["outcome"]
                r[oc == 2] = -1.0  # stalled out
                if done.any():
                    ei = env.episode_info()
                    for i in np.nonzero(done & (oc == 1))[0]:  # a win: the reward of this episode's curve at the exact end-HP fraction
                        ri = 1.0 + utility.HP_BONUS * float(np.interp(float(ei["hp_end"][i]), utility.FRAC, curves[i]))
                        if is_lin[i]:
                            inv_err[0] = max(inv_err[0], abs(ri - float(r[i])))
                            inv_err[1] += 1
                        r[i] = ri
                b_rew[t] = torch.from_numpy(r)
                b_done[t] = torch.from_numpy(done.astype(np.float32))
                if a.heads:
                    b_term[t] = -1
                    if done.any():
                        cls = H.end_class(oc == 1, ei["hp_end_abs"])
                        cls = np.where((oc == 3) | (oc == 4), -2, cls)  # aborted: no ending to learn from
                        b_term[t] = torch.from_numpy(np.where(done, cls, -1).astype(np.int64))
                if done.any():
                    if a.hold_prob > 0:
                        draw_rules(np.nonzero(done)[0])
                    draw_curves(np.nonzero(done)[0])
                    for i in np.nonzero(done)[0]:
                        ep_stats.append((int(oc[i]), float(ei["hp_lost"][i]), int(ei["length"][i])))
                        if cur is not None and oc[i] in (1, -1, 2):
                            cur["capped"] = cur.get("capped", [0, 0])
                            cur["capped"][0] += 1
                            cur["capped"][1] += oc[i] == -1 and ei["turns"][i] > env_cap[0]
                            if band[int(ei["scenario"][i])] == cur["stage"]:
                                cur["ep"] = cur.get("ep", [0, 0.0])
                                cur["ep"][0] += 1
                                cur["ep"][1] += (1.0 + 0.5 * float(ei["hp_end"][i])) if oc[i] == 1 else -1.0
                        if a.adaptive and oc[i] in (1, -1, 2):
                            si = int(ei["scenario"][i])
                            s_n[si] += 1
                            s_w[si] += oc[i] == 1
                steps += N
            if a.pot_head:
                _, last_v, last_ol, last_pl = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), ufeat=torch.from_numpy(feat.copy()).to(DEV), outcome=True, potuse=True)
                b_pout[T] = torch.softmax(last_ol, 1).cpu()
                b_ppot[T] = torch.sigmoid(last_pl).cpu()
            elif a.heads:
                _, last_v, last_ol = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), ufeat=torch.from_numpy(feat.copy()).to(DEV), outcome=True)
                b_pout[T] = torch.softmax(last_ol, 1).cpu()
            else:
                _, last_v = net(torch.from_numpy(obs.copy()).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), ufeat=torch.from_numpy(feat.copy()).to(DEV))
            b_val[T] = last_v.cpu()
        t_roll = time.time() - t_roll
        if inv_err[1] and inv_err[0] > 1e-4:  # the reward override must reproduce the env's own return for the linear curve
            raise SystemExit(f"reward invariant broken: max |overridden - env| = {inv_err[0]:.6f} over {inv_err[1]} linear wins (hp_end is not the HP fraction the env pays)")
        # ---- GAE ----
        adv = torch.zeros(T, N)
        last = torch.zeros(N)
        for t in reversed(range(T)):
            nd = 1.0 - b_done[t]
            delta = b_rew[t] + a.gamma * b_val[t + 1] * nd - b_val[t]
            last = delta + a.gamma * a.lam * nd * last
            adv[t] = last
        ret = adv + b_val[:T]
        if a.heads:  # the outcome head's targets: lambda-mix of the next state's prediction and the one-hot ending, backwards through the rollout
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
                y = torch.where(aborted.unsqueeze(1), b_pout[t], y)  # no target: its own prediction, weight 0
                wt[t] = (~aborted).float()
                tgt[t] = y
        if a.pot_head:  # potion-use targets: 1 when used at this step, 0 when the fight ends with it, else lambda-mix of the next prediction and the next target
            ptgt = torch.zeros(T, N, KP)
            yp = b_ppot[T].clone()
            for t in reversed(range(T)):
                done_t = b_done[t].unsqueeze(1) > 0
                boot = (1 - a.lam_head) * b_ppot[t + 1] + a.lam_head * yp
                yp = torch.where(b_pused[t] > 0, torch.ones_like(boot), torch.where(done_t, torch.zeros_like(boot), boot))
                ptgt[t] = yp
            pocc = (b_obs[:, :, _po:_po + 2 * KP:2] > 0).float()  # slots holding a potion at each observation
            pw_ = pocc * wt.unsqueeze(-1)  # aborted episodes weigh 0
        # ---- update ----
        net.train()
        t_upd = time.time()
        fo, fm, ff = b_obs.view(T * N, -1), b_mask.view(T * N, -1), b_feat.view(T * N, -1)
        fa, flp, fadv, fret = b_act.view(-1), b_lp.view(-1), adv.view(-1), ret.view(-1)
        if a.heads:
            ftgt, fwt = tgt.view(T * N, -1), wt.view(-1)
        stats = {"pl": 0.0, "vl": 0.0, "ent": 0.0, "kl": 0.0, "clip": 0.0}
        if a.pot_head:
            fptgt, fpw = ptgt.view(T * N, KP), pw_.view(T * N, KP)
            stats.update(potl=0.0, pot_brier=0.0, pot_base=0.0)
        nb = 0
        for ep in range(a.epochs):
            perm = torch.randperm(T * N)
            for s in range(0, T * N, a.mb):
                ix = perm[s:s + a.mb]
                ixd = ix
                if a.pot_head:
                    lg, v, ol, pl_ = net(fo[ix].to(DEV), fm[ix].long().to(DEV), ufeat=ff[ix].to(DEV), outcome=True, potuse=True)
                elif a.heads:
                    lg, v, ol = net(fo[ix].to(DEV), fm[ix].long().to(DEV), ufeat=ff[ix].to(DEV), outcome=True)
                else:
                    lg, v = net(fo[ix].to(DEV), fm[ix].long().to(DEV), ufeat=ff[ix].to(DEV))
                logp = F.log_softmax(lg, 1)
                nlp = logp.gather(1, fa[ix, None].to(DEV)).squeeze(1)
                ratio = (nlp - flp[ix].to(DEV)).exp()
                ad = fadv[ix].to(DEV)
                ad = (ad - ad.mean()) / (ad.std() + 1e-8)
                pl = -torch.min(ratio * ad, ratio.clamp(1 - a.clip, 1 + a.clip) * ad).mean()
                if a.heads:  # cross-entropy against the lambda-targets (aborted endings weigh 0)
                    w_ = fwt[ix].to(DEV)
                    vl = (-(ftgt[ix].to(DEV) * F.log_softmax(ol, 1)).sum(1) * w_).sum() / w_.sum().clamp(min=1)
                else:
                    vl = F.smooth_l1_loss(v, fret[ix].to(DEV))
                p = logp.exp()
                ent = -(p * logp.clamp(min=-30) * (fm[ix].to(DEV) > 0)).sum(1).mean()
                o_ = opt_w if warm else opt
                loss = a.vf * vl if warm else pl + a.vf * vl - a.ent * ent
                if dist_d is not None and not warm:
                    di = torch.randint(0, len(dist_d["cls"]), (a.distill_mb,))
                    d_obs, d_mask = dist_d["obs"][di].float().to(DEV), dist_d["mask"][di].long().to(DEV)
                    d_opts, d_tgt, d_cls = dist_d["opts"][di].long().to(DEV), dist_d["tgt"][di].to(DEV), dist_d["cls"][di].long().to(DEV)
                    if a.heads:
                        dlg, _, dol = net(d_obs, d_mask, outcome=True)[:3]
                    else:
                        dlg, _ = net(d_obs, d_mask)
                    dlp = F.log_softmax(dlg.float(), 1).gather(1, d_opts.clamp(min=0))
                    dpl = -(d_tgt * torch.where(d_opts >= 0, dlp, torch.zeros_like(dlp))).sum(1).mean()
                    dvl = F.cross_entropy(dol.float()[d_cls >= 0], d_cls[d_cls >= 0]) if a.heads and (d_cls >= 0).any() else torch.zeros((), device=DEV)
                    loss = loss + a.distill_coef * (dpl + a.vf * dvl)
                    stats["dpl"] = stats.get("dpl", 0.0) + dpl.item()
                    stats["dvl"] = stats.get("dvl", 0.0) + dvl.item()
                if a.pot_head:
                    yt, ww = fptgt[ix].to(DEV), fpw[ix].to(DEV)
                    potl = (F.binary_cross_entropy_with_logits(pl_, yt, reduction="none") * ww).sum() / ww.sum().clamp(min=1)
                    loss = loss + a.pot_coef * potl
                    with torch.no_grad():
                        stats["potl"] += potl.item()
                        stats["pot_brier"] += ((torch.sigmoid(pl_) - yt) ** 2 * ww).sum().item() / max(ww.sum().item(), 1)
                        stats["pot_base"] += ((yt - (yt * ww).sum() / ww.sum().clamp(min=1)) ** 2 * ww).sum().item() / max(ww.sum().item(), 1)
                o_.zero_grad(set_to_none=True)
                net.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_([q for g in o_.param_groups for q in g["params"]], 0.5)  # the norm of what this optimizer steps
                o_.step()
                stats["pl"] += pl.item(); stats["vl"] += vl.item(); stats["ent"] += ent.item()
                stats["kl"] += ((ratio - 1) - (nlp - flp[ix].to(DEV))).mean().item()
                stats["clip"] += ((ratio - 1).abs() > a.clip).float().mean().item()
                nb += 1
        t_upd = time.time() - t_upd
        rec = {"inv": [round(inv_err[0], 6), inv_err[1]], "it": it, "warm": warm, "steps": steps, "sps": int((steps - steps0) / (time.time() - t0)), "t_roll": round(t_roll, 1), "t_upd": round(t_upd, 1), "lr": lr}
        rec.update({k: round(v / nb, 4) for k, v in stats.items()})
        if adapt_rec:
            rec["adaptive"] = adapt_rec
        if cur is not None and not warm:
            cur["since"] += 1
            rec["stage"] = cur["stage"]
            if cur["since"] % a.cur_window == 0:
                n, w_ = cur.pop("ep", [0, 0.0])
                win = w_ / max(n, 1)  # the band's mean return
                rec["stage_ret"] = round(win, 4)
                cn, cc = cur.pop("capped", [0, 0])
                rec["capped"] = round(cc / max(cn, 1), 4)
                done_ = cur["last"] is not None and cur["since"] >= a.cur_min and win - cur["last"] < a.cur_eps
                cur["last"] = win
                if (done_ or cur["since"] >= a.cur_max) and cur["stage"] + 1 < cur["n_stages"]:
                    cur.update(stage=cur["stage"] + 1, since=0, last=None)
                    env.set_weights(cur_weights(cur["stage"]))
                    env.set_turn_cap(cap_of(cur["stage"]))
                    env_cap[0] = cap_of(cur["stage"])
                    rec["stage_advance"] = cur["stage"]
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
