# Evidence for `sts2-deckbuilding` and the solver

Not a skill: open to question a rule or add a result (result first, then the rule changes). `[sim]` unless marked.

## Pick policy
**Bench 1** (`python -m agent.deckstudy`; 10 mock reward screens of 3 random Ironclad cards + skip from the starter deck, 8 sequences, final-deck win at 80 HP vs Lagavulin Matriarch): skip 0.00, random 0.01, plain greedy 0.31, **smooth greedy 0.70**, always-pick smooth 0.68, hindsight beam (knows future offers) 0.63 plain / 0.81 smooth.

**Aeonglass study** (`evals/ds_aeonglass.json`: 20 reward screens from the starter deck, 6 paired sequences, smooth = win averaged over start HP x1/2/3/4/6). Mean smooth score, kilo-fights per deck, difference to smooth greedy (se ~0.04):

| approach | smooth | kfights | vs greedy |
|---|---|---|---|
| skip / plain greedy | 0.000 | 0 / 2 | -0.18 +- 0.09 |
| always-pick smooth | 0.188 | 19 | +0.005 |
| smooth greedy (adopted) | 0.183 | 19 | ref |
| mock completion | 0.222 | 36 | +0.04 +- 0.03 |
| sampled-future rollouts R=4 tag / R=4 random / R=8 tag | 0.235 / 0.243 / 0.246 | 49 / 49 / 98 | +0.05..+0.06 +- 0.04 |
| elite-frequency picker | 0.168 | 0 online, 38 offline | -0.015 +- 0.05 |
| myopic hindsight beam | 0.098 | 46 | -0.09 +- 0.05 |
| ceiling: best 20-card deck of any cards | 0.618 | 38 offline | 91% at 3x HP, 0% at 80 HP |

- Plain greedy scores 0 on every sequence; the myopic beam prunes setup lines (keeps 2.3 picks). Lookahead +20-30% at 2-5x compute is not significant at 6 sequences; random continuation matches bucket continuation (gain = averaging over futures).
- The gap to the ceiling (0.25 vs 0.62) is which cards are OFFERED. Sequence variance is huge (early Demon Form / Inferno 0.5-0.6, most 0): +0.05 needs ~30 sequences. Open: ~24 more sequences, 30 screens plus upgrades.
- 30 screens + 10 upgrades, smooth greedy: Aeonglass win 16% at 2x, 41% at 3x, 64% at 4x, 87% at 6x HP, 0% at 80 HP (`evals/db4_aeonglass_up.json`).
- Hand-built vs Aeonglass: exhaust (Fiend Fire, Feel No Pain, Dark Embrace, Second Wind, True Grit) 0.00, plus Inflame and Demon Form 0.21; self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle) 0.61. Aeonglass is a damage race. Corruption is Ancient rarity (not in reward pools).
- `eval` / `reward` price combat only. Se of a 96-attempt win rate ~0.03-0.05; smooth needs ~4x the fights of a plain eval.

## Solver fidelity and calibration
- **High energy** `[sim]` (500 held-out scenarios per set, 4 attempts, ckpt b128; `agent/energy_gap.py`, deleted): the training mix has 4+ energy in 3% of scenarios, 5+ in 0.03%, yet the gap search closes over the network is not wider at 4-7 energy (win +4.9, HP +4.8) than at 3 (win +7.5, HP +5.1); by energy 4/5/6/7: +5.5/+5.8/+3.0/+3.2 win points. No relative policy bias against energy-heavy decks (a bias shared through the common value net is not excluded): no extra discount. `tools/gen_train.py --energy-prob` adds a high-energy slice.
- **Sync** `[code]`: the live sim is set to the observed hand, discard, exhaust and draw pile (multiset) after each action. Randomly generated cards (Stoke, Discovery, Infernal Blade ...) are replaced by the ones the game rolled; a sim-created card lacks the real card's keywords (a Retain from Choices Paradox is lost until played).
- **Power sync** `[code]`: after each action the sim takes the powers (id, amount) the game shows on player and enemies, so a hidden draw or random target that resolved differently (Hellraiser's Strikes, Havoc, Mayhem, Cascade) leaves no stale Slippery / Vulnerable / Strength. Not synced: relic counters and props (Joss Paper, Iron Club, Pael's Legion after Whispering Earring's hidden first hand).
- **Sweeps** `[sim]` (`python -m agent.fidelity_sweep`, `agent.fidelity_report`; each case a fresh dev-console run): no rule bugs in 159 relics, 65 potions, ~120 Ironclad cards, random decks of five characters; leftovers are random rolls a replay cannot reproduce.
- **Thorns** `[sim]` `[played]`: Rust `ThornsPower` matches the game (every powered hit, each hit of a multi-hit card, blockable). Calibration (`python -m agent.calibrate`, one fixed deck replayed in the real game): Spiny Toad real 23.8 HP lost (se 3.0, 14 fights, 14 wins) vs sim 23.9 (se 1.6); Axebots real 50.4 (se 5.4, 11/14 wins) vs sim 55.2 (se 2.8, 35/40). A 37-HP Toad loss (predicted 10) was the tail of the sim distribution (4-34 HP); forcing an end-turn candidate was neutral (20.3 vs 18.9 HP).

## Run data `[played]`
- Run 3 (died to the first Act 3 boss): Act 1 picks (Inferno, Anger, Setup Strike, Inflame+Tremble vs Lagavulin) moved win 20-70 points; by Act 2-3 almost every offered card was under 2 se (12 skips in Act 2) and the deck stalled.

## Rejected: demand-vs-capacity radar
Prototype (passing-turn enemy schedule, per-card effects, perfect-order planner, bisection on needed efficiency): Spearman -0.55 vs the solver over 88 fights, 10 of 80 solver-won fights called impossible (cannot value triggered damage, engines, boss reactions); cost equal to `eval`. Deleted. Survivors: `sts2.provably_unwinnable`, `eval --future`, `eval` sensitivity variants (add Strength / Block / energy).

## Tags and literature
`data/card_buckets_ironclad.json` (regenerate: `python -m agent.card_tags`) holds buckets per Ironclad card (simulator measurements plus overrides); tags propose, `eval` decides. Literature: Garcia-Sanchez et al. (Hearthstone deck evolution); Zhang et al., arXiv 2112.03534 (MAP-Elites with a surrogate); `agent/deckstudy.py` has `ga_search`.
