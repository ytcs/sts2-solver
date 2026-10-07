#!/usr/bin/env python3
"""The solver's search, driven by the Rust state machine (`sts2env::search`): this file only evaluates networks.

The engine plays many fights at once, each at its own pace. Whenever a fight (or one of its play-outs) needs a decision the engine writes
one observation row into a request buffer; this driver runs the policy on all policy rows and the value ensemble on all value rows in one batch
and hands the answers back. Two engines (`groups`) alternate so the CPU simulates one while the GPU evaluates the other.

  fs = FastSearch(net, value_nets=[...], M=3, K=8)
  rows = fs.run(scenario_dicts, job_scen, job_seed)      # [n_jobs, 8]: scenario, outcome, hp_lost, hp_end, length, finished, end HP (absolute), potions kept (bits)

With an outcome-head network (`rl/heads.py`) and no extra value networks, value rows come back as the head's class probabilities and Rust combines them
with each job's worth (`run(..., worth=)`: per scenario None = today's linear return, or a table over the classes and per-slot potion prices; the decision
layer of `docs/rl_redesign.md` 3.2).
"""
import json, os, sys, time, collections
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import utility
import heads
from model import DEV, SEC, C

OBS, ACT = sts2.OBS_SIZE, sts2.ACTIONS
_NAMES = sts2.names()
NC, POT = _NAMES["head_nc"], _NAMES["pot"]
assert NC == heads.NC and _NAMES["head_bin"] == heads.BIN, "rl/heads.py and the Rust search disagree on the outcome classes"
WORTH_W = 1 + NC + POT


def worth_row(w):
    """One scenario's worth for the engine: None = linear; else dict(u=[NC] class worths, price=[POT] per belt slot, default 0)."""
    r = np.zeros(WORTH_W, np.float32)
    if w is None:
        return r
    u = np.asarray(w["u"], np.float32)
    assert u.shape == (NC,), u.shape
    r[0] = 1.0
    r[1:1 + NC] = u
    pr = np.asarray(w.get("price", np.zeros(POT)), np.float32)
    r[1 + NC:1 + NC + len(pr)] = pr
    return r
_E0, _ES, _EN = SEC["enemies"][0], C["ENEMY_F"], C["OBS_MAX_ENEMIES"]
# Play-out depth in player turns before the value network takes over. 2 since 2026-10-06: decision regret vs a Monte Carlo referee 0.0038 vs 0.0099 at
# depth 1 (150 recorded states, `tools/bench_search.py`, paired fight-clustered CI of the difference excludes 0); whole fights with the live search shape
# (5x32, 1200 eval fights x 4) win +0.88 % +- 0.37 %, HP lost -1.1 % of max +- 0.15 % (`tools/ab_leaf.py`, evals/ab_leaf_5x32.json). Costs ~2x per decision.
LEAF_TURNS = 2

_PILES = [SEC[n][0] for n in ("draw", "discard", "exhaust")]
_DEC = SEC["decision"][0]
if DEV.type == "cuda":
    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True


def available_cpus():
    """CPUs this process may really use: the affinity mask, capped by the container's CPU quota (cgroup v2 `cpu.max` or v1 `cfs_quota_us`):
    `os.cpu_count()` reports the host's cores, which oversubscribes a container (threads fighting for a 10-CPU quota ran 2x slower)."""
    n = len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else (os.cpu_count() or 8)
    try:
        if os.path.exists("/sys/fs/cgroup/cpu.max"):
            q, per = open("/sys/fs/cgroup/cpu.max").read().split()
            if q != "max":
                n = min(n, max(1, int(int(q) / int(per) + 0.5)))
        elif os.path.exists("/sys/fs/cgroup/cpu/cpu.cfs_quota_us"):
            q, per = int(open("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").read()), int(open("/sys/fs/cgroup/cpu/cpu.cfs_period_us").read())
            if q > 0:
                n = min(n, max(1, int(q / per + 0.5)))
    except (OSError, ValueError):
        pass
    return n


def host_shapes(obs):
    """Shapes the network needs for this batch, from the host-side observation (no device syncs): occupied enemy slots, longest pile list,
    and the rows that have a pending card selection."""
    occ = (obs[:, _E0:_E0 + _EN * _ES:_ES] > 0.5).any(0)
    E = int(np.nonzero(occ)[0].max()) + 1 if occ.any() else 1
    L = 1
    for o in _PILES:
        L = max(L, int((obs[:, o:o + 128:2] > 0).sum(1).max()))
    return E, L, obs[:, _DEC] > 0.5


def _sample(pr, u):
    """One action per row of the probabilities `pr` [B, ACT], by inverse CDF with the engine's uniform `u` [B]: the job seed, not the GPU's random state,
    decides every play-out move, so two variants of a deck evaluated with the same seeds meet the same play-out luck (common random numbers)."""
    c = pr.cumsum(1)
    return (c < u.unsqueeze(1) * c[:, -1:]).sum(1).clamp_max(pr.shape[1] - 1)


class GraphFn:
    """`fn(obs [B, OBS], mask [B, ACT] | None[, u [B]]) -> [B, ...]` replayed as a CUDA graph per padded batch size: one replay instead of several hundred
    kernel launches (the network is launch-bound at the batch sizes the search produces). `with_u`: a per-row uniform (`_sample`) is a third input."""

    def __init__(self, fn, buckets, with_mask, pool, label="", events=None, with_u=False, legacy_pad=False):
        self.fn, self.buckets, self.with_mask, self.pool, self.with_u = fn, tuple(sorted(buckets)), with_mask, pool, with_u
        self.legacy_pad = legacy_pad  # pad every remainder to the next bucket (the plan before 2026-10-07: reproduces older tables bit for bit)
        self.graphs = {}
        self.label, self.events = label, events  # events: a list collecting (cuda event pair, rows) per replay when profiling

    def _capture(self, B):
        sobs = torch.zeros(B, OBS, device=DEV)
        smask = torch.zeros(B, ACT, dtype=torch.uint8, device=DEV) if self.with_mask else None
        if smask is not None:
            smask[:, 0] = 1  # every padded row has a legal action (no NaN in the softmax of rows that are ignored)
        su = torch.full((B,), 0.5, device=DEV) if self.with_u else None
        args = (sobs, smask) + ((su,) if self.with_u else ())
        st = torch.cuda.Stream()
        st.wait_stream(torch.cuda.current_stream())
        with torch.cuda.stream(st), torch.no_grad():
            for _ in range(2):
                self.fn(*args)
        torch.cuda.current_stream().wait_stream(st)
        g = torch.cuda.CUDAGraph()
        with torch.no_grad(), torch.cuda.graph(g, pool=self.pool):
            out = self.fn(*args)
        self.graphs[B] = (g, sobs, smask, su, out)

    @torch.no_grad()
    def __call__(self, obs, mask, idx=None, u=None):
        """Rows `idx` (a device index tensor) of obs / mask (/ u), or all of them; returns a fresh [rows, ...] tensor."""
        n = len(obs) if idx is None else len(idx)
        outs = []
        a = 0
        for m, B in self.plan(n):
            if B not in self.graphs:
                self._capture(B)
            g, sobs, smask, su, out = self.graphs[B]
            sel = slice(a, a + m) if idx is None else idx[a:a + m]
            if idx is None:
                sobs[:m].copy_(obs[sel])
                if smask is not None:
                    smask[:m].copy_(mask[sel])
                if su is not None:
                    su[:m].copy_(u[sel])
            else:
                torch.index_select(obs, 0, sel, out=sobs[:m])
                if smask is not None:
                    torch.index_select(mask, 0, sel, out=smask[:m])
                if su is not None:
                    torch.index_select(u, 0, sel, out=su[:m])
            if self.events is not None:
                e0, e1 = torch.cuda.Event(enable_timing=True), torch.cuda.Event(enable_timing=True)
                e0.record()
                g.replay()
                e1.record()
                self.events.append((e0, e1, m, B))
            else:
                g.replay()
            outs.append(out[:m].clone())
            a += m
        return outs[0] if len(outs) == 1 else torch.cat(outs)

    def plan(self, n):
        """(rows, padded batch) per replay covering `n` rows: whole top buckets, then the remainder in the bucket that holds it when that pads by at
        most the smallest bucket, else the largest bucket below it, and again. Padding the remainder to the next bucket wasted 22 % of the policy
        rows and 77 % of the value rows (5x32, roots 1024); the network's time is close to proportional to the padded batch."""
        bs, out = self.buckets, []
        if self.legacy_pad:
            return [(min(bs[-1], n - a), next(b for b in bs if b >= min(bs[-1], n - a))) for a in range(0, n, bs[-1])]
        while n > 0:
            if n >= bs[-1]:
                out.append((bs[-1], bs[-1]))
                n -= bs[-1]
                continue
            b = next(x for x in bs if x >= n)
            if b == bs[0] or b - n <= bs[0]:
                out.append((n, b))
                break
            lo = max(x for x in bs if x <= n)
            out.append((lo, lo))
            n -= lo
        return out



class HostBuffers:
    """The request / answer buffers of one engine group, kept for the life of the FastSearch and reused by every `run` (grown when a run needs more rows).

    The arrays the GPU copies from or into (observations, masks, uniforms, answers) are page-locked at their exact size with `cudaHostRegister` on numpy
    memory, so the copies stay asynchronous. Not `torch.empty(pin_memory=True)`: torch's pinned allocator rounds every block up to a power of two and
    caches freed blocks for the life of the process (a 1.27 GB request commits 2.00 GB, measured), and a fresh set per run left the old size classes cached.
    `pol_kind` / `val_kind` never leave the host and are not pinned."""

    PINNED = ("pol_obs", "pol_mask", "pol_u", "val_obs", "pol_out", "val_out")

    def __init__(self, pin):
        self.pin = pin
        self.a = {}  # name -> numpy array (registered when pinned)
        self.t = {}  # name -> torch view of the same memory
        self._reg = []  # registered base pointers

    def ensure(self, pc, vc, pol_w, val_w, shared=False):
        """Room for `pc` policy and `vc` value rows. `shared` (the engine's `advance_shared`): one observation buffer `pol_obs` of `pc` rows holds
        both kinds (value rows from its end backwards) and there is no `val_obs`."""
        want = {"pol_obs": (pc, OBS), "pol_mask": (pc, ACT), "pol_kind": (pc,), "pol_u": (pc,), "val_obs": (vc, OBS), "val_kind": (vc,),
                "pol_out": (pc, pol_w), "val_out": (vc, val_w)}
        if shared:
            del want["val_obs"]
            self._free("val_obs")
        dts = {"pol_mask": np.uint8, "pol_kind": np.uint8, "val_kind": np.uint8}
        for name, shape in want.items():
            cur = self.a.get(name)
            if cur is not None and cur.shape[0] >= shape[0] and cur.shape[1:] == shape[1:]:
                continue
            self._free(name)
            arr = np.empty(shape, dts.get(name, np.float32))
            if self.pin and name in self.PINNED and arr.nbytes:
                err = torch.cuda.cudart().cudaHostRegister(arr.ctypes.data, arr.nbytes, 0)
                if int(err) != 0:
                    raise RuntimeError(f"cudaHostRegister failed for {name} ({arr.nbytes / 2**30:.2f} GB): {err}")
                self._reg.append((name, arr.ctypes.data))
            self.a[name], self.t[name] = arr, torch.from_numpy(arr)

    def nbytes(self):
        return sum(a.nbytes for a in self.a.values())

    def _free(self, name):
        for i, (n, ptr) in enumerate(self._reg):
            if n == name:
                torch.cuda.cudart().cudaHostUnregister(ptr)
                del self._reg[i]
                break
        self.a.pop(name, None)
        self.t.pop(name, None)

    def __del__(self):
        try:
            for name in list(self.a):
                self._free(name)
        except Exception:  # interpreter shutdown: the process is going away with its memory
            pass


class FastSearch:
    def __init__(self, net, value_nets=None, M=3, K=8, conf=1.01, roll_cap=None, max_steps=300, hp_bonus=0.5, greedy_roll=False,
                 roots=512, groups=2, threads=None, roll_net=None, use_graphs=True, graph_E=8, buckets=None, amp=False, value_amp=None, record=False, profile_gpu=False, compile=True, lead=True, merge_dec=True, carry=True, strat=True, dist_head=None, leaf_turns=None, dist=None, legacy_pad=False):
        """`net`: ranks the options of the real fight's decisions; `roll_net` (default: `net`): plays the play-outs (a cheaper network is fine:
        the play-outs only have to finish the turn plausibly); `value_nets`: extra networks whose value heads are averaged with `net`'s."""
        self.net, self.value_nets = net, value_nets or []
        self.roll_net = roll_net if roll_net is not None else net
        self.M, self.K, self.conf = M, K, conf
        # play-out depth: the ONE default for live play and every batch table (agent.engine, rl/solver.py; evals/bench_search*.jsonl, evals/ab_leaf_5x32.json)
        self.leaf_turns = LEAF_TURNS if leaf_turns is None else leaf_turns  # player turns a play-out runs before the value network (1 = this turn; large = to the fight's end)
        roll_cap = roll_cap if roll_cap is not None else 60 * self.leaf_turns if self.leaf_turns < 100 else 400  # step cap of a play-out, scaled with its depth
        self.roll_cap, self.max_steps, self.hp_bonus, self.greedy_roll = roll_cap, max_steps, hp_bonus, greedy_roll
        self.roots, self.groups = roots, groups
        self.threads = threads or max(2, available_cpus() - 1)
        self.timers = collections.defaultdict(float)
        self.stats = {}
        self.cuda = DEV.type == "cuda"
        self.graph_E = graph_E
        self.compile = compile and self.cuda  # torch.compile (inductor fusion, dynamic batch) inside the CUDA graphs: about 1.7x faster networks
        self.profile_gpu = profile_gpu and self.cuda  # CUDA events around every graph replay: where the GPU time goes (`gpu_ms`)
        self._ev = collections.defaultdict(list)
        self.strat = strat  # stratified determinizations (rotations of one shuffle)
        self.carry = carry  # follow the line of the chosen option: its estimate is reused at the next decision instead of searching it again
        self.lead = lead  # share the in-turn play of an option between its futures until hidden information is needed
        self.record = record  # keep the moves of every fight (`moves`, `replay`): play-by-play traces
        # the fight's HP-worth curve (`rl/utility.py`, `set_util`): the networks read it as an input (U at 8 HP points, one buffer for every row) and the
        # Rust terminal scores a finished fight by it; default linear = the original return
        self.dist_head = dist_head  # unused (kept for callers); the add-on end-HP head was not adopted
        # value rows as the outcome head's class probabilities, combined in Rust per job (default: whenever the network has the head and no extra value nets)
        self.dist = (bool(getattr(net, "heads", False)) and not self.value_nets) if dist is None else dist
        if self.dist and (self.value_nets or not getattr(net, "heads", False)):
            raise ValueError("dist value rows need one outcome-head network (no extra value nets)")
        self.pot = self.dist and bool(getattr(net, "pot", False))  # the potion-use head's per-slot probabilities follow the classes in each value row
        self.val_w = (NC + POT if self.pot else NC) if self.dist else 1
        self.util = None
        self._ufeat_t = torch.tensor(utility.LINEAR_FEATS, device=DEV)
        self._runs = []
        self._bufs = []  # one HostBuffers per engine group, reused across runs
        self.amp = amp  # bf16 autocast inside the graphs (the networks are compute-bound there)
        self.value_amp = amp if value_amp is None else value_amp
        # padded batch sizes of the graphs (`GraphFn.plan`); `legacy_pad`: the buckets and padding before 2026-10-07 (bit-identical to older tables; ~15 % slower at 5x32)
        self.legacy_pad = legacy_pad
        self.buckets = ((1024, 2048, 4096, 8192, 16384) if legacy_pad else (256, 512, 1024, 2048, 4096, 8192, 16384)) if buckets is None else buckets
        self.dec_buckets = (64, 256, 1024, 4096)
        self.merge_dec = merge_dec  # one graph per head for rows with and without a pending selection (the candidate branch costs less than a second replay)
        self._graphs = {}
        self._pool = torch.cuda.graph_pool_handle() if self.cuda and use_graphs else None
        self.use_graphs = self.cuda and use_graphs

    def set_util(self, curve):
        """`curve`: the fight's HP-worth curve (101 floats, `rl/utility.py`) or None for the linear return. Sets the networks' input (in place: the
        captured graphs read this buffer) and the Rust terminal table."""
        if curve is None:
            self.util = None
            self._ufeat_t.copy_(torch.from_numpy(utility.LINEAR_FEATS).to(DEV))
            return
        u = utility.normalize(curve)
        self.util = utility.table(u)
        self._ufeat_t.copy_(torch.from_numpy(utility.feats(u)).to(DEV))

    def _uf(self, o):
        return self._ufeat_t.expand(o.shape[0], 8)

    # ---- network side ----
    def _run(self, fn, obs_np, obs_t, mask_t=None, u_t=None):
        """`fn(obs, mask, [u=,] **shape)` on the rows of `obs_t` (device) split into rows without / with a pending card selection (each static in shape)."""
        E, L, dec = host_shapes(obs_np)
        nd = int(dec.sum())
        uk = lambda i: {} if u_t is None else {"u": u_t if i is None else u_t[i]}
        if nd == 0:
            return fn(obs_t, mask_t, E=E, L=L, has_dec=False, **uk(None))
        if nd == len(dec):
            return fn(obs_t, mask_t, E=E, L=L, has_dec=True, **uk(None))
        i1 = torch.from_numpy(np.flatnonzero(dec)).to(DEV)
        i0 = torch.from_numpy(np.flatnonzero(~dec)).to(DEV)
        r0 = fn(obs_t[i0], None if mask_t is None else mask_t[i0], E=E, L=L, has_dec=False, **uk(i0))
        r1 = fn(obs_t[i1], None if mask_t is None else mask_t[i1], E=E, L=L, has_dec=True, **uk(i1))
        out = r0.new_empty((len(dec),) + r0.shape[1:])
        out[i0] = r0
        out[i1] = r1
        return out

    def warm(self):
        """Captures every graph up front (a few seconds) so a timed run does not pay for it."""
        if not self.use_graphs:
            return
        t = time.perf_counter()
        for net in {id(self.net): self.net, id(self.roll_net): self.roll_net}.values():
            for hd in ((True,) if self.merge_dec else (False, True)):
                fn = self._pol_graph(net, hd)
                for B in fn.buckets:
                    fn._capture(B)
        for hd in ((True,) if self.merge_dec else (False, True)):
            fn = self._val_graph(hd)
            for B in fn.buckets:
                fn._capture(B)
        torch.cuda.synchronize()
        self.timers["warm"] += time.perf_counter() - t

    def _pol_graph(self, net, has_dec):
        key = ("pol", id(net), has_dec)
        if key not in self._graphs:
            M, greedy, E = self.M, self.greedy_roll, self.graph_E

            amp = self.amp
            logits = lambda o, m: net(o, m, value=False, E=E, L=64, has_dec=has_dec, ufeat=self._uf(o))[0]
            if self.compile:
                logits = torch.compile(logits, dynamic=True)

            def fn(o, m, u):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    lg = logits(o, m)
                pr = torch.softmax(lg.float(), 1)
                act = pr.argmax(1) if greedy else _sample(pr, u)
                tp, ti = pr.topk(M, 1)
                return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
            self._graphs[key] = GraphFn(fn, self.buckets if (has_dec and self.merge_dec) else (self.dec_buckets if has_dec else self.buckets), True, self._pool, f"pol dec={has_dec}", self._ev[f"pol dec={has_dec}"] if self.profile_gpu else None, with_u=True, legacy_pad=self.legacy_pad)
        return self._graphs[key]

    def _val_graph(self, has_dec):
        key = ("val", has_dec)
        if key not in self._graphs:
            nets, E = [self.net] + list(self.value_nets), self.graph_E

            amp = self.value_amp
            if self.dist:
                net0, pot = self.net, self.pot

                def ens(o):
                    ol, pl = net0.heads_out(o, E=E, L=64, has_dec=has_dec, ufeat=self._uf(o))
                    p = torch.softmax(ol, 1)
                    return torch.cat([p, torch.sigmoid(pl)], 1) if pot else p
            else:
                ens = lambda o: sum(n(o, None, policy=False, E=E, L=64, has_dec=has_dec, ufeat=self._uf(o))[1].float() for n in nets)
            if self.compile:
                ens = torch.compile(ens, dynamic=True)

            def fn(o, m):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    v = ens(o)
                return v.float() if self.dist else (v / len(nets)).unsqueeze(1)
            self._graphs[key] = GraphFn(fn, self.buckets if (has_dec and self.merge_dec) else (self.dec_buckets if has_dec else self.buckets), False, self._pool, f"val dec={has_dec}", self._ev[f"val dec={has_dec}"] if self.profile_gpu else None, legacy_pad=self.legacy_pad)
        return self._graphs[key]

    @torch.no_grad()
    def _evaluate_graphs(self, G, n_pol, n_val):
        M = self.M
        if n_pol:
            obs = G["pol_obs_t"][:n_pol].to(DEV, non_blocking=True)
            mask = G["pol_mask_t"][:n_pol].to(DEV, non_blocking=True)
            kind = G["pol_kind"][:n_pol]
            u = G["pol_u_t"][:n_pol].to(DEV, non_blocking=True)
            res = torch.empty(n_pol, 2 * M + 1, device=DEV)
            sim, dec = (kind & 1) != 0, (kind & 2) != 0
            if self.merge_dec:
                dec = np.ones_like(dec)
            split = self.roll_net is not self.net
            # classes: (network, has_dec); with a separate play-out network the real fight's decisions go to the main network
            for use_main in ((False, True) if split else (None,)):
                for hd in (False, True):
                    sel = (dec == hd) if use_main is None else ((dec == hd) & (sim != use_main))
                    k = int(sel.sum())
                    if k == 0:
                        continue
                    net = self.net if (use_main or not split) else self.roll_net
                    fn = self._pol_graph(net, hd)
                    if k == n_pol:
                        res.copy_(fn(obs, mask, u=u))
                    else:
                        idx = torch.from_numpy(np.flatnonzero(sel)).to(DEV, non_blocking=True)
                        res[idx] = fn(obs, mask, idx, u=u)
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            _, vo = self._val_obs(G, n_val)
            dec = (G["val_kind"][:n_val] & 2) != 0
            if self.merge_dec:
                dec = np.ones_like(dec)
            nd = int(dec.sum())
            if nd == 0:
                out = self._val_graph(False)(vo, None)
            elif nd == n_val:
                out = self._val_graph(True)(vo, None)
            else:
                out = torch.empty(n_val, self.val_w, device=DEV)
                for hd in (False, True):
                    idx = torch.from_numpy(np.flatnonzero(dec == hd)).to(DEV, non_blocking=True)
                    out[idx] = self._val_graph(hd)(vo, None, idx)
            G["val_out_t"][:n_val].copy_(out.view(n_val, self.val_w), non_blocking=True)
        G["event"].record()

    @torch.no_grad()
    def _evaluate(self, g, n_pol, n_val):
        if self.use_graphs:
            return self._evaluate_graphs(g, n_pol, n_val)
        """Launches the networks on group g's requests; the answers land in the group's host buffers (call `_collect` before reading)."""
        G = g
        M = self.M
        if n_pol:
            obs = G["pol_obs_t"][:n_pol].to(DEV, non_blocking=True)
            mask = G["pol_mask_t"][:n_pol].to(DEV, non_blocking=True)
            u = G["pol_u_t"][:n_pol].to(DEV, non_blocking=True)
            def pol_fn(net):
                def pol(o, m, u, **shape):
                    lg, _ = net(o, m, value=False, ufeat=self._uf(o), **shape)
                    p = torch.softmax(lg.float(), 1)
                    tp, ti = p.topk(M, 1)
                    act = ti[:, 0] if self.greedy_roll else _sample(p, u)
                    return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
                return pol
            res = self._run(pol_fn(self.roll_net), G["pol_obs"][:n_pol], obs, mask, u)
            if self.roll_net is not self.net:  # decisions of the real fight go to the (stronger) main network
                ir = np.flatnonzero(G["pol_kind"][:n_pol] == 0)
                if len(ir):
                    ir_t = torch.from_numpy(ir).to(DEV)
                    res[ir_t] = self._run(pol_fn(self.net), G["pol_obs"][ir], obs[ir_t], mask[ir_t], u[ir_t])
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            vo_np, vo = self._val_obs(G, n_val)
            def val(o, m, **shape):
                if self.dist:
                    ol, pl = self.net.heads_out(o, ufeat=self._uf(o), **shape)
                    p = torch.softmax(ol, 1)
                    return torch.cat([p, torch.sigmoid(pl)], 1) if self.pot else p
                v = self.net(o, None, policy=False, ufeat=self._uf(o), **shape)[1]
                for n2 in self.value_nets:
                    v = v + n2(o, None, policy=False, ufeat=self._uf(o), **shape)[1]
                return (v / (1 + len(self.value_nets))).unsqueeze(1)
            G["val_out_t"][:n_val].copy_(self._run(val, vo_np, vo).view(n_val, self.val_w), non_blocking=True)
        if self.cuda:
            G["event"].record()

    @staticmethod
    def _val_obs(G, n):
        """The `n` value rows of group G in row order: (host view, device tensor). In the shared layout row r sits at `shared - 1 - r`."""
        s = G["shared"]
        if s:
            return G["pol_obs"][s - n:s][::-1], G["pol_obs_t"][s - n:s].to(DEV, non_blocking=True).flip(0)
        return G["val_obs"][:n], G["val_obs_t"][:n].to(DEV, non_blocking=True)

    @staticmethod
    def _advance(G, pol=None, val=None):
        if G["shared"]:
            return G["eng"].advance_shared(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["pol_u"], G["val_kind"], pol, val)
        return G["eng"].advance(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["pol_u"], G["val_obs"], G["val_kind"], pol, val)

    def _collect(self, g):
        if self.cuda:
            g["event"].synchronize()

    # ---- driver ----
    def run(self, scenarios, job_scen, job_seed, verbose=False, starts=None, worth=None):
        """`worth`: per scenario None (linear) or dict(u=[NC], price=[POT]) (`worth_row`); needs dist value rows."""
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        wt = None
        if worth is not None and any(w is not None for w in worth):
            if not self.dist:
                raise ValueError("a worth table needs dist value rows (an outcome-head network, no extra value nets)")
            assert len(worth) == len(scenarios)
            wt = np.stack([worth_row(w) for w in worth])
        job_scen = np.ascontiguousarray(job_scen, np.uint32)
        job_seed = np.ascontiguousarray(job_seed, np.uint64)
        nj = len(job_scen)
        sj = [json.dumps(s) for s in scenarios]
        groups = []
        # the previous run's engines (kept for `moves`) hold every block's play-out combats (~19 KB each: 3 GB per group at 1024 blocks x 5x32): drop
        # them before building new ones instead of holding two sets at the peak
        self._runs = []
        t0 = time.perf_counter()
        for gi in range(self.groups):
            idx = np.arange(gi, nj, self.groups)  # interleaved jobs: every group sees the whole mix
            if len(idx) == 0:
                continue
            nb = min(max(1, self.roots // self.groups), len(idx))  # blocks of this engine: more threads than blocks only cost the pool's start (~1 ms of a live round)
            eng = sts2._SearchEngine(sj, job_scen[idx], job_seed[idx], max(1, self.roots // self.groups), self.M, self.K, self.conf, 0.0, 0.0,
                                     self.roll_cap, self.max_steps, 1.0, -1.0, self.hp_bonus, min(self.threads, nb), self.record, self.lead, self.carry, self.strat, starts,
                                     None if self.util is None else [float(x) for x in self.util], leaf_turns=self.leaf_turns, turn_cap=heads.TURN_CAP,
                                     val_w=self.val_w, worth=wt)
            # one observation buffer for policy and value rows when the engine supports it (`advance_shared`: half the pinned memory); an older
            # extension gets the two buffers of `max_rows`
            shared = eng.shared_rows() if hasattr(eng, "advance_shared") else 0
            pc, vc = (shared, shared) if shared else eng.max_rows()
            while len(self._bufs) <= gi:
                self._bufs.append(HostBuffers(self.cuda))
            B = self._bufs[gi]
            B.ensure(pc, vc, 2 * self.M + 1, self.val_w, shared=bool(shared))
            G = dict(eng=eng, idx=idx, n_pol=0, n_val=0, shared=shared)
            for name, arr in B.a.items():
                G[name], G[name + "_t"] = arr, B.t[name]
            if self.cuda:
                G["event"] = torch.cuda.Event()
            groups.append(G)
        self.timers["setup"] += time.perf_counter() - t0
        t0 = time.perf_counter()
        for G in groups:
            G["n_pol"], G["n_val"] = self._advance(G)
            self._evaluate(G, G["n_pol"], G["n_val"])
        active = list(groups)
        cycles = rows = 0
        peak_pol = peak_val = peak_rows = 0  # the most rows one group requested in one cycle (the buffers hold max_rows)
        while active:
            for G in list(active):
                t = time.perf_counter()
                self._collect(G)
                self.timers["wait net"] += time.perf_counter() - t
                t = time.perf_counter()
                npol, nval = G["n_pol"], G["n_val"]
                G["n_pol"], G["n_val"] = self._advance(G, G["pol_out"][:npol], G["val_out"][:nval].reshape(-1))
                self.timers["engine"] += time.perf_counter() - t
                cycles += 1
                rows += G["n_pol"] + G["n_val"]
                peak_pol, peak_val, peak_rows = max(peak_pol, G["n_pol"]), max(peak_val, G["n_val"]), max(peak_rows, G["n_pol"] + G["n_val"])
                if G["n_pol"] == 0 and G["n_val"] == 0:
                    assert G["eng"].finished()
                    active.remove(G)
                    continue
                t = time.perf_counter()
                self._evaluate(G, G["n_pol"], G["n_val"])
                self.timers["launch net"] += time.perf_counter() - t
            if verbose and cycles % 2000 == 0:
                print(f"  cycle {cycles}, {sum(int(G['eng'].results_done()) for G in groups) if hasattr(groups[0]['eng'], 'results_done') else '?'}", flush=True)
        self._runs = [(G["idx"], G["eng"]) for G in groups]
        self._seeds, self._scen, self._job_scen = job_seed, scenarios, job_scen
        out = np.zeros((nj, 8), np.float32)
        for G in groups:
            r = np.zeros((len(G["idx"]), 8), np.float32)
            G["eng"].results(r)
            out[G["idx"]] = r
        tot = collections.Counter()
        for G in groups:
            tot.update(G["eng"].stats())
        self.stats = dict(tot, cycles=cycles, rows_per_cycle=rows / max(cycles, 1), peak_pol=peak_pol, peak_val=peak_val, peak_rows=peak_rows,
                          cap_pol=max(G["eng"].max_rows()[0] for G in groups), cap_val=max(G["eng"].max_rows()[1] for G in groups))
        self.timers["run"] += time.perf_counter() - t0
        return out

    def decide(self, scenario, sim, seed=0):
        """Search ONE decision of a fight in progress. `sim` is an `sts2.Sim` aligned with the real fight (`agent.fight.Replayer.sim`); `scenario` is the fight-start scenario
        (any valid scenario of the same content). The root's M likeliest actions are tried on K determinized futures each (hidden information resampled, everything
        visible kept). Returns dict(action, opts, p, q, legal): the action to play and, per option, its dense action index, the policy's probability and the estimated
        return (win = +1 plus half the HP fraction left, loss = -1); `q` is NaN for options that were not tried (a forced move is not searched)."""
        old = (self.max_steps, self.record)
        self.max_steps, self.record = 1, True
        try:
            self.run([scenario], np.zeros(1, np.uint32), np.array([seed], np.uint64), starts=[sim])
            acts, searched, opts, p, q, legal = self._runs[0][1].moves(0)
        finally:
            self.max_steps, self.record = old
        return dict(action=int(acts[0]), searched=bool(searched[0]), opts=opts[0, :self.M].tolist(), p=p[0, :self.M].tolist(), q=q[0, :self.M].tolist(),
                    legal=legal[0, :self.M].astype(bool).tolist())

    def gpu_ms(self):
        """With `profile_gpu`: {label: (total ms, replays, rows, padded rows)} of the graph replays so far."""
        torch.cuda.synchronize()
        out = {}
        for k, evs in self._ev.items():
            out[k] = (sum(e[0].elapsed_time(e[1]) for e in evs), len(evs), sum(e[2] for e in evs), sum(e[3] for e in evs))
        return out
