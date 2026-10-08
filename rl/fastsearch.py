#!/usr/bin/env python3
"""The solver's search, driven by the Rust state machine (`sts2env::search`): this file only evaluates networks.

The engine plays many fights at once, each at its own pace. Whenever a fight (or one of its play-outs) needs a decision the engine writes
one observation row into a request buffer; this driver runs the policy on all policy rows and the value head on all value rows in one batch
and hands the answers back. Two engines (`groups`) alternate so the CPU simulates one while the GPU evaluates the other.

  fs = FastSearch(net, M=3, K=8)
  rows = fs.run(scenario_dicts, job_scen, job_seed)      # [n_jobs, 8]: scenario, outcome, hp_lost, hp_end, length, finished, end HP (absolute), potions kept (bits)

With an outcome-head network (`rl/heads.py`), value rows come back as the head's class probabilities and Rust combines them with each job's worth
(`run(..., worth=)`: per scenario None = the linear return, or a table over the classes).
"""
import json, os, sys, collections
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads
from model import DEV, layout, obs_version_of

OBS, ACT = sts2.OBS_SIZE, sts2.ACTIONS  # OBS: the version-1 row length (a `FastSearch` uses its network's: `self.OBS`)
NC = sts2.names()["head_nc"]
assert NC == heads.NC and sts2.names()["head_bin"] == heads.BIN, "rl/heads.py and the Rust search disagree on the outcome classes"
WORTH_W = 1 + NC
BUCKETS = (256, 512, 1024, 2048, 4096, 8192, 16384)
GRAPH_E = 8


def worth_row(w):
    """One scenario's worth for the engine: None = linear; else dict(u=[NC] class worths)."""
    r = np.zeros(WORTH_W, np.float32)
    if w is None:
        return r
    u = np.asarray(w["u"], np.float32)
    assert u.shape == (NC,), u.shape
    r[0] = 1.0
    r[1:] = u
    return r
# Play-out depth in player turns before the value network takes over. 2 since 2026-10-06: decision regret vs a Monte Carlo referee 0.0038 vs 0.0099 at
# depth 1 (150 recorded states, `tools/bench_search.py`, paired fight-clustered CI of the difference excludes 0); whole fights with the live search shape
# (5x32, 1200 eval fights x 4) win +0.88 % +- 0.37 %, HP lost -1.1 % of max +- 0.15 % (`tools/ab_leaf.py`, evals/ab_leaf_5x32.json). Costs ~2x per decision.
LEAF_TURNS = 2

_SHAPE_CONSTS = {}


def _shape_consts(width):
    """(enemies offset, ENEMY_F, OBS_MAX_ENEMIES, pile offsets, decision offset) of the observation version whose rows have `width` floats."""
    if width not in _SHAPE_CONSTS:
        v = obs_version_of(width)
        if v is None:
            raise ValueError(f"no observation version has rows of {width} floats")
        c, sec = layout(v)
        _SHAPE_CONSTS[width] = (sec["enemies"][0], c["ENEMY_F"], c["OBS_MAX_ENEMIES"], [sec[n][0] for n in ("draw", "discard", "exhaust")], sec["decision"][0])
    return _SHAPE_CONSTS[width]
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
    and the rows that have a pending card selection. The observation version is told by the row length."""
    _E0, _ES, _EN, _PILES, _DEC = _shape_consts(obs.shape[1])
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

    def __init__(self, fn, with_mask, pool, with_u=False, obs_size=OBS):
        self.fn, self.buckets, self.with_mask, self.pool, self.with_u = fn, BUCKETS, with_mask, pool, with_u
        self.obs_size = obs_size  # floats per observation row (the network's observation version)
        self.graphs = {}

    def _capture(self, B):
        sobs = torch.zeros(B, self.obs_size, device=DEV)
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
            g.replay()
            outs.append(out[:m].clone())
            a += m
        return outs[0] if len(outs) == 1 else torch.cat(outs)

    def plan(self, n):
        """(rows, padded batch) per replay covering `n` rows: whole top buckets, then the remainder in the bucket that holds it when that pads by at
        most the smallest bucket, else the largest bucket below it, and again. Padding the remainder to the next bucket wasted 22 % of the policy
        rows and 77 % of the value rows (5x32, roots 1024); the network's time is close to proportional to the padded batch."""
        bs, out = self.buckets, []
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

    PINNED = ("obs", "pol_mask", "pol_u", "pol_out", "val_out")

    def __init__(self, pin, obs_size=OBS):
        self.pin = pin
        self.obs_size = obs_size  # floats per observation row
        self.a = {}  # name -> numpy array (registered when pinned)
        self.t = {}  # name -> torch view of the same memory
        self._reg = []  # registered base pointers

    def ensure(self, rows, pol_w, val_w):
        """Room for `rows` rows of the engine's shared layout: policy rows from the front of `obs`, value rows from its end backwards."""
        want = {"obs": (rows, self.obs_size), "pol_mask": (rows, ACT), "pol_kind": (rows,), "pol_u": (rows,), "val_kind": (rows,),
                "pol_out": (rows, pol_w), "val_out": (rows, val_w)}
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
    def __init__(self, net, M=3, K=8, conf=1.01, max_steps=300, roots=512, groups=2, threads=None, roll_net=None, amp=False, record=False,
                 leaf_turns=None, clairvoyant=False):
        """`net`: ranks the options of the real fight's decisions; `roll_net` (default: `net`): plays the play-outs (a cheaper network is fine:
        the play-outs only have to finish the turn plausibly).

        `clairvoyant`: DIAGNOSTIC ONLY -- SEES HIDDEN INFORMATION (draw pile order, every RNG stream: the real future). The K futures of a decision are
        copies of the true state instead of determinizations (`SearchCfg::clairvoyant`), so the search plays with knowledge it can never have in a real
        game. It measures how winnable a fight set is (`tools/headroom.py`); it is not a player. Never set it for live play: `decide` (the live engine's
        entry point) refuses it."""
        self.net = net
        self.roll_net = roll_net if roll_net is not None else net
        # the observation version the networks read (their checkpoints'): the engines write rows of that version
        vs = {getattr(n, "obs_version", 1) for n in [self.net, self.roll_net]}
        if len(vs) > 1:
            raise ValueError(f"the search's networks read different observation versions {sorted(vs)}")
        self.obs_version = vs.pop()
        self.OBS = sts2.obs_size(self.obs_version)
        self.M, self.K, self.conf = M, K, conf
        # play-out depth: the ONE default for live play and every batch table (agent.engine, rl/solver.py; evals/bench_search*.jsonl, evals/ab_leaf_5x32.json)
        self.leaf_turns = LEAF_TURNS if leaf_turns is None else leaf_turns  # player turns a play-out runs before the value network (1 = this turn; large = to the fight's end)
        self.roll_cap = 60 * self.leaf_turns if self.leaf_turns < 100 else 400  # step cap of a play-out, scaled with its depth
        self.max_steps = max_steps
        self.roots, self.groups = roots, groups
        self.threads = threads or max(2, available_cpus() - 1)
        self.stats = {}
        self.cuda = DEV.type == "cuda"
        self.compile = self.cuda  # torch.compile (inductor fusion, dynamic batch) inside the CUDA graphs: about 1.7x faster networks
        self.record = record  # keep the moves of every fight (`moves`, `replay`): play-by-play traces
        # value rows as the outcome head's class probabilities, combined in Rust per job; a scalar value otherwise
        self.dist = bool(getattr(net, "heads", False))
        self.val_w = NC if self.dist else 1
        self._runs = []
        self._bufs = []  # one HostBuffers per engine group, reused across runs
        self.amp = amp  # bf16 autocast inside the graphs (the networks are compute-bound there)
        self.clairvoyant = bool(clairvoyant)  # DIAGNOSTIC ONLY: the futures are the true state (see the docstring); never for live play
        self._graphs = {}
        self._pool = torch.cuda.graph_pool_handle() if self.cuda else None
        self.use_graphs = self.cuda

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
        for net in {id(self.net): self.net, id(self.roll_net): self.roll_net}.values():
            fn = self._pol_graph(net)
            for B in fn.buckets:
                fn._capture(B)
        fn = self._val_graph()
        for B in fn.buckets:
            fn._capture(B)
        torch.cuda.synchronize()

    def _pol_graph(self, net):
        key = ("pol", id(net))
        if key not in self._graphs:
            M, amp = self.M, self.amp
            logits = lambda o, m: net(o, m, value=False, E=GRAPH_E, L=64, has_dec=True)[0]
            if self.compile:
                logits = torch.compile(logits, dynamic=True)

            def fn(o, m, u):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    lg = logits(o, m)
                pr = torch.softmax(lg.float(), 1)
                act = _sample(pr, u)
                tp, ti = pr.topk(M, 1)
                return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
            self._graphs[key] = GraphFn(fn, True, self._pool, with_u=True, obs_size=self.OBS)
        return self._graphs[key]

    def _val_graph(self):
        key = ("val",)
        if key not in self._graphs:
            net, amp = self.net, self.amp
            if self.dist:
                val = lambda o: torch.softmax(net.heads_out(o, E=GRAPH_E, L=64, has_dec=True)[0], 1)
            else:
                val = lambda o: net(o, None, policy=False, E=GRAPH_E, L=64, has_dec=True)[1].float()
            if self.compile:
                val = torch.compile(val, dynamic=True)

            def fn(o, m):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    v = val(o)
                return v.float() if self.dist else v.unsqueeze(1)
            self._graphs[key] = GraphFn(fn, False, self._pool, obs_size=self.OBS)
        return self._graphs[key]

    @torch.no_grad()
    def _evaluate_graphs(self, G, n_pol, n_val):
        M = self.M
        if n_pol:
            obs = G["obs_t"][:n_pol].to(DEV, non_blocking=True)
            mask = G["pol_mask_t"][:n_pol].to(DEV, non_blocking=True)
            sim = (G["pol_kind"][:n_pol] & 1) != 0
            u = G["pol_u_t"][:n_pol].to(DEV, non_blocking=True)
            res = torch.empty(n_pol, 2 * M + 1, device=DEV)
            split = self.roll_net is not self.net
            # with a separate play-out network the real fight's decisions go to the main network
            for use_main in ((False, True) if split else (None,)):
                sel = np.ones(n_pol, bool) if use_main is None else (sim != use_main)
                k = int(sel.sum())
                if k == 0:
                    continue
                fn = self._pol_graph(self.net if (use_main or not split) else self.roll_net)
                if k == n_pol:
                    res.copy_(fn(obs, mask, u=u))
                else:
                    idx = torch.from_numpy(np.flatnonzero(sel)).to(DEV, non_blocking=True)
                    res[idx] = fn(obs, mask, idx, u=u)
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            _, vo = self._val_obs(G, n_val)
            G["val_out_t"][:n_val].copy_(self._val_graph()(vo, None).view(n_val, self.val_w), non_blocking=True)
        G["event"].record()

    @torch.no_grad()
    def _evaluate(self, G, n_pol, n_val):
        """Launches the networks on group G's requests; the answers land in the group's host buffers (call `_collect` before reading)."""
        if self.use_graphs:
            return self._evaluate_graphs(G, n_pol, n_val)
        M = self.M
        if n_pol:
            obs = G["obs_t"][:n_pol].to(DEV, non_blocking=True)
            mask = G["pol_mask_t"][:n_pol].to(DEV, non_blocking=True)
            u = G["pol_u_t"][:n_pol].to(DEV, non_blocking=True)
            def pol_fn(net):
                def pol(o, m, u, **shape):
                    lg, _ = net(o, m, value=False, **shape)
                    p = torch.softmax(lg.float(), 1)
                    tp, ti = p.topk(M, 1)
                    act = _sample(p, u)
                    return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
                return pol
            res = self._run(pol_fn(self.roll_net), G["obs"][:n_pol], obs, mask, u)
            if self.roll_net is not self.net:  # decisions of the real fight go to the (stronger) main network
                ir = np.flatnonzero(G["pol_kind"][:n_pol] == 0)
                if len(ir):
                    ir_t = torch.from_numpy(ir).to(DEV)
                    res[ir_t] = self._run(pol_fn(self.net), G["obs"][ir], obs[ir_t], mask[ir_t], u[ir_t])
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            vo_np, vo = self._val_obs(G, n_val)
            def val(o, m, **shape):
                if self.dist:
                    return torch.softmax(self.net.heads_out(o, **shape)[0], 1)
                return self.net(o, None, policy=False, **shape)[1].unsqueeze(1)
            G["val_out_t"][:n_val].copy_(self._run(val, vo_np, vo).view(n_val, self.val_w), non_blocking=True)
        if self.cuda:
            G["event"].record()

    @staticmethod
    def _val_obs(G, n):
        """The `n` value rows of group G in row order: (host view, device tensor). Row r sits at `shared - 1 - r`."""
        s = G["shared"]
        return G["obs"][s - n:s][::-1], G["obs_t"][s - n:s].to(DEV, non_blocking=True).flip(0)

    @staticmethod
    def _advance(G, pol=None, val=None):
        return G["eng"].advance_shared(G["obs"], G["pol_mask"], G["pol_kind"], G["pol_u"], G["val_kind"], pol, val)

    def _collect(self, g):
        if self.cuda:
            g["event"].synchronize()

    # ---- driver ----
    def run(self, scenarios, job_scen, job_seed, starts=None, worth=None):
        """`worth`: per scenario None (linear) or dict(u=[NC]) (`worth_row`); needs dist value rows."""
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        wt = None
        if worth is not None and any(w is not None for w in worth):
            if not self.dist:
                raise ValueError("a worth table needs dist value rows (an outcome-head network)")
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
        for gi in range(self.groups):
            idx = np.arange(gi, nj, self.groups)  # interleaved jobs: every group sees the whole mix
            if len(idx) == 0:
                continue
            nb = min(max(1, self.roots // self.groups), len(idx))  # blocks of this engine: more threads than blocks only cost the pool's start (~1 ms of a live round)
            eng = sts2._SearchEngine(sj, job_scen[idx], job_seed[idx], n_roots=max(1, self.roots // self.groups), m=self.M, k=self.K, conf=self.conf,
                                     roll_cap=self.roll_cap, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, threads=min(self.threads, nb),
                                     record=self.record, lead=True, carry=True, strat=True, starts=starts, leaf_turns=self.leaf_turns,
                                     turn_cap=heads.TURN_CAP, val_w=self.val_w, worth=wt, clairvoyant=self.clairvoyant, obs_version=self.obs_version)
            shared = eng.shared_rows()
            while len(self._bufs) <= gi:
                self._bufs.append(HostBuffers(self.cuda, self.OBS))
            B = self._bufs[gi]
            B.ensure(shared, 2 * self.M + 1, self.val_w)
            G = dict(eng=eng, idx=idx, n_pol=0, n_val=0, shared=shared)
            for name, arr in B.a.items():
                G[name], G[name + "_t"] = arr, B.t[name]
            if self.cuda:
                G["event"] = torch.cuda.Event()
            groups.append(G)
        for G in groups:
            G["n_pol"], G["n_val"] = self._advance(G)
            self._evaluate(G, G["n_pol"], G["n_val"])
        active = list(groups)
        while active:
            for G in list(active):
                self._collect(G)
                npol, nval = G["n_pol"], G["n_val"]
                G["n_pol"], G["n_val"] = self._advance(G, G["pol_out"][:npol], G["val_out"][:nval].reshape(-1))
                if G["n_pol"] == 0 and G["n_val"] == 0:
                    assert G["eng"].finished()
                    active.remove(G)
                    continue
                self._evaluate(G, G["n_pol"], G["n_val"])
        self._runs = [(G["idx"], G["eng"]) for G in groups]
        out = np.zeros((nj, 8), np.float32)
        for G in groups:
            r = np.zeros((len(G["idx"]), 8), np.float32)
            G["eng"].results(r)
            out[G["idx"]] = r
        tot = collections.Counter()
        for G in groups:
            tot.update(G["eng"].stats())
        self.stats = dict(tot)
        return out

    def decide(self, scenario, sim, seed=0, worth=None):
        """Search ONE decision of a fight in progress. `sim` is an `sts2.Sim` aligned with the real fight (`agent.fight.Replayer.sim`); `scenario` is the fight-start scenario
        (any valid scenario of the same content). The root's M likeliest actions are tried on K determinized futures each (hidden information resampled, everything
        visible kept). Returns dict(action, opts, p, q, legal): the action to play and, per option, its dense action index, the policy's probability and the estimated
        return (win = +1 plus half the HP fraction left, loss = -1; with `worth` (`worth_row`) the table's units); `q` is NaN for options that were not tried (a forced move is not searched)."""
        if self.clairvoyant:
            raise RuntimeError("FastSearch(clairvoyant=True) sees hidden information: diagnostic only, never a live decision")
        old = (self.max_steps, self.record)
        self.max_steps, self.record = 1, True
        try:
            self.run([scenario], np.zeros(1, np.uint32), np.array([seed], np.uint64), starts=[sim], worth=None if worth is None else [worth])
            acts, searched, opts, p, q, legal = self._runs[0][1].moves(0)
        finally:
            self.max_steps, self.record = old
        W = self.M
        return dict(action=int(acts[0]), searched=bool(searched[0]), opts=opts[0, :W].tolist(), p=p[0, :W].tolist(), q=q[0, :W].tolist(),
                    legal=legal[0, :W].astype(bool).tolist())

    def job_actions(self, j):
        """With `record`: the dense actions job j of the last `run` took, in order (jobs are interleaved over the engine groups)."""
        for idx, eng in self._runs:
            k = int(np.searchsorted(idx, j))
            if k < len(idx) and idx[k] == j:
                return eng.moves(k)[0].tolist()
        raise IndexError(j)
