---
name: sts2-pathing
description: Use on every map fork, Neow / ancient choice and rest decision: the route procedure and rules R# (elites, rests, shops, forced lanes, Neow), with the tools that price them.
---

# Pathing, rests, Neow / ancients (all characters and acts)
Neow / ancient option and path are one decision. Limits: HP, gold, deck slots, potion slots: find the binding one, spend the others.

## Procedure
1. `m`; keep 3-5 paths that differ in kind (elite-heavy, safe, shop-first, rest-late). Later acts' maps are unknown until shown.
2. `routes` at every fork; `restcalc` at rests; Neow combat options via `eval`.
3. `draw` the route; re-plan each node.
4. `a <i> -- rules: R1,P5 ; numbers: <routes P(boss) per option> ; judgment: ...`.

## Rules
- R1 [hyp] Take an elite when its lane costs <= 0.02 of `routes` boss win, or while the boss stays >= 0.9 after it (the DP omits the elite's rewards, P5: its cost is an upper bound). Test: instrument: the elite lane is the "with elite rewards" best k, or within 0.02 of it; outcome: elites taken vs act clear and pair strength, ledger.
- R3 [sim] Rest when HP binds the next binding fight; else smith (E85). Test: instrument: `restcalc` boss win now vs after rest.
- R4 [sim] Never two elites without a rest between (second arrival below ~50% HP = a missing rest) (E145). Test: instrument: `routes` q10 arrival HP.
- R5 [hyp] Shop only when the gold buys something priced worth it (D5); else a fight or an event. Test: outcome: shop visits with no purchase, ledger.
- R7 [hyp] Neow / ancient: the option that moves the final pair or the route budget most; a curse costs a slot. Test: instrument: `eval` of the option's effect.
- R8 [hyp] Steep boss curve (`eval --boss --hp 34/45/60/80`) = HP-gated: rests, avoid late elites; flat and low = damage-gated: elites and damage picks. Test: instrument: the curve itself.
