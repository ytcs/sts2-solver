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


class FastSearch:
    def __init__(self, net, value_nets=None, M=3, K=8, conf=1.01, pmin=0.0, margin=0.0, roll_cap=60, max_steps=300, hp_bonus=0.5, greedy_roll=False,
                 roots=512, groups=2, threads=None, roll_net=None):
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

    @torch.no_grad()
    def _evaluate(self, g, n_pol, n_val):
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
                                     self.roll_cap, self.max_steps, 1.0, -1.0, self.hp_bonus, self.threads)
            pc, vc = eng.max_rows()
            pin = self.cuda
            G = dict(eng=eng, idx=idx, n_pol=0, n_val=0)
            for name, shape, dt in (("pol_obs", (pc, OBS), torch.float32), ("pol_mask", (pc, ACT), torch.uint8), ("pol_kind", (pc,), torch.uint8), ("val_obs", (vc, OBS), torch.float32),
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
            G["n_pol"], G["n_val"] = G["eng"].advance(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["val_obs"])
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
                G["n_pol"], G["n_val"] = G["eng"].advance(G["pol_obs"], G["pol_mask"], G["pol_kind"], G["val_obs"], G["pol_out"][:npol], G["val_out"][:nval])
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
