# STS2 self-play harness
Claude plays Slay the Spire 2 (v0.111.0, A10) in the real game: Rust combat simulator + search for combat, run model and calculators for macro, an outer loop for skills and networks. Goal: win the run; tiebreak end HP. Plan: `docs/plan.md`.

```
game + mods/AgentBridge :15555  <-  agent daemon :15556 (python -m agent <cmd>)  <-  Claude
oracle/combat `serve --port N` (headless game, same protocol)  <-  STS2_BRIDGE=127.0.0.1:N (daemon N+1)
```

## Setup
```bash
cargo test --workspace
uv venv .venv && uv pip install maturin numpy torch --python .venv/Scripts/python.exe
VIRTUAL_ENV=$PWD/.venv .venv/Scripts/maturin develop --release -m crates/sts2py/Cargo.toml
export STS2_DEVICE=cuda
bash mods/AgentBridge/dev.sh       # .NET 9, Steam 2868840: build + install mod, relaunch (GUI=1 window)
```

## Play
`python -m agent s` (daemon starts on first use), `python -m agent a 1 ironclad 10 SEED` (seeded run from the menu). Rules + skill loading `CLAUDE.md`; commands `sts2-harness`; bridge `mods/AgentBridge/README.md`. Human terminal: daemon with `STS2_SKILL_GATE=off`. `agent/fight.py` rebuilds the fight from visible state (hidden info = the simulator's own sample).
Dashboard: `python tools/dashboard/extract_assets.py` (once per game update), then `python tools/dashboard/serve.py` -> http://localhost:8777.

## Outer loop
`python -m agent.improve review|lessons|gaps|corpus`; `python -m agent.hindsight <fight> --log`; `python -m agent.fidelity_sweep`. Protocol: `sts2-harness` "Review loop". Records `runs/<id>/events.jsonl`, `evals/`.

## Training and evaluation
- Expert iteration: `tools/collect.sh <rl/exit.py collect args>` (`--ckpt --fights --out [--cover --K --futures]`), `rl/exit.py train --init --data --out` (policy + TD(0.8) value by default). Pools: `tools/gen_curriculum.py`, `signal_pool.py`, `gen_train.py`, `round_pool.py`. After a simulator change: `tools/prune_divergent.py`.
- Pods: `scripts/pod_round.sh` (combat-loop round), `pod_train.sh` (PPO `rl/ppo.py`), `pod_job.sh` (uploaded `job.sh`), shared `pod_setup.sh`.
- Benchmarks: `tools/bench.py build|build-plans|relabel|score|play|screen`, `tools/nearmiss_bench.py build|eval`, `tools/bench_search.py`, `tools/exact_turn_check.py`.
- Live networks: `models/current.json`. Expert re-enactment: `tools/expert.py`, skill `expert-reenact`.

## Accuracy gate
`bash tools/gate.sh` passes unchanged for any simulator/search/env change: search + env checksums, oracle regression traces, RNG goldens, information contract, sync, look-ahead cache exactness. A deliberate behaviour change updates its checksum in the same commit.

## Layout
| path | what |
|---|---|
| `agent/` | harness, bridge client, fight rebuild, engine, run model (`runmodel`, `price`, `plans`, `tracker`), calculators, guards, skill gate, outer loop |
| `mods/AgentBridge/` | game mod (C#) |
| `.claude/skills/` | strategy book |
| `rl/` | network + outcome head, `fastsearch`, `solver`, `exit`, `ppo`, `predictor` |
| `crates/` | `sts2sim` simulator, `sts2env` batch env + search, `sts2py` bindings, `sts2diff` differential tester |
| `oracle/` | the game's combat code headless, regression traces, RNG goldens |
| `tools/` | gate, benches, pool generators, dashboard, expert pipeline |
| `scripts/` | pod scripts, `skill_gate.py` (hook), `porting/` (rerun after a game update: `gen_defs.py`, `gen_ids.py`, `gen_relics.py`, `flag_multiplayer_cards.py`) |
| `tests/` | `agent/test_expert.py`, `agent/test_dashboard.py`, harness tests (`python tests/agent/run.py`) |
| `docs/` | `plan.md`, `simulator.md`, `solver.md`, `game-bugs.md`, `research/` (`evidence.md`, `game_code.md`) |
| `data/`, `models/` | catalogs, pools, plans, bench/train sets, expert records; networks |
| `runs/`, `evals/` | run records; gaps, judgments |
