#!/usr/bin/env python3
"""Policy improvement by determinized play-outs ("rollout search") on top of a trained network.

At every decision the root fight is forked M x K times (`VecEnv.fork_from`): the M most probable legal actions of the policy, each on K
determinized copies (pile orders and RNG streams resampled, everything the player can see unchanged; the same K seeds are used for every
action so the comparison is paired). Each copy plays the action, then the policy plays on until the end of the current player turn
(or the fight); the estimate is the final reward if the fight ended, else the value head at the start of the next turn. The root plays the
action with the best mean estimate (it must beat the policy's own top choice by `margin` to replace it).

  .venv/bin/python rl/search.py --ckpt target/runs/r3/ckpt.pt --eval target/train/eval.json --roots 100 --M 6 --K 8
"""
import argparse, collections, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import Net, C, DEV
from ppo import make_env, summarize, net_policy, evaluate

A = sts2.ACTIONS
sts2_C = sts2.layout()["consts"]
TURN = 1  # obs[:, 1] = the player's turn counter


def load(path):
    ck = torch.load(path)
    args = ck.get("args", {})
    net = Net(d=args.get("d", 64), rounds=args.get("rounds", 2))
    net.load_state_dict(ck["net"] if "net" in ck else ck)
    return net.to(DEV).eval()


@torch.no_grad()
def fwd(net, obs, mask, policy=True, value=True):
    lg, v = net(torch.from_numpy(obs).to(DEV), torch.from_numpy(mask.astype(np.int64)).to(DEV), policy=policy, value=value)
    return lg, v


@torch.no_grad()
def values(net, obs, chunk=4096):
    """Value head only, in big batches."""
    out = []
    for i in range(0, len(obs), chunk):
        _, v = net(torch.from_numpy(obs[i:i + chunk]).to(DEV), None, policy=False)
        out.append(v)
    return torch.cat(out).cpu().numpy() if out else np.zeros(0, np.float32)


class Timers(collections.defaultdict):
    def __init__(self):
        super().__init__(float)

    def clock(self, name, t0):
        self[name] += time.perf_counter() - t0
        return time.perf_counter()


class Searcher:
    def __init__(self, net, n_roots, M=6, K=8, margin=0.0, seed=0, hp_bonus=0.5, max_steps=600, roll_cap=60, conf=1.01, record=None, pmin=0.0, force=False, greedy_roll=False, full=False):
        self.net, self.R, self.M, self.K, self.margin = net, n_roots, M, K, margin
        self.rng = np.random.default_rng(seed)
        self.hp_bonus, self.max_steps, self.roll_cap = hp_bonus, max_steps, roll_cap
        self.sim = None
        self.timers = Timers()
        self.full = full  # play-outs run to the end of the fight: the estimate is the real final reward, the value head is not used
        self.greedy_roll = greedy_roll  # play-outs follow the policy's top action (no sampling blunders in the estimate)
        self.pmin = pmin  # candidates must have at least this policy probability (the top one always stays)
        self.force = force  # always also try ending the turn and the likeliest potion action, however unlikely the policy finds them
        self.conf = conf  # skip the search where the policy's top action has probability >= conf
        self.record = record  # list: gets one dict per searched decision (for expert iteration)
        self.mask_fn = None  # optional (obs, mask) -> mask applied to every decision, at the root and inside the play-outs (what-if rules)
        self.last_info = {}  # root -> what the latest `choose` considered (actions, policy probabilities, estimated returns)

    def make_sim(self, scen):
        n = self.R * self.M * self.K
        self.sim = sts2.VecEnv(n, scen, seed=int(self.rng.integers(1 << 30)), max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=self.hp_bonus)
        self.sim.set_autoreset(False)

    def choose(self, main, obs, mask, active):
        """Best action per root (int32 [R]); roots that are not `active` get 0."""
        R, M, K = self.R, self.M, self.K
        if self.mask_fn is not None:
            mask = self.mask_fn(obs, mask)
        t = time.perf_counter()
        lg, _ = fwd(self.net, obs.copy(), mask, value=False)
        prob = torch.softmax(lg, 1).cpu().numpy()
        t = self.timers.clock("net root", t)
        order = np.argsort(-prob, 1)[:, :M]
        best = order[:, 0].astype(np.int32)
        pm = np.take_along_axis(prob, order, 1)
        legal = (np.take_along_axis(mask, order, 1) > 0) & ((pm >= self.pmin) | (np.arange(M)[None, :] == 0))
        if self.force:
            pot = slice(sts2_C["OFF_POTION"], sts2_C["OFF_DISCARD"])
            for r in np.nonzero(active)[0]:
                forced = []
                if mask[r, 0] > 0:
                    forced.append(0)
                pl = np.where(mask[r, pot] > 0)[0]
                if len(pl):
                    forced.append(int(sts2_C["OFF_POTION"] + pl[np.argmax(prob[r, pot][pl])]))
                for act in forced:
                    if act in order[r][legal[r]]:
                        continue
                    free = np.nonzero(~legal[r])[0]
                    j = int(free[0]) if len(free) else M - 1
                    order[r, j] = act
                    legal[r, j] = True
        n_legal = legal.sum(1)
        self.last_info = {int(r): dict(acts=order[r].tolist(), p=prob[r, order[r]].tolist(), q=None, legal=legal[r].tolist()) for r in np.nonzero(active)[0]}
        todo = np.nonzero(active & (n_legal > 1) & (prob[np.arange(R), order[:, 0]] < self.conf))[0]
        if len(todo) == 0:
            return best
        # forks: slot (r, j, k) = (r * M + j) * K + k
        src, dst, acts, seeds = [], [], [], []
        for r in todo:
            ks = self.rng.integers(1 << 62, size=K, dtype=np.uint64)
            for j in range(M):
                if not legal[r, j]:
                    continue
                for k in range(K):
                    src.append(r); dst.append((r * M + j) * K + k); acts.append(order[r, j]); seeds.append(ks[k])
        src, dst = np.array(src, np.uint32), np.array(dst, np.uint32)
        acts_arr = np.array(acts, np.int32)
        t = time.perf_counter()
        self.sim.fork_from(main, src, dst, np.array(seeds, np.uint64))
        t = self.timers.clock("rust: fork", t)
        n = self.R * M * K
        sobs, smask = self.sim.reset()
        t = self.timers.clock("rust: observe forks", t)
        start_turn = sobs[:, TURN].copy()
        live = np.zeros(n, bool)
        live[dst] = True
        first = np.full(n, -1, np.int32)
        first[dst] = acts_arr
        est = np.zeros(n, np.float32)
        end_idx, end_obs = [], []
        a = first
        for it in range(self.max_steps if self.full else self.roll_cap):
            t = time.perf_counter()
            sobs, smask, rew, done, info = self.sim.step(a)
            t = self.timers.clock("rust: step+observe", t)
            est[live] += rew[live]
            fin = live & (done > 0)
            ended_turn = np.zeros(n, bool) if self.full else live & ~fin & (sobs[:, TURN] > start_turn)
            if ended_turn.any():  # bootstrap with the value of the next turn's first state (evaluated once, below, in big batches)
                idx = np.nonzero(ended_turn)[0]
                end_idx.append(idx)
                end_obs.append(sobs[idx].copy())
            live &= ~(fin | ended_turn)
            if not live.any():
                break
            idx = np.nonzero(live)[0]
            a = np.full(n, -1, np.int32)
            sm = smask[idx] if self.mask_fn is None else self.mask_fn(sobs[idx], smask[idx])
            forced = (sm > 0).sum(1) == 1  # one legal action: no network needed
            if forced.any():
                a[idx[forced]] = sm[forced].argmax(1)
            thinking = ~forced
            if thinking.any():
                t = time.perf_counter()
                lg, _ = fwd(self.net, sobs[idx[thinking]].copy(), sm[thinking], value=False)
                t = self.timers.clock("net rollout", t)
                a[idx[thinking]] = (lg.argmax(1) if self.greedy_roll else torch.multinomial(torch.softmax(lg, 1), 1).squeeze(1)).cpu().numpy()
        if end_idx:
            t = time.perf_counter()
            est[np.concatenate(end_idx)] += values(self.net, np.concatenate(end_obs))
            t = self.timers.clock("net value", t)
        if self.full:
            est[live] += -1.0  # still going when the cap hit: a stall counts as a loss
        # (copies still live after the cap: count what they have, no bootstrap)
        q = est.reshape(R, M, K).mean(2)
        for r in todo:
            self.last_info[int(r)]['q'] = q[r].tolist()
        if self.record is not None:
            for r in todo:
                self.record.append(dict(root=int(r), obs=np.clip(obs[r], -60000, 60000).astype(np.float16), mask=mask[r].copy(), acts=order[r].astype(np.int16),
                                        legal=legal[r].copy(), q=q[r].copy(), p=prob[r, order[r]].copy()))
        for r in todo:
            qs = np.where(legal[r], q[r], -1e9)
            j = int(np.argmax(qs))
            if j != 0 and qs[j] - qs[0] <= self.margin:
                j = 0
            best[r] = order[r, j]
        return best

    def play(self, scen_path, seed=777, verbose=True, with_records=False, trace=None, round_robin=False):
        scen = scen_path if isinstance(scen_path, list) else json.load(open(scen_path))
        R = self.R
        main = sts2.VecEnv(R, scen, seed=seed, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=self.hp_bonus, round_robin=round_robin)
        main.set_autoreset(False)
        self.make_sim(scen[:1])
        obs, mask = main.reset()
        finished = np.zeros(R, bool)
        rec = [None] * R
        t0 = time.time()
        for step in range(self.max_steps):
            active = ~finished
            if not active.any():
                break
            a = self.choose(main, obs, mask, active)
            a[finished] = -1
            if trace is not None:
                for r in np.nonzero(active)[0]:
                    trace[r].append(dict(obs=obs[r].copy(), a=int(a[r]), info=self.last_info.get(int(r))))
            obs, mask, rew, done, info = main.step(a)
            new = (done > 0) & ~finished
            if trace is not None:
                for r in np.nonzero(new)[0]:
                    trace[r].append(dict(obs=obs[r].copy(), a=None, info=None))
            if new.any():
                ei = main.episode_info()
                for i in np.nonzero(new)[0]:
                    rec[i] = (int(ei["scenario"][i]), int(info["outcome"][i]), float(ei["hp_lost"][i]), int(ei["length"][i]), float(ei["hp_end"][i]))
                finished |= new
            if verbose and step % 10 == 0:
                print(f"  decision step {step}: {finished.sum()}/{R} fights done, {time.time() - t0:.0f}s", flush=True)
        for i in range(R):
            if rec[i] is None:
                rec[i] = (i, 2, 1.0, self.max_steps, 0.0)
        return rec if with_records else summarize([r for r in rec], scen)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--eval", required=True)
    ap.add_argument("--roots", type=int, default=100)
    ap.add_argument("--M", type=int, default=6)
    ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--margin", type=float, default=0.0)
    ap.add_argument("--conf", type=float, default=1.01)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--max-steps", type=int, default=600)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    net = load(a.ckpt)
    # the first `roots` scenarios of the eval set played by the greedy policy (same fights, same seeds): paired baseline
    base = evaluate(net_policy(net), a.eval, a.roots, 1, 777, a.max_steps, 0.5)
    print("policy (greedy) ", json.dumps({k: base[k] for k in ("episodes", "win", "hp_lost_all", "stall")}), flush=True)
    s = Searcher(net, a.roots, a.M, a.K, a.margin, max_steps=a.max_steps, conf=a.conf)
    t0 = time.time()
    res = s.play(a.eval, verbose=False)
    print(f"search M={a.M} K={a.K} conf={a.conf} ({time.time() - t0:.0f}s)", json.dumps({k: res[k] for k in ("episodes", "win", "hp_lost_all", "stall")}), flush=True)


if __name__ == "__main__":
    main()
