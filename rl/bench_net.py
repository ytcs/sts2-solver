#!/usr/bin/env python3
"""Speed of the network alone on realistic batches (GPU): eager fp32/TF32, bf16 autocast, CUDA graph; policy and value heads, several batch sizes."""
import json, os, sys, time
import numpy as np
import torch
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import load
from fastsearch import host_shapes
from solver import DEFAULT_CKPT
from model import DEV

torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True
net = load(DEFAULT_CKPT)
scen = json.load(open("data/train/eval.json"))
N = 8192
env = sts2.VecEnv(N, scen, seed=3, max_steps=300, round_robin=False)
obs, mask = env.reset()
rng = np.random.default_rng(0)
for t in range(6):
    a = np.array([rng.choice(np.nonzero(m)[0]) if m.any() else 0 for m in mask], np.int32)
    obs, mask, *_ = env.step(a)
nodec = obs[:, 1037] <= 0.5
obs, mask = obs[nodec], mask[nodec]
print("rows", len(obs), "dec-free")
E, L, dec = host_shapes(obs)
print("E", E, "L", L)


def timeit(fn, n=20):
    for _ in range(3):
        fn()
    torch.cuda.synchronize()
    t = time.perf_counter()
    for _ in range(n):
        fn()
    torch.cuda.synchronize()
    return (time.perf_counter() - t) / n * 1000


for B in (512, 2048, 4096, 8192):
    if B > len(obs):
        continue
    o = torch.from_numpy(obs[:B]).to(DEV)
    m = torch.from_numpy(mask[:B]).to(DEV)
    kw = dict(E=E, L=L, has_dec=False)
    with torch.no_grad():
        pol = lambda: net(o, m, value=False, **kw)
        val = lambda: net(o, None, policy=False, **kw)
        r = {"pol eager": timeit(pol), "val eager": timeit(val)}

        def pol_bf16():
            with torch.autocast("cuda", dtype=torch.bfloat16):
                return net(o, m, value=False, **kw)
        r["pol bf16"] = timeit(pol_bf16)
        # CUDA graph
        try:
            s = torch.cuda.Stream()
            s.wait_stream(torch.cuda.current_stream())
            with torch.cuda.stream(s):
                for _ in range(3):
                    pol()
            torch.cuda.current_stream().wait_stream(s)
            g = torch.cuda.CUDAGraph()
            with torch.cuda.graph(g):
                out = pol()
            r["pol graph"] = timeit(lambda: g.replay())
        except Exception as e:
            r["pol graph"] = f"fail {type(e).__name__}: {str(e)[:80]}"
    print(B, {k: (round(v, 2) if not isinstance(v, str) else v) for k, v in r.items()}, "ms;  eager rows/s %.0fk" % (B / r["pol eager"]), flush=True)
