#!/usr/bin/env python3
"""The solver's search, driven by the Rust state machine (`sts2env::search`): this file only evaluates networks.

The engine plays many fights at once, each at its own pace. Whenever a fight (or one of its play-outs) needs a decision the engine writes
one observation row into a request buffer; this driver runs the policy on all policy rows and the value ensemble on all value rows in one batch
and hands the answers back. Two engines (`groups`) alternate so the CPU simulates one while the GPU evaluates the other.

  fs = FastSearch(net, value_nets=[...], M=3, K=8)
  rows = fs.run(scenario_dicts, job_scen, job_seed)      # [n_jobs, 6]: scenario, outcome, hp_lost, hp_end, length, finished
"""
import json, os, sys, time, collections
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import DEV, SEC, C

OBS, ACT = sts2.OBS_SIZE, sts2.ACTIONS
_E0, _ES, _EN = SEC["enemies"][0], C["ENEMY_F"], C["OBS_MAX_ENEMIES"]
_PILES = [SEC[n][0] for n in ("draw", "discard", "exhaust")]
_DEC = SEC["decision"][0]
if DEV.type == "cuda":
    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True


def host_shapes(obs):
    """Shapes the network needs for this batch, from the host-side observation (no device syncs): occupied enemy slots, longest pile list,
    and the rows that have a pending card selection."""
    occ = (obs[:, _E0:_E0 + _EN * _ES:_ES] > 0.5).any(0)
    E = int(np.nonzero(occ)[0].max()) + 1 if occ.any() else 1
    L = 1
    for o in _PILES:
        L = max(L, int((obs[:, o:o + 128:2] > 0).sum(1).max()))
    return E, L, obs[:, _DEC] > 0.5


class GraphFn:
    """`fn(obs [B, OBS], mask [B, ACT] | None) -> [B, ...]` replayed as a CUDA graph per padded batch size: one replay instead of several hundred kernel
    launches (the network is launch-bound at the batch sizes the search produces)."""

    def __init__(self, fn, buckets, with_mask, pool, label="", events=None):
        self.fn, self.buckets, self.with_mask, self.pool = fn, tuple(sorted(buckets)), with_mask, pool
        self.graphs = {}
        self.label, self.events = label, events  # events: a list collecting (cuda event pair, rows) per replay when profiling

    def _capture(self, B):
        sobs = torch.zeros(B, OBS, device=DEV)
        smask = torch.zeros(B, ACT, dtype=torch.uint8, device=DEV) if self.with_mask else None
        if smask is not None:
            smask[:, 0] = 1  # every padded row has a legal action (no NaN in the softmax of rows that are ignored)
        st = torch.cuda.Stream()
        st.wait_stream(torch.cuda.current_stream())
        with torch.cuda.stream(st), torch.no_grad():
            for _ in range(2):
                self.fn(sobs, smask)
        torch.cuda.current_stream().wait_stream(st)
        g = torch.cuda.CUDAGraph()
        with torch.no_grad(), torch.cuda.graph(g, pool=self.pool):
            out = self.fn(sobs, smask)
        self.graphs[B] = (g, sobs, smask, out)

    @torch.no_grad()
    def __call__(self, obs, mask, idx=None):
        """Rows `idx` (a device index tensor) of obs / mask, or all of them; returns a fresh [rows, ...] tensor."""
        n = len(obs) if idx is None else len(idx)
        outs = []
        top = self.buckets[-1]
        for a in range(0, n, top):
            m = min(top, n - a)
            B = next(b for b in self.buckets if b >= m)
            if B not in self.graphs:
                self._capture(B)
            g, sobs, smask, out = self.graphs[B]
            sel = slice(a, a + m) if idx is None else idx[a:a + m]
            if idx is None:
                sobs[:m].copy_(obs[sel])
                if smask is not None:
                    smask[:m].copy_(mask[sel])
            else:
                torch.index_select(obs, 0, sel, out=sobs[:m])
                if smask is not None:
                    torch.index_select(mask, 0, sel, out=smask[:m])
            if self.events is not None:
                e0, e1 = torch.cuda.Event(enable_timing=True), torch.cuda.Event(enable_timing=True)
                e0.record()
                g.replay()
                e1.record()
                self.events.append((e0, e1, m))
            else:
                g.replay()
            outs.append(out[:m].clone())
        return outs[0] if len(outs) == 1 else torch.cat(outs)


class FastSearch:
    def __init__(self, net, value_nets=None, M=3, K=8, conf=1.01, pmin=0.0, margin=0.0, roll_cap=60, max_steps=300, hp_bonus=0.5, greedy_roll=False,
                 roots=512, groups=2, threads=None, roll_net=None, use_graphs=True, graph_E=8, buckets=None, amp=False, value_amp=None, record=False, depth=1 << 30, profile_gpu=False):
        """`net`: ranks the options of the real fight's decisions; `roll_net` (default: `net`): plays the play-outs (a cheaper network is fine:
        the play-outs only have to finish the turn plausibly); `value_nets`: extra networks whose value heads are averaged with `net`'s."""
        self.net, self.value_nets = net, value_nets or []
        self.roll_net = roll_net if roll_net is not None else net
        self.M, self.K, self.conf, self.pmin, self.margin = M, K, conf, pmin, margin
        self.roll_cap, self.max_steps, self.hp_bonus, self.greedy_roll = roll_cap, max_steps, hp_bonus, greedy_roll
        self.roots, self.groups = roots, groups
        self.threads = threads or max(2, (os.cpu_count() or 8) - 2)
        self.timers = collections.defaultdict(float)
        self.stats = {}
        self.cuda = DEV.type == "cuda"
        self.graph_E = graph_E
        self.profile_gpu = profile_gpu and self.cuda  # CUDA events around every graph replay: where the GPU time goes (`gpu_ms`)
        self._ev = collections.defaultdict(list)
        self.depth = depth  # play-outs ask the value network after this many policy decisions (default: play to the end of the turn)
        self.record = record  # keep the moves of every fight (`moves`, `replay`): play-by-play traces
        self._runs = []
        self.amp = amp  # bf16 autocast inside the graphs (the networks are compute-bound there)
        self.value_amp = amp if value_amp is None else value_amp
        self.buckets = (1024, 2048, 4096, 8192, 16384) if buckets is None else buckets
        self.dec_buckets = (64, 256, 1024, 4096)
        self._graphs = {}
        self._pool = torch.cuda.graph_pool_handle() if self.cuda and use_graphs else None
        self.use_graphs = self.cuda and use_graphs

    # ---- network side ----
    def _run(self, fn, obs_np, obs_t, mask_t=None):
        """`fn(obs, mask, **shape)` on the rows of `obs_t` (device) split into rows without / with a pending card selection (each static in shape)."""
        E, L, dec = host_shapes(obs_np)
        nd = int(dec.sum())
        if nd == 0:
            return fn(obs_t, mask_t, E=E, L=L, has_dec=False)
        if nd == len(dec):
            return fn(obs_t, mask_t, E=E, L=L, has_dec=True)
        i1 = torch.from_numpy(np.flatnonzero(dec)).to(DEV)
        i0 = torch.from_numpy(np.flatnonzero(~dec)).to(DEV)
        r0 = fn(obs_t[i0], None if mask_t is None else mask_t[i0], E=E, L=L, has_dec=False)
        r1 = fn(obs_t[i1], None if mask_t is None else mask_t[i1], E=E, L=L, has_dec=True)
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
            for hd in (False, True):
                fn = self._pol_graph(net, hd)
                for B in fn.buckets:
                    fn._capture(B)
        for hd in (False, True):
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

            def fn(o, m):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    lg, _ = net(o, m, value=False, E=E, L=64, has_dec=has_dec)
                lg = lg.float()
                if greedy:
                    act = lg.argmax(1)
                else:  # sampling from softmax(lg) = argmax of the logits plus Gumbel noise
                    act = (lg - torch.log(-torch.log(torch.rand_like(lg).clamp_min(1e-20)))).argmax(1)
                tp, ti = torch.softmax(lg, 1).topk(M, 1)
                return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
            self._graphs[key] = GraphFn(fn, self.dec_buckets if has_dec else self.buckets, True, self._pool, f"pol dec={has_dec}", self._ev[f"pol dec={has_dec}"] if self.profile_gpu else None)
        return self._graphs[key]

    def _val_graph(self, has_dec):
        key = ("val", has_dec)
        if key not in self._graphs:
            nets, E = [self.net] + list(self.value_nets), self.graph_E

            amp = self.value_amp

            def fn(o, m):
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    v = sum(n(o, None, policy=False, E=E, L=64, has_dec=has_dec)[1].float() for n in nets)
                return (v / len(nets)).unsqueeze(1)
            self._graphs[key] = GraphFn(fn, self.dec_buckets if has_dec else self.buckets, False, self._pool, f"val dec={has_dec}", self._ev[f"val dec={has_dec}"] if self.profile_gpu else None)
        return self._graphs[key]

    @torch.no_grad()
    def _evaluate_graphs(self, G, n_pol, n_val):
        M = self.M
        if n_pol:
            obs = G["pol_obs_t"][:n_pol].to(DEV, non_blocking=True)
            mask = G["pol_mask_t"][:n_pol].to(DEV, non_blocking=True)
            kind = G["pol_kind"][:n_pol]
            res = torch.empty(n_pol, 2 * M + 1, device=DEV)
            sim, dec = (kind & 1) != 0, (kind & 2) != 0
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
                        res.copy_(fn(obs, mask))
                    else:
                        idx = torch.from_numpy(np.flatnonzero(sel)).to(DEV, non_blocking=True)
                        res[idx] = fn(obs, mask, idx)
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            vo = G["val_obs_t"][:n_val].to(DEV, non_blocking=True)
            dec = (G["val_kind"][:n_val] & 2) != 0
            nd = int(dec.sum())
            if nd == 0:
                out = self._val_graph(False)(vo, None)
            elif nd == n_val:
                out = self._val_graph(True)(vo, None)
            else:
                out = torch.empty(n_val, 1, device=DEV)
                for hd in (False, True):
                    idx = torch.from_numpy(np.flatnonzero(dec == hd)).to(DEV, non_blocking=True)
                    out[idx] = self._val_graph(hd)(vo, None, idx)
            G["val_out_t"][:n_val].copy_(out.squeeze(1), non_blocking=True)
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
            def pol_fn(net):
                def pol(o, m, **shape):
                    lg, _ = net(o, m, value=False, **shape)
                    p = torch.softmax(lg, 1)
                    tp, ti = p.topk(M, 1)
                    act = ti[:, 0] if self.greedy_roll else torch.multinomial(p, 1).squeeze(1)
                    return torch.cat([ti.float(), tp, act.float().unsqueeze(1)], 1)
                return pol
            res = self._run(pol_fn(self.roll_net), G["pol_obs"][:n_pol], obs, mask)
            if self.roll_net is not self.net:  # decisions of the real fight go to the (stronger) main network
                ir = np.flatnonzero(G["pol_kind"][:n_pol] == 0)
                if len(ir):
                    ir_t = torch.from_numpy(ir).to(DEV)
                    res[ir_t] = self._run(pol_fn(self.net), G["pol_obs"][ir], obs[ir_t], mask[ir_t])
            G["pol_out_t"][:n_pol].copy_(res, non_blocking=True)
        if n_val:
            vo = G["val_obs_t"][:n_val].to(DEV, non_blocking=True)
            def val(o, m, **shape):
                v = self.net(o, None, policy=False, **shape)[1]
                for n2 in self.value_nets:
                    v = v + n2(o, None, policy=False, **shape)[1]
                return v / (1 + len(self.value_nets))
            G["val_out_t"][:n_val].copy_(self._run(val, G["val_obs"][:n_val], vo), non_blocking=True)
        if self.cuda:
            G["event"].record()

    def _collect(self, g):
        if self.cuda:
            g["event"].synchronize()

    # ---- driver ----
    def run(self, scenarios, job_scen, job_seed, verbose=False):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        job_scen = np.ascontiguousarray(job_scen, np.uint32)
        job_seed = np.ascontiguousarray(job_seed, np.uint64)
        nj = len(job_scen)
        sj = [json.dumps(s) for s in scenarios]
        groups = []
        t0 = time.perf_counter()
        for gi in range(self.groups):
            idx = np.arange(gi, nj, self.groups)  # interleaved jobs: every group sees the whole mix
            if len(idx) == 0:
                continue
            eng = sts2._SearchEngine(sj, job_scen[idx], job_seed[idx], max(1, self.roots // self.groups), self.M, self.K, self.conf, self.pmin, self.margin,
                                     self.roll_cap, self.depth, self.max_steps, 1.0, -1.0, self.hp_bonus, self.threads, self.record)
            pc, vc = eng.max_rows()
            pin = self.cuda
            G = dict(eng=eng, idx=idx, n_pol=0, n_val=0)
            for name, shape, dt in (("pol_obs", (pc, OBS), torch.float32), ("pol_mask", (pc, ACT), torch.uint8), ("pol_kind", (pc,), torch.uint8), ("val_obs", (vc, OBS), torch.float32), ("val_kind", (vc,), torch.uint8),
                                    ("pol_out", (pc, 2 * self.M + 1), torch.float32), ("val_out", (vc,), torch.float32)):
                t = torch.empty(shape, dtype=dt, pin_memory=pin)
                G[name + "_t"] = t
                G[name] = t.numpy()
            if self.cuda:
                G["event"] = torch.cuda.Event()
            groups.append(G)
        self.timers["setup"] += time.perf_counter() - t0
        t0 = time.perf_counter()
        for G in groups:
            G["n_pol"], G["n_val"] = G["eng"].advance(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["val_obs"], G["val_kind"])
            self._evaluate(G, G["n_pol"], G["n_val"])
        active = list(groups)
        cycles = rows = 0
        while active:
            for G in list(active):
                t = time.perf_counter()
                self._collect(G)
                self.timers["wait net"] += time.perf_counter() - t
                t = time.perf_counter()
                if not self.cuda:
                    pass
                npol, nval = G["n_pol"], G["n_val"]
                G["n_pol"], G["n_val"] = G["eng"].advance(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["val_obs"], G["val_kind"], G["pol_out"][:npol], G["val_out"][:nval])
                self.timers["engine"] += time.perf_counter() - t
                cycles += 1
                rows += G["n_pol"] + G["n_val"]
                if G["n_pol"] == 0 and G["n_val"] == 0:
                    assert G["eng"].finished()
                    active.remove(G)
                    continue
                t = time.perf_counter()
                self._evaluate(G, G["n_pol"], G["n_val"])
                self.timers["launch net"] += time.perf_counter() - t
            if verbose and cycles % 200 == 0:
                print(f"  cycle {cycles}, {sum(int(G['eng'].results_done()) for G in groups) if hasattr(groups[0]['eng'], 'results_done') else '?'}", flush=True)
        self._runs = [(G["idx"], G["eng"]) for G in groups]
        self._seeds, self._scen, self._job_scen = job_seed, scenarios, job_scen
        out = np.zeros((nj, 6), np.float32)
        for G in groups:
            r = np.zeros((len(G["idx"]), 6), np.float32)
            G["eng"].results(r)
            out[G["idx"]] = r
        tot = collections.Counter()
        for G in groups:
            tot.update(G["eng"].stats())
        self.stats = dict(tot, cycles=cycles, rows_per_cycle=rows / max(cycles, 1))
        self.timers["run"] += time.perf_counter() - t0
        return out

    def trace(self, job):
        """The recorded fight of `job` of the latest run (needs `record=True`): a list of steps `dict(obs, a, info)` (the last one has a=None),
        where `info` (searched decisions only) holds the options considered: acts, p (policy probability), q (estimated return), legal."""
        for idx, eng in self._runs:
            pos = np.searchsorted(idx, job)
            if pos < len(idx) and idx[pos] == job:
                acts, searched, opts, p, q, legal = eng.moves(int(pos))
                break
        else:
            raise KeyError(job)
        scen = self._scen[int(self._job_scen[job])]
        obs, mask = sts2.replay(scen, self._seeds[job], acts)
        steps = []
        for i, a in enumerate(acts):
            info = dict(acts=opts[i, :self.M].tolist(), p=p[i, :self.M].tolist(), q=q[i, :self.M].tolist(), legal=legal[i, :self.M].astype(bool).tolist()) if searched[i] else None
            steps.append(dict(obs=obs[i], a=int(a), info=info))
        steps.append(dict(obs=obs[len(acts)], a=None, info=None))
        return steps

    def gpu_ms(self):
        """With `profile_gpu`: {label: (total ms, replays, rows)} of the graph replays so far."""
        torch.cuda.synchronize()
        out = {}
        for k, evs in self._ev.items():
            out[k] = (sum(a.elapsed_time(b) for a, b, _ in evs), len(evs), sum(n for _, _, n in evs))
        return out
