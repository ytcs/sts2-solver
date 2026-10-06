"""Route survival calculator: the whole act map priced with the solver, so a route's risk is one number and its price in rewards is visible.

`python -m agent routes [--attempts N] [--pf P]` (read-only). What it does:
1. Reads the act map (`m`) and the nodes on offer now (the MAP screen).
2. Plays the current deck + relics from a grid of start HPs against the encounters that can still appear, per fight kind (weak / regular / elite / boss), and keeps the
   end HP of every attempt (death = 0): a transition matrix T[hp, hp'] over the whole HP range, interpolated between the grid points. The belt is a BUDGET: every non-boss
   kind gets a table with no potion and one per potion (that potion alone available), the boss one per subset of the belt.
3. Exact backward dynamic programming over (node, monsters met, elites still required, potions still unspent, HP): F = probability of winning the act boss, assuming I choose
   the best child at every node with the HP I then have (I do re-plan at every node) and, at every fight, whether to throw one of the remaining potions: a potion counts at the
   fight where it helps most (an elite or the boss) and only once along a route. A potion chosen for a fight is assumed spent (the table was played with it available; the solver
   may not have needed it, so what is left for later is a lower bound). A rest heals 30% of max HP; shop / treasure change nothing; an unknown room is a regular fight with
   probability `pf` (default 0.15 `[hyp]`), played without a potion, else nothing. Events that cost HP or give HP are NOT modelled.
4. The price of rewards: the best survival with at least k more elites (k = 0 .. elites on the map), per option on offer, and for each k the representative route (following
   the policy at the expected HP) with its fixed-path stats: P(reach the boss alive), expected HP on arrival, P(win the boss), counts of fights / elites / rests / shops /
   unknowns, and the potion plan. Risk against reward stays a judgment: an elite is a relic plus rare odds (`sts2-pathing`), a rest is HP, a shop needs gold.
"""
import json
import re
from itertools import combinations

import numpy as np

from agent import macro, pools, screen

GRID = 9  # start-HP grid points per fight kind
KINDS = ("weak", "regular", "elite", "boss")
HEAL = 0.3
MAX_BELT = 3  # potions priced as a budget (the belt has 2-3 slots; the boss tables grow as 2^n)
# Reward value of a node, in "one monster's card reward" units (a judgment `[hyp]`, override with --w E=4,M=1,...): an elite is a relic, a rare chance and gold (~4 cards),
# a treasure a relic, a shop buys cards / relics / a removal with the gold in hand, a rest is HP (counted low: the DP already prices HP), an unknown is an event or a fight.
# An elite: a relic (3.5) + a card reward with ~3x the rare odds (~1.3) = 5 (its gold is counted through the shops below); a treasure: a relic. "$" is a MULTIPLIER on the
# shop's value, which is what the gold I arrive with buys (`shop_buy`).
WEIGHTS = {"M": 1.0, "E": 5.0, "T": 3.5, "$": 1.0, "?": 1.2, "R": 0.3}
NONE = frozenset()
# Gold on the way `[code]` (A10 Poverty x0.75): monster 7-15 (EncounterModel.MinGoldReward/MaxGoldReward 10-20), elite 26-33 (35-45), treasure 31-39
# (OneOffSynchronizer.DoTreasureRoomRewards 42-52); unknowns `[hyp]` ~10.
GOLD_GAIN = {"M": 11, "E": 30, "?": 10, "T": 35, "R": 0, "$": 0}
GOLD_STEP, GOLD_CAP = 10, 600
# What a shop sells `[code]` (MerchantInventory, MerchantCardEntry, MerchantRelicEntry, MerchantPotionEntry, MerchantCardRemovalEntry): 5 class cards at 50 / 75 / 150
# (common / uncommon / rare, x0.95-1.05, one of them on sale at half), 2 colorless (x1.15), 3 relics at 175 / 225 / 275 (x0.85-1.15), 3 potions at 50 / 75 / 100, a removal
# at 100 (+50 per removal bought, A10 Inflation). Value per item in card-reward units `[hyp]`: the best card of the 7 ~1.0 (about 60 gold with the sale), a second card 0.6,
# a removal 0.8, a relic 3.5 (~225), a potion 0.4.
SHOP_ITEMS = (("card", 60, 1.0), ("card2", 75, 0.6), ("removal", 100, 0.8), ("relic", 225, 3.5), ("potion", 60, 0.4))


def shop_buy(g):
    """(value, gold spent) of the best basket a shop sells for g gold (exact over the 32 baskets of SHOP_ITEMS): below ~60 gold a shop buys nothing, 60-160 one or two cheap
    items, ~225 a relic."""
    best = (0.0, 0)
    n = len(SHOP_ITEMS)
    for m in range(1 << n):
        cost = sum(SHOP_ITEMS[i][1] for i in range(n) if m >> i & 1)
        if cost <= g:
            v = sum(SHOP_ITEMS[i][2] for i in range(n) if m >> i & 1)
            if v > best[0] + 1e-9 or (abs(v - best[0]) < 1e-9 and cost < best[1]):
                best = (v, cost)
    return best


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
    return [(int(m.group(1)), int(m.group(2))) for _, label in screen.options(state_text) for m in [re.match(r"\w+ r(\d+)c(\d+)", label)] if m]


def subsets(n):
    return [frozenset(c) for r in range(n + 1) for c in combinations(range(n), r)]


def _hist(ends, maxhp):
    h = np.zeros(maxhp + 1)
    for e in ends:
        h[int(min(maxhp, max(0, round(e))))] += 1
    return h / max(h.sum(), 1)


def _matrix(hists, grid, maxhp):
    """T[hp, hp'] from the end-HP histograms at the grid start HPs (linear interpolation between grid points)."""
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


UNIT = 4  # attempts per scenario in the one big batch: a table that wants more attempts gets replicated scenarios (the solver's job ids differ, so the replicas are independent)
_CACHE = {}  # signature -> tables: they depend on the deck, relics, belt and what can still appear, not on my HP, so repeated `routes` calls at the same deck cost nothing


def _signature(deck_json, act, ctx, attempts):
    deck = tuple(sorted((c["id"], c.get("upgrade", 0), json.dumps(c.get("enchantment"), sort_keys=True)) for c in deck_json["deck"]))
    relics = tuple((r["id"], json.dumps(r.get("props"), sort_keys=True), r.get("counter")) for r in deck_json.get("relics", []))
    pots = tuple(p["id"] for p in deck_json.get("potions", []))
    return (deck, relics, pots, deck_json["max_hp"], act, tuple(ctx.get("seen", [])), tuple(ctx.get("bosses", [])), attempts)


def build_tables(engine, deck_json, act, ctx, attempts):
    """tabs[(kind, S)] = transition matrix with the potions S (a frozenset of belt indices) available; tabs["_belt"] = the potion ids. One batched solver call for every table.
    Potion variants exist for elites and the boss only: a potion thrown in a hallway fight is not what the route decision is about."""
    key = _signature(deck_json, act, ctx, attempts)
    if key in _CACHE:
        return _CACHE[key]
    maxhp = deck_json["max_hp"]
    belt = list(deck_json.get("potions", []))[:MAX_BELT]
    per = dict(weak=max(8, attempts), regular=max(4, attempts // 3), elite=max(16, attempts * 2), boss=max(32, attempts * 4))
    grid = sorted({max(6, int(round(x))) for x in np.linspace(6, maxhp, GRID)})
    scen, owners, tables = [], [], []
    for k in KINDS:
        encs = macro.narrow(pools.pool(act, k), k, ctx)
        if not encs:
            continue
        if k == "boss":
            sets = subsets(len(belt))
        elif k == "elite":
            sets = [NONE] + [frozenset({i}) for i in range(len(belt))]
        else:
            sets = [NONE]
        reps = max(1, per[k] // UNIT)
        for S in sets:
            tables.append((k, S))
            pots = [belt[i] for i in sorted(S)]
            for gi, g in enumerate(grid):
                for e in encs:
                    for r in range(reps):
                        scen.append(dict(deck_json, hp=g, potions=pots, name=f"{k}@{e}@{g}#{r}", encounter=e, seed=f"routes{k}{g}"))
                        owners.append((k, S, gi))
    res = engine.solve(scen, attempts=UNIT)
    ends = {}
    for r, (k, S, gi) in zip(res, owners):
        ends.setdefault((k, S, gi), []).extend(r["ends"])
    tabs = {"_belt": [p["id"] for p in belt]}
    for k, S in tables:
        tabs[(k, S)] = _matrix([_hist(ends[(k, S, gi)], maxhp) for gi in range(len(grid))], grid, maxhp)
    if len(_CACHE) >= 3:
        _CACHE.pop(next(iter(_CACHE)))
    _CACHE[key] = tabs
    return tabs


class Calc:
    def __init__(self, nodes, boss_row, tabs, maxhp, weak_fights, pf=0.15, goal="win"):
        self.weights = dict(WEIGHTS)
        self.goal = goal  # "win": P(win the boss); "reach": P(alive on arrival), when the boss is out of reach for the deck and the rewards on the way must close the gap
        self.nodes, self.boss_row, self.tabs, self.H, self.weak_fights, self.pf = nodes, boss_row, tabs, maxhp, weak_fights, pf
        self.belt = tabs.get("_belt", [])
        self.full = frozenset(range(len(self.belt)))
        self.heal = int(round(HEAL * maxhp))
        self.cache = {}

    def has(self, kind):
        return (kind, NONE) in self.tabs

    def T(self, kind, S=NONE):
        return self.tabs[(kind, S)]

    def _terminal(self, S):
        if self.goal == "reach":
            v = np.ones(self.H + 1)
            v[0] = 0.0
            return v
        return self.T("boss", S)[:, 1:].sum(axis=1)

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
            return "weak" if w < self.weak_fights and self.has("weak") else "regular"
        return None

    def kind_of_unknown(self, w):
        return "weak" if w < self.weak_fights and self.has("weak") else "regular"

    def _options(self, fk, S):
        """The ways to play a fight of kind fk with the potions S still unspent: (None, S) = no potion, (i, S - {i}) = throw potion i."""
        opts = [(None, S)]
        for i in sorted(S):
            if (fk, frozenset({i})) in self.tabs:
                opts.append((i, S - {i}))
        return opts

    def F(self, key, w, k, S=None):
        """Vector over HP: P(win the boss) when about to enter `key` with `w` monsters met, `k` more elites required and the potions S unspent (default: the whole belt)."""
        S = self.full if S is None else S
        if key == "BOSS":
            return self._terminal(S) if k == 0 else np.zeros(self.H + 1)
        ck = (key, w, k, S)
        if ck in self.cache:
            return self.cache[ck]
        t = self.nodes[key]["type"]
        fk = self.kind_of(key, w)
        w2 = min(w + 1, self.weak_fights) if t == "M" else w
        k2 = max(0, k - 1) if t == "E" else k

        def G(w_, k_, S_):  # best child with the HP I then have
            vs = [self.F(c, w_, k_, S_) for c in self.kids(key)]
            return np.max(vs, axis=0) if vs else np.zeros(self.H + 1)

        if fk:
            out = None
            for i, S2 in self._options(fk, S):
                g = G(w2, k2, S2).copy()
                g[0] = 0.0
                v = self.T(fk, NONE if i is None else frozenset({i})) @ g
                out = v if out is None else np.maximum(out, v)  # the best way to play this fight, chosen with the HP I arrive with
        elif t == "R":
            g = G(w, k, S)
            idx = np.minimum(np.arange(self.H + 1) + self.heal, self.H)
            out = g[idx]
            out[0] = 0.0
        else:
            g = G(w, k, S)
            out = g.copy()
            if t == "?" and self.pf > 0 and self.has("regular"):
                fk2 = self.kind_of_unknown(w)
                g2 = G(min(w + 1, self.weak_fights), k, S).copy()
                g2[0] = 0.0
                out = self.pf * (self.T(fk2) @ g2) + (1 - self.pf) * g
        self.cache[ck] = out
        return out

    @staticmethod
    def _gold_after(t, g):
        """Gold leaving a node of type t entered with g (expected income; a shop spends what its best basket costs), on the GOLD_STEP grid."""
        g = g - shop_buy(g)[1] if t == "$" else g + GOLD_GAIN.get(t, 0)
        return min(GOLD_CAP, int(round(g / GOLD_STEP)) * GOLD_STEP)

    def Rw(self, key, w, g=0):
        """Vector over HP: expected reward collected from `key` onward (rewards count only while alive), the best child at every node, every fight played without a potion. Boss = 0.
        g = gold on entering `key`: a shop's weight is scaled by shop_scale(g)."""
        if key == "BOSS":
            return np.zeros(self.H + 1)
        g = min(GOLD_CAP, int(round(g / GOLD_STEP)) * GOLD_STEP)
        ck = ("R", key, w, g)
        if ck in self.cache:
            return self.cache[ck]
        t = self.nodes[key]["type"]
        r = self.weights.get(t, 0.0) * (shop_buy(g)[0] if t == "$" else 1.0)
        fk = self.kind_of(key, w)
        w2 = min(w + 1, self.weak_fights) if t == "M" else w
        g2 = self._gold_after(t, g)

        def G(w_):
            vs = [self.Rw(c, w_, g2) for c in self.kids(key)]
            return np.max(vs, axis=0) if vs else np.zeros(self.H + 1)

        if fk:
            g = G(w2) + r
            g[0] = 0.0
            out = self.T(fk) @ g
        elif t == "R":
            g = G(w)
            idx = np.minimum(np.arange(self.H + 1) + self.heal, self.H)
            out = r + g[idx]
            out[0] = 0.0
        else:
            g = G(w)
            out = r + g
            if t == "?" and self.pf > 0 and self.has("regular"):
                g2 = G(min(w + 1, self.weak_fights)) + r
                g2[0] = 0.0
                out = self.pf * (self.T(self.kind_of_unknown(w)) @ g2) + (1 - self.pf) * (r + g)
            out[0] = 0.0
        self.cache[ck] = out
        return out

    def reward_path(self, start, w, hp, g=0):
        """Follow the best child of the reward DP at the expected HP (and the expected gold)."""
        path, key, cur = [], start, float(hp)
        while key != "BOSS":
            path.append(key)
            t = self.nodes[key]["type"]
            fk = self.kind_of(key, w)
            g = self._gold_after(t, g)
            if t == "M":
                w = min(w + 1, self.weak_fights)
            if fk:
                row = self.T(fk)[max(1, min(self.H, int(round(cur))))]
                cur = float((row[1:] * np.arange(1, self.H + 1)).sum() / max(row[1:].sum(), 1e-9))
            elif t == "R":
                cur = min(self.H, cur + self.heal)
            kids = self.kids(key)
            hp_i = max(1, min(self.H, int(round(cur))))
            key = max(kids, key=lambda c: self.Rw(c, w, g)[hp_i]) if kids else "BOSS"
        return path

    # ------------------------------------------------------------------------------------------------------------ fixed path statistics
    def policy_path(self, start, w, k, hp):
        """Follow the DP at the expected HP: the best child and, at every fight, the best way to play it (a potion or none). Returns (nodes from `start` to the last node before
        the boss, plan = {node: potion index thrown there}); the potions left over go to the boss."""
        path, plan, key, cur, S = [], {}, start, float(hp), self.full
        while key != "BOSS":
            path.append(key)
            t = self.nodes[key]["type"]
            fk = self.kind_of(key, w)
            w_in = w
            if t == "M":
                w = min(w + 1, self.weak_fights)
            if t == "E":
                k = max(0, k - 1)
            hp_i = max(1, min(self.H, int(round(cur))))
            if fk:
                best, best_v = (None, S), -1.0
                for i, S2 in self._options(fk, S):
                    kids = self.kids(key)
                    g = (np.max([self.F(c, w, k, S2) for c in kids], axis=0) if kids else np.zeros(self.H + 1)).copy()
                    g[0] = 0.0
                    v = float((self.T(fk, NONE if i is None else frozenset({i})) @ g)[hp_i])
                    if v > best_v + 1e-9:
                        best, best_v = (i, S2), v
                i, S = best
                if i is not None:
                    plan[key] = i
                row = self.T(fk, NONE if i is None else frozenset({i}))[hp_i]
                cur = float((row[1:] * np.arange(1, self.H + 1)).sum() / max(row[1:].sum(), 1e-9))
            elif t == "R":
                cur = min(self.H, cur + self.heal)
            kids = self.kids(key)
            hp_i = max(1, min(self.H, int(round(cur))))
            key = max(kids, key=lambda c: self.F(c, w, k, S)[hp_i]) if kids else "BOSS"
        plan["BOSS"] = S
        return path, plan

    def path_stats(self, path, w, hp, plan=None):
        """Fixed path (and potion plan): P(reach the boss alive), E[HP on arrival | alive], P(win the boss), HP quantiles on arrival. Without a plan no potion is thrown before the boss."""
        plan = plan or {}
        d = np.zeros(self.H + 1)
        d[max(1, min(self.H, int(hp)))] = 1.0
        counts = dict(M=0, E=0, R=0, **{"$": 0, "?": 0, "T": 0})
        elites = []  # (node, P(alive on arrival), mean HP, q10 HP) at every elite on the path: two elites without a rest between them show up as a falling mean
        used = set()
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
                i = plan.get(key)
                if i is not None:
                    used.add(i)
                d = d @ self.T(fk, NONE if i is None else frozenset({i}))
            elif t == "R":
                nd = np.zeros_like(d)
                for h in range(self.H + 1):
                    nd[min(self.H, h + self.heal) if h > 0 else 0] += d[h]
                d = nd
            elif t == "?" and self.pf > 0 and self.has("regular"):
                d = (1 - self.pf) * d + self.pf * (d @ self.T(self.kind_of_unknown(w)))
        alive = d[1:].sum()
        mean = float((d[1:] * np.arange(1, self.H + 1)).sum() / max(alive, 1e-9))
        cdf = np.cumsum(d[1:]) / max(alive, 1e-9)
        q = lambda p: int(np.searchsorted(cdf, p) + 1)
        S_boss = self.full - used
        pwin = float(d @ self.T("boss", S_boss)[:, 1:].sum(axis=1))  # the boss win probability whatever the goal, with the potions left
        return dict(alive=float(alive), mean_hp=mean, q10=q(0.1), q50=q(0.5), win=pwin, counts=counts, elites=elites, boss_potions=sorted(S_boss))


def _fmt_path(calc, path):
    return " ".join(calc.nodes[k]["type"] + f"{k[0]}c{k[1]}" for k in path)


def _fmt_plan(calc, plan):
    if not calc.belt:
        return "no potions"
    parts = [f"{calc.belt[i]} at {calc.nodes[n]['type']}{n[0]}c{n[1]}" for n, i in plan.items() if n != "BOSS"]
    left = [calc.belt[i] for i in sorted(plan.get("BOSS", NONE))]
    if left:
        parts.append(f"{', '.join(left)} at the boss")
    return "; ".join(parts) or "no potion thrown"


def continuation_util(engine, deck_json, map_text, ctx, act, attempts=24, pf=0.15):
    """What each ending of the fight at the current map node is worth for the rest of the act: the HP-worth curve (`rl/utility.py`, 101 floats over the HP
    fraction) of V(hp) = P(win the act boss | leave this node with hp), best child, at least 0 more elites; P(reach the boss alive) when the boss is out of
    reach for the deck. Returns (curve or None, one-line description)."""
    import utility  # rl/ (on the path through agent.engine)
    nodes, boss_row = parse_map(map_text)
    vis = [k for k, v in nodes.items() if v["visited"]]
    if not nodes or boss_row is None or not vis:
        return None, "no map position"
    cur = max(vis)
    tabs = build_tables(engine, deck_json, act, ctx, attempts)
    if ("boss", NONE) not in tabs:
        return None, "boss unknown"
    maxhp = deck_json["max_hp"]
    not_monster = set(pools.pool(act, "elite")) | set(pools.pool(act, "boss"))
    w0 = min(sum(1 for e in ctx.get("seen", []) if e not in not_monster), pools.ACTS[act]["weak_fights"])
    goal = "win"
    for goal in ("win", "reach"):
        calc = Calc(nodes, boss_row, tabs, maxhp, pools.ACTS[act]["weak_fights"], pf, goal=goal)
        kids = calc.kids(cur)
        V = np.max([calc.F(c, w0, 0) for c in kids], axis=0) if kids else np.zeros(maxhp + 1)
        if V[maxhp] >= 0.05:
            break
    if V.max() <= 1e-6:
        return None, "no continuation value"
    u = utility.from_values(V)
    pts = " ".join(f"{p}%:{u[p]:.2f}" for p in (10, 25, 50, 75, 100))
    return u, f"HP worth = P({goal} the act boss) from the next node, relative to full HP: {pts}"


def analyse(engine, deck_json, map_text, state_text, ctx, act, attempts=24, pf=0.15, tabs=None, weights=None):
    nodes, boss_row = parse_map(map_text)
    if not nodes or boss_row is None:
        return "routes: no act map"
    hp_now = (screen.hp(state_text) or (deck_json["hp"],))[0]
    maxhp = deck_json["max_hp"]
    if tabs is None:
        tabs = build_tables(engine, deck_json, act, ctx, attempts)
    if ("boss", NONE) not in tabs:
        return "routes: boss unknown"
    calc = Calc(nodes, boss_row, tabs, maxhp, pools.ACTS[act]["weak_fights"], pf)
    calc.weights.update(weights or {})
    note = ""
    starts = [k for k in offered(state_text) if k in nodes]
    if not starts:  # not on a map screen: every unvisited node of the first unvisited row that is reachable from the visited ones
        vis = [k for k, v in nodes.items() if v["visited"]]
        cur = max(vis, default=None)
        starts = [c for c in (nodes[cur]["children"] if cur else []) if c in nodes] if cur else [k for k in nodes if k[0] == min(r for r, _ in nodes)]
    if not starts:
        return "routes: no node left before the boss: the next fight is the boss, price rest vs smith with `eval --boss` at both HPs"
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
    belt_txt = ", ".join(calc.belt) if calc.belt else "none"
    lines = [f"route survival (win the act boss) from {hp_now}/{maxhp} HP, {len(starts)} option(s) on offer, monsters met {w0}, unknown = regular fight {pf:.0%}; "
             f"events / shops / treasure not modelled", f"potions in the belt: {belt_txt}; each is thrown at most once along a route, at the fight where it helps most (an elite or the boss), never counted twice", ""]
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
        path, plan = calc.policy_path(best, w0, k, hp_now)
        st = calc.path_stats(path, w0, hp_now, plan)
        c = st["counts"]
        fights = c["M"] + c["E"]
        lines.append(f"{k:2d} {calc.F(best, w0, k)[hp_now]:6.3f} {st['alive']:6.3f} {st['mean_hp']:8.1f} {st['q10']:4d} {st['win']:6.3f}  {fights:6d} {c['E']:6d} {c['R']:5d} {c['$']:5d} {c['?']:3d} {c['T']:5d}  {_fmt_path(calc, path)}")
        lines.append(f"      potion plan: {_fmt_plan(calc, plan)}")
        if st["elites"]:
            lines.append("      elite arrivals (alive, mean HP, q10 HP): " + "; ".join(f"{calc.nodes[e[0]]['type']}{e[0][0]}c{e[0][1]} {e[1]:.2f}, {e[2]:.0f}, {e[3]}" for e in st["elites"]))
    lines.append("")
    gold = screen.gold(state_text) or 0
    wtxt = " ".join(f"{k}={v:g}" for k, v in calc.weights.items())
    lines.append(f"reward-weighted routes (expected rewards collected while alive, fights without potions; weights {wtxt}; change with --w E=5,$=1): per option on offer, then the best path")
    lines.append(f"  a shop is worth what the gold I arrive with buys ($ = multiplier): from {gold} gold now, +{GOLD_GAIN['M']} per monster, +{GOLD_GAIN['E']} per elite, +{GOLD_GAIN['T']} per treasure; "
                 "shop value at 50/100/150/250/400 gold: " + "/".join(f"{shop_buy(x)[0]:.1f}" for x in (50, 100, 150, 250, 400)))
    lines.append(f"{'option':10s} {'reward':>7s} {'reach':>6s} {'HP@boss':>8s} {'win':>6s}  fights elites rests shops unk treas  path")
    ropts = sorted(starts, key=lambda s_: -calc.Rw(s_, w0, gold)[hp_now])
    for s_ in ropts:
        path = calc.reward_path(s_, w0, hp_now, gold)
        st = calc.path_stats(path, w0, hp_now)
        c = st["counts"]
        lines.append(f"{nodes[s_]['type'] + str(s_[0]) + 'c' + str(s_[1]):10s} {calc.Rw(s_, w0, gold)[hp_now]:7.2f} {st['alive']:6.3f} {st['mean_hp']:8.1f} {st['win']:6.3f}  {c['M'] + c['E']:6d} {c['E']:6d} {c['R']:5d} {c['$']:5d} {c['?']:3d} {c['T']:5d}  {_fmt_path(calc, path)}")
    lines.append("")
    lines.append("F = adaptive optimum for that requirement (can exceed the fixed path's win); reach = alive on arrival at the boss; HP@boss / q10 = mean / 10th percentile HP then.")
    return "\n".join(lines)
