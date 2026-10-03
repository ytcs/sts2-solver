#!/usr/bin/env python3
"""What-if analysis of one fight with the trained model: which single card hurts the deck, and when should the Phrog Parasite be split.

  .venv/bin/python rl/whatif.py --ckpt target/runs/r4/ckpt.pt --scenario target/runs/phrog.json --attempts 3000 [--search-top 3]

* removal: for every distinct card (name, upgrade) of the deck, play the fight with one copy removed (the model sees the new deck in its observation) and
  compare the win rate / HP left with the full deck. The best candidates can be re-checked with the play-out search.
* split timing: the Phrog Parasite bursts into Wrigglers when it dies (Infested). The policy is forbidden to land a killing blow on it before turn T
  (`T = 1` is unconstrained); win rate, HP left and the turn the split really happened are reported per T, plus the unconstrained fights grouped by
  the turn the model chose.
"""
import argparse, collections, copy, json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import C, SEC
from search import load, Searcher
from ppo import net_policy

NAMES = sts2.names()
WRIGGLER = NAMES["monster"].index("WRIGGLER") + 1
OH, _ = SEC["hand"]
OE, _ = SEC["enemies"]
OG, _ = SEC["global"]
OP, _ = SEC["player"]
CF, EF, T13 = C["CARD_F"], C["ENEMY_F"], C["MAX_CREATURES"] + 1
TWIN = NAMES["card"].index("TWIN_STRIKE") + 1


def run(policy, scen, n, seed=1234, max_steps=400, track=True):
    """First episode of n copies of one fight. Returns dict of arrays."""
    env = sts2.VecEnv(n, [scen], seed=seed, max_steps=max_steps, win=1.0, loss=-1.0, hp_bonus=0.5)
    obs, mask = env.reset()
    got = np.zeros(n, bool)
    out = dict(win=np.zeros(n, bool), hp_end=np.zeros(n), valid=np.zeros(n, bool), split=np.full(n, -1), pot=np.full(n, -1))
    pot0 = obs[:, SEC["potions"][0]:SEC["potions"][0] + 2].copy()
    while not got.all():
        a = policy(obs, mask)
        prev_turn = obs[:, OG + 1].copy()
        obs, mask, r, d, info = env.step(a)
        if track:
            wr = ((obs[:, OE:OE + C["OBS_MAX_ENEMIES"] * EF].reshape(n, C["OBS_MAX_ENEMIES"], EF)[:, :, 2] == WRIGGLER) & (obs[:, OE:OE + C["OBS_MAX_ENEMIES"] * EF].reshape(n, C["OBS_MAX_ENEMIES"], EF)[:, :, 0] > 0.5)).any(1)
            new = wr & (out["split"] < 0) & ~got
            out["split"][new] = prev_turn[new]
            used = (a >= C["OFF_POTION"]) & (a < C["OFF_DISCARD"]) & (out["pot"] < 0) & ~got
            out["pot"][used] = prev_turn[used]
        new = (d > 0) & ~got
        if new.any():
            ei = env.episode_info()
            idx = np.nonzero(new)[0]
            oc = info["outcome"][idx]
            out["win"][idx] = oc == 1
            out["valid"][idx] = (oc == 1) | (oc == -1) | (oc == 2)
            out["hp_end"][idx] = ei["hp_end"][idx]
            got[idx] = True
    return out


def summary(o, max_hp):
    v = o["valid"]
    w = o["win"] & v
    return dict(win=float(w.sum() / max(v.sum(), 1)), hp_left=float(o["hp_end"][w].mean() * max_hp) if w.any() else 0.0, n=int(v.sum()),
                se=float(np.sqrt(max(w.sum() / max(v.sum(), 1) * (1 - w.sum() / max(v.sum(), 1)), 1e-9) / max(v.sum(), 1))))


def delay_kill(base, until_turn):
    """Forbid a (probable) killing blow on the Phrog Parasite before turn `until_turn`."""
    def act(obs, mask):
        m = mask.copy()
        if until_turn > 1:
            n = len(obs)
            turn = obs[:, OG + 1]
            en = obs[:, OE:OE + C["OBS_MAX_ENEMIES"] * EF].reshape(n, C["OBS_MAX_ENEMIES"], EF)
            hand = obs[:, OH:OH + C["MAX_HAND"] * CF].reshape(n, C["MAX_HAND"], CF)
            for k in range(C["OBS_MAX_ENEMIES"]):
                e = en[:, k]
                is_phrog = (e[:, 0] > 0.5) & (e[:, 6] > 0.5) & (e[:, 2] == NAMES["monster"].index("PHROG_PARASITE") + 1) & (turn < until_turn)
                if not is_phrog.any():
                    continue
                cid = e[:, 1].astype(int)
                life = e[:, 3] + e[:, 5]
                for h in range(C["MAX_HAND"]):
                    c = hand[:, h]
                    hits = np.where(c[:, 0] == TWIN, 2, 1)
                    lethal = is_phrog & (c[:, 6] * hits >= life) & (c[:, 6] > 0)
                    for i in np.nonzero(lethal)[0]:
                        m[i, 1 + h * T13 + cid[i]] = 0
            # never leave an env without a legal action
            empty = m.sum(1) == 0
            m[empty] = mask[empty]
        return base(obs, m)
    return act


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--attempts", type=int, default=3000)
    ap.add_argument("--search-top", type=int, default=0)
    ap.add_argument("--search-roots", type=int, default=240)
    ap.add_argument("--out", default="")
    ap.add_argument("--threads", type=int, default=8)
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    scen = json.load(open(a.scenario))
    net = load(a.ckpt)
    greedy = net_policy(net)
    max_hp = scen["max_hp"]
    res = {}
    base = run(greedy, scen, a.attempts)
    res["base"] = summary(base, max_hp)
    print("full deck:", json.dumps(res["base"]), flush=True)
    # ---- removal ----
    counts = collections.OrderedDict()
    for c in scen["deck"]:
        key = (c, 0) if isinstance(c, str) else (c["id"], c.get("upgrade", 0))
        counts[key] = counts.get(key, 0) + 1
    res["removal"] = []
    for (cid, up), k in counts.items():
        s2 = copy.deepcopy(scen)
        for i, c in enumerate(s2["deck"]):
            if (c if isinstance(c, str) else c["id"]) == cid and (0 if isinstance(c, str) else c.get("upgrade", 0)) == up:
                del s2["deck"][i]
                break
        r = summary(run(greedy, s2, a.attempts), max_hp)
        r.update(card=cid + ("+" if up else ""), copies=k)
        res["removal"].append(r)
        print(f"remove one {r['card']:20s} win {r['win']:.3f} (±{r['se']:.3f})  HP left {r['hp_left']:.1f}", flush=True)
    # ---- split timing ----
    res["split"] = []
    for T in range(1, 8):
        o = run(delay_kill(greedy, T), scen, a.attempts)
        r = summary(o, max_hp)
        sp = o["split"][o["split"] >= 0]
        r.update(T=T, mean_split=float(sp.mean()) if len(sp) else None)
        res["split"].append(r)
        print(f"no kill before turn {T}: win {r['win']:.3f} (±{r['se']:.3f}) HP left {r['hp_left']:.1f}; actual split turn {r['mean_split']}", flush=True)
    by = {}
    for t in sorted(set(base["split"][base["split"] >= 0])):
        sel = (base["split"] == t) & base["valid"]
        if sel.sum() >= 20:
            by[int(t)] = dict(share=float(sel.mean()), win=float(base["win"][sel].mean()), n=int(sel.sum()))
    res["split_observed"] = by
    print("unconstrained, grouped by the turn the model split:", json.dumps(by))
    pu = {}
    for t in sorted(set(base["pot"][base["pot"] >= 0])):
        sel = (base["pot"] == t) & base["valid"]
        if sel.sum() >= 20:
            pu[int(t)] = dict(share=float(sel.mean()), win=float(base["win"][sel].mean()))
    res["potion_turn"] = pu
    print("potion used on turn:", json.dumps(pu), " never used:", float(((base["pot"] < 0) & base["valid"]).mean()))
    if a.search_top:
        cand = sorted(res["removal"], key=lambda r: -r["win"])[:a.search_top]
        print("\nsearch check (M4 K4, %d roots each):" % a.search_roots, flush=True)
        res["search"] = {}
        for label, sc in [("full deck", scen)] + [("remove " + r["card"], None) for r in cand]:
            if sc is None:
                cid = label.split(" ", 1)[1]
                sc = copy.deepcopy(scen)
                for i, c in enumerate(sc["deck"]):
                    nm = c if isinstance(c, str) else c["id"] + ("+" if c.get("upgrade") else "")
                    if nm == cid:
                        del sc["deck"][i]
                        break
            path = a.out + ".tmp.json" if a.out else "target/runs/whatif_tmp.json"
            json.dump([sc], open(path, "w"))
            s = Searcher(net, a.search_roots, 4, 4, 0.0, seed=3, max_steps=300)
            rec = np.array(s.play(path, seed=21, verbose=False, with_records=True))
            w = rec[:, 1] == 1
            res["search"][label] = dict(win=float(w.mean()), hp_left=float(rec[w, 4].mean() * max_hp) if w.any() else 0.0)
            print(f"  {label:26s} win {w.mean():.3f}  HP left {res['search'][label]['hp_left']:.1f}", flush=True)
    if a.out:
        json.dump(res, open(a.out, "w"), indent=1)


if __name__ == "__main__":
    main()
