"""Route survival calculator: the whole act map priced with the solver, so a route's risk is one number and its price in rewards is visible.

`python -m agent routes [--attempts N] [--pf P]` (read-only). What it does:
1. Reads the act map (`m`) and the nodes on offer now (the MAP screen).
2. Plays the current deck + relics from a grid of start HPs against the encounters that can still appear, per fight kind (weak / regular / elite / boss), and keeps the
   end HP of every attempt (death = 0): a transition matrix T_kind[hp, hp'] over the whole HP range, interpolated between the grid points.
3. Exact backward dynamic programming over (node, monsters met, elites still required, HP): F = probability of winning the act boss, assuming I choose the best child
   at every node with the HP I then have (I do re-plan at every node). A rest heals 30% of max HP; shop / treasure change nothing; an unknown room is a regular fight with
   probability `pf` (default 0.15 `[hyp]`) else nothing. Events that cost HP or give HP are NOT modelled.
4. The price of rewards: the best survival with at least k more elites (k = 0 .. elites on the map), per option on offer, and for each k the representative route (following
   the policy at the expected HP) with its fixed-path stats: P(reach the boss alive), expected HP on arrival, P(win the boss), counts of fights / elites / rests / shops /
   unknowns. Risk against reward stays a judgment: an elite is a relic plus rare odds (`sts2-pathing`), a rest is HP, a shop needs gold.
"""
import math
import re

import numpy as np

from agent import macro, pools

GRID = 9  # start-HP grid points per fight kind
KINDS = ("weak", "regular", "elite", "boss")
HEAL = 0.3
# Reward value of a node, in "one monster's card reward" units (a judgment `[hyp]`, override with --w E=4,M=1,...): an elite is a relic, a rare chance and gold (~4 cards),
# a treasure a relic, a shop buys cards / relics / a removal with the gold in hand, a rest is HP (counted low: the DP already prices HP), an unknown is an event or a fight.
WEIGHTS = {"M": 1.0, "E": 4.0, "T": 3.0, "$": 1.5, "?": 1.2, "R": 0.3}


def parse_map(text):
    """nodes[(row, col)] = dict(type, children=[(row+1, col')], visited); boss_row. Types M E R $ ? T."""
    nodes, boss_row = {}, None
    for line in text.splitlines():
        m = re.match(r"^r(\d+):\s*(.*)$", line)
        if m:
            r = int(m.group(1))
            for tok in m.group(2).split():
                t = re.match(r"^(\*?)([MER$?T])c(\d+)>([\d,]*)$", tok)
                if t:
                    kids = [(r + 1, int(c)) for c in t.group(4).split(",") if c]
                    nodes[(r, int(t.group(3)))] = dict(type=t.group(2), children=kids, visited=bool(t.group(1)))
            continue
        b = re.match(r"^boss: (\d+)", line)
        if b:
            boss_row = int(b.group(1))
    return nodes, boss_row


def offered(state_text):
    """Node coordinates on offer on the MAP screen: lines like `0 Monster r7c5 -> Rc4,Mc6`."""
    return [(int(r), int(c)) for r, c in re.findall(r"^\d+ \w+ r(\d+)c(\d+)", state_text, re.M)]


def _hist(ends, maxhp):
    h = np.zeros(maxhp + 1)
    for e in ends:
        h[int(min(maxhp, max(0, round(e))))] += 1
    return h / max(h.sum(), 1)


def transition_matrix(engine, deck_json, encounters, attempts, tag, hold=()):
    """T[hp, hp'] for a fight drawn uniformly from `encounters`, played at every start HP (grid + linear interpolation of the end-HP distributions)."""
    maxhp = deck_json["max_hp"]
    grid = sorted({max(6, int(round(x))) for x in np.linspace(6, maxhp, GRID)})
    scen, idx = [], []
    for gi, g in enumerate(grid):
        for e in encounters:
            pots = deck_json.get("potions", [])
            if hold and tag != "boss":  # potions are spent only when worth it: every non-boss fight is priced without them (a lower bound)
                pots = [] if hold == "all" else [p for p in pots if p["id"] not in hold]
            scen.append(dict(deck_json, hp=g, potions=pots, name=f"{tag}@{e}@{g}", encounter=e, seed=f"routes{tag}{g}"))
            idx.append(gi)
    res = engine.solve(scen, attempts=attempts)
    hists = []
    for gi in range(len(grid)):
        ends = [x for r, i in zip(res, idx) if i == gi for x in r["ends"]]
        hists.append(_hist(ends, maxhp))
    T = np.zeros((maxhp + 1, maxhp + 1))
    T[0, 0] = 1.0
    for hp in range(1, maxhp + 1):
        if hp <= grid[0]:
            T[hp] = hists[0]
        elif hp >= grid[-1]:
            T[hp] = hists[-1]
        else:
            j = max(i for i, g in enumerate(grid) if g <= hp)
            w = (hp - grid[j]) / (grid[j + 1] - grid[j])
            T[hp] = (1 - w) * hists[j] + w * hists[j + 1]
    return T


def build_tables(engine, deck_json, act, ctx, attempts, hold=()):
    tabs = {}
    per = dict(weak=max(8, attempts), regular=max(4, attempts // 3), elite=max(16, attempts * 2), boss=max(32, attempts * 4))
    for k in KINDS:
        encs = macro.narrow(pools.pool(act, k), k, ctx)
        if encs:
            tabs[k] = transition_matrix(engine, deck_json, encs, per[k], k, hold)
    return tabs


class Calc:
    def __init__(self, nodes, boss_row, tabs, maxhp, weak_fights, pf=0.15, goal="win"):
        self.weights = dict(WEIGHTS)
        self.goal = goal  # "win": P(win the boss); "reach": P(alive on arrival), when the boss is out of reach for the deck and the rewards on the way must close the gap
        self.nodes, self.boss_row, self.tabs, self.H, self.weak_fights, self.pf = nodes, boss_row, tabs, maxhp, weak_fights, pf
        self.heal = int(round(HEAL * maxhp))
        self.cache = {}
        self.max_elites = 0  # filled by solve

    def _terminal(self):
        if self.goal == "reach":
            v = np.ones(self.H + 1)
            v[0] = 0.0
            return v
        return self.tabs["boss"][:, 1:].sum(axis=1)

    def kids(self, key):
        r = key[0]
        if r + 1 >= self.boss_row:
            return ["BOSS"]
        return [k for k in self.nodes[key]["children"] if k in self.nodes]

    def kind_of(self, key, w):
        t = self.nodes[key]["type"]
        if t == "E":
            return "elite"
        if t == "M":
            return "weak" if w < self.weak_fights and "weak" in self.tabs else "regular"
        return None

    def F(self, key, w, k):
        """Vector over HP: P(win the boss) when about to enter `key` with `w` monsters met and `k` more elites required."""
        if key == "BOSS":
            ok = self._terminal() if k == 0 else np.zeros(self.H + 1)
            return ok
        ck = (key, w, k)
        if ck in self.cache:
            return self.cache[ck]
        t = self.nodes[key]["type"]
        fk = self.kind_of(key, w)
        w2 = min(w + 1, self.weak_fights) if t == "M" else w
        k2 = max(0, k - 1) if t == "E" else k

        def G(w_, k_):  # best child with the HP I then have
            vs = [self.F(c, w_, k_) for c in self.kids(key)]
            return np.max(vs, axis=0) if vs else np.zeros(self.H + 1)

        if fk:
            g = G(w2, k2)
            g = g.copy()
            g[0] = 0.0
            out = self.tabs[fk] @ g
        elif t == "R":
            g = G(w, k)
            idx = np.minimum(np.arange(self.H + 1) + self.heal, self.H)
            out = g[idx]
            out[0] = 0.0
        else:
            g = G(w, k)
            out = g.copy()
            if t == "?" and self.pf > 0 and "regular" in self.tabs:
                fk2 = self.kind_of_unknown(w)
                g2 = G(min(w + 1, self.weak_fights), k).copy()
                g2[0] = 0.0
                out = self.pf * (self.tabs[fk2] @ g2) + (1 - self.pf) * g
        self.cache[ck] = out
        return out

    def Rw(self, key, w):
        """Vector over HP: expected reward collected from `key` onward (rewards count only while alive), the best child at every node. Boss = 0."""
        if key == "BOSS":
            return np.zeros(self.H + 1)
        ck = ("R", key, w)
        if ck in self.cache:
            return self.cache[ck]
        t = self.nodes[key]["type"]
        r = self.weights.get(t, 0.0)
        fk = self.kind_of(key, w)
        w2 = min(w + 1, self.weak_fights) if t == "M" else w

        def G(w_):
            vs = [self.Rw(c, w_) for c in self.kids(key)]
            return np.max(vs, axis=0) if vs else np.zeros(self.H + 1)

        if fk:
            g = G(w2) + r
            g[0] = 0.0
            out = self.tabs[fk] @ g
        elif t == "R":
            g = G(w)
            idx = np.minimum(np.arange(self.H + 1) + self.heal, self.H)
            out = r + g[idx]
            out[0] = 0.0
        else:
            g = G(w)
            out = r + g
            if t == "?" and self.pf > 0 and "regular" in self.tabs:
                g2 = G(min(w + 1, self.weak_fights)) + r
                g2[0] = 0.0
                out = self.pf * (self.tabs[self.kind_of_unknown(w)] @ g2) + (1 - self.pf) * (r + g)
            out[0] = 0.0
        self.cache[ck] = out
        return out

    def reward_path(self, start, w, hp):
        """Follow the best child of the reward DP at the expected HP."""
        path, key, cur = [], start, float(hp)
        while key != "BOSS":
            path.append(key)
            t = self.nodes[key]["type"]
            fk = self.kind_of(key, w)
            if t == "M":
                w = min(w + 1, self.weak_fights)
            if fk:
                row = self.tabs[fk][max(1, min(self.H, int(round(cur))))]
                cur = float((row[1:] * np.arange(1, self.H + 1)).sum() / max(row[1:].sum(), 1e-9))
            elif t == "R":
                cur = min(self.H, cur + self.heal)
            kids = self.kids(key)
            hp_i = max(1, min(self.H, int(round(cur))))
            key = max(kids, key=lambda c: self.Rw(c, w)[hp_i]) if kids else "BOSS"
        return path

    def kind_of_unknown(self, w):
        return "weak" if w < self.weak_fights and "weak" in self.tabs else "regular"

    # ------------------------------------------------------------------------------------------------------------ fixed path statistics
    def policy_path(self, start, w, k, hp):
        """Follow the best child at the expected HP (rounded). Returns the node list from `start` to the last node before the boss."""
        path, key, cur = [], start, float(hp)
        while key != "BOSS":
            path.append(key)
            t = self.nodes[key]["type"]
            fk = self.kind_of(key, w)
            if t == "M":
                w = min(w + 1, self.weak_fights)
            if t == "E":
                k = max(0, k - 1)
            hp_i = int(round(cur))
            if fk:
                row = self.tabs[fk][max(1, min(self.H, hp_i))]
                alive = row[1:].sum()
                cur = float((row[1:] * np.arange(1, self.H + 1)).sum() / max(alive, 1e-9))
            elif t == "R":
                cur = min(self.H, cur + self.heal)
            kids = self.kids(key)
            hp_i = int(round(cur))
            key = max(kids, key=lambda c: self.F(c, w, k)[max(1, min(self.H, hp_i))]) if kids else "BOSS"
        return path

    def path_stats(self, path, w, hp):
        """Fixed path: P(reach the boss alive), E[HP on arrival | alive], P(win the boss), HP quantiles on arrival."""
        d = np.zeros(self.H + 1)
        d[max(1, min(self.H, int(hp)))] = 1.0
        counts = dict(M=0, E=0, R=0, **{"$": 0, "?": 0, "T": 0})
        elites = []  # (node, P(alive on arrival), mean HP, q10 HP) at every elite on the path: two elites without a rest between them show up as a falling mean
        for key in path:
            t = self.nodes[key]["type"]
            counts[t] += 1
            if t == "E":
                al = d[1:].sum()
                if al > 1e-9:
                    cd = np.cumsum(d[1:]) / al
                    elites.append((key, float(al), float((d[1:] * np.arange(1, self.H + 1)).sum() / al), int(np.searchsorted(cd, 0.1) + 1)))
            fk = self.kind_of(key, w)
            if t == "M":
                w = min(w + 1, self.weak_fights)
            if fk:
                d = d @ self.tabs[fk]
            elif t == "R":
                nd = np.zeros_like(d)
                for h in range(self.H + 1):
                    nd[min(self.H, h + self.heal) if h > 0 else 0] += d[h]
                d = nd
            elif t == "?" and self.pf > 0 and "regular" in self.tabs:
                d = (1 - self.pf) * d + self.pf * (d @ self.tabs[self.kind_of_unknown(w)])
        alive = d[1:].sum()
        mean = float((d[1:] * np.arange(1, self.H + 1)).sum() / max(alive, 1e-9))
        cdf = np.cumsum(d[1:]) / max(alive, 1e-9)
        q = lambda p: int(np.searchsorted(cdf, p) + 1)
        pwin = float(d @ self.tabs["boss"][:, 1:].sum(axis=1))  # the boss win probability whatever the goal
        return dict(alive=float(alive), mean_hp=mean, q10=q(0.1), q50=q(0.5), win=pwin, counts=counts, elites=elites)


def _fmt_path(calc, path):
    return " ".join(calc.nodes[k]["type"] + f"{k[0]}c{k[1]}" for k in path)


def analyse(engine, deck_json, map_text, state_text, ctx, act, attempts=24, pf=0.15, tabs=None, weights=None, hold=()):
    nodes, boss_row = parse_map(map_text)
    if not nodes or boss_row is None:
        return "routes: no act map"
    hp_now = int(re.search(r"HP (\d+)/", state_text).group(1)) if re.search(r"HP (\d+)/", state_text) else deck_json["hp"]
    maxhp = deck_json["max_hp"]
    if tabs is None:
        tabs = build_tables(engine, deck_json, act, ctx, attempts, hold)
    if "boss" not in tabs:
        return "routes: boss unknown"
    calc = Calc(nodes, boss_row, tabs, maxhp, pools.ACTS[act]["weak_fights"], pf)
    calc.weights.update(weights or {})
    note = ""
    starts = [k for k in offered(state_text) if k in nodes]
    if not starts:  # not on a map screen: every unvisited node of the first unvisited row that is reachable from the visited ones
        vis = [k for k, v in nodes.items() if v["visited"]]
        cur = max(vis, default=None)
        starts = [c for c in (nodes[cur]["children"] if cur else []) if c in nodes] if cur else [k for k in nodes if k[0] == min(r for r, _ in nodes)]
    not_monster = set(pools.pool(act, "elite")) | set(pools.pool(act, "boss"))
    w0 = min(sum(1 for e in ctx.get("seen", []) if e not in not_monster), calc.weak_fights)
    if max(calc.F(s, w0, 0)[hp_now] for s in starts) < 0.05:
        calc = Calc(nodes, boss_row, tabs, maxhp, pools.ACTS[act]["weak_fights"], pf, goal="reach")
        calc.weights.update(weights or {})
        note = "BOSS OUT OF REACH for this deck (best route < 0.05 win): F below is P(reach the boss alive); the win column is the real boss win. Rewards on the way must close the gap."
    max_e = 0
    # most elites on any path (upper bound for k): count elite nodes ahead
    for k in range(0, 5):
        if max(calc.F(s, w0, k)[hp_now] for s in starts) > 0:
            max_e = k
        else:
            break
    lines = [f"route survival (win the act boss) from {hp_now}/{maxhp} HP, {len(starts)} option(s) on offer, monsters met {w0}, unknown = regular fight {pf:.0%}; "
             f"events / shops / treasure not modelled", ""]
    if note:
        lines.append(note)
        lines.append("")
    lines.append(f"per option on offer: P({'reach boss alive' if calc.goal == 'reach' else 'win boss'}) with at least k more elites on the way (adaptive: best child at each node)")
    head = f"{'option':10s}" + "".join(f"{'k>=' + str(k):>8s}" for k in range(max_e + 1))
    lines.append(head)
    for s in starts:
        lines.append(f"{nodes[s]['type'] + str(s[0]) + 'c' + str(s[1]):10s}" + "".join(f"{calc.F(s, w0, k)[hp_now]:8.3f}" for k in range(max_e + 1)))
    lines.append("")
    lines.append("representative route per elite requirement (fixed path from the best option; stats of that path)")
    lines.append(f"{'k':>2s} {'F':>6s} {'reach':>6s} {'HP@boss':>8s} {'q10':>4s} {'win':>6s}  fights elites rests shops unk treas  path")
    for k in range(max_e + 1):
        best = max(starts, key=lambda s: calc.F(s, w0, k)[hp_now])
        path = calc.policy_path(best, w0, k, hp_now)
        st = calc.path_stats(path, w0, hp_now)
        c = st["counts"]
        fights = c["M"] + c["E"]
        lines.append(f"{k:2d} {calc.F(best, w0, k)[hp_now]:6.3f} {st['alive']:6.3f} {st['mean_hp']:8.1f} {st['q10']:4d} {st['win']:6.3f}  {fights:6d} {c['E']:6d} {c['R']:5d} {c['$']:5d} {c['?']:3d} {c['T']:5d}  {_fmt_path(calc, path)}")
        if st["elites"]:
            lines.append("      elite arrivals (alive, mean HP, q10 HP): " + "; ".join(f"{calc.nodes[e[0]]['type']}{e[0][0]}c{e[0][1]} {e[1]:.2f}, {e[2]:.0f}, {e[3]}" for e in st["elites"]))
    lines.append("")
    wtxt = " ".join(f"{k}={v:g}" for k, v in calc.weights.items())
    lines.append(f"reward-weighted routes (expected rewards collected while alive; weights {wtxt}; change with --w E=5,$=1): per option on offer, then the best path")
    lines.append(f"{'option':10s} {'reward':>7s} {'reach':>6s} {'HP@boss':>8s} {'win':>6s}  fights elites rests shops unk treas  path")
    ropts = sorted(starts, key=lambda s_: -calc.Rw(s_, w0)[hp_now])
    for s_ in ropts:
        path = calc.reward_path(s_, w0, hp_now)
        st = calc.path_stats(path, w0, hp_now)
        c = st["counts"]
        lines.append(f"{nodes[s_]['type'] + str(s_[0]) + 'c' + str(s_[1]):10s} {calc.Rw(s_, w0)[hp_now]:7.2f} {st['alive']:6.3f} {st['mean_hp']:8.1f} {st['win']:6.3f}  {c['M'] + c['E']:6d} {c['E']:6d} {c['R']:5d} {c['$']:5d} {c['?']:3d} {c['T']:5d}  {_fmt_path(calc, path)}")
    lines.append("")
    lines.append("F = adaptive optimum for that requirement (can exceed the fixed path's win); reach = alive on arrival at the boss; HP@boss / q10 = mean / 10th percentile HP then.")
    return "\n".join(lines)
