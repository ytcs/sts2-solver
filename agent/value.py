"""Learned macro value, iteration 0 (plan: self-improvement loop, gate 2): run-state features from the raw predictor (deterministic: fixed
shuffles), heads fitted on logged screens with realised outcomes (`tools/value_fit.py` -> `models/value.json`).
Heads: reach_k = P(reach the end of act k); str_k = deck strength (solver P(win) at full HP) at act k's bosses; final = the run's final deck
vs the final pair. V(s) = log reach_0 + log reach_1 + log final, each for acts not yet passed."""
import json
import os

import numpy as np

from agent import pools, runmodel as R

MULTS = (1.0, 1.5, 2.0, 3.0)
COMPOSITION = ["n_attack", "n_skill", "n_power", "n_basic", "n_upgraded", "n_bad"]
NAMES = (["act1", "act2", "act3", "floor", "hp", "max_hp", "potions", "gold", "deck", "cur_boss_hp", "cur_elite_hp", "cur_hall_hp"] + COMPOSITION
         + [f"A{b + 1}_boss_x{m:g}" for b in range(3) for m in MULTS] + [f"A{b + 1}_elite" for b in range(3)])
SHUFFLES, SEED = 4, 1000


def _logit(p):
    p = np.clip(p, 0.005, 0.995)
    return np.log(p / (1 - p))


def composition(deck):
    t = [R.CARD_ROWS.get(c["id"], {}).get("type") for c in deck]
    basic = sum(c["id"].startswith(R.BASICS[:2]) for c in deck)
    return dict(n_attack=t.count("Attack") / 10.0, n_skill=t.count("Skill") / 10.0, n_power=t.count("Power") / 3.0, n_basic=basic / 10.0,
                n_upgraded=sum(bool(c.get("upgrade")) for c in deck) / 10.0, n_bad=(t.count("Curse") + t.count("Status")) / 3.0)


def _act_name(st, b):
    return st.act_name if b == st.act else R.ACT_NAMES[b][0]


def probes(st):
    """[(feature slot, scenario)] the predictor scores for st; slots repeat (means over encounters)"""
    out = []
    bosses = [b for b in (st.bosses or []) if b] or pools.pool(st.act_name, "boss")
    for e in bosses:
        out.append(("cur_boss_hp", st.scenario(e)))
    for e in pools.pool(st.act_name, "elite"):
        out.append(("cur_elite_hp", st.scenario(e)))
    for e in pools.pool(st.act_name, "regular"):
        out.append(("cur_hall_hp", st.scenario(e)))
    for b in range(st.act, 3):
        name = _act_name(st, b)
        for e in (bosses if b == st.act else pools.pool(name, "boss")):
            for m in MULTS:
                h = int(st.max_hp * m)
                out.append((f"A{b + 1}_boss_x{m:g}", dict(st.scenario(e, hp=h, potions=[], act=b), max_hp=max(st.max_hp, h))))
        for e in pools.pool(name, "elite"):
            out.append((f"A{b + 1}_elite", st.scenario(e, hp=st.max_hp, potions=[], act=b)))
    return out


def features(states, pred):
    """feature matrix [len(states), len(NAMES)]: logits of predictor P(win), 1.0 for acts already passed"""
    import predictor as PRD
    jobs = [(i, slot, sc) for i, st in enumerate(states) for slot, sc in probes(st)]
    P = PRD.p_win(pred.fight_start([sc for _, _, sc in jobs], SHUFFLES, SEED)) if jobs else []
    acc = {}
    for (i, slot, _), p in zip(jobs, P):
        acc.setdefault((i, slot), []).append(float(p))
    X = np.zeros((len(states), len(NAMES)))
    for i, st in enumerate(states):
        f = dict(act1=st.act == 0, act2=st.act == 1, act3=st.act == 2, floor=getattr(st, "abs_floor", 0) / 50.0, hp=st.hp / st.max_hp,
                 max_hp=st.max_hp / 80.0, potions=len(st.potions) / 3.0, gold=st.gold / 300.0, deck=len(st.deck) / 30.0, **composition(st.deck))
        for j, n in enumerate(NAMES):
            if n in f:
                X[i, j] = float(f[n])
            elif (i, n) in acc:
                X[i, j] = _logit(np.mean(acc[(i, n)]))
            else:
                X[i, j] = _logit(1.0)
    return X


class Value:
    def __init__(self, path=None):
        m = json.load(open(path or os.path.join(os.path.dirname(__file__), "..", "models", "value.json")))
        self.heads = {k: (np.array(v["w"]), np.array(v["mu"]), np.array(v["sd"])) for k, v in m["heads"].items()}

    def head(self, X, k):
        w, mu, sd = self.heads[k]
        Z = np.column_stack([np.ones(len(X)), (X - mu) / sd])
        return 1 / (1 + np.exp(-(Z @ w)))

    def __call__(self, X, acts):
        """V = sum of log heads for the acts not yet passed"""
        acts = np.asarray(acts)
        v = np.log(np.clip(self.head(X, "final"), 1e-3, 1))
        for k in (0, 1):
            v = v + np.where(acts <= k, np.log(np.clip(self.head(X, f"reach_{k}"), 1e-3, 1)), 0.0)
        return v
