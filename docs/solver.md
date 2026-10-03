# Combat solver (`rl/`)

Goal: given a deck, relics, potions, HP and an encounter (Ascension 10), play the fight to **win first, then lose as little HP as
possible**. The run-level optimizer that will call this as a subroutine is out of scope; the primary metrics are therefore

* `win` — fraction of fights won,
* `hp_lost_all` — mean fraction of max HP lost over *all* fights; a loss (or a stall past `max_steps`) is charged all the HP the player
  started with. (`hp_lost_on_win` is reported too but hides losses.)

Training reward: +1 for a win (+ `0.5 x HP fraction left`), -1 for a loss or a stall, 0 for aborted episodes (unported content /
capacity overflow; never seen in the training distribution so far).

## Pieces
| file | what |
|---|---|
| `tools/gen_train.py` | realistic A10 scenario sets (starter deck + act-scaled picks, relics, potions, every encounter); different `--seed` = disjoint set |
| `rl/model.py` | policy/value network: entity encoders (player, enemies with intents + expert look-ahead, hand cards, potions, pending-selection candidates, pile multisets as card-embedding bags), pooled context, 2 message-passing rounds, **pointer action head** scoring exactly the env's dense action space; clicking an already selected card again is masked out |
| `rl/ppo.py` | PPO (CPU), resumable checkpoints, held-out evaluation every N iterations |
| `rl/baselines.py` | random, untrained-greedy, scripted heuristic; also evaluates any checkpoint (`ckpt:PATH`) |
| `rl/search.py` | determinized play-out search on top of a network (below) |
| `rl/exit.py` | expert iteration: collect search targets, distill them into the network |

Observation features are signed-log scaled (`S(x) = sign(x) log(1+|x|)`): HP, damage, counters and power amounts are unbounded in this game
(a first run without it showed value-loss spikes up to 1600 and a frozen policy).

## Search (`rl/search.py`)
`VecEnv.fork_from` copies a fight and **determinizes** it (`Combat::determinize`: draw / discard / exhaust orders and all nine RNG streams are
resampled; everything the player can see is unchanged, tested). At each decision the policy's top-M legal actions are each played on K
determinized copies (the same K seeds for every action: a paired comparison); a copy then follows the policy to the end of the
current player turn and is scored by the final reward (fight over) or the value head at the start of the next turn. The root plays the
best mean; where the policy's top action has probability >= `conf` the search is skipped. This is one step of policy improvement over the
base policy and needs no extra training.

## Results
Eval set: `target/train/eval.json` (1500 fights, 5 characters, 3 acts, seed 22), greedy policy, first finished episode of 500 envs x 3.

| policy | win | hp_lost_all |
|---|---|---|
| random | 0.145 | 0.686 |
| untrained net | 0.031 | 0.725 |
| scripted heuristic | 0.363 | 0.572 |
| PPO, 2.5M steps (early) | 0.477 | 0.526 |

Paired comparison on 100 fights (early checkpoint, M=5, K=6): policy 0.51 / 0.516 -> policy + search 0.63 / 0.446.

(Final numbers: see the end of this file.)
