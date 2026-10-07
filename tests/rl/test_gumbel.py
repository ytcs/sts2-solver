"""The Gumbel root of the search (`rl/fastsearch.py` `root="gumbel"`, `sts2env::search` `RootMode::Gumbel`) driven by the real network on the CPU, the
format-2 records `rl/exit.py` trains on, and the scenario-file states of `tools/bench_search.py`.

  python tests/rl/test_gumbel.py          (or pytest)

Needs an `sts2` extension built from this tree (`root=` / `moves_gumbel`); the Gumbel tests are skipped with an older one. A few minutes on the CPU.
"""
import argparse
import json
import os
import sys
import tempfile
import time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
sys.path.insert(0, os.path.join(ROOT, "tools"))
os.environ.setdefault("STS2_DEVICE", "cpu")
import sts2  # noqa: E402
import torch  # noqa: E402

HAS_GUMBEL = hasattr(sts2._SearchEngine, "moves_gumbel")
CKPT = os.path.join(ROOT, "models", "solver_h128.pt")
_NET = []


def net():
    if not _NET:
        from model import load
        torch.set_num_threads(4)
        _NET.append(load(CKPT))
    return _NET[0]


def scenarios(n=3):
    return json.load(open(os.path.join(ROOT, "data", "train", "eval.json")))[:n]


def fs_(root, **kw):
    from fastsearch import FastSearch
    return FastSearch(net(), M=3, K=4, roots=kw.pop("roots", 4), groups=kw.pop("groups", 1), record=True, root=root, gumbel_m=8, gumbel_n=24,
                      threads=kw.pop("threads", 4), **kw)


def skip_without_gumbel():
    if not HAS_GUMBEL:
        print("  (skipped: the sts2 extension predates the Gumbel root; rebuild it from this tree)")
        return True
    return False


def test_topm_default_unchanged():
    from fastsearch import FastSearch
    assert FastSearch(net()).root == "topm"
    fs = fs_("topm")
    r = fs.run(scenarios(2), np.arange(2, dtype=np.uint32), np.array([5, 6], np.uint64))
    assert set(r[:, 1]) <= {1, -1, 2}
    assert fs.stats.get("g_cand", 0) == 0


def test_gumbel_runs_reproducibly():
    if skip_without_gumbel():
        return
    sc = scenarios(3)
    js, jd = np.arange(3, dtype=np.uint32), np.array([5, 6, 7], np.uint64)
    out = []
    for roots, groups, threads in ((4, 1, 4), (1, 1, 1), (4, 2, 2)):
        fs = fs_("gumbel", roots=roots, groups=groups, threads=threads)
        r = fs.run(sc, js, jd)
        assert fs.stats["searched"] > 0 and fs.stats["g_cand"] >= 2 * fs.stats["searched"]
        acts = [fs.job_actions(j) for j in range(3)]
        pis = []
        for idx, eng in fs._runs:
            for jl in range(len(idx)):
                pis.append((int(idx[jl]), eng.moves_gumbel(jl)[2]))
        out.append((r[:, [1, 3, 4]], acts, dict(pis)))
    # the engine is deterministic given its answers (crates/sts2env/tests/search.rs); here the network's answers can move in the last bits with the
    # batch they are computed in (padded shapes follow the batch), so the improved policy is compared to a tolerance and the play exactly
    for o in out[1:]:
        assert np.array_equal(o[0], out[0][0])
        assert o[1] == out[0][1]
        for j in range(3):
            assert o[2][j].shape == out[0][2][j].shape and np.abs(o[2][j] - out[0][2][j]).max() < 1e-4, np.abs(o[2][j] - out[0][2][j]).max()


def test_pi_is_the_softmax_of_logits_plus_adv():
    """The recorded pi' of each candidate = softmax(prior logits + adv) at that state, with adv = 0 for the actions not sampled: what `exit.py --target
    gumbel` rebuilds from the replayed observation and the init network."""
    if skip_without_gumbel():
        return
    sc = scenarios(1)[0]
    sim = sts2.Sim(json.dumps(sc), 11)
    while sim.stage() != "over" and len(sim.legal()) < 3:
        sim.step(sim.legal()[0][0])
    fs = fs_("gumbel")
    d = fs.decide(sc, sim, seed=3)
    assert d["searched"]
    obs, mask = np.zeros(sts2.OBS_SIZE, np.float32), np.zeros(sts2.ACTIONS, np.uint8)
    sim.copy().observe(obs, mask)
    with torch.no_grad():
        lg = net()(torch.from_numpy(obs)[None], torch.from_numpy(mask)[None], value=False)[0].float()[0]
    shift = torch.zeros_like(lg)
    nc = sum(d["legal"])
    for a, ad in zip(d["opts"][:nc], d["adv"][:nc]):
        shift[a] = ad
    pi = torch.softmax(lg + shift, 0)
    for a, p_rec, pi_rec in zip(d["opts"][:nc], d["p"][:nc], d["pi"][:nc]):
        assert abs(float(torch.softmax(lg, 0)[a]) - p_rec) < 1e-4
        assert abs(float(pi[a]) - pi_rec) < 1e-4, (a, float(pi[a]), pi_rec)
    assert sum(d["n"][:nc]) == 24 and d["action"] in d["opts"][:nc]


def test_exit_collects_and_loads_the_gumbel_target():
    if skip_without_gumbel():
        return
    import exit as X
    with tempfile.TemporaryDirectory() as tmp:
        fights = os.path.join(tmp, "f.json")
        json.dump(scenarios(2), open(fights, "w"))
        a = argparse.Namespace(fights=[fights], look_legacy=False, ckpt=CKPT, M=3, K=4, roots=4, attempts=1, out=os.path.join(tmp, "g.npz"), first_timeout=30.0,
                               chunk_timeout=50.0, chunk=2, chunks_per_process=5, max_minutes=30, seed=101, root="gumbel", gumbel_m=8, gumbel_n=24)
        X.collect(a)
        part = os.path.join(tmp, "g_000.npz")
        z = np.load(part)
        assert int(z["version"]) == 2 and z["d_adv"].shape == z["d_q"].shape == z["d_pi"].shape == (len(z["d_fight"]), 8)
        assert len(z["d_fight"]) > 0
        data = X.Data([part], tau=0.02, keep_mp=True)
        assert data.has_gumbel()
        rows = data.rows(data.index)
        assert len(rows) == 7 and rows[6].shape == rows[2].shape
        assert np.all((rows[2] >= 0) | (rows[6] == 0))  # no shift on padded / untried slots
        assert np.abs(rows[6]).sum() > 0
        z.close()


def test_bench_states_from_a_scenario_file():
    """`bench_search --from-scenarios`: search-played fights replayed with `sts2.replay` and an `sts2.Sim` stepped through the same actions."""
    import bench_search as B
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "s.json")
        json.dump([{"scenario": s} for s in scenarios(3)], open(path, "w"))
        st = B.collect_states_scen(path, 6, seed=1, max_cands=8, n_fights=2)
        assert 0 < len(st) <= 6
        for s in st:
            assert s["sim"].stage() == "play" and len(s["keys"]) >= 3 and len(s["cands"]) <= 8
            assert s["file"].startswith("s.json#")
        # deterministic given the seed
        st2 = B.collect_states_scen(path, 6, seed=1, max_cands=8, n_fights=2)
        assert [(s["file"], s["step"]) for s in st] == [(s["file"], s["step"]) for s in st2]
        assert [s["sim"].snapshot() for s in st] == [s["sim"].snapshot() for s in st2]


def test_bench_report_ranks_and_pairs(tmp_path=None):
    import bench_search as B
    ref = {"a": dict(v=0.5, se=0.01, win=1, hp=0.5, n=64), "b": dict(v=0.2, se=0.01, win=1, hp=0.2, n=64), "c": dict(v=-0.1, se=0.01, win=0, hp=0, n=64)}
    rec = dict(file="f#1", step=3, encounter="X_BOSS", kind="boss", cands=list(ref), ref=ref, potion=False, picks={"topm5x32": "b", "gumbel16x160": "a"},
               prior={"a": 0.05, "b": 0.6, "c": 0.35}, secs={"topm5x32": 0.1, "gumbel16x160": 0.2}, rows={"topm5x32": [10, 20], "gumbel16x160": [12, 25]})
    assert B.prior_rank(rec) == 3
    assert abs(B.regret(rec, "topm5x32") - 0.3) < 1e-9 and B.regret(rec, "gumbel16x160") == 0
    with tempfile.TemporaryDirectory() as tmp:
        p = os.path.join(tmp, "r.jsonl")
        open(p, "w").write(json.dumps(rec) + "\n" + json.dumps(dict(rec, file="f#2")) + "\n")
        B.report(p, ["topm5x32", "gumbel16x160"])


if __name__ == "__main__":
    t_all, failed = time.time(), []
    for name, fn in sorted((k, v) for k, v in dict(globals()).items() if k.startswith("test_") and callable(v)):
        t = time.time()
        try:
            fn()
            print(f"ok   {name} ({time.time() - t:.0f}s)", flush=True)
        except Exception as e:  # noqa: BLE001
            import traceback
            traceback.print_exc()
            failed.append(name)
            print(f"FAIL {name}: {e!r}", flush=True)
    print(f"{'FAILED ' + ', '.join(failed) if failed else 'all passed'} in {time.time() - t_all:.0f}s")
    sys.exit(1 if failed else 0)
