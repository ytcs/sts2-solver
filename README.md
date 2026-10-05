# STS2 self-play harness

A harness that lets an agent (Claude) play **Slay the Spire 2** end to end in the real game, with a combat solver for the turn-by-turn decisions, tools to price
the macro decisions (cards, relics, shops, rests, routes), and an outer loop that turns experience into a better strategy book and a better solver.

**Target:** Ascension 10 (A8 tough-enemy HP, A9 deadly-enemy damage, 2 potion slots, Ascender's Bane, a second boss; the run starts at 64/80 HP for the Ironclad).
Game build: v0.111.0 public beta. Winning the run is the goal, finishing with the most HP the second one.

```
 game (Godot + C#)                          python -m agent <cmd>
 └─ mods/AgentBridge  :15555  ◄────────────  agent daemon :15556  ◄──── me (shell)
    state, actions, visible fight export       ├─ fight.py    rebuild the fight in the simulator from observations only
                                               ├─ engine.py   network + determinized search (micro), batch solver (macro / predictions)
                                               ├─ macro.py    price a choice: variants of the current deck vs encounters
                                               ├─ harness.py  commands, run record      └─ improve.py  outer loop
```

## Quick start
```bash
# one-time: Rust + Python environment
cargo test --workspace
uv venv .venv && uv pip install maturin numpy torch pillow yt-dlp --python .venv/Scripts/python.exe
VIRTUAL_ENV=$PWD/.venv .venv/Scripts/maturin develop --release -m crates/sts2py/Cargo.toml
export STS2_DEVICE=cuda                                  # GPU for the networks (a compile of ~2 min the first time)

# the game with the bridge mod (needs .NET 9; Steam game id 2868840): kills the game, builds, installs, relaunches, waits for the bridge
bash mods/AgentBridge/dev.sh

python -m agent s                  # state with numbered options (the daemon starts hidden on first use; loading takes about a minute)
python -m agent a 1 ironclad 10 SEED   # menu: new run (option 0) or custom run with a seed (option 1); then `a <i> [target] [-- why]` for everything else
python -m agent adv 5              # solver advice with a 5 s search budget (default: auto, 1-15 s from the fight's predicted danger; early stop on a clear winner or tie) plus the enemies' expected damage next turns
python -m agent turn | combat      # let the solver play this turn / the whole fight (`budget <s>` sets the default search time)
python -m agent eval --pool Hive:elite --v "+Card|add=ID" --v "smith|upgrade=ID"   # combat value of choices against encounter pools
python -m agent route M E R S B --hp 50 --act Hive    # HP budget along a planned route
python -m agent draw r1c6 r2c6 ...  # draw the planned route on the in-game map; `d` deck, `p draw` a pile, `m` the map, `relics` counters
python -m agent status             # replay fidelity of the current fight, engine, run id
python -m agent quit
```
Commands are documented in [`agent/harness.py`](agent/harness.py); the bridge protocol in [`mods/AgentBridge/README.md`](mods/AgentBridge/README.md). Guards: `adv`/`turn` refuse and say so loudly when the simulator is out of sync with the game, and a map click onto an elite or boss below 60% HP needs `!`.

## How a decision is made
* **Micro (every combat action).** The bridge exports the fight start (deck, relics, potions, HP, encounter) and the visible state after every action. `agent/fight.py` rebuilds the fight
  in the simulator: it replays the actions taken, resamples enemy turns until the intents match what the game shows, and aligns the visible state (hand, piles as multisets, HP, block, energy).
  Hidden information (draw order, RNG, random enemy branches) is the simulator's own random sample, never read from the game. The search then tries the likeliest actions on determinized
  futures and the action is sent to the game. About 10 ms per decision after the load.
* **Macro (everything else).** I decide, using the strategy book (`.claude/skills/sts2-*`, rooted at the `sts2` skill) and `eval`, which plays variants of the deck against the encounters ahead
  and reports win rate and HP lost with their margins. The act's encounter pools (weak / regular / elite / boss) are in `agent/pools.py`.
* **No cheating:** no dev console or god mode in a scored run, no hidden state (draw order, RNG streams, the pre-rolled encounter and elite order), no restarts. The run seed is not exported.

## The outer loop (`agent/improve.py`)
Every decision and fight is recorded in `runs/<id>/events.jsonl` (the solver's prediction at each fight start, every action with the options weighed, outcomes, my reasons for macro choices).
```bash
python -m agent.improve review          # predicted vs actual, how often search overrides the policy, simulator fidelity, follow-ups
python -m agent.improve lessons         # the strategy book's open hypotheses = the experiment backlog
python -m agent.improve corpus          # fights of all runs -> data/corpus (train / held-out split by run)
python -m agent.improve finetune        # PPO fine-tune of the current network on corpus + base distribution
python -m agent.improve gate CKPT       # candidate vs current on the corpus holdout and the fixed eval set, paired seeds -> evals/ledger.jsonl
python -m agent.improve adopt CKPT --as NAME   # only after the gate passed
```
Strategy lessons are adopted by the same rule as checkpoints: a claim in a skill moves from `[hyp]` to `[sim]` / `[played]` only with a test behind it.

## Layout
| Path | What |
|---|---|
| `agent/` | The harness: bridge client, fight rebuild, engine, macro evaluation, run record, outer loop, daemon + CLI, self-tests (`selftest`, `validate`), `video/` (watch a recorded run). |
| `mods/AgentBridge/` | The game mod (C#): state, actions, fight export. |
| `.claude/skills/` | The strategy book. |
| `rl/` | The solver: `model.py` (network), `ppo.py` (training), `fastsearch.py` (search driver), `solver.py` (batch solver). |
| `crates/sts2sim` | The simulator (engine, content, observation, action space). No dependencies. |
| `crates/sts2env` | `BatchEnv` and the search state machine (`search.rs`). |
| `crates/sts2py` | PyO3 bindings: `sts2.VecEnv`, `sts2.Sim` (one fight, steppable, alignable), the search engine. |
| `crates/sts2diff` | Differential tester: replays real-game traces in Rust and compares full state. |
| `oracle/`, `verify/` | The game's own combat code running headless, and the scripts that diff the simulator against it. |
| `scripts/porting/` | Generators for the simulator's id and definition tables (when the game updates). |
| `docs/` | `design.md`, `spec/`, `oracle.md`, `env-api.md`, `solver.md`, `porting-guide.md`. |
| `models/`, `data/` | Trained networks; the fixed eval sets (`data/train/eval.json`, `mid.json`); `data/corpus` from my runs. |
| `runs/`, `evals/` | Run records; the gap list and the adoption ledger. |

## The simulator and the solver
A bit-exact, allocation-free Rust re-implementation of Slay the Spire 2 combat, differential-tested against the real game's own code (every card, relic, potion, power and monster, step by step,
including all nine RNG streams). The contract is in [`docs/env-api.md`](docs/env-api.md): the action space and observation mirror what a human has; draw, discard and exhaust piles are unordered
multisets, RNG is hidden, upcoming enemy turns are exposed as expert pattern knowledge (exact cycles, odds at random branches).

The solver (`rl/`): a PPO-trained policy/value network plus determinized play-out search (`VecEnv.fork_from` copies a fight and resamples exactly what a player cannot see), with a value
ensemble. On 1,500 held-out A10 fights it wins 72.6% and loses 36% of max HP on average (network alone 65.5% / 42%). Numbers and design: [`docs/solver.md`](docs/solver.md).

How fidelity is checked: specs from the game source (`docs/spec/`), the oracle (`docs/oracle.md`), differential tests (`cargo run -p sts2diff`, `verify/regress.py`), and, for the live game,
`python -m agent.validate` which plays real fights and counts every time the simulator's own prediction of the visible state was wrong. Unported content never runs silently
(`Combat::missing`).
