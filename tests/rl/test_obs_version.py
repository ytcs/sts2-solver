import argparse
import hashlib
import json
import os
import sys
import time

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
os.environ.setdefault("STS2_DEVICE", "cpu")
import sts2  # noqa: E402

FIX = os.path.join(ROOT, "tests", "rl", "fixtures", "obs_v1.npz")


def row_hash(o, m):
    return np.frombuffer(hashlib.blake2b(o.tobytes() + m.tobytes(), digest_size=8).digest(), np.uint64)[0]


def _fights(parts):
    for p in parts:
        z = np.load(p)
        scen = json.loads(str(z["scenarios"]))
        fs, seeds, off, acts = z["f_scen"], z["f_seed"], z["f_off"], z["acts"]
        for i in range(len(fs)):
            yield scen[fs[i]], int(seeds[i]), acts[off[i]:off[i + 1]].astype(np.int32)


def _replay_hashes(scen, seed, acts, **kw):
    o, m = sts2.replay(scen, seed, acts, **kw)
    return [row_hash(o[t], m[t]) for t in range(len(o))]


def _sim_hashes(scen, seed, acts, **kw):
    s = sts2.Sim(json.dumps(scen), seed)
    n = kw.pop("size", sts2.OBS_SIZE)
    ob, mk = np.zeros(n, np.float32), np.zeros(sts2.ACTIONS, np.uint8)
    out = []
    for t in range(len(acts) + 1):
        s.observe(ob, mk, **kw)
        out.append(row_hash(ob, mk))
        if t < len(acts):
            s.step(int(acts[t]))
    return out


def regen_v1(parts, per_char=50, longest=50):
    all_f = list(_fights(parts))
    first = list(_fights(parts[:1]))
    pick, seen, per = [], set(), {}
    for k, (sc, seed, ac) in enumerate(first):
        if per.get(sc["character"], 0) < per_char:
            per[sc["character"]] = per.get(sc["character"], 0) + 1
            pick.append((sc, seed, ac))
            seen.add((json.dumps(sc, sort_keys=True), seed))
    for sc, seed, ac in sorted(all_f, key=lambda f: -len(f[2]))[:longest]:
        if (json.dumps(sc, sort_keys=True), seed) not in seen:
            pick.append((sc, seed, ac))
    scen_list, scen_idx, f_scen, f_seed, acts, off, rh, roff, sh = [], {}, [], [], [], [0], [], [0], []
    for sc, seed, ac in pick:
        key = json.dumps(sc, sort_keys=True)
        if key not in scen_idx:
            scen_idx[key] = len(scen_list)
            scen_list.append(sc)
        f_scen.append(scen_idx[key])
        f_seed.append(seed)
        acts.extend(ac.tolist())
        off.append(len(acts))
        rh.extend(_replay_hashes(sc, seed, ac))
        sh.extend(_sim_hashes(sc, seed, ac))
        roff.append(len(rh))
    assert rh == sh, "replay and Sim.observe disagree"
    np.savez_compressed(FIX, scenarios=np.array(json.dumps(scen_list)), f_scen=np.array(f_scen, np.int32), f_seed=np.array(f_seed, np.uint64),
                        acts=np.array(acts, np.int16), f_off=np.array(off, np.int64), hashes=np.array(rh, np.uint64), h_off=np.array(roff, np.int64),
                        obs_size=np.array(sts2.OBS_SIZE))
    print(f"{FIX}: {len(pick)} fights, {len(rh)} rows")


def _fixture():
    z = np.load(FIX)
    scen = json.loads(str(z["scenarios"]))
    fs, seeds, off, acts, hs, hoff = z["f_scen"], z["f_seed"], z["f_off"], z["acts"], z["hashes"], z["h_off"]
    return [(scen[fs[i]], int(seeds[i]), acts[off[i]:off[i + 1]].astype(np.int32), hs[hoff[i]:hoff[i + 1]]) for i in range(len(fs))], int(z["obs_size"])


def test_v1_is_bit_identical():
    fights, size = _fixture()
    assert sts2.obs_size(1) == size == sts2.OBS_SIZE
    for i, (sc, seed, ac, want) in enumerate(fights):
        assert _replay_hashes(sc, seed, ac) == list(want), f"fight {i}: default replay differs from v1"
        assert _replay_hashes(sc, seed, ac, obs_version=1) == list(want), f"fight {i}: replay(obs_version=1) differs from v1"
        if i % 4 == 0:
            assert _sim_hashes(sc, seed, ac) == list(want), f"fight {i}: Sim.observe differs from v1"
            assert _sim_hashes(sc, seed, ac, version=1) == list(want), f"fight {i}: Sim.observe(version=1) differs from v1"


def _sec(v):
    return {n: (o, s) for n, o, s in sts2.layout(v)["sections"]}, sts2.layout(v)["consts"]


def test_every_producer_writes_v2():
    assert sts2.obs_version() == 1
    assert sts2.obs_size(2) == sts2.layout(2)["consts"]["OBS_SIZE"] > sts2.obs_size(1)
    fights, _ = _fixture()
    for sc, seed, ac, _h in fights[:12]:
        o, m = sts2.replay(sc, seed, ac, obs_version=2)
        assert o.shape == (len(ac) + 1, sts2.obs_size(2))
        assert _sim_hashes(sc, seed, ac, version=2, size=sts2.obs_size(2)) == [row_hash(o[t], m[t]) for t in range(len(o))]
        steps = np.array([0, len(ac) // 2, len(ac)], np.uint32)
        ro, rm = sts2.replay_rows([sc], [0], [seed], ac, [0, len(ac)], steps, [0, 3], obs_version=2)
        assert np.array_equal(ro, o[steps]) and np.array_equal(rm, m[steps])
    env = sts2.VecEnv(4, [fights[0][0]], seed=1, obs_version=2)
    obs, mask = env.reset()
    assert env.obs_version == 2 and obs.shape == (4, sts2.obs_size(2))
    assert sts2.VecEnv(2, [fights[0][0]], seed=1).obs.shape == (2, sts2.obs_size(1))
    obs, mask, *_ = env.step(np.flatnonzero(mask[0])[:1].repeat(4).astype(np.int32))
    assert obs.shape == (4, sts2.obs_size(2))


def test_v2_shows_what_v1_hid_on_recorded_fights():
    fights, _ = _fixture()
    s1, c1 = _sec(1)
    s2, c2 = _sec(2)
    CF1, CF2 = c1["CARD_F"], c2["CARD_F"]
    calc = {i for i, n in enumerate(sts2.names()["card"]) if n in ("BODY_SLAM", "PERFECTED_STRIKE", "GOLD_AXE", "REND", "UNLEASH", "MURDER", "BULLY",
                                                                   "STACK", "EXPECT_A_FIGHT", "MIRAGE", "NORMALITY", "ASHEN_STRIKE", "DEATH_MARCH")}
    n_dec = n_src = n_played = n_wide = n_calc = n_calc_v1 = 0
    for sc, seed, ac, _h in fights:
        o1, _ = sts2.replay(sc, seed, ac, obs_version=1)
        o2, _ = sts2.replay(sc, seed, ac, obs_version=2)
        dec = o2[:, s2["decision"][0]] > 0.5
        n_dec += int(dec.sum())
        n_src += int((o2[dec, s2["dec_source"][0]] > 0).sum())
        n_played += int((o2[dec, s2["played"][0]] > 0).sum())
        n_wide += int((o2[dec, s2["decision"][0] + 8 + 16 * (CF2 + 1)] > 0).sum())
        for k in range(c1["MAX_HAND"]):
            cid = o2[:, s2["hand"][0] + k * CF2] - 1
            sel = np.isin(cid, list(calc))
            v2 = o2[sel, s2["hand"][0] + k * CF2 + 6: s2["hand"][0] + k * CF2 + 8].sum(1) + o2[sel, s2["hand"][0] + k * CF2 + 12]
            v2 = v2 + o2[sel, s2["osty"][0] + 4 + 3 * c2["OBS_POWERS"] + k]
            v1 = o1[sel, s1["hand"][0] + k * CF1 + 6: s1["hand"][0] + k * CF1 + 8].sum(1)
            n_calc += int((v2 != 0).sum())
            n_calc_v1 += int((v1 != 0).sum())
    print(f"  selections {n_dec}: source {n_src}, card in play {n_played}, > 16 candidates {n_wide}; calculated numbers shown v2 {n_calc} vs v1 {n_calc_v1}")
    assert n_dec > 100 and n_src == n_dec and n_played > 0 and n_wide > 0
    assert n_calc_v1 == 0 and n_calc > 200


def test_v2_hides_what_a_player_cannot_see_on_recorded_fights():
    fights, _ = _fixture()
    n2 = sts2.obs_size(2)
    ob, ob2 = np.zeros(n2, np.float32), np.zeros(n2, np.float32)
    mk, mk2 = np.zeros(sts2.ACTIONS, np.uint8), np.zeros(sts2.ACTIONS, np.uint8)
    checked = 0
    for i, (sc, seed, ac, _h) in enumerate(fights):
        s = sts2.Sim(json.dumps(sc), seed)
        for t in range(len(ac) + 1):
            s.observe(ob, mk, version=2)
            d = s.copy()
            if d.determinize(1000 * i + t):
                d.observe(ob2, mk2, version=2)
                assert np.array_equal(ob, ob2) and np.array_equal(mk, mk2), f"fight {i} step {t}: hidden state changed the v2 observation"
                checked += 1
            if t < len(ac):
                s.step(int(ac[t]))
    print(f"  {checked} states determinized, v2 observation unchanged")
    assert checked > 10000


def test_network_reads_its_checkpoints_version():
    import tempfile
    import torch
    import model as M
    net = M.Net(d=32, heads=True, pot=True, obs_version=2)
    with tempfile.TemporaryDirectory() as tmp:
        p = os.path.join(tmp, "v2.pt")
        torch.save({"net": net.state_dict(), "args": {"d": 32, "heads": True, "pot_head": True, "obs_version": 2}}, p)
        n2 = M.load(p, set_version=False)
        assert n2.obs_version == 2 and sts2.obs_version() == 1
        fights, _ = _fixture()
        sc, seed, ac, _h = fights[1]
        o2, m2 = sts2.replay(sc, seed, ac, obs_version=2)
        o1, m1 = sts2.replay(sc, seed, ac, obs_version=1)
        with torch.no_grad():
            a = n2(torch.from_numpy(o2), torch.from_numpy(m2.astype(np.int64)))
            b = net.eval()(torch.from_numpy(o2), torch.from_numpy(m2.astype(np.int64)))
        assert torch.equal(a[0], b[0])
        for nn_, o, m in ((n2, o1, m1), (M.Net(d=32), o2, m2)):
            try:
                nn_(torch.from_numpy(o), torch.from_numpy(m.astype(np.int64)))
                raise AssertionError("a row of the other observation version was accepted")
            except ValueError as e:
                assert "observation version" in str(e)
        old = set(M._CLAIMED)
        try:
            M._CLAIMED.clear()
            M._CLAIMED.add(1)
            try:
                M.load(p)
                raise AssertionError("load set a second process-wide version")
            except RuntimeError:
                pass
        finally:
            M._CLAIMED.clear()
            M._CLAIMED.update(old)
            sts2.set_obs_version(1)


def test_search_runs_a_v2_network():
    import torch
    import model as M
    from fastsearch import FastSearch
    torch.manual_seed(0)
    net = M.Net(d=32, heads=True, pot=True, obs_version=2).eval()
    fights, _ = _fixture()
    sc = [f[0] for f in fights[:2]]
    fs = FastSearch(net, M=2, K=2, roots=2, groups=1, threads=2, record=True, max_steps=60)
    assert fs.OBS == sts2.obs_size(2)
    r = fs.run(sc, np.arange(2, dtype=np.uint32), np.array([3, 4], np.uint64))
    assert fs.stats["searched"] > 0 and r.shape[0] == 2
    acts = fs._runs[0][1].moves(0)[0]
    o, _ = sts2.replay(sc[0], 3, acts, obs_version=2)
    assert o.shape[1] == sts2.obs_size(2)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--regen-v1", nargs="+", metavar="PART", help="rebuild fixtures/obs_v1.npz from these recorded parts with this build")
    a = ap.parse_args()
    if a.regen_v1:
        regen_v1(a.regen_v1)
        sys.exit(0)
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
