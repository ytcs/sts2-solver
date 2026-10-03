# STS2 combat simulator — an RL environment that matches the real game

A bit-exact, allocation-free Rust re-implementation of **Slay the Spire 2 combat** (v0.111.0 public-beta, single player),
built to run 10⁴–10⁵ fights in parallel as a reinforcement-learning environment. Fidelity is not assumed: every card,
relic, potion, power and monster is differential-tested against the real game's own code, step by step, including the
state of all nine RNG streams.

**Target:** Ascension 10 (the only level that matters here): A8 tough-enemy HP, A9 deadly-enemy damage, 2 potion slots,
Ascender's Bane in the deck. Lower ascensions run through the same code but are not the validation target.

## What the agent sees and chooses (the contract)
See [`docs/env-api.md`](docs/env-api.md). In short: the action space and observation mirror what a human has —
play card (with each legal target), use/discard potion, end turn, and the exact click-then-confirm card-selection screens;
the draw, discard and exhaust piles are visible only as unordered multisets (a player knows what is in a pile, never its order),
RNG is hidden, upcoming enemy turns are exposed as the expert pattern knowledge a veteran has by heart (exact cycles; odds at random
branches; realized random outcomes stay hidden), and tests perturb hidden state to prove it can't leak into the observation.

## Layout
| Path | What |
|---|---|
| `crates/sts2sim` | The simulator (engine, content, observation, action space). No dependencies. |
| `crates/sts2env` | `BatchEnv`: thousands of auto-resetting envs stepped with rayon into flat buffers. |
| `crates/sts2py` | PyO3 bindings → `import sts2; env = sts2.VecEnv(...)` (zero-copy NumPy). |
| `crates/sts2diff` | Differential tester: replays real-game traces in Rust and compares full state. |
| `oracle/` | The oracle: the game's own C# combat code running headless (no Godot runtime), driven by scenarios. |
| `tools/` | Generators (`gen_ids.py`, `gen_defs.py`, …), `mk_scenario.py`, `diff_sweep.py`, `regress.py`, `coverage.py`. |
| `docs/` | `design.md` (architecture, performance), `spec/01–05` (verified engine semantics), `oracle.md`, `env-api.md`, `porting-guide.md`. |
| `decomp/` | Decompiled game source (gitignored; regenerate with `ilspycmd`, see `docs/design.md`). |

## Quick start
```bash
# build + test the simulator
cargo test --workspace

# Python environment (venv with maturin + numpy)
uv venv .venv && uv pip install maturin numpy --python .venv/bin/python
(cd crates/sts2py && VIRTUAL_ENV=$PWD/../../.venv ../../.venv/bin/maturin develop --release)
.venv/bin/python tools/py_smoke.py 4096

# differential testing against the real game
(cd oracle/combat && dotnet build -c Release)            # needs the game install (sts2.dll)
cargo build -p sts2diff
python3 tools/mk_scenario.py --encounter NIBBITS_WEAK --starter --deck ANGER,SHRUG_IT_OFF:2 > /tmp/t.json
python3 tools/diff_sweep.py /tmp/t.json --n 40            # A10 by default
python3 tools/regress.py --n 3                            # every template in oracle/templates
python3 tools/coverage.py                                 # what is ported
```
```python
import sts2, json
env = sts2.VecEnv(n_envs=4096, scenarios=[json.load(open("scenario.json"))], seed=0)
obs, mask = env.reset()                                   # obs float32 [n, OBS_SIZE], mask uint8 [n, ACTIONS]
obs, mask, reward, done, info = env.step(actions)         # actions: int32 dense action indices (must be legal)
```

## Episodes, rewards, scenario distributions
* `done[i] = 1` ends an episode; `info["outcome"]`: `1` win, `-1` loss, `2` truncated (`max_steps`), `3` unimplemented content reached,
  `4` capacity overflow (the last two mean the fight would not be faithful — drop or resample those episodes). Reward defaults to +1/−1
  (configurable: `win`, `loss`, `hp_bonus`·final_hp/max_hp, per-step).
* A scenario is the oracle JSON format (character, ascension, encounter, ordered deck with upgrades, relics, potions, HP, seed, …);
  `VecEnv(scenarios=[...])` samples one uniformly per episode and redraws every RNG stream. Build distributions with
  `tools/mk_scenario.py` or the randomized generators (`tools/fuzz_gen.py`, `fuzz_gen_orb_pet.py`, `fuzz_gen_mix.py`) — they produce
  realistic Ascension-10 runs (starter deck + Ascender's Bane, random additions/upgrades/relics/potions) over every encounter.
* Exclude the colorless card `STRATAGEM` from training decks (a few of its draw contexts are not yet resumable; see `docs/design.md`).

## How fidelity is guaranteed
1. **Specs from the source** (`docs/spec/`): exact hook order, damage pipeline (decimal arithmetic), draw/shuffle (including
   .NET introsort tie behaviour), RNG streams, monster state machines.
2. **The oracle** (`docs/oracle.md`): real game code, headless, deterministic; records full state after every step.
3. **Differential tests**: `sts2diff` replays the same scripted actions in Rust and compares every field — HP, block, powers
   (list order), piles (order), intents, relic counters, orbs, Osty, stars, and the counter/state of all RNG streams.
4. **Corpus + fuzzing**: `oracle/templates/` (hundreds of scenarios, run by `tools/regress.py`) and randomized fuzz rounds
   across characters × decks × relics × potions × every encounter.
5. **Unported content never runs silently**: using anything without a Rust port sets `Combat::missing`; envs abort such
   episodes (`OUTCOME_UNIMPLEMENTED`).

## Status
See the coverage table in [`docs/design.md`](docs/design.md) and run `python3 tools/coverage.py`. Known gaps and open
problems are tracked in `docs/design.md`.
