---
name: sts2-pathing
description: Use on every map fork, Neow / ancient choice and rest decision: the route procedure and rules R# (elites, rests, shops, forced lanes, Neow), with the tools that price them.
---

# Pathing, rests, Neow / ancients (all characters and acts)
Neow / ancient option and path are one decision. Binding limits: HP, gold, deck slots, potion slots: find the binding one, spend the others.

## Procedure
1. `m`; keep 3-5 paths that differ in kind (elite-heavy, safe, shop-first, rest-late). Later acts' maps are unknown until shown.
2. `routes` at every fork (guards refuse a fork without it); `restcalc` at rests; combat options of Neow via `eval`.
3. `draw` the route; re-plan each node.
4. `a <i> -- rules: R1,P5 ; numbers: <routes P(boss) per option> ; judgment: ...`.

## Rules
- R1 [hyp] Take an elite while `routes` keeps this act's boss win >= 0.9 after it; the elite pays through relic and rare odds (P5). Test: outcome: elites taken vs act clear, ledger.
- R2 [sim] Avoid lanes that force an elite the deck cannot win: forced elites drove the simulator's act-2 deaths (E93, E94). Test: instrument: `routes` elite arrival HP and P(win) on the forced elite.
- R3 [sim] Rest when HP binds the next binding fight; else smith (HP saved early is mostly refunded by later heals, E85). Test: instrument: `restcalc` boss win now vs after rest.
- R4 [hyp] Never two elites without a rest between (the second arrival below ~50% HP means a missing rest). Test: instrument: `routes` q10 arrival HP.
- R5 [hyp] Shop only when the gold buys something priced worth it (D5); else a fight or an event. Test: outcome: shop visits with no purchase, ledger.
- R6 [hyp] Prefer lanes that keep a fork in the next 2-3 nodes; commit late. Test: outcome.
- R7 [hyp] Neow / ancient: the option that moves the final pair or the route budget most; a curse costs a slot. Test: instrument: `eval` of the option's effect.
- R8 [hyp] Steep boss curve (`eval --boss --hp 34/45/60/80`) = HP-gated: rests, avoid late elites; flat and low = damage-gated: elites and damage picks. Test: instrument: the curve itself.
