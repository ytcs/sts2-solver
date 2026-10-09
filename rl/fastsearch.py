#!/usr/bin/env python3
import json, os, sys, collections
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
import heads
from model import C, DEV, SEC

OBS, ACT = sts2.OBS_SIZE, sts2.ACTIONS
NC = sts2.names()["head_nc"]
assert NC == heads.NC and sts2.names()["head_bin"] == heads.BIN, "rl/heads.py and the Rust search disagree on the outcome classes"
WORTH_W = 1 + NC
BUCKETS = (256, 512, 1024, 2048, 4096, 8192, 16384)
GRAPH_E = 8


def worth_row(w):
    r = np.zeros(WORTH_W, np.float32)
    if w is None:
        return r
    u = np.asarray(w["u"], np.float32)
    assert u.shape == (NC,), u.shape
    r[0] = 1.0
    r[1:] = u
    return r

LEAF_TURNS = 2
EXACT_TURN = dict(loss=-0.9, tie=0.0, dets=8, cap=5000)

DEC = SEC["decision"][0]

if DEV.type == "cuda":
    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True


def available_cpus():
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
    e0, es = SEC["enemies"][0], C["ENEMY_F"]
    occ = (obs[:, e0:e0 + C["OBS_MAX_ENEMIES"] * es:es] > 0.5).any(0)
    E = int(np.nonzero(occ)[0].max()) + 1 if occ.any() else 1
    L = 1
    for o in (SEC[n][0] for n in ("draw", "discard", "exhaust")):
        L = max(L, int((obs[:, o:o + 128:2] > 0).sum(1).max()))
    return E, L, obs[:, DEC] > 0.5


# play-out moves use the engine's uniform, never torch's RNG: the same job seeds give the same play-outs
def _sample(pr, u):
    c = pr.cumsum(1)
    return (c < u.unsqueeze(1) * c[:, -1:]).sum(1).clamp_max(pr.shape[1] - 1)


class GraphFn:
    """CUDA graphs of fn per padded batch size. With `full` (the same network evaluated on every row as a decision row) fn also takes a fixed-size
    list of decision rows (-1 = padding, a quarter of the batch); a batch with more decision rows replays `full`."""

    def __init__(self, fn, with_mask, pool, with_u=False, full=None):
        self.fn, self.buckets, self.with_mask, self.pool, self.with_u = fn, BUCKETS, with_mask, pool, with_u
        self.full = full
        self.graphs = {}

    @staticmethod
    def cap(B):
        return max(1, B // 4)

    def _capture(self, B):
        sobs = torch.zeros(B, OBS, device=DEV)
        smask = torch.zeros(B, ACT, dtype=torch.uint8, device=DEV) if self.with_mask else None
        if smask is not None:
            # padded rows need a legal action (no NaN softmax)
            smask[:, 0] = 1
        su = torch.full((B,), 0.5, device=DEV) if self.with_u else None
        srows = torch.full((self.cap(B),), -1, dtype=torch.long, device=DEV) if self.full is not None else None
        args = (sobs, smask) + ((su,) if self.with_u else ()) + ((srows,) if srows is not None else ())
        st = torch.cuda.Stream()
        st.wait_stream(torch.cuda.current_stream())
        with torch.cuda.stream(st), torch.no_grad():
            for _ in range(2):
                self.fn(*args)
        torch.cuda.current_stream().wait_stream(st)
        g = torch.cuda.CUDAGraph()
        with torch.no_grad(), torch.cuda.graph(g, pool=self.pool):
            out = self.fn(*args)
        self.graphs[B] = (g, sobs, smask, su, srows, out)

    def rows_plan(self, dec):
        """Per planned batch: its decision rows padded with -1 to the cap, or None (more than the cap: `full` replays it)."""
        out, a = [], 0
        if self.full is None:
            return out
        for m, B in self.plan(len(dec)):
            d = np.flatnonzero(dec[a:a + m])
            r = None
            if len(d) <= self.cap(B):
                r = np.full(self.cap(B), -1, np.int64)
                r[:len(d)] = d
            out.append(r)
            a += m
        return out

    @torch.no_grad()
    def __call__(self, obs, mask, idx=None, u=None, rows=None):
        n = len(obs) if idx is None else len(idx)
        outs = []
        a = 0
        for k, (m, B) in enumerate(self.plan(n)):
            if self.full is not None and (rows is None or rows[k] is None):
                if idx is None:
                    outs.append(self.full(obs[a:a + m], None if mask is None else mask[a:a + m], u=None if u is None else u[a:a + m]))
                else:
                    outs.append(self.full(obs, mask, idx[a:a + m], u=u))
                a += m
                continue
            if B not in self.graphs:
                self._capture(B)
            g, sobs, smask, su, srows, out = self.graphs[B]
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
            if srows is not None:
                srows.copy_(rows[k])
            g.replay()
            outs.append(out[:m].clone())
            a += m
        return outs[0] if len(outs) == 1 else torch.cat(outs)

    def plan(self, n):
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
    PINNED = ("obs", "pol_mask", "pol_u", "pol_out", "val_out", "ix")

    def __init__(self, pin):
        self.pin = pin
        self.a = {}
        self.t = {}
        self._reg = []

    def ensure(self, rows, pol_w, val_w):
        want = {"obs": (rows, OBS), "pol_mask": (rows, ACT), "pol_kind": (rows,), "pol_u": (rows,), "val_kind": (rows,),
                "pol_out": (rows, pol_w), "val_out": (rows, val_w), "ix": (rows + 1024,)}
        dts = {"pol_mask": np.uint8, "pol_kind": np.uint8, "val_kind": np.uint8, "ix": np.int64}
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
        except Exception:
            pass


class FastSearch:
    def __init__(self, net, M=3, K=8, max_steps=300, roots=512, groups=2, threads=None, roll_net=None, amp=False, record=False,
                 leaf_turns=None, clairvoyant=False, cover=False, futures=0, exact_turn=None):
        self.net = net
        self.roll_net = roll_net if roll_net is not None else net
        self.cover, self.futures = bool(cover), int(futures)
        self.exact = None if not exact_turn else {**EXACT_TURN, **(exact_turn if isinstance(exact_turn, dict) else {})}
        self.M, self.K = (sts2.names()["max_m"] if self.cover else M), K
        self.leaf_turns = LEAF_TURNS if leaf_turns is None else leaf_turns
        self.roll_cap = 60 * self.leaf_turns if self.leaf_turns < 100 else 400
        self.max_steps = max_steps
        self.roots, self.groups = roots, groups
        self.threads = threads or max(2, available_cpus() - 1)
        self.stats = {}
        self.cuda = DEV.type == "cuda"
        self.compile = self.cuda
        self.record = record
        self.dist = bool(getattr(net, "heads", False))
        self.val_w = NC if self.dist else 1
        self._runs = []
        self._bufs = []
        self.amp = amp
        # DIAGNOSTIC ONLY: clairvoyant futures see hidden information; decide() refuses it
        self.clairvoyant = bool(clairvoyant)
        self._graphs = {}
        self._pool = torch.cuda.graph_pool_handle() if self.cuda else None
        self._copy = torch.cuda.Stream() if self.cuda else None
        self.use_graphs = self.cuda

    def _run(self, fn, obs_np, obs_t, mask_t=None, u_t=None):
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
        if not self.use_graphs:
            return
        fns = [self._pol_graph(net) for net in {id(self.net): self.net, id(self.roll_net): self.roll_net}.values()] + [self._val_graph()]
        for fn in fns + [f.full for f in fns if f.full is not None]:
            for B in fn.buckets:
                fn._capture(B)
        torch.cuda.synchronize()

    def _pol_graph(self, net):
        key = ("pol", id(net))
        if key not in self._graphs:
            M, amp = self.M, self.amp

            def logits(o, m, rows=None):
                shp = dict(has_dec=True) if rows is None else dict(rows=rows.clamp(min=0), rows_w=(rows >= 0).float())
                return net(o, m, value=False, E=GRAPH_E, L=64, **shp)[0]
            if self.compile:
                logits = torch.compile(logits, dynamic=True)

            def fn(o, m, u, rows=None):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    lg = logits(o, m, rows)
                pr = torch.softmax(lg.float(), 1)
                act = _sample(pr, u)
                tp, ti = pr.topk(M, 1)
                return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
            self._graphs[key] = GraphFn(fn, True, self._pool, with_u=True, full=GraphFn(fn, True, self._pool, with_u=True))
        return self._graphs[key]

    def _val_graph(self):
        key = ("val",)
        if key not in self._graphs:
            net, amp = self.net, self.amp

            def val(o, rows=None):
                shp = dict(has_dec=True) if rows is None else dict(rows=rows.clamp(min=0), rows_w=(rows >= 0).float())
                if self.dist:
                    return torch.softmax(net.heads_out(o, E=GRAPH_E, L=64, **shp), 1)
                return net(o, None, policy=False, E=GRAPH_E, L=64, **shp)[1].float()
            if self.compile:
                val = torch.compile(val, dynamic=True)

            def fn(o, m, rows=None):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    v = val(o, rows)
                return v.float() if self.dist else v.unsqueeze(1)
            self._graphs[key] = GraphFn(fn, False, self._pool, full=GraphFn(fn, False, self._pool))
        return self._graphs[key]

    def _upload(self, G, n_pol, n_val, plans):
        main = torch.cuda.current_stream()
        k = 0
        for p in plans:
            for r in p:
                if r is not None:
                    G["ix"][k:k + len(r)] = r
                    k += len(r)
        with torch.cuda.stream(self._copy):
            up = [G[k_][:n_pol].to(DEV, non_blocking=True) for k_ in ("obs_t", "pol_mask_t", "pol_u_t")] if n_pol else [None] * 3
            s = G["shared"]
            up.append(G["obs_t"][s - n_val:s].to(DEV, non_blocking=True) if n_val else None)
            up.append(G["ix_t"][:k].to(DEV, non_blocking=True) if k else None)
        main.wait_stream(self._copy)
        for t in up:
            if t is not None:
                t.record_stream(main)
        rows, k = [], 0
        for p in plans:
            rows.append([])
            for r in p:
                rows[-1].append(None if r is None else up[4][k:k + len(r)])
                k += 0 if r is None else len(r)
        return up[:4], rows

    @torch.no_grad()
    def _evaluate_graphs(self, G, n_pol, n_val):
        M = self.M
        calls = []
        if n_pol:
            sim = (G["pol_kind"][:n_pol] & 1) != 0
            dec = G["obs"][:n_pol, DEC] > 0.5
            split = self.roll_net is not self.net
            for use_main in ((False, True) if split else (None,)):
                sel = np.ones(n_pol, bool) if use_main is None else (sim != use_main)
                if sel.any():
                    fn = self._pol_graph(self.net if (use_main or not split) else self.roll_net)
                    calls.append((fn, sel, fn.rows_plan(dec[sel])))
        if n_val:
            s = G["shared"]
            vfn = self._val_graph()
            vplan = vfn.rows_plan(G["obs"][s - n_val:s, DEC][::-1] > 0.5)
        (obs, mask, u, vrows), rows = self._upload(G, n_pol, n_val, [c[2] for c in calls] + ([vplan] if n_val else []))
        if n_pol:
            res = torch.empty(n_pol, 2 * M + 1, device=DEV)
            for (fn, sel, _), r in zip(calls, rows):
                if sel.all():
                    res.copy_(fn(obs, mask, u=u, rows=r))
                else:
                    idx = torch.from_numpy(np.flatnonzero(sel)).to(DEV, non_blocking=True)
                    res[idx] = fn(obs, mask, idx, u=u, rows=r)
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            G["val_out_t"][:n_val].copy_(vfn(vrows.flip(0), None, rows=rows[-1]).view(n_val, self.val_w), non_blocking=True)
        G["event"].record()

    @torch.no_grad()
    def _evaluate(self, G, n_pol, n_val):
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
            if self.roll_net is not self.net:
                ir = np.flatnonzero(G["pol_kind"][:n_pol] == 0)
                if len(ir):
                    ir_t = torch.from_numpy(ir).to(DEV)
                    res[ir_t] = self._run(pol_fn(self.net), G["obs"][ir], obs[ir_t], mask[ir_t], u[ir_t])
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            vo_np, vo = self._val_obs(G, n_val)
            def val(o, m, **shape):
                if self.dist:
                    return torch.softmax(self.net.heads_out(o, **shape), 1)
                return self.net(o, None, policy=False, **shape)[1].unsqueeze(1)
            G["val_out_t"][:n_val].copy_(self._run(val, vo_np, vo).view(n_val, self.val_w), non_blocking=True)
        if self.cuda:
            G["event"].record()

    @staticmethod
    def _val_obs(G, n):
        s = G["shared"]
        # value row r sits at shared - 1 - r
        return G["obs"][s - n:s][::-1], G["obs_t"][s - n:s].to(DEV, non_blocking=True).flip(0)

    @staticmethod
    def _advance(G, pol=None, val=None):
        return G["eng"].advance_shared(G["obs"], G["pol_mask"], G["pol_kind"], G["pol_u"], G["val_kind"], pol, val)

    def _collect(self, g):
        if self.cuda:
            g["event"].synchronize()

    def run(self, scenarios, job_scen, job_seed, starts=None, worth=None):
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
        self._runs = []
        for gi in range(self.groups):
            idx = np.arange(gi, nj, self.groups)
            if len(idx) == 0:
                continue
            nb = min(max(1, self.roots // self.groups), len(idx))
            eng = sts2._SearchEngine(sj, job_scen[idx], job_seed[idx], n_roots=max(1, self.roots // self.groups), m=self.M, k=self.K,
                                     roll_cap=self.roll_cap, max_steps=self.max_steps, win=1.0, loss=-1.0, hp_bonus=0.5, threads=min(self.threads, nb),
                                     record=self.record, starts=starts, leaf_turns=self.leaf_turns,
                                     turn_cap=heads.TURN_CAP, val_w=self.val_w, worth=wt, clairvoyant=self.clairvoyant,
                                     cover=self.cover, futures=self.futures,
                                     **({} if self.exact is None else {"exact": True, **{"ex_" + k: v for k, v in self.exact.items()}}))
            shared = eng.shared_rows()
            while len(self._bufs) <= gi:
                self._bufs.append(HostBuffers(self.cuda))
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
        if self.clairvoyant:
            raise RuntimeError("FastSearch(clairvoyant=True) sees hidden information: diagnostic only, never a live decision")
        old = (self.max_steps, self.record)
        self.max_steps, self.record = 1, True
        try:
            self.run([scenario], np.zeros(1, np.uint32), np.array([seed], np.uint64), starts=[sim], worth=None if worth is None else [worth])
            acts, searched, opts, p, q, legal, exact = self._runs[0][1].moves(0)
        finally:
            self.max_steps, self.record = old
        W = self.M
        return dict(action=int(acts[0]), searched=bool(searched[0]), opts=opts[0, :W].tolist(), p=p[0, :W].tolist(), q=q[0, :W].tolist(),
                    legal=legal[0, :W].astype(bool).tolist(), exact=bool(exact[0]))

    def job_actions(self, j):
        for idx, eng in self._runs:
            k = int(np.searchsorted(idx, j))
            if k < len(idx) and idx[k] == j:
                return eng.moves(k)[0].tolist()
        raise IndexError(j)
