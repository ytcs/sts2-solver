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

## Provably unwinnable fights (`sts2sim::bounds`, `sts2.provably_unwinnable`)
Many generated fights cannot be won by anyone (a starter deck against a boss). `provably_unwinnable(scenario)` returns a sentence proving it, or `None`
(= nothing proven). It relaxes the game in the player's favour and checks that even then the enemy cannot be killed before the player dies:

* every card of the deck must be **pure**: found by probing it in the simulator (five different situations, the same card played twice, a Vulnerable
  enemy) to spend only its energy, deal a fixed damage / gain a fixed block (+ Vulnerable / Weak), move cards between piles and change nothing else
  (no energy, powers, upgrades, cost changes, extra plays, growth). 28 Ironclad and 28 Silent cards qualify; Defect / Necrobinder / Regent are not covered;
* relics must only hook things that cannot matter in a fight (144 of 300), no potions, no starting powers, no enchantments;
* one enemy that cannot react to damage (no stun / sleep / flee intents, no damage-reactive start powers) and whose behaviour is bounded from below
  by the cheapest damage over every state its move machine can be in each turn (every branch counts as possible);
* per turn the player may play any subset of its deck (hand size and draw order ignored) whose cost fits the energy: a damage / block frontier; block
  does not carry over. A DP over turns (state = HP lost, value = damage dealt) shows whether the enemy's HP is ever reachable.

Soundness is checked empirically, not just argued: `tools/check_bounds.py` generates decks of pure cards against every encounter, and no policy
(random, scripted heuristic, the trained network sampled and greedy) may ever win a fight the prover flags. The first run found a real hole (Terror
Eel's Shriek stuns it when damaged, which lowers its damage: reactive enemies are now refused); since then 3 seeds x ~530 flagged fights x 20k
episodes per policy gave 0 wins. Coverage is small on the random-deck training distribution (decks nearly always contain a card, relic or potion the
prover cannot vouch for), so `gen_train.py --drop-unwinnable` filters little there; it matters for the starter-deck-like cases a run-level optimizer
would ask about, and the verdict is a cheap "do not even simulate this".
