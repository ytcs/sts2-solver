# STS2 self-play harness
Claude plays Slay the Spire 2 (v0.111.0, Ascension 10) end to end in the real game: Rust combat simulator + search solver for combat, calculators and a run model for macro decisions, an outer loop for the strategy book and the networks. Goal: win the run; tiebreak end HP. Plan: `docs/rebuild.md`.

```
game + mods/AgentBridge :15555  <-  agent daemon :15556 (python -m agent <cmd>)  <-  Claude
                                     fight.py rebuild fight from observations | engine.py search + predictor
                                     harness.py commands, guards, run record  | improve.py outer loop
```

## Setup
```bash
cargo test --workspace
uv venv .venv && uv pip install maturin numpy torch --python .venv/Scripts/python.exe
VIRTUAL_ENV=$PWD/.venv .venv/Scripts/maturin develop --release -m crates/sts2py/Cargo.toml
export STS2_DEVICE=cuda
bash mods/AgentBridge/dev.sh       # .NET 9, Steam 2868840: build + install mod, relaunch headless (GUI=1 window), wait for bridge
```

## Play
```bash
python -m agent s                      # state; daemon starts on first use (~1 min)
python -m agent a 1 ironclad 10 SEED   # main menu: custom seeded run; then `a <i> [target] [-- why]`
python -m agent status
```
Rules + skill loading: `CLAUDE.md`. Commands: `.claude/skills/sts2-harness/SKILL.md`, `agent/harness.py`. Bridge: `mods/AgentBridge/README.md`. Gate off for a human terminal: start the daemon with `STS2_SKILL_GATE=off`.
Micro: `agent/fight.py` rebuilds the fight from visible state (hidden info = the simulator's own sample, never read from the game); the search picks the action. Macro: skills + `eval`/`reward`/`routes`/`rmcalc`/`pickplan`/`price`/`plans`.

Dashboard (read-only, follows `runs/CURRENT`): `python tools/dashboard/extract_assets.py` (once per game update, needs Pillow), then `python tools/dashboard/serve.py` -> http://localhost:8777.

## Outer loop
`python -m agent.improve review|lessons|gaps|corpus|finetune|gate CKPT|adopt CKPT --as NAME|ledger`; `python -m agent.hindsight <fight> --log`; `python -m agent.fidelity_sweep`. Protocol: `sts2-harness` "Review loop". Records: `runs/<id>/events.jsonl`, `evals/ledger.jsonl`.

## Training and evaluation
- Expert iteration: `tools/collect.sh <rl/exit.py collect args>` (`--ckpt --fights --out [--M --K | --cover --K --futures]`), `rl/exit.py train --init --data --out [--value-target td --lam 0.8]`. Pools: `tools/gen_curriculum.py`, `tools/signal_pool.py`, `tools/gen_train.py`, `tools/round_pool.py` (a combat-loop round: signal + plan decks + enabler pairs + big late decks -> `target/round/pool.json`). After a simulator change: `tools/prune_divergent.py`.
- Combat-loop round on a pod: `scripts/pod_round.sh` (builds, waits for `/root/inputs/READY`, collects with cover search, trains TD(0.8); env vars at its top).
- PPO: `rl/ppo.py`. GPU pod: `scripts/pod_train.sh` (local GPU by default).
- Benchmarks: `tools/bench.py build|build-plans|score|play|screen`, `tools/nearmiss_bench.py build|eval|turns` (near-miss flips; optimality bracket), `tools/bench_search.py`.
- Current networks: `models/current.json`.

## Accuracy gate
`bash tools/gate.sh` must pass unchanged for any simulator/search/env change: search + env checksums (obs v1, v2), oracle regression traces, RNG goldens, information contract, sync, look-ahead cache exactness, obs v1 identity. A deliberate behaviour change updates its checksum in the same commit. Harness tests: `python tests/agent/run.py`.

## Layout
| path | what |
|---|---|
| `agent/` | harness, bridge client, fight rebuild, engine, calculators, run model (`runmodel`, `price`, `plans`, `tracker`), guards, skill gate, outer loop |
| `mods/AgentBridge/` | game mod (C#) |
| `.claude/skills/` | strategy book |
| `rl/` | network, search driver (`fastsearch`), solver, `exit` (ExIt), `ppo`, `predictor` |
| `crates/` | `sts2sim` simulator, `sts2env` batch env + search, `sts2py` bindings, `sts2diff` differential tester |
| `oracle/` | the game's combat code headless (`combat/`), regression traces, RNG goldens |
| `tools/` | benches, generators, gate, dashboard |
| `scripts/` | `pod_train.sh`, `skill_gate.py` (hook), `porting/` (id/def generators, rerun after a game update) |
| `docs/` | `rebuild.md` plan, `simulator.md`, `solver.md`, `game-bugs.md`, `research/` (`evidence.md`, `game_code.md`) |
| `data/`, `models/` | catalogs, pools, plans, bench/train sets; networks |
| `runs/`, `evals/` | run records; gaps, judgments, ledger |
