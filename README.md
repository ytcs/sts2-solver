# STS2 self-play harness

A harness that lets an agent (Claude) play **Slay the Spire 2** end to end in the real game, with a combat solver for the turn-by-turn decisions, tools to price the macro decisions (cards, relics, shops, rests, routes), and an outer loop that turns experience into a better strategy book and a better solver.

**Target:** Ascension 10 (A8 tough-enemy HP, A9 deadly-enemy damage, 2 potion slots, Ascender's Bane, a second boss; the Ironclad run starts at 64/80 HP). Game build: v0.111.0 public beta. Winning the run is the goal, finishing with the most HP the second.

```
 game (Godot + C#)                          python -m agent <cmd>
 └─ mods/AgentBridge  :15555  ◄────────────  agent daemon :15556  ◄──── Claude (shell)
    state, actions, visible fight export       ├─ fight.py    rebuild the fight in the simulator from observations only
                                               ├─ engine.py   network + determinized search (micro), batch solver (macro / predictions)
                                               ├─ macro.py    price a choice: variants of the current deck vs encounters
                                               ├─ harness.py  commands, run record      └─ improve.py  outer loop
```

## Quick start
```bash
# one-time: Rust + Python environment
cargo test --workspace
uv venv .venv && uv pip install maturin numpy torch --python .venv/Scripts/python.exe
VIRTUAL_ENV=$PWD/.venv .venv/Scripts/maturin develop --release -m crates/sts2py/Cargo.toml
export STS2_DEVICE=cuda                 # GPU for the networks (the first run compiles for about 2 minutes)

# the game with the bridge mod (.NET 9; Steam game id 2868840): kills the game, builds, installs, relaunches, waits for the bridge
bash mods/AgentBridge/dev.sh

python -m agent s                       # state with numbered options; the daemon starts hidden on first use (loading takes about a minute)
python -m agent a 1 ironclad 10 SEED    # main menu: custom run with a seed; afterwards `a <i> [target] [-- why]` for everything
python -m agent status                  # replay fidelity of the current fight, engine, run id
```
Commands, batching, the solver commands (`adv`, `turn`, `combat`, `eval`, `route`, ...) and what `REFUSED` means: `.claude/skills/sts2-harness/SKILL.md`. Rules for playing and the order in which skills load: `CLAUDE.md`. Command implementations: `agent/harness.py`. Bridge protocol: `mods/AgentBridge/README.md`.

`adv` and `turn` refuse loudly when the simulator is out of sync with the game. A map click onto an elite or boss below 60% HP needs `!`.

## How a decision is made
* **Micro (every combat action).** The bridge exports the fight start (deck, relics, potions, HP, encounter) and the visible state after every action. `agent/fight.py` rebuilds the fight in the simulator: it replays the actions taken, resamples enemy turns until the intents match what the game shows, and aligns the visible state (hand, piles as multisets, HP, block, energy). Hidden information (draw order, RNG, random enemy branches) is the simulator's own random sample, never read from the game. The search tries the likeliest actions on determinized futures (about 10 ms per round; rounds repeat until the time budget or a clear winner) and the action is sent to the game.
* **Macro (everything else).** The agent decides with the strategy book (`.claude/skills/sts2-*`, rooted at the `sts2` skill) and `eval`, which plays variants of the deck against the encounters ahead and reports win rate and HP lost with their margins. The act's encounter pools (weak / regular / elite / boss) are in `agent/pools.py`.
* **No cheating:** no dev console or god mode in a scored run, no hidden state (draw order, RNG streams, the pre-rolled encounter and elite order), no restarts. The run seed is not exported.

## The outer loop (`agent/improve.py`)
Every decision and fight is recorded in `runs/<id>/events.jsonl` (the solver's prediction at each fight start, every action with the options weighed, outcomes, the reasons for macro choices). `python -m agent.improve` has `review` (predicted vs actual, search overrides, simulator fidelity), `lessons` (the book's open hypotheses), `corpus` (fights of all runs into `data/corpus`, split by run), `finetune`, `gate CKPT` (candidate vs current on the corpus holdout and the fixed eval sets, appended to `evals/ledger.jsonl`), `adopt CKPT --as NAME` (only after the gate passed), `ledger`. Protocol: the "Review loop" in `sts2-harness`. A strategy claim moves from `[hyp]` to `[sim]` only with a measurement behind it; a checkpoint is adopted only through the gate.

## Layout
| Path | What |
|---|---|
| `agent/` | The harness: bridge client, fight rebuild, engine, macro evaluation, run record, outer loop, daemon + CLI; fidelity tools (`fidelity_sweep`, `fidelity_report`, `fidelity_trace`, `calibrate`), `hindsight`, `deckstudy`, `autopilot`, `skillgate` |
| `mods/AgentBridge/` | The game mod (C#): state, actions, fight export |
| `.claude/skills/` | The strategy book |
| `rl/` | The solver: `model.py` (network), `ppo.py` (training), `fastsearch.py` (search driver), `solver.py` (batch solver), `bench_fast.py` |
| `crates/sts2sim` | The simulator (engine, content, observation, action space). No dependencies |
| `crates/sts2env` | `BatchEnv` and the search state machine (`search.rs`) |
| `crates/sts2py` | PyO3 bindings: `sts2.VecEnv`, `sts2.Sim` (one fight, steppable, alignable), the search engine |
| `crates/sts2diff` | Differential tester: replays real-game traces in Rust and compares full state |
| `oracle/`, `verify/` | The game's own combat code running headless; the scripts that diff the simulator against it |
| `tools/` | Fuzzers (`fuzz_gen*.py`) and the scenario-set generator (`gen_train.py`) |
| `scripts/` | `pod_train.sh` (training on a GPU pod), `skill_gate.py` (hook entry), `porting/` (generators for the simulator's id and definition tables, rerun after a game update) |
| `docs/` | `design.md`, `env-api.md`, `solver.md`, `oracle.md`, `porting-guide.md`, `relics.md`, `colorless.md`, `spec/` (engine semantics from the game source) |
| `models/`, `data/` | Trained networks; `data/train/` fixed eval sets (`eval.json`, `mid.json`); `data/corpus` (written by `improve corpus`) |
| `runs/`, `evals/` | Run records; sweep outputs and the adoption ledger (`ledger.jsonl`, written by `improve gate`) |

## The simulator and the solver
A bit-exact, allocation-free Rust re-implementation of Slay the Spire 2 combat, differential-tested against the real game's own code (every card, relic, potion, power and monster, step by step, including all nine RNG streams). The contract is in [`docs/env-api.md`](docs/env-api.md): the action space and observation mirror what a human has; draw, discard and exhaust piles are unordered multisets, RNG is hidden, upcoming enemy turns are exposed as expert pattern knowledge (exact cycles, odds at random branches). Design: [`docs/design.md`](docs/design.md).

The solver (`rl/`): a PPO-trained policy/value network plus determinized play-out search (`VecEnv.fork_from` copies a fight and resamples exactly what a player cannot see), with a value ensemble. On the 1,500 held-out A10 fights of `data/train/eval.json` the default solver wins 73.7-73.9% and loses 36% of max HP on average; the network alone wins 65.5% and loses 42% (table "Default solver on `eval.json`" in [`docs/solver.md`](docs/solver.md), which also has the settings and throughput).

Fidelity checks: specs from the game source (`docs/spec/`), the oracle (`docs/oracle.md`), differential tests (`cargo run -p sts2diff`, `verify/regress.py`), and for the live game `python -m agent.fidelity_sweep`, which plays real fights and counts every time the simulator's prediction of the visible state was wrong. Unported content never runs silently (`Combat::missing`).
