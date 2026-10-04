#!/usr/bin/env python3
"""Reference policies for the combat environment, evaluated with the same protocol as the learned ones.

  .venv/bin/python rl/baselines.py --eval target/train/eval.json [--policies random,untrained,heuristic]

heuristic: what a competent but unsophisticated player does each turn: finish off an enemy it can kill, cover the incoming damage
with the best block cards, play powers / utility cards, then spend the remaining energy on the best damage per energy aimed at the
weakest enemy; never uses potions; ends the turn when nothing is left. Card effects are read only from the observation (damage /
block previews, cost), so it knows nothing about card text.
"""
import argparse, json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import C, SEC, Net, DEV
from ppo import evaluate, net_policy

H, E, T = C["MAX_HAND"], C["OBS_MAX_ENEMIES"], C["MAX_CREATURES"] + 1
OFF_PLAY, OFF_CONFIRM, OFF_PICK = C["OFF_PLAY"], C["OFF_CONFIRM"], C["OFF_PICK"]
CF, EF = C["CARD_F"], C["ENEMY_F"]


def random_policy(seed=0):
    rng = np.random.default_rng(seed)

    def act(obs, mask):
        p = mask.astype(np.float64)
        p /= p.sum(1, keepdims=True)
        return (p.cumsum(1) < rng.random((len(p), 1))).sum(1).astype(np.int32)
    return act


def heuristic_policy():
    oh, _ = SEC["hand"]
    oe, _ = SEC["enemies"]
    od, _ = SEC["decision"]
    op, _ = SEC["player"]

    def act(obs, mask):
        n = len(obs)
        out = np.zeros(n, np.int32)
        for i in range(n):
            m, o = mask[i], obs[i]
            if o[od] > 0.5:  # a card selection: confirm when allowed, otherwise click the first candidate not yet selected
                if m[OFF_CONFIRM]:
                    out[i] = OFF_CONFIRM
                    continue
                cands = o[od + 8:od + 8 + 16 * (CF + 1)].reshape(16, CF + 1)
                for k in range(16):
                    if m[OFF_PICK + k] and cands[k, CF] < 0.5:
                        out[i] = OFF_PICK + k
                        break
                else:
                    out[i] = int(np.nonzero(m)[0][0])
                continue
            hand = o[oh:oh + H * CF].reshape(H, CF)
            en = o[oe:oe + E * EF].reshape(E, EF)
            alive = [(int(e[1]), e[3], e[5], e) for e in en if e[0] > 0.5 and e[6] > 0.5]
            incoming = 0.0
            for _, _, _, e in alive:
                for j in range(3):
                    kind, dmg, hits = e[40 + 3 * j:43 + 3 * j]
                    if kind == 1:
                        incoming += dmg * max(hits, 1)
            block = o[op + 2]
            energy = o[op + 3]
            need = max(0.0, incoming - block)
            weakest = min(alive, key=lambda x: x[1] + x[2]) if alive else None
            best, best_s = 0, -1e9
            for h in range(H):
                c = hand[h]
                if c[0] <= 0:
                    continue
                cost = max(c[2], 0.0)
                dmg, blk = c[6], c[7]
                for t in range(T):
                    a = OFF_PLAY + h * T + t
                    if not m[a]:
                        continue
                    sc = 0.0
                    tgt_hp = None
                    if t < T - 1:
                        for cid, hp, bl, _ in alive:
                            if cid == t:
                                tgt_hp = hp + bl
                    if dmg > 0 and tgt_hp is not None:
                        if dmg >= tgt_hp:
                            sc = 100 + tgt_hp  # a kill
                        else:
                            sc = 10 + dmg / max(cost, 0.5) + (5 if weakest is not None and t == weakest[0] else 0)
                    elif dmg > 0:
                        sc = 10 + dmg / max(cost, 0.5)
                    if blk > 0:
                        sc = max(sc, (60 if need > 0 else 5) + min(blk, need) / max(cost, 0.5))
                    if dmg <= 0 and blk <= 0:
                        sc = 30  # powers / utility: play them while energy lasts
                    if sc > best_s:
                        best, best_s = a, sc
            out[i] = best if best_s > -1e8 else 0  # 0 = end turn
        return out
    return act


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval", required=True)
    ap.add_argument("--policies", default="random,untrained,heuristic")
    ap.add_argument("--envs", type=int, default=512)
    ap.add_argument("--per-env", type=int, default=3)
    ap.add_argument("--max-steps", type=int, default=600)
    ap.add_argument("--hp-bonus", type=float, default=0.5)
    ap.add_argument("--threads", type=int, default=12)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    torch.manual_seed(0)
    for name in a.policies.split(","):
        if name == "random":
            pol = random_policy()
        elif name == "untrained":
            pol = net_policy(Net().to(DEV).eval())
        elif name == "heuristic":
            pol = heuristic_policy()
        elif name.startswith("ckpt:"):  # a trained network, greedy
            from model import load as load_ckpt  # one checkpoint, or several joined by commas (an ensemble)
            pol = net_policy(load_ckpt(name[5:]))
        res = evaluate(pol, a.eval, a.envs, a.per_env, 777, a.max_steps, a.hp_bonus)
        print(name, json.dumps(res), flush=True)


if __name__ == "__main__":
    main()
