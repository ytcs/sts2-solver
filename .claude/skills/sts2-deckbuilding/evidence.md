# Evidence for `sts2-deckbuilding` and the solver
Not a skill: read to question a rule; add the result first, then change the rule. `[sim]` unless marked.

## Pick policy
- Bench 1 (10 mock reward screens of 3 random Ironclad cards + skip from starter, 8 sequences, final deck vs Lagavulin Matriarch at 80 HP): skip 0.00, random 0.01, plain greedy 0.31, smooth greedy 0.70, always-pick smooth 0.68, hindsight beam (knows future offers) 0.63 plain / 0.81 smooth.
- Aeonglass study (20 screens from starter, 6 paired sequences, smooth over start HP x1/2/3/4/6; se ~0.04):

| approach | smooth | vs greedy |
|---|---|---|
| skip / plain greedy | 0.000 | -0.18 +- 0.09 |
| always-pick smooth | 0.188 | +0.005 |
| smooth greedy (adopted) | 0.183 | ref |
| sampled-future rollouts R=4..8 | 0.235-0.246 | +0.05..+0.06 +- 0.04 (2-5x compute, n.s.) |
| elite-frequency picker | 0.168 | -0.015 +- 0.05 |
| myopic hindsight beam | 0.098 | -0.09 +- 0.05 |
| ceiling: best 20-card deck of any cards | 0.618 | (offline) |

- Gap to ceiling (0.25 vs 0.62) = which cards are offered. Sequence variance huge: +0.05 needs ~30 sequences.
- 30 screens + 10 upgrades, smooth greedy: Aeonglass win 16% at 2x, 41% at 3x, 64% at 4x, 87% at 6x HP, 0% at 80 HP.
- Hand-built vs Aeonglass: exhaust deck 0.00 (+Inflame, Demon Form 0.21); self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle) 0.61. Corruption is Ancient rarity (not in reward pools).
- `eval`/`reward` price combat only. Se of a 96-attempt win ~0.03-0.05; smooth needs ~4x the fights.

## Solver fidelity and calibration
- High energy (500 held-out scenarios per set, b128): search-over-network gap not wider at 4-7 energy (win +4.9, HP +4.8) than at 3 (+7.5, +5.1); by energy 4/5/6/7: +5.5/+5.8/+3.0/+3.2. No energy discount. `tools/gen_train.py --energy-prob` adds a high-energy slice.
- Sync `[code]`: live sim set to observed hand, discard, exhaust, draw multiset after each action; random generated cards replaced by the game's rolls; sim-created card lacks real keywords until played.
- Power sync `[code]`: sim takes the shown powers (id, amount) of player and enemies after each action. Not synced: relic counters/props the game hides.
- Sweeps (`python -m agent.fidelity_sweep`): no rule bugs in 159 relics, 65 potions, ~120 Ironclad cards, random decks of five characters; leftovers are unreplayable random rolls.
- Thorns: Rust `ThornsPower` matches the game. Real-game replay calibration: Spiny Toad real 23.8 HP lost (se 3.0, n=14) vs sim 23.9 (se 1.6); Axebots real 50.4 (se 5.4, 11/14 wins) vs sim 55.2 (se 2.8, 35/40).

## Rejected
- Demand-vs-capacity radar: Spearman -0.55 vs solver over 88 fights, 10/80 solver wins called impossible, cost = `eval`. Survivors: `sts2.provably_unwinnable`, `eval --future`, `eval` sensitivity variants.

## Tags
`data/card_buckets_ironclad.json` (regenerate: `python -m agent.card_tags`): buckets per Ironclad card; tags propose, `eval` decides.
