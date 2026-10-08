
## Summary
| fight | decisions (forced) | live = his | + ties / immaterial | beyond-noise disagreements -> verdict |
|---|---|---|---|---|
| A Terror Eel (elite) | 32 (0) | 27 (84%) | 29 | #11 expert error, small (1.5 HP-eq) |
| B Gremlin Merc | 16 (2) | 10 (63%) | 15 | #7 unresolved |
| C Skulking Colony (elite) | 21 (4) | 13 (62%) | 19 | #18 **solver gap, decisive** (misses a forced win); #17 tie (R1 artifact) |
| D Phantasmal Gardeners (elite) | 17 (3) | 12 (71%) | 14 | #1 **solver gap** (8.7 HP-eq) |
| E Lagavulin Matriarch T1-3 (boss) | 12 (0) | 9 (75%); win-only objective 8 | 11 | #0 **solver gap** (attacks the sleeping boss) |
| total | 98 | 71 (72%) | 88 (90%) | solver gaps 3, expert errors 1 (small), unresolved 1 |

- On material, resolved disagreements the decisive references side with him 3 times out of 4. The three solver gaps are turns where the live player commits to the wrong thing: a Defend that throws away forced lethal, Outbreak into four attackers on turn 1, an attack that wakes a sleeping boss. His one error is a block card instead of a Strike at 32 HP (1.5 HP-eq).
- R1 (the K=256 search, which plays out the rest of the turn with its rollout policy) cannot adjudicate line comparisons. It scored his forced win at C#17 as -1.03 and ranked every option equal (-1) at C#18. It had the wrong sign at D#1 and overstated A#11 and E#1 by 7-30x. Sixteen seeds give it a small se, but the estimate is biased, and the bias comes from the current-turn rollout, which R2 removes.

## Solver gaps for the near-miss / setup-turn work (E26)
1. **C#18 (forced lethal under a damage cap).** 9 HP vs 12x2; Skulking Colony at 22 HP, Hardened Shell 20/turn, Pen Nib on the 10th attack, Snakebite in hand. Every line that plays S (Pen Nib x2), the shivs and SB wins, because the poison ticks on the enemy's turn after the cap is spent. Every D-first line loses. The live player (2 s) picks D. R1 cannot see the difference (every option scores -1). Exhaustive enumeration solves it in 1 s (141 lines). Candidate rule: when every searched option is near a loss, enumerate the turn exactly (hands this small are cheap). Bench state: `fights/hMrQSndDvPc_F12_SKULKING_COLONY_ELITE.json`, log index 18 (C#17 is the same turn one card earlier).
2. **D#1 (setup turn vs four attackers).** Turn 1, 30 HP, four Phantasmal Gardeners (Skittish), hand HD SB OB D* AB DF D. The live player plays Outbreak (3 energy, all poison) first. His block-first line (DF, Spiral Defend, SB, HD shivs) ends the fight 8.7 HP higher in paired playouts. The live search overrates setup that costs the whole turn while the enemies attack. Bench: `fights/hMrQSndDvPc_F15_PHANTASMAL_GARDENERS_ELITE.json` index 1 (use a simulator seeded to the slot HPs, or fix duplicate-monster sync first; see Caveats).
3. **E#0 (sleeping boss).** Lagavulin Matriarch is asleep (wakes on HP loss or after 3 turns, and is stunned when woken by damage). Under both objectives the live player opens with Dagger Spray instead of NF/N setup. R2 favors his line by +0.16 (0.05). Bench: `fights/hMrQSndDvPc_F17_LAGAVULIN_MATRIARCH_BOSS.json` index 0.
4. Method: line comparisons need R2 (exact current turn) and R3 (full-fight playouts). A 2-turn-leaf search with rollout continuation mis-ranks sequencing-heavy turns (C#17, D#1).

## Re-enactment notes (build matches, so re-enactment is permitted)
- Seed `YMY1KELG18SC`, Silent, A10, v0.111.0. Neow pick: Silken Tress (3rd option). Every fight action is in each record's `log` (bridge action JSON; `times` maps each action to the video). Card rewards, events and rests: macro table above. Not readable: map node coordinates (floor order only), the floor-15 second card reward, and the boss actions from turn 4 on (stream cut).
