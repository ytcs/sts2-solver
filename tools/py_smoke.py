"""Smoke test + throughput for the Python VecEnv (masked random policy)."""
import json, subprocess, sys, time
import numpy as np
import sts2

scen = json.loads(subprocess.check_output([sys.executable, "tools/mk_scenario.py", "--encounter", "NIBBITS_WEAK", "--starter", "--deck", "ANGER,SHRUG_IT_OFF:2,POMMEL_STRIKE"]))
n = int(sys.argv[1]) if len(sys.argv) > 1 else 4096
env = sts2.VecEnv(n, [scen], seed=3)
obs, mask = env.reset()
print("obs", obs.shape, "mask", mask.shape, "legal/env", mask.sum(1).mean())
rng = np.random.default_rng(0)
t = time.time(); steps = 100; eps = wins = 0
for _ in range(steps):
    p = mask.astype(np.float64); p /= p.sum(1, keepdims=True)
    cum = p.cumsum(1); a = (cum < rng.random((n, 1))).sum(1).astype(np.int32)
    obs, mask, r, d, info = env.step(a)
    assert info["illegal"].sum() == 0
    eps += int(d.sum()); wins += int((info["outcome"] == 1).sum())
dt = time.time() - t
print(f"{n*steps/dt/1e6:.2f}M env-steps/s incl. numpy policy; episodes {eps}, win rate {wins/max(eps,1):.2f}")
