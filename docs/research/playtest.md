# Playtest: what the operator is missing

A Silent A10 run played from the screen, ignoring the old calculators (the decision guards were turned off with `STS2_DECISION_GUARDS=off`). Each note records the calculation the operator wanted at that decision, and the friction it met.

## Neow (F1): Lead Paperweight / Winged Boots / Precarious Shears
- **Wanted:** each option priced in one unit, the value of the run from here.
  - Lead Paperweight: the expected value of picking the better of 2 random colorless cards.
  - Winged Boots: how much 3 path-ignores are worth on this map, which needs a route model with "jump" moves.
  - Shears: two removals vs -16 HP, where the HP partly comes back at the Act 2 ancient (80% of missing HP).
- **Had:** none of these. Chose by feel.
- **Need:** a run-value model where deck changes, HP, map flexibility and relics are all priced as V.

## Combat F2 (Fuzzy Wurm Crawler)
- Bug: `SIMULATOR CHOICE DIFFERS` on Survivor's discard. The game shows "Defend" and the simulator says "DEFEND_SILENT", so the comparison checks names against ids. The fidelity check raised a false alarm and the pick had to be made by hand.
- The predictor said win 1.0, q90 loss 4 HP. Fine.

## Card reward F2: Echoing Slash / Hidden Daggers / Flechettes
- The REWARDS screen shows only card names; the text appears only after opening the card reward. To compare options the operator needs the text and the numbers up front.
- **Wanted:** V(deck + card) - V(deck) over this act's remaining fights (the known boss The Kin, the elites still possible, the hallways), plus later acts. Each value needs a fight-start predictor for the new deck, averaged over opening hands.

## Shop F4 (123 gold)
- The decision is a basket under a budget: removal 100, or Predator 49 + Backflip 51, or Poisoned Stab + ..., with potions at about 50. A relic (Lantern 152) is out of reach.
- **Wanted:** V(deck after the basket) for each affordable basket, plus the value of gold held for the next shops. Needs the run model (when the next shop comes, and its stock distribution from `MerchantInventory`).

## Elite F7 (Byrdonis), 53/70 HP, belt Strength + Attack potion
- Predicted win 0.98 with no potions, end HP about 18. The actual result was 16, so the predictor was right.
- The potion output gave three different numbers: throw-now vs never (+7.6 HP), spend vs keep in P(win act boss) (0.18 vs 0.54), and "this fight with it". The "spend 0.18" figure was unreadable: why would spending it cut boss P(win) by 0.36? I kept both, lost 37 HP, and then reached 16/70, a state where a single hallway fight could kill.
- **What was missing:** one number, V(after this fight | use) vs V(after this fight | keep), that counts the HP lost against the route ahead (rests, the elites on the path, the act-2 heal at the ancient).

## Events (Wellspring, Sapphire Seed, Dense Vegetation)
- Each needed the game source to price:
  - Guilty curse: unplayable, leaves after 5 combats (`Guilty.cs`).
  - Dense Vegetation "Rest": heal 30%, then fight 4 Wrigglers (`DenseVegetationEventEncounter`).
- With the predictor, an event that leads into a fight is just a fight-start prediction at the healed HP. The simulator already ports event encounters.
- **Need:** an event catalog generated from the decomp: options, effects, encounters, random ranges (Dense Vegetation gold 61-99).

## Combat friction
- Dagger Throw / Survivor discard prompts: the simulator's hand differs after a mid-card draw (it drew a random card, the game drew another). The hand has to be re-synced from the screen before a mid-card selection is answered. The name-vs-id comparison bug is fixed in `agent/live.py`.
- Wrigglers and Fogmog: fine.

## Rest F16 before The Kin (52/70)
- The predictor gave: rest (70 HP) win 0.996, smith at 52 HP win 0.99 for each candidate, HP lost -4 points per upgrade. The ancient heals 80% of missing HP, so the HP left after the boss matters little, and I smithed.
- This was the right shape of decision: P(win this fight), then the run value of what carries over. It should be automatic.

## Boss (The Kin), 52/70
- The fight-start prediction (0.99) was made with the belt available. After a bad turn 1 (52 → 38, Frail), win without potions was 0.62-0.80, so the potions were needed after all.
- Two lessons:
  - A fight-start number has to say which potions it assumes.
  - The live per-turn re-prediction with use / keep is exactly the proposal flow the user asked for.
- Committing both potions won the fight at 27/70.

## Act 2 ancient (Darv): Velvet Choker / Snecko Eye / Ectoplasm, at 61/70 (healed from 27: 80% of missing)
- Combat side from the predictor:
  - Act 2 elites: base 0.30; Choker 0.88, Ectoplasm 0.89, Snecko 0.83.
  - Knowledge Demon (Act 2 boss): 0.00 for every option (Snecko 0.03).
- Economy side (Ectoplasm: no more gold) had to be weighed by feel.
- **Key finding, sparse value:** under h128 search this deck never beat the act boss (399 HP, heals, Strength, curses). That is a lower bound under this solver, not proof the deck can't win. So P(win boss) is 0 for every card, relic and route option. Greedy pricing on P(win) has no gradient far from the threshold, and that is exactly when the deck most needs to be steered.
  - The predictor should also output the shape of a loss: enemy HP remaining at death, turns survived, damage dealt.
  - Then "how far from beating the boss" becomes a smooth target for macro shaping, alongside P(win).
  - The old "smooth" objective (win averaged over 1-3x HP) was an indirect attempt at this.
- Also needed: what each boss asks for (damage per turn needed, scaling, curse handling), derived from the source and the simulator, so Act 2 picks can be aimed at it.

## Run 2 (Defect A10, `price`-driven, predictor_r1)
- Neow: ancient options aren't priced (no pickup effects catalogued); an agent is building `data/ancient_relics.json`.
- Map F1: `price` gave distinct P(clear act) per option (0.84 / 0.69 / 0.65 / 0.52, paired se ~0.05); P(win run) is 0 for every option under the base policy.
- Card F2: the four options were within 1.5 paired se at 128 rollouts. Card effects need ~512 rollouts per option (~2 min for 4 options).
- Fidelity bug (F3, slimes): Lightning orbs hit random enemies (hidden RNG), so the simulator's sample killed a different slime than the game. After the deaths, `Sim.sync` matched enemies by list position, giving `SIMULATOR DIFFERS: .enemies[0].id LEAF_SLIME_S vs TWIG_SLIME_M`, and the solver stalled. Fix: match enemies by identity and HP.
- The old potion alerts stop hallway fights repeatedly (S4 removes them).
