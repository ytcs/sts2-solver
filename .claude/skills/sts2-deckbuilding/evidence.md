# Evidence for `sts2-deckbuilding`
Read to question a rule; add the result before changing it. `[sim]` unless marked.
- Pick policy, 10 mock screens to Lagavulin Matriarch at 80 HP: skip 0.00, plain greedy 0.31, smooth greedy 0.70, hindsight beam 0.81 smooth.
- Aeonglass, 20 screens, 6 paired sequences (se ~0.04): smooth greedy 0.183 (adopted), plain greedy 0.000, sampled-future rollouts 0.24 (n.s., 2-5x compute), ceiling (best 20-card deck) 0.618. Gap = which cards are offered; +0.05 needs ~30 sequences.
- Self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle) 0.61 vs Aeonglass; exhaust decks 0.00-0.21. Corruption is Ancient rarity.
- High energy (500 scenarios per set): search gain not wider at 4-7 energy than at 3: no energy discount.
- Se of a 96-attempt win ~0.03-0.05; smooth needs ~4x the fights.
- Rejected: demand-vs-capacity radar (Spearman -0.55 vs solver, 88 fights).
- Buckets per Ironclad card: `data/card_buckets_ironclad.json` (`python -m agent.card_tags`); tags propose, `eval` decides.
