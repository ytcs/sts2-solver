#!/usr/bin/env python3
"""Disagreement mining: where does the search overrule the policy by a lot, and is the overruling real?

  .venv/bin/python rl/mine.py --ckpt C --train target/train/train.json --roots 300 --out target/exit/mine1.npz

The search plays whole fights (rl/search.py). At every searched decision where its best action differs from the policy's top action and the
estimated gain is above `gap`, the state is re-evaluated with `confirm` x more simulated futures (fresh seeds, the same paired design) for just
those two actions. Only decisions whose gain survives the confirmation are kept: observation, mask, the policy's choice, the search's choice, the
confirmed Q of both. These are the states where the network is demonstrably wrong, with labels that are far less noisy than a plain search
pass. The summary lists them by the kind of action the search preferred (potion / end turn / card), so patterns like "drinks the potion too
early" show up directly. `rl/exit.py train` / the PPO auxiliary loss consume the archive.
"""
import argparse, collections, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import Searcher, load, fwd, values, sts2_C

C = sts2_C


def kind(a):
    if a == 0:
        return "end turn"
    if a < C["OFF_POTION"]:
        return "play card"
    if a < C["OFF_DISCARD"]:
        return "use potion"
    if a < C["OFF_PICK"]:
        return "discard potion"
    return "pick / confirm"


class Confirmer(Searcher):
    """Re-estimates chosen (root, action) pairs with many more futures, in a simulator of its own."""

    def make_confirm_sim(self, scen, cap, K):
        self.cap, self.cK = cap, K
        self.csim = sts2.VecEnv(cap * 2 * K, scen[:1], seed=int(self.rng.integers(1 << 30)), max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=self.hp_bonus)
        self.csim.set_autoreset(False)

    def estimate(self, main, roots, acts):
        """roots: root indices (at most `cap`), acts: [len(roots), 2] actions to compare. Returns (mean, standard error) of shape [len(roots), 2]."""
        K, n = self.cK, self.cap * 2 * self.cK
        assert len(roots) <= self.cap
        ks = self.rng.integers(1 << 62, size=(len(roots), K), dtype=np.uint64)
        src, dst, first, seeds = [], [], [], []
        for i, r in enumerate(roots):
            for j in range(2):
                for k in range(K):
                    src.append(r); dst.append((i * 2 + j) * K + k); first.append(acts[i][j]); seeds.append(ks[i, k])
        self.csim.fork_from(main, np.array(src, np.uint32), np.array(dst, np.uint32), np.array(seeds, np.uint64))
        sobs, smask = self.csim.reset()
        start = sobs[:, 1].copy()
        live = np.zeros(n, bool); live[dst] = True
        a = np.full(n, -1, np.int32); a[dst] = first
        est = np.zeros(n, np.float32)
        ei, eo = [], []
        for _ in range(self.roll_cap):
            sobs, smask, rew, done, info = self.csim.step(a)
            est[live] += rew[live]
            fin = live & (done > 0)
            ended = live & ~fin & (sobs[:, 1] > start)
            if ended.any():
                idx = np.nonzero(ended)[0]; ei.append(idx); eo.append(sobs[idx].copy())
            live &= ~(fin | ended)
            if not live.any():
                break
            idx = np.nonzero(live)[0]
            a = np.full(n, -1, np.int32)
            sm = smask[idx]
            forced = (sm > 0).sum(1) == 1
            if forced.any():
                a[idx[forced]] = sm[forced].argmax(1)
            th = ~forced
            if th.any():
                lg, _ = fwd(self.net, sobs[idx[th]].copy(), sm[th], value=False)
                a[idx[th]] = (lg.argmax(1) if self.greedy_roll else torch.multinomial(torch.softmax(lg, 1), 1).squeeze(1)).numpy()
        if ei:
            est[np.concatenate(ei)] += values(self.net, np.concatenate(eo))
        d = np.array(dst).reshape(len(roots), 2, K)
        return est[d].mean(2), est[d].std(2) / np.sqrt(K)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True); ap.add_argument("--train", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--roots", type=int, default=300); ap.add_argument("--chunks", type=int, default=1)
    ap.add_argument("--M", type=int, default=5); ap.add_argument("--K", type=int, default=8); ap.add_argument("--pmin", type=float, default=0.03)
    ap.add_argument("--gap", type=float, default=0.08, help="minimum estimated gain of the search's action over the policy's")
    ap.add_argument("--confirm", type=int, default=48, help="futures per action in the confirmation pass")
    ap.add_argument("--greedy-roll", action="store_true")
    ap.add_argument("--threads", type=int, default=8); ap.add_argument("--seed", type=int, default=1); ap.add_argument("--max-steps", type=int, default=300)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    net = load(a.ckpt)
    keep = collections.defaultdict(list)
    stats = collections.Counter()
    t0 = time.time()
    for chunk in range(a.chunks):
        rec = []
        s = Confirmer(net, a.roots, a.M, a.K, 0.0, seed=a.seed + chunk, max_steps=a.max_steps, pmin=a.pmin, greedy_roll=a.greedy_roll, record=rec)
        # the confirmation pass needs a bigger simulator: roots x 2 actions x confirm futures
        scen = json.load(open(a.train))
        main = sts2.VecEnv(a.roots, scen, seed=a.seed * 1000 + chunk, max_steps=a.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5)
        main.set_autoreset(False)
        s.make_sim(scen[:1])
        s.make_confirm_sim(scen, 48, a.confirm)
        obs, mask = main.reset()
        finished = np.zeros(a.roots, bool)
        while not finished.all():
            active = ~finished
            before = len(rec)
            act = s.choose(main, obs, mask, active)
            # which roots disagreed with the policy?
            info = s.last_info
            cand = []
            for r, d in info.items():
                if d["q"] is None or not active[r]:
                    continue
                top = d["acts"][0]
                j = int(np.argmax([q if lg else -1e9 for q, lg in zip(d["q"], d["legal"])]))
                if d["acts"][j] != top and d["q"][j] - d["q"][0] > a.gap and act[r] == d["acts"][j]:
                    cand.append((r, top, d["acts"][j], d["q"][0], d["q"][j], d["p"][0]))
            stats["searched"] += sum(1 for d in info.values() if d["q"] is not None)
            stats["disagree"] += len(cand)
            if cand:
                roots = [c[0] for c in cand]
                acts = [[c[1], c[2]] for c in cand]
                # confirmation: chunked to the simulator's size
                per = s.cap
                for i0 in range(0, len(roots), per):
                    rr, aa = roots[i0:i0 + per], acts[i0:i0 + per]
                    q, se = s.estimate(main, rr, aa)
                    for i, r in enumerate(rr):
                        gain = q[i][1] - q[i][0]
                        if gain > a.gap and gain > 2 * np.hypot(se[i][0], se[i][1]):
                            c = cand[i0 + i]
                            keep["obs"].append(np.clip(obs[r], -60000, 60000).astype(np.float16)); keep["mask"].append(mask[r].copy())
                            keep["policy"].append(c[1]); keep["search"].append(c[2]); keep["q"].append(q[i].copy()); keep["p_policy"].append(c[5])
                            stats["confirmed"] += 1
                            stats["confirmed: prefers " + kind(c[2]) + " over " + kind(c[1])] += 1
            a_ = act.copy(); a_[finished] = -1
            obs, mask, rew, done, inf = main.step(a_)
            finished |= done > 0
        print(f"chunk {chunk} {time.time() - t0:.0f}s: {dict(stats)}", flush=True)
        os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
        np.savez_compressed(a.out, **{k: np.array(v) for k, v in keep.items()})  # (saved after every chunk: a long run can be stopped any time)
    os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
    np.savez_compressed(a.out, **{k: np.array(v) for k, v in keep.items()})
    print("searched decisions:", stats["searched"], "| search disagreed (gain > gap):", stats["disagree"], "| confirmed:", stats["confirmed"])
    for k, v in sorted(stats.items()):
        if k.startswith("confirmed: "):
            print("  ", k, v)


if __name__ == "__main__":
    main()
