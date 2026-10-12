---
name: sts2-deckbuilding
description: Use on every card reward, rewards, shop, rest / smith, upgrade, removal, transform or relic choice: the protocol, the draft rules D#, the five buckets and what the numbers cannot see. Plan first (sts2-plan); evidence.md only to question a rule.
---

# Deck building (draft, removal, upgrades, shops)
Numbers are one input; the plan (`sts2-plan`) and these rules weigh what they cannot see.

## Protocol (every deck screen)
1. `brief`: buckets, gaps, bottleneck line (cards left at end of turn: no energy -> energy / cost cuts; with energy -> removal or a combo piece). Name the weakest upcoming binding fight and the final pair's ask.
2. Numbers: `price` on card rewards / shops / events; `eval --boss --v ...` for the act boss; the final pair (or the act-3 boss pool) for act-2/3 picks (P2).
3. `a ~Card -- rules: D1,P2 ; numbers: <best, chosen margin> ; judgment: <plan fit>` (guards refuse a pick without these fields).

## Rules
- D1 [sim] From act 2, skip by default; take a card only when it is on-plan (engine / payoff) or a binding fight within reach needs it (arm C: act-2 boss deck +0.19, act-2 reach +0.20; E84). Test: instrument: the pick's `eval` gain vs the final pair > 2 se on the current deck or on the target deck once the target's pieces are mostly in (a target the run never reaches biases toward skipping; run 2: two picks the search check rated > 2 se were skipped on a target deck whose adds never came).
- D2 [sim] In act 1 take what the act-1 elites and boss need (front damage pays there, E85); prefer cards that still scale. Test: instrument: `eval --boss` gain at the act-1 boss.
- D3 [sim] Before an act-3 pick, price it against the known pair with and without (`eval --boss --v`); the card must raise pair strength (E86: act-1-deck proxies get the sign wrong). Test: instrument.
- D4 [sim][expert] Removal order: Strikes and Defends first, then act-1 damage picks (remove-down to 15 cards +0.14 to +0.76 at the pair, review section 4; Baalorlord removes Strikes before bad drafted cards). Test: instrument: `eval --boss --v "r|remove=ID"` vs the pair.
- D5 [sim][expert] Shop: the relic that scores best vs the final pair before a card; removals once few off-plan cards remain (review section 4, Xu relics over cards 42 -> 47%). Test: instrument: `eval` with `relics_add` / `remove`.
- D6 [sim][expert: user] Upgrade the engine piece or what the next binding fight asks; a basic only when it measures among the best (within 1 se of the top non-basic or above it; 25% of past smiths went to basics, review section 8; run 3: Defend+ 0.354 vs Bully+ 0.344 at the pair). Test: instrument: `eval --v "u|upgrade=ID"` at the next binding fight.
- D7 [hyp] Enablers (Barricade, Demon Form, Feel No Pain, Dark Embrace, Corruption) only with >= 2 payoffs likely and a survivable interim. Test: instrument: bundle `eval` (enabler + payoff vs neither).
- D8 [hyp] Potions beat a marginal card in the same shop when a hard boss is next. Test: instrument: potion as an `eval` variant.
- D9 [hyp] Past ~20 cards, skip ties and prefer removal (28-card act-2 deck: removing 8 starters +0.20; E74-era). Test: instrument: `eval` of the removal vs the pick.
- D10 [code] Multi-pick screens (Orrery, bundles): opening and skipping loses nothing until Proceed; price bundles and leave-one-out at real HP.

## The five buckets
FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration. A bucket is solved when no fight ahead asks more of it; a card only into solved buckets needs a clear gain on the pair or the weakest fight (D1). Buckets per card: `data/card_buckets_ironclad.json`.

## What the numbers cannot see
Economy (gold, max HP, slots, curses) is not in `eval`: price in HP / gold. Rules that buckets hide (Slippery, Artifact, Thorns, Hard to Kill): boss-alone `eval`. HP-cliff bosses: re-price with `--seed 1`. Enchants: `--v "x|enchant=ID:ENCH"`.
