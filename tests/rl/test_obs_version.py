"""Observation versions (`sts2.set_obs_version`, `crates/sts2sim/src/observe.rs`).

  python tests/rl/test_obs_version.py          (or pytest)

* v1 is bit-identical to the observation every network before v2 was trained on: `fixtures/obs_v1.npz` holds recorded fights (search-played,
  `target/exit/r4s_*.npz`) and a hash of every observation + mask row the v1 encoder produced for them, from the replay path (`sts2.replay`, what
  `VecEnv`, the search engine and `rl/exit.py` share) and from `Sim.observe`. Regenerate it only for an intended change of the simulator:
  `python tests/rl/test_obs_version.py --regen-v1 target/exit/r4s_000.npz ...` (with a build whose v1 output is the reference).
* v2 carries the visible information v1 left out (calculated card numbers, the selection screen's source, power display numbers, 64 candidates).
"""
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
    """(scenario, seed, actions) of every fight of recorded ExIt parts (`rl/exit.py` `_save`)."""
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
    """The fixture: `per_char` fights of each character from the first part, plus the `longest` longest fights of all parts (big piles, long
    selections), with the hashes of the current build's (default, v1) observations."""
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
    """Every v1 row of the fixture fights, through every producer: the default (process version 1), an explicit version 1, and `Sim.observe`."""
    fights, size = _fixture()
    assert sts2.obs_size(1) == size == sts2.OBS_SIZE
    for i, (sc, seed, ac, want) in enumerate(fights):
        assert _replay_hashes(sc, seed, ac) == list(want), f"fight {i}: default replay differs from v1"
        assert _replay_hashes(sc, seed, ac, obs_version=1) == list(want), f"fight {i}: replay(obs_version=1) differs from v1"
        if i % 4 == 0:
            assert _sim_hashes(sc, seed, ac) == list(want), f"fight {i}: Sim.observe differs from v1"
            assert _sim_hashes(sc, seed, ac, version=1) == list(want), f"fight {i}: Sim.observe(version=1) differs from v1"


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
