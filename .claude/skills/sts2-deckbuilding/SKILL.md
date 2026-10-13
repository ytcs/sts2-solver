---
name: sts2-deckbuilding
description: Use on every card reward, rewards, shop, rest / smith, upgrade, removal, transform or relic choice: the protocol, the draft rules D#, the five buckets and what the numbers cannot see. Plan first (sts2-plan).
---

# Deck building (draft, removal, upgrades, shops)
Numbers are one input; the plan (`sts2-plan`) and these rules weigh what they cannot see.

## Protocol (every deck screen)
1. `brief`: buckets, gaps, the plan's open answers (which gimmick of which fight ahead the deck cannot answer yet), bottleneck line (cards left at end of turn: no energy -> energy / cost cuts; with energy -> removal or a combo piece). Name the weakest upcoming binding fight and the final pair's ask.
2. Numbers: `price` on card rewards / shops / events; `eval --boss --v ...` for the act boss; the final pair (or the act-3 boss pool) for act-2/3 picks (P2).
3. `a ~Card -- rules: D1,P2 ; numbers: <best, chosen margin> ; judgment: <plan fit>`.

## Rules
- D1 [sim] From act 2, skip by default; take a card only when it is in the `target` (P8) or a binding fight within reach needs it (E84). Before skipping a pick `price` rates > 1 se positive, check it against the pair or the act boss with `eval --boss --v` (E130). An engine piece's `eval` is a lower bound (weak player, a lone piece outside its package, E135, E136): the target decides engine adds, not a single-card number. Test: instrument: the pick's `eval` gain vs the final pair > 2 se on the current deck, or on the target deck once its pieces are mostly in (a target never reached biases toward skipping, E100).
- D2 [sim] In act 1 take what the act-1 elites and boss need (front damage pays there, E85); prefer cards that still scale. Test: instrument: `eval --boss` gain at the act-1 boss.
- D3 [sim] Before an act-3 pick, price it against the known pair with and without (`eval --boss --v`); it must raise pair strength (E86). Test: instrument.
- D4 [sim][expert] Removal order: Strikes and Defends first, then act-1 damage picks (E131). Test: instrument: `eval --boss --v "r|remove=ID"` vs the pair.
- D5 [sim][expert] Shop: the relic that scores best vs the final pair before a card; removals once few off-plan cards remain (E131). Test: instrument: `eval` with `relics_add` / `remove`.
- D6 [sim][expert: user] Upgrade the engine piece or what the next binding fight asks; a basic only when it measures within 1 se of the top non-basic or above (E131). At a smith, `eval` every upgradable card in one call, not a hand-picked list (E130). Test: instrument: `eval --v "u|upgrade=ID"` at the next binding fight.
- D7 [hyp] Enablers (Barricade, Demon Form, Feel No Pain, Dark Embrace, Corruption) only with >= 2 payoffs likely and a survivable interim. Test: instrument: bundle `eval` (enabler + payoff vs neither).
- D8 [hyp] Potions beat a marginal card in the same shop when a hard boss is next. Test: instrument: potion as an `eval` variant.
- D9 [hyp] Past ~20 cards, skip ties and prefer removal (E134). Test: instrument: `eval` of the removal vs the pick.
- D10 [code] Multi-pick screens (Orrery, bundles): opening and skipping loses nothing until Proceed; price bundles and leave-one-out at real HP.

## The five buckets
FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration. A bucket is solved when no fight ahead asks more of it; a card only into solved buckets needs a clear gain on the pair or the weakest fight (D1). Per card: `data/card_buckets_ironclad.json` (tags propose, `eval` decides).

## What the numbers cannot see
Economy (gold, max HP, slots, curses) is not in `eval`: price in HP / gold. Rules that buckets hide (Slippery, Artifact, Thorns, Hard to Kill): boss-alone `eval`. HP-cliff bosses: re-price with `--seed 1`. Enchants: `--v "x|enchant=ID:ENCH"`.
