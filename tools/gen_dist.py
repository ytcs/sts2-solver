#!/usr/bin/env python3
"""Training data for the end-HP distribution head (`rl/dist.py`): fights played by the solver's own search (FastSearch, the networks it uses live), and for
every state the value network is asked about (the first state of each player turn, plus the fight start) the fight's final result: lost, or the HP fraction
left on a win (after end-of-combat heals, as the search's returns count it).

  tools/gen_dist.py --scenarios data/train/mid.json,target/dist/gen.json --attempts 2 --hp-low 0.25 --out target/dist/data_a.npz

Start HP is spread (`--hp-low`..1 of max HP, half of the jobs) so low-HP states, where the shape of the distribution matters most, are well covered.
Output: obs [N, OBS] float16, label [N] int8 (0 = loss, 1..20 = win with HP fraction in bin b-1 of 20 equal bins), plus scenario / job ids.
"""
import argparse, json, os, sys, time
import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "rl"))
import sts2  # noqa: E402

NB = 20  # HP-fraction bins of a win; class 0 = loss


def label_of(outcome, hp_frac):
    if outcome != 1:
        return 0
    return 1 + min(NB - 1, int(hp_frac * NB))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scenarios", required=True, help="comma-separated JSON lists of scenarios")
    ap.add_argument("--attempts", type=int, default=2)
    ap.add_argument("--hp-low", type=float, default=0.25)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    rng = np.random.default_rng(a.seed)
    scen = []
    for p in a.scenarios.split(","):
        scen += json.load(open(p))
    if a.limit:
        scen = scen[:a.limit]
    # one job per (scenario, attempt); half the jobs start at a random HP fraction in [hp_low, 1]
    jobs = []
    for i, s in enumerate(scen):
        for k in range(a.attempts):
            s2 = dict(s)
            if k % 2 == 1:
                s2["hp"] = max(1, int(round(s["max_hp"] * rng.uniform(a.hp_low, 1.0))))
            jobs.append(s2)
    from solver import Solver
    S = Solver()
    fs = S.fs
    fs.record = True
    js = np.arange(len(jobs), dtype=np.uint32)
    jd = np.uint64(a.seed) * np.uint64(1_000_003) + np.arange(len(jobs), dtype=np.uint64)
    t0 = time.time()
    rows = fs.run(jobs, js, jd)
    print(f"{len(jobs)} fights in {time.time() - t0:.0f}s", flush=True)
    obs_l, lab_l, sid_l = [], [], []
    by_job = {}
    for gi_idx, eng in fs._runs:
        for local, j in enumerate(gi_idx):
            by_job[int(j)] = (eng, local)
    bad = 0
    for j in range(len(jobs)):
        outcome, hp_end = int(rows[j, 1]), float(rows[j, 3])
        if outcome not in (1, -1):  # aborted / truncated: no label
            continue
        eng, local = by_job[j]
        acts = eng.moves(local)[0].astype(np.int32)
        try:
            o, _m = sts2.replay(jobs[j], int(jd[j]), acts)
        except Exception as e:  # noqa: BLE001
            if not bad:
                print("first replay failure:", repr(e)[:300], flush=True)
            bad += 1
            continue
        keep = [0] + [i + 1 for i, x in enumerate(acts) if x == 0 and i + 1 < len(o) - 1]  # fight start + the state after every end turn (a new player turn)
        obs_l.append(o[keep].astype(np.float16))
        lab_l.append(np.full(len(keep), label_of(outcome, hp_end), np.int8))
        sid_l.append(np.full(len(keep), j, np.int32))
    if not obs_l:
        raise SystemExit(f"no labelled fight ({bad} replays failed; outcomes {np.unique(rows[:, 1], return_counts=True)})")
    obs = np.concatenate(obs_l)
    lab = np.concatenate(lab_l)
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    np.savez_compressed(a.out, obs=obs, label=lab, job=np.concatenate(sid_l))
    print(f"{len(obs)} states from {len(obs_l)} fights ({bad} replays failed) -> {a.out}; loss share {np.mean(lab == 0):.3f}", flush=True)


if __name__ == "__main__":
    main()
