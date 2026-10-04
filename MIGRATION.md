# Moving the workspace to a new machine (GPU)

Everything needed to continue is in this repository except three things that must not be redistributed or are rebuilt locally:
the game install (`sts2.dll`), the decompiled game source (`decomp/`), and build outputs (`target/`, `.venv/`). The repository is **private**:
the simulator is a port of the game's logic.

## 1. Prerequisites
| need | why | notes |
|---|---|---|
| Linux x86-64 | the oracle's Godot stub is raw x86-64 machine code | the Rust crates and the training code run anywhere |
| Rust (stable, 1.94 used) | simulator, env, Python extension | `rustup` |
| Python 3.13, `uv` | training / search | `uv venv .venv && uv pip install maturin numpy` |
| PyTorch | training | CPU build here; on the GPU box install the CUDA build and run with `STS2_DEVICE=cuda` (see 4) |
| .NET 9+ SDK (10.0 used) + the game (Steam: Slay the Spire 2, v0.111.0 public beta) | the oracle (differential testing), only for fidelity work | not needed to train or to run the solver |
| `ilspycmd` | regenerate `decomp/` | only for porting new content, see `docs/design.md` |

## 2. Setup
```bash
git clone <this repo> && cd sts2-solver
cargo test --workspace                                   # 240+ tests, about a minute
uv venv .venv && uv pip install maturin numpy --python .venv/bin/python
uv pip install torch --python .venv/bin/python           # or the CUDA wheel from pytorch.org
(cd crates/sts2py && VIRTUAL_ENV=$PWD/../../.venv ../../.venv/bin/maturin develop --release)
tools/setup_data.sh                                      # restores target/train, target/runs, target/exit from data/
.venv/bin/python tools/py_smoke.py 4096                  # env smoke test
.venv/bin/python rl/solver.py --scenarios data/examples/phrog_dup.json --attempts 32   # the solver on the example fight
```
Optional, for differential testing against the real game: `cd oracle/combat && dotnet build -c Release` (see `oracle/combat/README.md`,
`GameDir` points at the game's `data_sts2_linuxbsd_x86_64`), then `python3 tools/regress.py --n 3` and `python3 tools/regress_cache.py`.

## 3. What is where
* `crates/` simulator (`sts2sim`), batched env (`sts2env`), Python bindings (`sts2py`), differential tester (`sts2diff`). `docs/` has the design,
  the verified engine semantics (`docs/spec/`), the observation / action contract (`docs/env-api.md`), the solver write-up (`docs/solver.md`).
* `rl/` the solver: `model.py` (network), `ppo.py`, `search.py` (play-out search), `solver.py` (the API to use), `mine.py` + `train_mined.py`
  (disagreement mining and fine-tuning), `whatif.py` / `potion_whatif.py` / `potion_search.py` / `trace.py` (analysis of one fight, the HTML page),
  `winnable.py`, `baselines.py`, `bench_search.py`.
* `models/solver_b128.pt` the best network (128-wide, PPO 314M steps); `solver_c128.pt`, `solver_d128.pt` (same, other seeds: value ensemble), `solver_a64.pt`
  (64-wide); `solver_base.pt` the first 64-wide network, `solver_base_full.pt` with the optimizer state (`ppo.py --resume`). `Solver()` loads b128 + the value
  heads of c128 / d128.
* `data/` the card / relic / encounter catalog, the scenario sets (`train.json` 6000 fights, `eval.json` 1500, `mid.json` 786 mid-difficulty fights,
  `iron0_*`), the example fights, the analysis outputs, the first mining archive. `tools/setup_data.sh` copies them where the scripts expect them
  (`target/train/`, ...). New sets: `tools/gen_train.py` (needs only `data/catalog.json`, not the game).

## 4. Moving to the GPU
`rl/model.py` reads `STS2_DEVICE` (default `cpu`); `ppo.py`, `search.py`, `solver.py`, `baselines.py`, `mine.py` move tensors to it and bring results
back as numpy, because the simulator (Rust, rayon) always runs on the CPU and produces numpy batches. **This path is untested** (the machine these
were written on has no GPU); the CPU path is tested. `train_mined.py` and `exit.py` are still CPU-only. Things to expect and tune:
* The network is small (460k parameters), so a GPU mostly pays off through **large batches**: raise `--envs` (PPO: 4096+), `Solver(batch=...)` and the
  search `roots` (thousands). Search is network-bound (about 85% of its time was inference on the CPU); the simulator part is about 10%.
* `Net.encode` slices to the occupied enemy slots / pile lengths of the batch (`.item()` syncs); fine on a GPU, but if it shows up in a profile pad
  to fixed sizes instead.
* torch threads: on the CPU box the best setting was about 8 of 12 threads; with a GPU keep 2-4 and give the rest to rayon (the env).
* The simulator speed itself (about 1M env-steps/s on 12 cores including observations) will not change; PPO collection was 3-4k samples/s here, mostly
  network time, so expect a large gain from the GPU.

## 5. State of the work (end of the GPU session; the Runpod pod has been terminated)
Results are in `docs/solver.md` (tables) and `README.md`. In short: PPO policy 63% win / 0.443 HP lost on the 1500-fight eval set (random 14%,
heuristic 36%); the same network with play-out search is +7 to +23 points depending on the set (paired seeds); the search is the strong solver and the
network alone is the weak part. Open items, in the order I would do them:
1. **Mining run** (`rl/mine.py`, mid-difficulty fights, `--confirm 32`): archive `data/analysis/mine2.npz` if it finished, else restart it. Then
   `rl/train_mined.py` (lr 1e-4, 15 epochs was the best of what was tried on 516 states; with thousands of states re-tune), compare greedy play on
   `data/train/mid.json` (`rl/baselines.py --policies ckpt:...`) and iterate (mine again with the improved network).
2. **PPO with potion-hold randomization**: `rl/ppo.py ... --hold-prob 0.3 --resume models/solver_base_full.pt --warm` (implemented, only smoke-tested).
3. **Search speed** for many deck variants: batch bigger; adaptive budget (spend futures where the top two options are close); a smaller distilled
   network for the play-outs. Timings on the old machine varied 2-5x between identical runs (thermal throttling), so compare work, not wall time.
4. Run-level use: `Solver().solve(variants, attempts=...)` returns win rate (+ standard error), HP lost, HP left per scenario; 64 attempts give
   about +-5 points of win rate, 600 give +-2.
5. Known gaps (`docs/design.md`): Defect / Necrobinder / Regent are in the training mix but the provable-unwinnable prover covers Ironclad / Silent only;
   "known top card" after put-on-top effects is not tracked in the observation; capacity limits (160 cards, 16 powers).

## 6. Reproducing the key numbers
```bash
.venv/bin/python rl/baselines.py --eval data/train/eval.json --policies random,heuristic,ckpt:models/solver_base.pt --envs 500 --per-env 3
.venv/bin/python rl/bench_search.py --ckpt models/solver_base.pt --eval data/train/mid.json --roots 200 --greedy --configs "5,8,0,0,0,0,0"
.venv/bin/python rl/whatif.py --ckpt models/solver_base.pt --scenario data/examples/phrog_dup.json --attempts 3000     # card removal, split timing
.venv/bin/python rl/trace.py --ckpt models/solver_base.pt --scenario data/examples/phrog_dup.json --out trace.html --roots 240   # best / worst line page
.venv/bin/python tools/check_bounds.py --n 1500 --episodes 20000 --ckpt models/solver_base.pt    # soundness of the unwinnable-fight prover
```
(The scripts read `target/train/...`; run `tools/setup_data.sh` first, or point `--eval` at `data/train/...` as above.)

## 7. Runpod notes (what worked, what did not)
Budget agreed with the user: **$50 lifetime** on Runpod. Everything below is driven from the local machine with the Runpod plugin (`claude plugin install runpod@runpod`,
MCP sign-in via OAuth) and plain `ssh` (a dedicated key, `~/.ssh/runpod_sts2`, registered on the account; `~/.ssh/config` host `sts2pod`).
* Use the **secure cloud RTX 4090 ($0.74/h)**, image `runpod/pytorch:1.0.2-cu1281-torch280-ubuntu2404` (torch 2.8 + CUDA 12.8; the older cu124 image does not
  support Blackwell cards). The community-cloud 4090 ($0.34/h) landed on a host whose GPU failed `cuInit` (error 999) twice and one host with no TCP port mapping:
  test `python -c "import torch; print(torch.cuda.is_available())"` right after boot and terminate on failure (a failed attempt costs cents).
* Direct SSH needs `ports: ["22/tcp"]` and `startSsh: true`; the pod only exposes the port after about a minute (`get-pod` -> `ssh.direct`). The SSH proxy
  (`ssh.runpod.io`) hung for non-interactive use, scp/rsync need the direct port. The container disk is wiped when a pod is terminated: copy results back first.
* Setup on a fresh pod takes about 3 minutes: Rust via rustup, `git clone` (the repo is public), `uv venv --system-site-packages` (reuses the image's CUDA torch),
  `maturin develop --release`, `tools/setup_data.sh`. The script used is `tools/runpod_setup.sh`.
* Measured on the pod (16 vCPUs): PPO about 50k samples/s per run (two runs side by side: about 100k/s; the laptop did 3-4k), search about 35 fights/s on a short
  fight; the miner (`rl/mine.py`) is CPU-bound (single-threaded Python around the Rust env): run several in parallel (`tools/runpod_loop.sh` does).
* `tools/runpod_loop.sh ROUNDS ROOTS CHUNKS`: unattended DAgger-style loop (mine disagreements, fine-tune from the base on everything mined, evaluate, repeat,
  search benchmark at the end). Logs in `target/exit/loop.log`.
