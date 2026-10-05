"""Crystal Sphere event minigame: faithful simulator, posterior sampler, click policies, benchmark (stdlib + numpy only).

Source of truth (decompiled sts2.dll, `[code]`): Events/Custom/CrystalSphereEvent/CrystalSphereMinigame.cs, CrystalSphereItem.cs, CrystalSphereItems/*, Models/Events/CrystalSphere.cs,
Multiplayer/Game/OneOffSynchronizer.cs (rewards), Models/Cards/Doubt.cs and Debt.cs. Skill: `.claude/skills/sts2-crystal-sphere/SKILL.md`.

Faithful rules: 11x11 grid, cell index = x*11+y (x column, y row); the 4 corners plus two rounds of horizontal/vertical neighbour expansion start clear (6 cells per
corner); 15 items placed in a fixed order, each at a uniformly random top-left anchor among the anchors whose whole footprint is still fog and free (no adjacency rule;
items may touch); a failed placement aborts the pass, the pass repeats up to 10 times and keeps the items of the earlier passes (their cells stay occupied). Big tool clears the
3x3 block clipped at the edge, Small one cell; an item pays when its last cell clears; rewards come at the end; the curse (Doubt) is added the moment it is revealed.

Public information only for policies: which cells are clear and which item KIND shows through a clear cell (R P C X g), never the hidden anchors. A policy's observation is
`(hidden (11,11) bool, kinds (11,11) int8)`; kinds[x,y] != 0 only on clear cells covered by an item.

CLI (never touches the game):
    python agent/crystal_sphere.py advise [--n N] [--k K]  < grid.txt     # best clicks for the bridge's CRYSTAL_SPHERE text
    python agent/crystal_sphere.py bench --n 3 --games 20000 [--policies a,b,c] [--procs 16]
    python agent/crystal_sphere.py opening --n 6                          # the opening clicks of the playbook policy
"""
import argparse
import json
import os
import re
import sys
import time

for _v in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS"):  # one BLAS thread per process: the benchmark runs one process per core
    os.environ.setdefault(_v, "1")

import numpy as np

W = 11
NC = W * W
KIND = {"R": 1, "P": 2, "C": 3, "X": 4, "g": 5}
LETTER = {v: k for k, v in KIND.items()}
# generative order of CrystalSphereMinigame.PopulateItems: (name, kind, width, height)
ITEMS = (
    [("relic", "R", 4, 4)]
    + [("potion_c", "P", 1, 3)] * 2
    + [("potion_r", "P", 2, 2)]
    + [("card_c", "C", 2, 2), ("card_u", "C", 2, 2), ("card_r", "C", 2, 2)]
    + [("curse", "X", 2, 2)]
    + [("gold_s", "g", 1, 1)] * 5
    + [("gold_b", "g", 2, 1)] * 2
)
NI = len(ITEMS)
CURSE_I = 7
KIND_OF = np.array([KIND[k] for _, k, _, _ in ITEMS])

# value of each revealed item in gold-equivalents. `[code]` prices / `[hyp]` worth: see the skill ("Values").
#   relic: RelicReward random rarity 50/33/17 %, merchant cost 175/225/275 -> 208.5
#   potion: merchant cost common 50, rare 100;  card reward (pick 1 of 3 of that rarity, skip allowed): merchant cost 50 / 75 / 150
#   gold: 10 / 30 face;  curse Doubt: a removal (75, 100 at inflation ascensions) incl. the dead card
VALUES = np.array([208.5, 50, 50, 100, 50, 75, 150, -100] + [10] * 5 + [30] * 2, dtype=np.float64)
DEBT_COST = {"shop_ahead": 30.0, "no_shop": 130.0}  # [hyp] gold-eq of the Debt curse (Payment Plan), see the skill


def _expand_clear():
    pts = [(0, 0), (W - 1, 0), (W - 1, W - 1), (0, W - 1)]

    def horiz(x, y):
        return [(x + i, y) for i in (-1, 1) if 0 <= x + i < W]

    def vert(x, y):
        return [(x, y + i) for i in (-1, 1) if 0 <= y + i < W]

    for _ in range(2):
        pts = pts + [p for c in pts for p in horiz(*c)] + [p for c in pts for p in vert(*c)]
    m = np.zeros((W, W), bool)
    for x, y in pts:
        m[x, y] = True
    return m


INIT_CLEAR = _expand_clear()  # 24 cells, 6 per corner


def _blocks():
    big = np.zeros((NC, NC), np.float32)
    for x in range(W):
        for y in range(W):
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    if 0 <= x + dx < W and 0 <= y + dy < W:
                        big[x * W + y, (x + dx) * W + y + dy] = 1
    return big


BIG = _blocks()
CAND = np.concatenate([BIG, np.eye(NC, dtype=np.float32)])  # candidate j<121: big at cell j; j>=121: small at cell j-121


def cand_name(j):
    c = j % NC
    return ("big" if j < NC else "small"), c // W, c % W


# ---------------------------------------------------------------- the real game (scalar, exact)
class Game:
    """One minigame instance with the hidden placement. Mirrors the C# constructor / PopulateItems / PlaceItem (including the retry quirk)."""

    def __init__(self, rng):
        self.hidden = ~INIT_CLEAR.copy()
        self.owner = np.full((W, W), -1, np.int16)
        self.items = []  # (spec_index, mask(121 bool) | None)
        passes = 0
        while True:
            ok = self._populate(rng)
            passes += 1
            if ok or passes >= 10:
                break
        self.passes = passes
        self.placed_all = ok
        self.revealed = []
        self.kinds = np.zeros((W, W), np.int8)
        for j, (si, mask) in enumerate(self.items):
            if mask is not None:
                self.kinds.reshape(-1)[mask] = KIND_OF[si]

    def _populate(self, rng):
        flag = True
        for si, (_, _, w, h) in enumerate(ITEMS):
            idx = len(self.items)
            mask = None
            if flag:
                free = (~INIT_CLEAR) & (self.owner < 0)  # PlaceItem: needs fog (initial clear cells are not hidden) and no item
                nx, ny = W - w + 1, W - h + 1
                valid = np.ones((nx, ny), bool)
                for dx in range(w):
                    for dy in range(h):
                        valid &= free[dx:dx + nx, dy:dy + ny]
                cells = np.flatnonzero(valid)
                if len(cells) == 0:
                    flag = False
                else:
                    a = int(cells[rng.integers(len(cells))])
                    ax, ay = a // ny, a % ny
                    mask = np.zeros(NC, bool)
                    for dx in range(w):
                        for dy in range(h):
                            self.owner[ax + dx, ay + dy] = idx
                            mask[(ax + dx) * W + ay + dy] = True
            self.items.append((si, mask))
        return flag

    def obs(self):
        return self.hidden.copy(), np.where(self.hidden, 0, self.kinds).astype(np.int8)

    def click(self, tool, x, y):
        b = BIG[x * W + y] > 0 if tool == "big" else CAND[NC + x * W + y] > 0
        self.hidden.reshape(-1)[b] = False
        hid = self.hidden.reshape(-1)
        new = []
        for j, (si, mask) in enumerate(self.items):
            if mask is not None and j not in self.revealed and not (mask & hid).any():
                self.revealed.append(j)
                new.append(si)
        return new

    def counts(self):
        c = np.zeros(NI, np.int16)
        for j in self.revealed:
            c[self.items[j][0]] += 1
        return c


# ---------------------------------------------------------------- posterior sampler (vectorised sequential importance sampling)
# _CAP[code][i]: cells that the items AFTER item i can still cover with that kind
_CAP = {c: [sum(w * h for j, (_, k, w, h) in enumerate(ITEMS) if KIND[k] == c and j > i) for i in range(NI)] for c in range(1, 6)}


def _wall(arr, w, h):
    nx, ny = W - w + 1, W - h + 1
    res = arr[:, 0:nx, 0:ny].copy()
    for dx in range(w):
        for dy in range(h):
            if dx or dy:
                res &= arr[:, dx:dx + nx, dy:dy + ny]
    return res


def _wany(arr, w, h):
    nx, ny = W - w + 1, W - h + 1
    res = arr[:, 0:nx, 0:ny].copy()
    for dx in range(w):
        for dy in range(h):
            if dx or dy:
                res |= arr[:, dx:dx + nx, dy:dy + ny]
    return res


def sample_placements(hidden, kinds, K, rng, pf=0.6):
    """K weighted placements consistent with the observation. Target = the game's sequential uniform placement (single pass), restricted to placements
    that reproduce every clear cell (a clear cell shows kind k only if covered by an item of kind k; clear '.' cells are uncovered). Proposal = uniform over
    consistent anchors, with probability pf over the anchors that cover a still-unexplained letter cell of that kind; weight = P/Q. Returns M (K,NI,121) bool, w (K,)."""
    base_free = ~INIT_CLEAR
    occ = np.zeros((K, W, W), bool)
    M = np.zeros((K, NI, NC), bool)
    w = np.ones(K)
    ar = np.arange(K)
    clearletters = (~hidden) & (kinds > 0)
    for i, (_, kd, iw, ih) in enumerate(ITEMS):
        code = KIND[kd]
        allowed = (hidden | (kinds == code)) & base_free
        let = (~hidden) & (kinds == code)
        A = _wall((base_free[None] & ~occ), iw, ih).reshape(K, -1)
        C = _wall((allowed[None] & ~occ), iw, ih).reshape(K, -1)
        nx, ny = W - iw + 1, W - ih + 1
        nA, nC = A.sum(1), C.sum(1)
        if let.any():
            F = C & _wany((let[None] & ~occ), iw, ih).reshape(K, -1)
            nF = F.sum(1)
        else:
            F, nF = None, np.zeros(K, int)
        useF = (nF > 0) & (rng.random(K) < pf)
        sel = np.where(useF[:, None], F, C) if F is not None else C
        nsel = np.where(useF, nF, nC)
        alive = (nC > 0) & (nA > 0) & (w > 0)
        r = np.floor(rng.random(K) * np.maximum(nsel, 1)).astype(np.int64)
        idx = (np.cumsum(sel, 1) > r[:, None]).argmax(1)
        inF = F[ar, idx] if F is not None else np.zeros(K, bool)
        q = np.where(nF > 0, pf * inF / np.maximum(nF, 1) + (1 - pf) / np.maximum(nC, 1), 1.0 / np.maximum(nC, 1))
        w = np.where(alive, w * (1.0 / np.maximum(nA, 1)) / np.maximum(q, 1e-300), 0.0)
        ax, ay = idx // ny, idx % ny
        a = ar[alive]
        for dx in range(iw):
            for dy in range(ih):
                flat = (ax[alive] + dx) * W + ay[alive] + dy
                occ.reshape(K, NC)[a, flat] = True
                M[a, i, flat] = True
        if clearletters.any():
            # prune: letters of a kind that the items still to come cannot cover
            unc = (clearletters[None] & ~occ)
            bad = np.zeros(K, bool)
            for code in range(1, 6):
                lk = clearletters & (kinds == code)
                if lk.any():
                    bad |= (unc & lk[None]).reshape(K, -1).sum(1) > _CAP[code][i]
            w = np.where(bad, 0.0, w)
            tot = w.sum()
            if tot > 0 and i < NI - 1 and tot ** 2 / (w ** 2).sum() < 0.5 * K:  # systematic resampling
                pos = (rng.random() + np.arange(K)) / K
                src = np.minimum(np.searchsorted(np.cumsum(w) / tot, pos), K - 1)
                occ, M = occ[src], M[src]
                w = np.full(K, tot / K)
    if clearletters.any():
        w = np.where((clearletters[None] & ~occ).any((1, 2)), 0.0, w)
    return M, w


class Post:
    """Weighted posterior samples plus the batched value evaluation of candidate clicks."""

    def __init__(self, hidden, kinds, K=700, rng=None, values=VALUES, min_ess=60, kmax=6000):
        rng = rng or np.random.default_rng()
        Ms, ws = [], []
        tot = 0
        while True:
            M, w = sample_placements(hidden, kinds, K, rng)
            Ms.append(M)
            ws.append(w)
            tot += K
            ww = np.concatenate(ws)
            ess = ww.sum() ** 2 / max((ww ** 2).sum(), 1e-300)
            if ess >= min_ess or tot >= kmax:
                break
        self.M = np.concatenate(Ms)
        self.w = np.concatenate(ws)
        self.ess = float(ess)
        self.ok = self.w.sum() > 0
        if not self.ok:  # observation not reproducible by a single pass (retry quirk): fall back to the prior
            self.M, self.w = sample_placements(~INIT_CLEAR, np.zeros((W, W), np.int8), K, rng)
        keep = self.w > 0
        self.M, self.w = self.M[keep], self.w[keep]
        self.w = self.w / self.w.sum()
        k = len(self.w)
        self.k = k
        self.Mf = self.M.reshape(k * NI, NC).astype(np.float32)
        self.wv = (self.w[:, None] * values[None, :]).reshape(-1)
        cu = np.zeros(NI)
        cu[CURSE_I] = 1
        self.wc = (self.w[:, None] * cu[None, :]).reshape(-1)
        self.values = values

    def evaluate(self, hm, block=24000):
        """Expected value (E) and probability of revealing the curse (Pc) of each of the 242 candidate clicks when the fog is `hm` (121 float32, 1 = fog)."""
        n = self.Mf.shape[0]
        E = np.zeros(2 * NC)
        Pc = np.zeros(2 * NC)
        for s in range(0, n, block):
            T = self.Mf[s:s + block] * hm
            hc = T.sum(1)
            cov = T @ CAND.T
            newly = ((cov >= hc[:, None] - 0.5) & (hc[:, None] > 0.5)).astype(np.float32)
            E += self.wv[s:s + block] @ newly
            Pc += self.wc[s:s + block] @ newly
        return E, Pc


def _post_set_value(self, hm, U):
    """Expected value of the items that the union U (121 float32, 1 = cleared) would reveal, given fog hm."""
    n = self.Mf.shape[0]
    tot = 0.0
    for s in range(0, n, 40000):
        T = self.Mf[s:s + 40000] * hm
        hc = T.sum(1)
        cov = T @ U
        tot += float(self.wv[s:s + 40000] @ ((cov >= hc - 0.5) & (hc > 0.5)).astype(np.float64))
    return tot


def _post_shaped(self, hm, hm_full, gamma, block=24000):
    """Shaped marginal gain of each candidate: sum w*v*(f_after^gamma - f_before^gamma), f = fraction of the item's originally hidden cells that are cleared
    (hm_full = fog before any virtual clear, hm = fog now). Used only to SEED plans for items that need several clicks (the relic)."""
    n = self.Mf.shape[0]
    G = np.zeros(2 * NC)
    for s in range(0, n, block):
        M0 = self.Mf[s:s + block] * hm_full
        h0 = M0.sum(1)
        T = self.Mf[s:s + block] * hm
        hc = T.sum(1)
        cov = T @ CAND.T
        ok = h0 > 0.5
        h0s = np.maximum(h0, 1)
        fb = (1 - hc / h0s)
        fa = (1 - (hc[:, None] - cov) / h0s[:, None])
        g = (np.maximum(fa, 0) ** gamma - np.maximum(fb, 0)[:, None] ** gamma) * ok[:, None]
        G += self.wv[s:s + block] @ g.astype(np.float32)
    return G


Post.set_value = _post_set_value
Post.shaped = _post_shaped


FLAGS = {"small": True}  # False: the policy may only use the Big tool (set by make_policy for the *_big variants)


def valid_mask(hm):
    ok = np.ones(2 * NC, bool)
    ok[NC:] = (hm > 0.5) & FLAGS["small"]
    ok[:NC] = (BIG @ hm) > 0.5  # a big click that clears nothing is pointless
    return ok


def best_of(E, hm, eps=1e-3):
    sc = E + eps * (CAND @ hm)
    sc = np.where(valid_mask(hm), sc, -1e18)
    return int(np.argmax(sc))


# ---------------------------------------------------------------- policies
def _union(S):
    U = np.zeros(NC, np.float32)
    for j in S:
        U = np.maximum(U, CAND[j])
    return U


def plan_set(post, hm0, r, rounds=1, gammas=(None, 1.0, 2.0, 4.0), polish=2):
    """Open-loop plan for the r remaining clicks. Seeds: greedy marginal construction on the true value (gamma None) and on shaped values (gamma>0: credits partial
    progress, which is the only way to start a 4-click relic cover); the best `polish` seeds are improved by single-click swaps on the TRUE set value; the best wins."""
    seeds, seen = [], set()
    for gm in gammas:
        hm = hm0.copy()
        S = []
        for _ in range(r):
            E = post.evaluate(hm)[0] if gm is None else post.shaped(hm, hm0, gm)
            j = best_of(E, hm)
            S.append(j)
            hm = hm * (1 - CAND[j])
        key = tuple(sorted(S))
        if key not in seen:
            seen.add(key)
            seeds.append((post.set_value(hm0, _union(S)), S))
    seeds.sort(key=lambda t: -t[0])
    best, bestv = None, -1e18
    for _, S in seeds[:polish]:
        for _ in range(rounds):
            for t in range(r):
                hm = hm0.copy()
                for u, j in enumerate(S):
                    if u != t:
                        hm = hm * (1 - CAND[j])
                E, _ = post.evaluate(hm)
                j = best_of(E, hm)
                if E[j] > E[S[t]] + 1e-9:
                    S[t] = j
        v = post.set_value(hm0, _union(S))
        if v > bestv:
            best, bestv = list(S), v
    return best


def pick_from_plan(post, hm0, S):
    E, Pc = post.evaluate(hm0)
    sc = [E[j] + 1e-3 * float(CAND[j] @ hm0) for j in S]
    return S[int(np.argmax(sc))]


def lookahead(post, hm0, r, E1, m=14):
    """Two-step expectimax: value(c1) = E1[c1] + E over outcomes of c1 of max_c2 E[c2 | outcome]. Outcomes are grouped by the letters c1 would show."""
    ok = valid_mask(hm0)
    order = np.argsort(-(np.where(ok, E1 + 1e-3 * (CAND @ hm0), -1e18)))[:m]
    k = post.k
    kind_of = np.tile(KIND_OF, k).astype(np.int64)
    L = (post.Mf * kind_of[:, None]).reshape(k, NI, NC).sum(1)  # kind letter at each cell, per sample (items never overlap)
    rnd = np.random.default_rng(7).integers(1, 2 ** 31, NC).astype(np.int64)
    best, bestv = int(order[0]), -1e18
    for j in order:
        B = CAND[j]
        cleared = (hm0 > 0.5) & (B > 0.5)
        sig = (L[:, cleared].astype(np.int64) * rnd[cleared]).sum(1) if cleared.any() else np.zeros(k, np.int64)
        _, inv = np.unique(sig, return_inverse=True)
        ng = inv.max() + 1
        hm1 = hm0 * (1 - B)
        T = post.Mf * hm1
        hc = T.sum(1)
        cov = T @ CAND.T
        newly = ((cov >= hc[:, None] - 0.5) & (hc[:, None] > 0.5)).astype(np.float32)
        val = (post.wv[:, None] * newly).reshape(k, NI, -1).sum(1)  # (k, 242) weighted
        G = np.zeros((ng, 2 * NC))
        np.add.at(G, inv, val)
        G = np.where(valid_mask(hm1)[None, :], G, -1e18)
        v = E1[j] + G.max(1).sum()
        if v > bestv:
            best, bestv = int(j), v
    return best


def make_policy(name, n, K=450, cache=None):
    """policy(hidden, kinds, r, rng) -> candidate index. `cache` maps (name, r) -> first move for the identical opening state."""
    cache = cache if cache is not None else {}

    def pol(hidden, kinds, r, rng):
        hm = hidden.reshape(-1).astype(np.float32)
        key = (name, r)
        FLAGS["small"] = not name.endswith("_big")
        first = (r == n)
        if first and key in cache:
            return cache[key]
        k = 3000 if first else K
        post = Post(hidden, kinds, k, rng)
        base = name[:-4] if name.endswith("_big") else name
        if base == "rules":
            return rules_choice(hidden, kinds, r, OPENING[n])
        if base == "greedy":
            E, _ = post.evaluate(hm)
            j = best_of(E, hm)
        elif base == "setplan":
            S = plan_set(post, hm, r)
            j = pick_from_plan(post, hm, S)
        elif base == "look2":
            E, _ = post.evaluate(hm)
            j = best_of(E, hm) if r == 1 else lookahead(post, hm, r, E)
        elif base == "hybrid":  # open-loop plan while 3+ clicks remain, exact two-step expectimax at 2, greedy at 1
            E, _ = post.evaluate(hm)
            if r == 1:
                j = best_of(E, hm)
            elif r == 2:
                j = lookahead(post, hm, r, E, m=24)
            else:
                S = plan_set(post, hm, r)
                j = pick_from_plan(post, hm, S)
        else:
            raise ValueError(name)
        if first:
            cache[key] = j
        return j

    return pol



# Hand-executable fallback (the rules of the skill): open letters get a Big click centred on / next to them; X cells and their 2-ring are never clicked.
RULE_WEIGHT = {1: 208.0, 2: 70.0, 3: 90.0, 5: 20.0}  # relic, potion, card: how much a visible part of that kind is worth finishing (gold is never chased)
OPENING = {3: [(6, 1), (3, 1), (3, 4)], 6: [(5, 1), (9, 4), (9, 7), (6, 7), (6, 4), (8, 1)]}  # overwritten by `opening` output; see the skill


def open_letters(kinds):
    """Visible letter cells whose item may be unfinished: 8-connected same-kind clusters smaller than the item (card 4, potion 3 in a line or 4, relic 16)."""
    seen = np.zeros((W, W), bool)
    out = []
    for x, y in np.argwhere((kinds > 0) & (kinds != 4)):
        if seen[x, y]:
            continue
        code, stack, cl = int(kinds[x, y]), [(x, y)], []
        seen[x, y] = True
        while stack:
            a, b = stack.pop()
            cl.append((a, b))
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    u, v = a + dx, b + dy
                    if 0 <= u < W and 0 <= v < W and not seen[u, v] and kinds[u, v] == code:
                        seen[u, v] = True
                        stack.append((u, v))
        n = len(cl)
        line = len({c[0] for c in cl}) == 1 or len({c[1] for c in cl}) == 1
        done = (code == 5 and n >= 2 and line) or (code == 3 and n >= 4) or (code == 2 and ((n == 3 and line) or n >= 4)) or (code == 1 and n >= 16)
        if not done:
            out += [(a, b, code) for a, b in cl]
    return out


_DENS = {}


def density_map():
    """Prior value per fogged cell (gold-eq): sum over items of value/size * P(cell covered). The static table behind the hand rules."""
    if "d" not in _DENS:
        post = Post(~INIT_CLEAR, np.zeros((W, W), np.int8), 8000, np.random.default_rng(3), min_ess=1)
        size = np.array([w * h for _, _, w, h in ITEMS], float)
        cov = (post.M * post.w[:, None, None]).sum(0)  # (NI, 121)
        dv = np.maximum(VALUES, 0) / size
        dv[0] = 0  # the relic needs 4 clicks: it is not part of what one click can bank
        _DENS["d"] = (cov * dv[:, None]).sum(0).reshape(W, W)
    return _DENS["d"]


def rules_choice(hidden, kinds, r, opening):
    hid = hidden
    xs = np.argwhere(kinds == 4)
    danger = np.zeros((W, W), bool)  # centres within Chebyshev 2 of a visible curse cell
    for x, y in xs:
        danger[max(0, x - 2):x + 3, max(0, y - 2):y + 3] = True
    best, bestv = None, 0.0
    for x, y, code in open_letters(kinds):
        if code == 1 and r < 4:
            continue  # a relic needs 4 clicks: never start it with fewer left
        lo_x, hi_x, lo_y, hi_y = max(0, x - 1), min(W, x + 2), max(0, y - 1), min(W, y + 2)
        if not hid[lo_x:hi_x, lo_y:hi_y].any():
            continue  # nothing hidden next to this letter: the item is complete or needs a different centre
        for cx in range(lo_x, hi_x):
            for cy in range(lo_y, hi_y):
                if danger[cx, cy]:
                    continue
                v = RULE_WEIGHT[code] * hid[max(0, cx - 1):cx + 2, max(0, cy - 1):cy + 2].sum()
                if v > bestv:
                    best, bestv = (cx, cy), v
    if best is None:
        d = density_map() * hid
        sc = np.full((W, W), -1.0)
        for cx in range(W):
            for cy in range(W):
                if not danger[cx, cy]:
                    sc[cx, cy] = d[max(0, cx - 1):cx + 2, max(0, cy - 1):cy + 2].sum()
        best = tuple(int(v) for v in np.unravel_index(np.argmax(sc), sc.shape))
    if best is None:  # leftover clicks: the fogged cell with the most fog around it, away from the curse
        sc = np.zeros((W, W))
        for cx in range(W):
            for cy in range(W):
                sc[cx, cy] = -1 if danger[cx, cy] else hid[max(0, cx - 1):cx + 2, max(0, cy - 1):cy + 2].sum()
        best = tuple(int(v) for v in np.unravel_index(np.argmax(sc), sc.shape))
    return best[0] * W + best[1]


def blind_policy(clicks):
    """Fixed click list (candidate indices), played in order regardless of what shows."""

    def pol(hidden, kinds, r, rng):
        return clicks[len(clicks) - r]

    return pol


def tile_clicks(n, kind="row"):
    """Naive tilings of the 11x11 grid with non-overlapping 3x3 blocks (centres on the 1,4,7 lattice)."""
    lat = [(x, y) for y in (1, 4, 7) for x in (1, 4, 7)]
    if kind == "centre":
        lat = [(4, 4), (7, 4), (4, 7), (7, 7), (1, 4), (4, 1), (7, 1), (1, 7), (1, 1)]
    return [x * W + y for x, y in lat][:n]


def blind_opt_clicks(n, K=6000, seed=1, values=VALUES, rounds=3):
    """Best non-adaptive set of n big clicks under the prior: greedy construction + swap passes (the expected value of a fixed set is exact in the samples)."""
    rng = np.random.default_rng(seed)
    post = Post(~INIT_CLEAR, np.zeros((W, W), np.int8), K, rng, values=values, min_ess=1)
    hm0 = (~INIT_CLEAR).reshape(-1).astype(np.float32)

    def allowed_best(E, hm):
        E = E.copy()
        E[NC:] = -1e18
        return best_of(E, hm)

    S = []
    hm = hm0.copy()
    for _ in range(n):
        E, _ = post.evaluate(hm)
        j = allowed_best(E, hm)
        S.append(j)
        hm = hm * (1 - CAND[j])
    for _ in range(rounds):
        for t in range(n):
            hm = hm0.copy()
            for u, j in enumerate(S):
                if u != t:
                    hm = hm * (1 - CAND[j])
            E, _ = post.evaluate(hm)
            j = allowed_best(E, hm)
            if E[j] > E[S[t]] + 1e-9:
                S[t] = j
    # order: highest standalone value first (irrelevant for a blind set, but keeps the list readable)
    E, _ = post.evaluate(hm0)
    return sorted(S, key=lambda j: -E[j])


# ---------------------------------------------------------------- play + benchmark
def play(game, pol, n, rng):
    for r in range(n, 0, -1):
        hidden, kinds = game.obs()
        j = pol(hidden, kinds, r, rng)
        t, x, y = cand_name(j)
        if t == "small" and not game.hidden[x, y]:
            raise RuntimeError("policy picked a clear cell with the small tool")
        game.click(t, x, y)
    return game.counts()


_G = {}


def _init(n, names, cache, clicks):
    os.environ.setdefault("OMP_NUM_THREADS", "1")
    _G.update(n=n, names=names, cache=cache, clicks=clicks)


def _work(args):
    name, lo, hi, seed = args
    n = _G["n"]
    pol = blind_policy(_G["clicks"][name]) if name in _G["clicks"] else make_policy(name, n, cache=_G["cache"])
    out = np.zeros((hi - lo, NI), np.int16)
    passes = np.zeros(hi - lo, np.int8)
    ess = 0.0
    for i in range(lo, hi):
        g = Game(np.random.default_rng([seed, i]))
        out[i - lo] = play(g, pol, n, np.random.default_rng([seed, i, 1]))
        passes[i - lo] = g.passes
    return lo, out, passes


def summarize(counts, values=VALUES):
    tot = counts @ values
    n = len(tot)
    d = {"mean": tot.mean(), "se": tot.std(ddof=1) / np.sqrt(n), "p_curse": (counts[:, CURSE_I] > 0).mean(),
         "p_curse_se": np.sqrt((counts[:, CURSE_I] > 0).mean() * (1 - (counts[:, CURSE_I] > 0).mean()) / n),
         "relic": counts[:, 0].mean(), "potion": counts[:, 1:4].sum(1).mean(), "card": counts[:, 4:7].sum(1).mean(), "gold": (counts[:, 8:13].sum(1) * 10 + counts[:, 13:15].sum(1) * 30).mean(),
         "items": counts[:, [i for i in range(NI) if i != CURSE_I]].sum(1).mean()}
    return d


def bench(n, games, names, procs=16, seed=12345, chunk=0):
    chunk = chunk or max(5, min(250, games // (procs * 4)))
    from multiprocessing import Pool
    clicks = {}
    cache = {}
    if "tile_row" in names:
        clicks["tile_row"] = tile_clicks(n, "row")
    if "tile_centre" in names:
        clicks["tile_centre"] = tile_clicks(n, "centre")
    if "blind_opt" in names:
        clicks["blind_opt"] = blind_opt_clicks(n)
    res = {}
    for name in names:
        t0 = time.time()
        jobs = [(name, lo, min(lo + chunk, games), seed) for lo in range(0, games, chunk)]
        if name not in clicks:  # compute the cached opening once in the parent
            make_policy(name, n, cache=cache)(*Game(np.random.default_rng([seed, 0])).obs(), n, np.random.default_rng(99))
            if (name, n) in cache:
                print(f"      opening {name}: {cand_name(cache[(name, n)])} ({time.time() - t0:.0f}s)", flush=True)
        with Pool(procs, initializer=_init, initargs=(n, names, cache, clicks)) as pool:
            parts = pool.map(_work, jobs)
        parts.sort(key=lambda p: p[0])
        counts = np.concatenate([p[1] for p in parts])
        passes = np.concatenate([p[2] for p in parts])
        s = summarize(counts)
        s["retry_games"] = float((passes > 1).mean())
        s["secs"] = time.time() - t0
        s["opening"] = [cand_name(cache[(name, n)]) for _ in [0]] if (name, n) in cache else [cand_name(j) for j in clicks.get(name, [])]
        res[name] = (s, counts)
        print(f"n={n} {name:12s} mean {s['mean']:7.1f} +-{s['se']:.1f}  P(curse) {s['p_curse']:.3f}  relic {s['relic']:.2f} potion {s['potion']:.2f} card {s['card']:.2f} gold {s['gold']:.1f}  ({s['secs']:.0f}s)", flush=True)
        if len(res) > 1:
            first = next(iter(res))
            d = counts @ VALUES - res[first][1] @ VALUES
            print(f"      paired vs {first}: {d.mean():+.1f} +-{d.std(ddof=1) / np.sqrt(len(d)):.1f}", flush=True)
    return res


# ---------------------------------------------------------------- reading the bridge text
def parse_grid(text):
    """Parse the bridge's CRYSTAL_SPHERE text. Returns (hidden, kinds, divinations_left or None)."""
    hidden = np.zeros((W, W), bool)
    kinds = np.zeros((W, W), np.int8)
    rows = 0
    left = None
    m = re.search(r"(\d+) divinations? left", text)
    if m:
        left = int(m.group(1))
    for line in text.splitlines():
        m = re.match(r"^\s*(\d{1,2})\s{1,3}([#.RPCXg?]{11})\s*$", line)
        if not m:
            continue
        y = int(m.group(1))
        for x, ch in enumerate(m.group(2)):
            hidden[x, y] = ch == "#"
            kinds[x, y] = KIND.get(ch, 0)
        rows += 1
    if rows != W:
        raise ValueError(f"expected {W} grid rows, found {rows}")
    return hidden, kinds, left


def advise(text, n=None, k=3000, seed=None):
    hidden, kinds, left = parse_grid(text)
    r = n or left
    if not r:
        raise SystemExit("divinations left unknown: pass --n")
    rng = np.random.default_rng(seed)
    hm = hidden.reshape(-1).astype(np.float32)
    post = Post(hidden, kinds, k, rng, min_ess=150, kmax=20000)
    E, Pc = post.evaluate(hm)
    S = plan_set(post, hm, r)
    j_plan = pick_from_plan(post, hm, S)
    j_greedy = best_of(E, hm)
    out = [f"{r} divinations left; posterior samples {post.k}, ESS {post.ess:.0f}" + ("" if post.ok else "  (observation not reproducible: prior used)")]
    out.append("planned set (play the marked one first, replan after each click): " + ", ".join(f"{t} {x} {y}" for t, x, y in map(cand_name, S)))
    out.append(f"recommended: {cand_text(j_plan)}   (greedy single best: {cand_text(j_greedy)})")
    ok = valid_mask(hm)
    order = np.argsort(-np.where(ok, E, -1e18))[:8]
    out.append("top clicks by expected immediate value (gold-eq) / P(reveal curse):")
    for j in order:
        out.append(f"  {cand_text(j):28s} E={E[j]:7.1f}  Pc={Pc[j]:.3f}")
    out.append("safest clicks (P(curse) < 0.5%) with the most value: " + ", ".join(f"{cand_text(j)}={E[j]:.1f}" for j in np.argsort(-np.where(ok & (Pc < 0.005), E, -1e18))[:5]))
    # P(curse | cell) hints
    return "\n".join(out)


def cand_text(j):
    t, x, y = cand_name(j)
    return f"{t} tool at x={x} y={y}  (a 1 / a 2 then a 0 {x} {y})" if True else ""


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["advise", "bench", "opening"])
    ap.add_argument("--n", type=int, default=3)
    ap.add_argument("--games", type=int, default=2000)
    ap.add_argument("--policies", default="tile_row,tile_centre,blind_opt,greedy,greedy_big,setplan,look2")
    ap.add_argument("--procs", type=int, default=16)
    ap.add_argument("--k", type=int, default=3000)
    ap.add_argument("--seed", type=int, default=12345)
    ap.add_argument("--json", default="")
    a = ap.parse_args(argv)
    if a.cmd == "advise":
        print(advise(sys.stdin.read(), a.n if "--n" in (argv or sys.argv) else None, a.k))
    elif a.cmd == "opening":
        print("blind_opt:", [cand_name(j) for j in blind_opt_clicks(a.n)])
        cache = {}
        for nm in ("greedy", "setplan", "look2"):
            make_policy(nm, a.n, cache=cache)(*Game(np.random.default_rng(1)).obs(), a.n, np.random.default_rng(2))
            print(nm, cand_name(cache[(nm, a.n)]))
    else:
        res = bench(a.n, a.games, a.policies.split(","), a.procs, a.seed)
        if a.json:
            json.dump({k: {kk: (vv if not isinstance(vv, np.generic) else vv.item()) for kk, vv in v[0].items()} for k, v in res.items()}, open(a.json, "w"), indent=1, default=str)
            np.savez_compressed(a.json + ".npz", **{k: v[1] for k, v in res.items()})


if __name__ == "__main__":
    main()
