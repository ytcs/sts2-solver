---
name: sts2-deckbuilding-evidence
description: Evidence behind the deck-building rules (open only when questioning a rule or adding a result): the pick-policy benches and the Aeonglass study with numbers, the rejected radar, run lessons, tags and literature.
---

# Evidence for `sts2-deckbuilding`

Every number here is `[sim]` unless marked `[played]` or `[expert]`. Rules in the main skill cite this file; a new result goes here first, then the rule changes.

## Pick policy: smooth greedy was adopted on this evidence
**Bench 1** (`python -m agent.deckbench`, 10 mock reward screens of 3 random Ironclad cards + skip from the starter deck, 8 sequences, win rate of the final deck at full 80 HP vs Lagavulin Matriarch): skip 0.00, random 0.01, plain greedy (best win rate now, skip when no gain) **0.31**, smooth greedy **0.70**, always-pick smooth greedy **0.68**, hindsight beam (knows the future offers) 0.63 plain / 0.81 smooth.

**Aeonglass study** (`python -m agent.deckstudy`, `evals/ds_aeonglass.json`: 20 reward screens from the starter deck, 6 paired sequences, smooth = win averaged over start HP x1/2/3/4/6, each approach with its own evaluation cache). Mean smooth score, kilo-fights of simulation per deck built, difference to smooth greedy (standard error ~0.04):

| approach | smooth | kfights | vs smooth greedy |
|---|---|---|---|
| skip / plain greedy | 0.000 | 0 / 2 | -0.18 +- 0.09 |
| always-pick smooth | 0.188 | 19 | +0.005 |
| smooth greedy (adopted) | 0.183 | 19 | reference |
| mock completion | 0.222 | 36 | +0.04 +- 0.03 |
| sampled-future rollouts R=4 tag / R=4 random / R=8 tag | 0.235 / 0.243 / 0.246 | 49 / 49 / 98 | +0.05..+0.06 +- 0.04 |
| elite-frequency picker (offline genetic search) | 0.168 | 0 online, 38 offline once | -0.015 +- 0.05 |
| myopic hindsight beam | 0.098 | 46 | -0.09 +- 0.05 |
| ceiling: best complete 20-card deck of any cards | 0.618 | 38 offline | (91% at 3x HP, 0% at 80 HP) |

Reading: (1) the smooth objective is essential (plain greedy picks nothing and scores 0 on every sequence); (2) a myopic beam is worse than greedy because it prunes setup lines (it keeps 2.3 picks); (3) lookahead is nominally +20-30% at 2-5x the compute but not significant at 6 sequences, and a random continuation does as well as the bucket continuation, so any gain is averaging over unknown futures; (4) the elite-frequency picker matches greedy with no simulator call at pick time; (5) the gap to the ceiling (0.25 vs 0.62) is which cards are OFFERED, not how they are chosen; (6) variance between sequences is huge (early Demon Form / Inferno gives 0.5-0.6, most sequences 0): detecting +0.05 needs ~30 sequences. Open: round 2 with ~24 more sequences, 30 screens plus upgrades (smooth greedy reaches 0.42 there, 87% at 6x HP), and the best deck buildable from the offered cards only.

Other facts from the same work: with 30 screens + 10 upgrades smooth greedy builds a deck that wins 16% at 2x, 41% at 3x, 64% at 4x, 87% at 6x HP vs Aeonglass and 0% at 80 HP (`evals/db4_aeonglass_up.json`). Hand-built exhaust decks vs Aeonglass: Fiend Fire / Feel No Pain / Dark Embrace / Second Wind / True Grit 0.00, plus Inflame and Demon Form 0.21, the self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle) 0.61. Aeonglass is a damage race; Withers (unplayable, hurt at turn end in hand, +3 per Increasing Intensity, one added per 6 cards played) are the minor part; exhaust packages and Wither damage match the real game. Corruption is Ancient rarity (not in reward pools).

## Mechanics of the numeric layer (so its blind spots are understood)
- `eval` / `reward` price the COMBAT side only: the solver's win rate and HP lost against fixed encounters from the deck as it is now. Gold, shops, events, max HP, relic pickup effects, route and future offers are not in it.
- Value is not additive: a card's value depends on the deck (Blood Wall -5 -> +10 once Inferno was in the deck).
- Standard error of a 96-attempt win rate is ~0.03-0.05; a difference under 2 SE is noise. The smooth objective needs ~4x the fights of a plain eval.

## Run lessons `[played]`
- Run 3 (Act 3, died to the first Act 3 boss): early game is where power is added: Act 1 picks (Inferno, Anger, Setup Strike, Inflame+Tremble vs Lagavulin) moved win rates 20-70 points; by Act 2-3 almost every offered card was below the 2 SE bar (12 skips in Act 2) and the deck stalled. When a boss pool is at ~0%, a +5 pick will not save the run.
- Pre-check the next act's boss on the Act N boss reward: evaluate against all three possible bosses.
- Potions are cheap power: against hard bosses Powdered Demise (+14), Flex (+11), Block Potion (+12) beat every card in the same shop.
- Run 4's deck read FD 12, SD 3, FB 8, SB 0, ACC 3: the empty bucket (scaling block) matched what lost the long fights.

## Rejected: demand-vs-capacity radar
A Python prototype (enemy schedule measured by passing turns, per-card effects probed, perfect-order planner, bisection on the efficiency needed to win) took 0.33 s per fight against 0.27 s for `eval` at 64 attempts and agreed with the solver only moderately (Spearman -0.55 over 88 recorded fights; 10 of 80 fights the solver wins were called impossible: it cannot value triggered damage, engines like Primal Force + Demon Form, or boss reactions). Deleted. Survivors: `sts2.provably_unwinnable` (sound, narrow), `eval --future`, reading the enemy's passive schedule by hand (pass turns with a huge-HP player), and `eval` sensitivity variants (add Strength / Block / energy) to see which resource binds.

## Tags and literature
- `data/card_buckets_ironclad.json` (regenerate: `python -m agent.card_tags`): buckets per Ironclad card from simulator measurements plus an override table; `card_tags.deck_line`, `deficiencies`, `candidates`. Tags propose; `eval` decides.
- Evolutionary deck building with a simulator as fitness (Garcia-Sanchez et al., Hearthstone), whose heuristic mutation mimics a human (swap in a card that fills a weakness); quality-diversity search (MAP-Elites) with a learned surrogate when evaluations are expensive (Zhang et al., arXiv 2112.03534). `agent/deckstudy.py` has the genetic search (`ga_search`), mock completion, rollouts and the elite-frequency picker.
