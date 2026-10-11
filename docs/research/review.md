# First-principles review (2026-10-11, in progress)
Goal: decide where to go after 120+ scored runs gave 1 win (E77 BASE0003). Gate (plan "Next"): a written lever ranking that the next arm tests. Rounds with the advisor; new numbers between rounds.

## 1. Problem statement (from our own numbers)
- Every macro arm since E70 moved act-1 pass and act-2 reach, never final-pair deck strength: control 0.14, E71 0.25, E73 0.20, E77 0.26, E78 0.12, E80 0.18 (mean P(win) per final boss, full HP, no potions).
- Final-pair oracle (E83, 5 runs): better choices among the cards the run was offered buy +0.00 to +0.11 (holdout); potions held at the pair +0.13 to +0.27; 14 HP of arrival -0.24.
- Every arm since E70 tuned card pricing among offers; the oracle says that is not what binds. Stress test of that claim: section 4.

## 2. Mechanisms (testable, not essay)
1. Exact decomposition, wrong estimator. P(win) = prod P(clear act_i | reached) is a chain rule. Each conditional is estimated by today's deck at a fixed HP vs that boss; it drops (a) the deck we will actually have there (future picks: enablers/scaling undervalued, front damage overvalued) and (b) the HP / potions we will arrive with (hallway attrition and rests invisible to card pricing; only `routes` carries HP, only at forks).
2. No model of the offer distribution. Pools, rarity offset and shop prices are public (information contract), so take-now-vs-wait is an optimal-stopping problem with a known distribution (E[best future offer | floors left, pools, rarity offset]), not a 2-se constant.
3. Access has no value. Elites (relic + ~3x rare odds), shops (gold -> relics), events, gold carried are priced only through P(win act boss). Expert priors live here.
4. The run value is handcrafted. Mature single-player RL learns the run value from outcomes and improves the policy by search against it (expert iteration). Ingredients exist (run model, predictor with validated deck-strength correlation, 120+ logged runs); open: run-model fidelity and data cost.

## 3. Access in our runs (logs, no GPU; `target/review/access.json`, 110 runs E75a/E77/E78/E80/E81)
| subset | n | elites A1 / A2 / A3 | relics | shops | bought card / relic / potion / removal | potions offered in shops | deck | gold at end |
|---|---|---|---|---|---|---|---|---|
| all | 110 | 0.81 / 0.51 / 0.32 | 8.5 | 2.5 | 1.85 / 0.26 / 1.89 / 0.48 | 18.1 | 23.2 | 118 |
| reached act 3 | 44 | 0.91 / 0.55 / 0.80 | 12.1 | 3.6 | 2.39 / 0.57 / 2.77 / 0.61 | 25.5 | 27.0 | 110 |
| reached final pair | 31 | 1.06 / 0.65 / 0.74 | 12.9 | 3.5 | 2.45 / 0.71 / 2.71 / 0.61 | 25.5 | 27.2 | 100 |
| won (E77 BASE0003) | 1 | 1 / 0 / 2 | 13 | 6 | 4 / 0 / 5 / 0 | 39 | 24 | 152 |
- The bot fights ~1 act-1 elite and ~2 elites per run. Experts (Spire Clinic 6T4bZkpdxbA) take the max-elite act-1 path (3 on that map) while keeping lanes that can dodge them; Spire Cast: "Ironclad farms every act-1 elite".
- Within our narrow range, final deck strength of act-3 runs is uncorrelated with elites per act, relics, deck size, shops, potions bought, gold (|Spearman| <= 0.22, n 44): no within-sample signal; our runs barely vary in access.

## 4. Oracle stress (`tools/oracle_stress.py`, `target/review/oracle_stress.txt`)
Pending: remove-down to 18 and 15 cards (greedy, holdout-scored); best pair of adds among the top-10 single adds from the run's own offers.

## 5. Expert corpus (Japanese Export; `data/expert/japaneseexport/README.md` ranks 9 recurring claims over 11 videos)
Both Spire Clinic reviews read in full (6T4bZkpdxbA Ironclad A10, l6leE93iqPE Silent A10). The reviewer's diagnosis of each lost run is macro, not micro, and is the same both times: early decisions snowball; convert early surplus into end-game power.
- Path: max elites in act 1 on a lane that can bail out; early shop only to fix an immediate problem; delay commitments while the map allows.
- Gold: do not spend in act 1 unless it fixes a now-problem; walk into act 2 with 300+ gold for run-winning relics (Anchor, Pocket Watch); removals are low value unless you also skip aggressively; investing gold that pays back via an elite is fine.
- Cards: value each card by what upcoming fight it solves and what function the deck lacks (FD/SD/FB/SB/ACC); once a function is solved (e.g. scaling damage), more of it is negative; over-index on what the deck does well rather than patching weaknesses with off-plan cards; skip anything not serving the end-game plan once the current plan works ("if every fight has a bad draw order, the deck is bad").
- Upgrades: smith the engine (Inflame, Noxious Fumes, Snakebite) over block or HP when the next fights are survivable; "upgrade debt" is a real cost.
- Potions: bank them once strong (accept 15-20 HP in a hallway to keep a Fire Potion for an elite); arriving at the final boss with one potion "means something went wrong".
- Relics / ancients: choose by when the run stabilizes (immediate power vs long-term); evaluate in context (Velvet Choker downside irrelevant for a 3-card-per-turn deck).
- Process: spend thinking time early, where the snowball is largest; assume your mistakes, not luck.
Mapping to our levers: elites (access), gold into relics, potion banking for the pair, upgrade targeting, function-aware skipping. None is valued by the current estimator except skipping (deadline rule) and rests (routes).

## 6. Literature (searched 2026-10-11)
- Learned macro for StS1: REINFORCE MLP (~100k params, 15k games) for all non-combat decisions, MCTS combat, sts_lightspeed simulator; Ironclad A0, 50 held-out seeds: 14% win vs 6% stock heuristics, +11 floors; authors: the stock bot's biggest weakness "was walking the map at random". https://huggingface.co/Jialeiv/sts-rl-agent
- StS2 LLM agents (AgenticSTS, Alaya Lab / SJTU): Silent A0, 6/10 wins with a skill library vs 3/10 without; competitors 0/5; humans ~16% at A0 per the developers. https://the-decoder.com/ai-agents-win-at-slay-the-spire-2-after-researchers-replace-growing-chat-logs-with-structured-memory/ (dataset https://huggingface.co/datasets/ShandaAI/AgenticSTS-trajectories)
- StS2 aggregate human win rate ~25% over all runs; community stats ~17% at A10 (unverified). StS1 A20 humans ~3% overall, top players 40-50% (forum estimates, unverified).
- Card drafting by RL (Legends of Code and Magic arena): trained draft agents beat the best heuristic drafters by building very different decks; earlier work approximated deck strength by heuristic play, later end-to-end. https://arxiv.org/abs/2001.01326, https://sol.sbc.org.br/index.php/sbgames_estendido/article/view/19743
- Stochastic MuZero (afterstates, chance nodes) for planning in stochastic environments (2048, backgammon). https://iclr.cc/virtual/2022/poster/6832
- Imitation of 77M human StS card picks: 53.5% pick accuracy; a bigger model played worse in-game (dev.to series, unverified details). https://dev.to/doronsinger/part-3-good-models-finish-last-2n6a
- No published agent reports A20 / A10 win rates near ours; a 90% A10 bot would exceed every published result. The binding gap in the one learned-macro result was pathing.

## 7. Candidate approaches (round 2 to rank)
- A. Macro expert iteration: learn V_run from run-model rollouts (+ real runs); search macro decisions against it. Pre-experiment: run-model fidelity (simulate the logged runs' decisions; compare act reach / deck strength distributions) and run-model runs per hour.
- B. Resource DP: extend `routes`' act DP with a coarse deck-strength state and the known offer distribution, pricing picks, elites, shops, rests and potions with one value over (HP, gold, potions, strength bucket). Pre-experiment: re-price the E80 final-pair runs' act-3 routes.
- C. Expert-prior policy as the new control: max-elite act 1 while `routes` P(boss) stays high, gold carried into act 2, smith default, potion banking for elites / the pair, function-aware skipping. Cheapest; the principled methods must beat it.
