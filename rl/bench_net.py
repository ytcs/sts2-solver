#!/usr/bin/env python3
"""Speed of the network alone on realistic batches (GPU): CUDA-graph replay time per batch size for fp32 / bf16, 128- and 64-wide, policy and value heads,
and the kernels that dominate one forward (torch profiler).   STS2_DEVICE=cuda .venv/bin/python rl/bench_net.py [--profile] [--compile]"""
import argparse, json, os, sys, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from model import load, DEV

ap = argparse.ArgumentParser()
ap.add_argument("--profile", action="store_true")
ap.add_argument("--compile", action="store_true")
ap.add_argument("--E", type=int, default=8)
a = ap.parse_args()
torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True
M = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
nets = {"b128": load(os.path.join(M, "solver_b128.pt")), "a64": load(os.path.join(M, "solver_a64.pt"))}
scen = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data/train/eval.json")))
N = 16384
env = sts2.VecEnv(N, scen, seed=3, max_steps=300)
obs, mask = env.reset()
rng = np.random.default_rng(0)
for t in range(6):
    act = np.array([rng.choice(np.nonzero(m)[0]) if m.any() else 0 for m in mask], np.int32)
    obs, mask, *_ = env.step(act)
ok = obs[:, 1037] <= 0.5
obs, mask = obs[ok], mask[ok]
print("rows", len(obs), flush=True)


def timeit(fn, n=30):
    for _ in range(3):
        fn()
    torch.cuda.synchronize()
    t = time.perf_counter()
    for _ in range(n):
        fn()
    torch.cuda.synchronize()
    return (time.perf_counter() - t) / n * 1000


def graph(fn):
    s = torch.cuda.Stream()
    s.wait_stream(torch.cuda.current_stream())
    with torch.cuda.stream(s), torch.no_grad():
        for _ in range(3):
            fn()
    torch.cuda.current_stream().wait_stream(s)
    g = torch.cuda.CUDAGraph()
    with torch.no_grad(), torch.cuda.graph(g):
        fn()
    return g


for B in (1024, 2048, 4096, 8192):
    if B > len(obs):
        continue
    o = torch.from_numpy(obs[:B]).to(DEV)
    m = torch.from_numpy(mask[:B]).to(DEV)
    row = {}
    for name, net in nets.items():
        for amp in (False, True):
            def pol():
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    return net(o, m, value=False, E=a.E, L=64, has_dec=False)

            def val():
                with torch.autocast("cuda", dtype=torch.bfloat16, enabled=amp):
                    return net(o, None, policy=False, E=a.E, L=64, has_dec=False)
            tag = f"{name}{'/bf16' if amp else '/fp32'}"
            row[tag + " pol"] = timeit(graph(pol).replay)
            row[tag + " val"] = timeit(graph(val).replay)
    print(B, "  ".join(f"{k} {v:.2f}ms" for k, v in row.items()), flush=True)

if a.profile:
    from torch.profiler import profile, ProfilerActivity
    B = 4096
    o = torch.from_numpy(obs[:B]).to(DEV)
    m = torch.from_numpy(mask[:B]).to(DEV)
    net = nets["b128"]
    with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
        for _ in range(3):
            net(o, m, value=False, E=a.E, L=64, has_dec=False)
        torch.cuda.synchronize()
        with profile(activities=[ProfilerActivity.CUDA]) as prof:
            for _ in range(5):
                net(o, m, value=False, E=a.E, L=64, has_dec=False)
            torch.cuda.synchronize()
    print(prof.key_averages().table(sort_by="cuda_time_total", row_limit=18, max_name_column_width=60))

if a.compile:
    net = nets["b128"]
    for mode in ("default", "max-autotune-no-cudagraphs"):
        torch._dynamo.reset()
        cf = torch.compile(lambda o, m: net(o, m, value=False, E=a.E, L=64, has_dec=False), mode=mode, dynamic=True)
        cv = torch.compile(lambda o: net(o, None, policy=False, E=a.E, L=64, has_dec=False), mode=mode, dynamic=True)
        t = time.time()
        for B in (1024, 2048, 4096, 8192):
            o = torch.from_numpy(obs[:B]).to(DEV)
            m = torch.from_numpy(mask[:B]).to(DEV)
            with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
                cf(o, m)
                cv(o)
                g = graph(lambda: cf(o, m))
                gv = graph(lambda: cv(o))
                print(f"{mode} dynamic B={B}: pol {timeit(g.replay):.2f} ms  val {timeit(gv.replay):.2f} ms   (elapsed incl. compile {time.time() - t:.0f}s)", flush=True)
