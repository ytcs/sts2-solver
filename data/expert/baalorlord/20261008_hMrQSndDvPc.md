# Baalorlord "New Poison Deck!" (A10 Silent): full run vs the live player
- url https://youtu.be/hMrQSndDvPc | uploaded 2026-10-08 | 47 min edited stream | build `v0.111.0 (2026.08.14)` (pinned), `MODDED (2)` (no gameplay effect seen) | seed `YMY1KELG18SC` | won: Queen (F49) dead, 23/77 HP.
- Run record: `hMrQSndDvPc.compact.jsonl` (28 KB): 199 steps, 668 actions (195 macro, 473 combat in 25 fights), actions only. It replays the whole run in the real game (`python -m agent replay`, 2026-10-09: 661/661 logged actions, every fight opening matched, win at 23/77; the 8 F17 bridge actions were played by hand in that run). Fight states come from the game: `tools/expert.py build` writes `replay/<id>/*.json` (git-ignored) from the replay log; the simulator cannot redraw his hands.
- Verdicts: `hMrQSndDvPc.divergences.json`, rows keyed by `k` (compact action index).

## Transcription (2026-10-09)
- Frames 1080p at 2 fps (5644, git-ignored; the yt-dlp `bv*` m3u8 download returned 403, format 299 worked). Act 1 F2-F8 by hand, the rest by five parallel transcribers; merged in game order while the live replay followed.
- Fixed after the live replay: F45 card rewards were reversed (he took Well-Laid Plans before Anticipate+; the opening shuffle runs over deck order, so F48's hands diverged); F12 T3 and F15 T2 hands lacked a card.
- F17 boss: the stream drops for 3 s (15:22-15:25, in-game timer 15:18 -> 17:09) across end of T4, enemy T4, T5, enemy T5. Turn-5 hand by pile accounting; lookahead found 7 orderings in 2 end-of-turn states that all reach the same turn-6 state; rank 1 recorded with `inferred`, verified twice in the game. No other gaps; no other lookahead inferences.
- Random game effects recorded as he got them: F28 Study power (Infinite Blades), F48 Power Potion offer (Fan of Knives); the seed reproduced both.

## Simulator fidelity found (frame builds and game-state replays)
- Happy Flower counter: taken from neither scenario nor sync (energy one off on its turn).
- Kaiser Crab: Surrounded facing is lost on sync; Crab Rage on Crusher's death (Rocket +99 block, +6 Strength) is not simulated.
- `Sim.sync` cannot set a monster's next move (a mid-fight start opens asleep).
- `Sim.sync` with fewer real enemies than simulated detaches the wrong monster (F49: the Amalgam kept, the Queen detached). `agent.fight.Replayer` now re-rolls a play's random effects when the alive enemies differ; on game states all 25 fights replay ok with every end turn matched.
- Afflictions (Chains of Binding: Bound) are in neither snapshot nor sync; the frame builder re-rolls end turns until the next turn's plays apply.
- Random draws/targets/offers (Escape Plan draw, Serpent Form target, Power Potion offer, spawned minion HP) differ until synced.

## Compare (live player r6)
- Live player `Engine()` = `models/current.json` `solver_r6.pt`, cover, exact turn search on; `decide(seed 1, rounds 8, keep_potions)`, harness objective. R1 K=256 x 8 seeds; R2 turn check (K=8 leaves, 2+8 determinizations, <= 1500 leaves); R3 12 paired playouts at 1 round/decision when nothing else resolves. CPU, F45/F48/F49 on the GPU. Decision states: the game's (replay log).
- New verdict guard: a class with a line that wins this turn losing no HP is optimal; a reference ranking the other class above it by more than noise over-values a non-terminal leaf and is dropped (it flipped F19 #0 from "expert error" to tie).
- 473 decisions, 24 forced; of 449: agree 271 (60.4%), tie 103, held (potions) 8 -> 85.1% no disagreement beyond noise; our gap 16, expert error 10, unresolved 41.
- Macro (`price`, real screens; card rewards and rests only, n 32): 38 screens: agree 13, tie 21, price prefers another option 4 (F8 smith Dagger Spray vs rest, F15 Outbreak vs Assassinate, F16 smith Snakebite vs rest, F17 Burst vs Afterimage); per the rule this questions both, decides nothing.
- Pilot gaps (r5-era pilot) under r6 + exact turn search: closed F11 #0, F11 #7, F12 #18 (forced lethal: now tie, exact), F15 #0, F17 #0 (sleeping boss: now agrees), F17 #2. Still open: F15 #1 (Outbreak turn 1 into four attackers: our gap, R3 +0.0524 (0.0066) n 12, 7.3 HP-eq). F09 #11 (pilot "expert error, small") and F17 #1 are unresolved.
- Act 3 holds most verdicts beyond noise (F48 Test Subject: 5 expert errors, 14 unresolved, 2 gaps; F49 Queen 2/5/2): long, high-variance fights with Serpent Form random targets, where R3 at n 12 resolves little.

## Summary
| fight | decisions (forced) | agree | tie | held | our gap | expert error | unresolved |
|---|---|---|---|---|---|---|---|
| hMrQSndDvPc_F02_SLUDGE_SPINNER_WEAK | 25 (5) | 15 | 5 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F03_TOADPOLES_WEAK | 12 (0) | 10 | 2 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F04_SEAPUNK_WEAK | 13 (0) | 9 | 4 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F06_HAUNTED_SHIP_NORMAL | 27 (0) | 23 | 4 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F09_TERROR_EEL_ELITE | 32 (0) | 26 | 2 | 2 | 0 | 0 | 2 |
| hMrQSndDvPc_F11_GREMLIN_MERC_NORMAL | 18 (2) | 8 | 8 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F12_SKULKING_COLONY_ELITE | 25 (4) | 12 | 7 | 0 | 0 | 0 | 2 |
| hMrQSndDvPc_F15_PHANTASMAL_GARDENERS_ELITE | 20 (3) | 10 | 4 | 0 | 1 | 0 | 2 |
| hMrQSndDvPc_F17_LAGAVULIN_MATRIARCH_BOSS | 29 (0) | 23 | 5 | 0 | 0 | 0 | 1 |
| hMrQSndDvPc_F19_BOWLBUGS_WEAK | 2 (0) | 0 | 2 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F21_EXOSKELETONS_WEAK | 4 (0) | 1 | 3 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F22_HUNTER_KILLER_NORMAL | 13 (0) | 10 | 2 | 1 | 0 | 0 | 0 |
| hMrQSndDvPc_F25_ENTOMANCER_ELITE | 17 (0) | 8 | 4 | 1 | 0 | 1 | 3 |
| hMrQSndDvPc_F27_INFESTED_PRISMS_ELITE | 19 (3) | 11 | 2 | 1 | 1 | 0 | 1 |
| hMrQSndDvPc_F30_DECIMILLIPEDE_ELITE | 10 (1) | 3 | 4 | 0 | 2 | 0 | 0 |
| hMrQSndDvPc_F31_OVICOPTER_NORMAL | 11 (0) | 7 | 4 | 0 | 0 | 0 | 0 |
| hMrQSndDvPc_F33_KAISER_CRAB_BOSS | 17 (0) | 9 | 6 | 0 | 0 | 0 | 2 |
| hMrQSndDvPc_F35_DEVOTED_SCULPTOR_WEAK | 5 (0) | 1 | 2 | 0 | 2 | 0 | 0 |
| hMrQSndDvPc_F36_TURRET_OPERATOR_WEAK | 7 (0) | 3 | 2 | 0 | 0 | 0 | 2 |
| hMrQSndDvPc_F38_OWL_MAGISTRATE_NORMAL | 27 (1) | 12 | 3 | 2 | 4 | 1 | 4 |
| hMrQSndDvPc_F39_CONSTRUCT_MENAGERIE_NORMAL | 10 (1) | 4 | 4 | 0 | 0 | 0 | 1 |
| hMrQSndDvPc_F42_SOUL_NEXUS_ELITE | 24 (4) | 13 | 3 | 0 | 2 | 1 | 1 |
| hMrQSndDvPc_F45_MECHA_KNIGHT_ELITE | 28 (0) | 18 | 9 | 0 | 0 | 0 | 1 |
| hMrQSndDvPc_F48_TEST_SUBJECT_BOSS | 50 (0) | 21 | 7 | 1 | 2 | 5 | 14 |
| hMrQSndDvPc_F49_QUEEN_BOSS | 28 (0) | 14 | 5 | 0 | 2 | 2 | 5 |


## Beyond noise (our gap / expert error)
| k | fight | # | t | his | live | R1 | R2 | R3 | verdict (by) |
|---|---|---|---|---|---|---|---|---|---|
| 204 | F15_PHANTASMAL_GARDENERS_ELITE | 1 | 11:43 | DEFEND | OUTBREAK | -0.0268 (0.0061) | +0.0423 (0.0178) | +0.0524 (0.0066) n 12 | **our gap** (r3) |
| 322 | F25_ENTOMANCER_ELITE | 4 | 20:50 | DEFLECT | end turn | -0.0384 (0.0028) | -0.0186 (0.0272) | -0.0345 (0.0134) n 12 | **expert error** (r3) |
| 353 | F27_INFESTED_PRISMS_ELITE | 9 | 23:33 | pick 2 (STRIKE) | pick 1 (DEFLECT) | +0.0132 (0.0089) | +0.0071 (0.0000) |  | **our gap** (r2) |
| 378 | F30_DECIMILLIPEDE_ELITE | 3 | 24:52 | INFINITE_BLADES | STRIKE>e0 | -0.0085 (0.0080) | +0.0123 (0.0046) |  | **our gap** (r2) |
| 379 | F30_DECIMILLIPEDE_ELITE | 4 | 24:53 | DEFEND | STRIKE>e0 | -0.0080 (0.0080) | +0.0123 (0.0046) |  | **our gap** (r2) |
| 435 | F35_DEVOTED_SCULPTOR_WEAK | 0 | 29:12 | BULLET_TIME | OUTBREAK | -0.0430 (0.0015) | +0.0285 (0.0080) | -0.0124 (0.0084) n 12 | **our gap** (r2) |
| 438 | F35_DEVOTED_SCULPTOR_WEAK | 3 | 29:17 | OUTBREAK | end turn | -0.0325 (0.0032) | -0.0419 (0.0052) | +0.0108 (0.0035) n 12 | **our gap** (r3) |
| 459 | F38_OWL_MAGISTRATE_NORMAL | 1 | 30:48 | AFTERIMAGE | BURST | +0.0045 (0.0009) | +0.0219 (0.0032) |  | **our gap** (r2) |
| 460 | F38_OWL_MAGISTRATE_NORMAL | 2 | 30:49 | DASH>e0 | BURST | +0.0055 (0.0023) | -0.0136 (0.0079) | +0.0189 (0.0067) n 12 | **our gap** (r3) |
| 477 | F38_OWL_MAGISTRATE_NORMAL | 19 | 31:47 | ACCELERANT | SNAKEBITE>e0 | +0.0000 (0.0000) | +0.0120 (0.0121) | +0.0308 (0.0057) n 12 | **our gap** (r3) |
| 478 | F38_OWL_MAGISTRATE_NORMAL | 20 | 31:47 | DEFEND | SNAKEBITE>e0 | +0.0000 (0.0000) | +0.0120 (0.0121) | -0.0065 (0.0000) n 12 | **expert error** (r3) |
| 479 | F38_OWL_MAGISTRATE_NORMAL | 21 | 31:48 | SNAKEBITE>e0 | end turn | +0.1183 (0.0002) | -0.0120 (0.0121) | +0.0844 (0.0000) n 12 | **our gap** (r3) |
| 509 | F42_SOUL_NEXUS_ELITE | 1 | 33:55 | BACKFLIP | OUTBREAK | -0.0563 (0.0289) | +0.2756 (0.0324) |  | **our gap** (r2) |
| 513 | F42_SOUL_NEXUS_ELITE | 5 | 34:07 | ESCAPE_PLAN | ACCELERANT | -0.0872 (0.0043) | -0.0922 (0.0334) |  | **expert error** (r2) |
| 517 | F42_SOUL_NEXUS_ELITE | 9 | 34:18 | DASH>e0 | SNAKEBITE>e0 | -0.0165 (0.0033) | -0.0014 (0.0167) | +0.0319 (0.0148) n 12 | **our gap** (r3) |
| 590 | F48_TEST_SUBJECT_BOSS | 1 | 39:08 | OUTBREAK | ANTICIPATE | -0.0001 (0.0010) | -0.1361 (0.0537) |  | **expert error** (r2) |
| 606 | F48_TEST_SUBJECT_BOSS | 17 | 40:09 | ESCAPE_PLAN | ESCAPE_PLAN | +0.0034 (0.0100) | -0.1380 (0.0353) |  | **expert error** (r2) |
| 612 | F48_TEST_SUBJECT_BOSS | 23 | 42:53 | pick 2 (NOXIOUS_FUMES) | pick 0 (FAN_OF_KNIVES) | -0.0018 (0.0176) | -0.2291 (0.0618) |  | **expert error** (r2) |
| 623 | F48_TEST_SUBJECT_BOSS | 34 | 44:16 | STRIKE>e0 | CALCULATED_GAMBLE | -0.0501 (0.0203) | +0.0819 (0.0189) | -0.0352 (0.0169) n 12 | **expert error** (r3) |
| 625 | F48_TEST_SUBJECT_BOSS | 36 | 44:17 | pick 4 (CALCULATED_GAMBLE) | pick 2 (BULLET_TIME) | -0.1345 (0.0545) | +0.0751 (0.0326) | -0.0639 (0.0170) n 12 | **expert error** (r3) |
| 626 | F48_TEST_SUBJECT_BOSS | 37 | 44:18 | SHIV | end turn | -0.0722 (0.0221) | +0.0102 (0.0018) | +0.0438 (0.0081) n 12 | **our gap** (r3) |
| 627 | F48_TEST_SUBJECT_BOSS | 38 | 44:19 | SHIV | end turn | -0.0091 (0.0169) | +0.0047 (0.0019) | +0.0482 (0.0058) n 12 | **our gap** (r3) |
| 640 | F49_QUEEN_BOSS | 0 | 44:58 | BACKFLIP | BULLET_TIME | -0.6210 (0.0137) |  | +0.0595 (0.0213) n 12 | **our gap** (r3) |
| 657 | F49_QUEEN_BOSS | 17 | 45:58 | CALCULATED_GAMBLE | DEFLECT | -0.0164 (0.0043) | -0.1051 (0.0108) |  | **expert error** (r2) |
| 659 | F49_QUEEN_BOSS | 19 | 46:02 | ESCAPE_PLAN | WELL_LAID_PLANS | -0.0295 (0.0065) | +0.0923 (0.0071) | +0.0276 (0.0108) n 12 | **our gap** (r3) |
| 660 | F49_QUEEN_BOSS | 20 | 46:08 | STRANGLE>e0 | WELL_LAID_PLANS | -0.0303 (0.0022) | -0.0888 (0.0092) |  | **expert error** (r2) |

Unresolved (no reference resolves 1 HP-eq, or R2 and R1 disagree in sign): k114 k115 k171 k172 k211 k215 k234 k321 k324 k325 k351 k417 k418 k446 k447 k458 k461 k473 k474 k491 k508 k553 k595 k600 k601 k607 k608 k610 k613 k614 k616 k617 k618 k621 k622 k624 k642 k650 k652 k653 k655.

## Macro vs `price`
| k | floor | screen | played | price best | horizon | d (se) | verdict |
|---|---|---|---|---|---|---|---|
| 30 | 2 | CARD_REWARD | Dagger Spray | Prepared | act | -0.0625 (0.0435) n 32 | tie |
| 46 | 3 | CARD_REWARD | Dash | Dash | act |  | agree |
| 63 | 4 | CARD_REWARD | Hidden Daggers | Hidden Daggers | act |  | agree |
| 97 | 6 | CARD_REWARD | Snakebite | Expose | act | -0.0938 (0.0524) n 32 | tie |
| 101 | 8 | RESTSITE | Smith | rest | act | -0.2188 (0.0742) n 32 | price prefers another option |
| 139 | 9 | CARD_REWARD | Strangle | Strangle | act |  | agree |
| 164 | 11 | CARD_REWARD | Noxious Fumes | Flick-Flack | act | -0.0625 (0.0435) n 32 | tie |
| 194 | 12 | CARD_REWARD | Outbreak | Outbreak | act |  | agree |
| 196 | 12 | CARD_REWARD | Deflect | Deflect | act |  | agree |
| 198 | 13 | RESTSITE | Rest | rest | act |  | agree |
| 227 | 15 | CARD_REWARD | Outbreak | Assassinate | act | -0.1250 (0.0594) n 32 | price prefers another option |
| 230 | 16 | RESTSITE | Smith | rest | act | -0.1250 (0.0594) n 32 | price prefers another option |
| 265 | 17 | CARD_REWARD | Burst | Afterimage | ready | -0.0756 (0.0089) n 32 | price prefers another option |
| 274 | 19 | CARD_REWARD | Anticipate+ | Expertise | act | -0.0625 (0.0770) n 32 | tie |
| 289 | 21 | CARD_REWARD | Footwork | skip | act | -0.0312 (0.0312) n 32 | tie |
| 309 | 22 | CARD_REWARD | Skip | skip | act |  | agree |
| 315 | 24 | RESTSITE | Smith | smith OUTBREAK | act |  | agree |
| 338 | 25 | CARD_REWARD | Bullet Time | skip | act | +0.0000 (0.0449) n 32 | tie |
| 340 | 25 | CARD_REWARD | Backflip | Dagger Throw | act | -0.0938 (0.0524) n 32 | tie |
| 366 | 27 | CARD_REWARD | Skip | Cloak and Dagger | act | -0.0312 (0.0312) n 32 | tie |
| 368 | 27 | CARD_REWARD | Outbreak | skip | act | +0.0000 (0.0449) n 32 | tie |
| 373 | 29 | RESTSITE | Rest | rest | act |  | agree |
| 389 | 30 | CARD_REWARD | Accelerant | Expose+ | act | +0.0000 (0.0449) n 32 | tie |
| 391 | 30 | CARD_REWARD | Skip | The Hunt+ | act | -0.0312 (0.0312) n 32 | tie |
| 407 | 31 | CARD_REWARD | Skip | Skewer+ | act | -0.0312 (0.0312) n 32 | tie |
| 410 | 32 | RESTSITE | Smith | smith OUTBREAK | win |  | agree |
| 432 | 33 | CARD_REWARD | Afterimage | Afterimage | act |  | agree |
| 443 | 35 | CARD_REWARD | Escape Plan+ | Anticipate | act | -0.0312 (0.0312) n 32 | tie |
| 454 | 36 | CARD_REWARD | Skip | Dagger Throw+ | act | -0.0312 (0.0312) n 32 | tie |
| 487 | 38 | CARD_REWARD | Skip | Dagger Throw+ | act | -0.0312 (0.0312) n 32 | tie |
| 501 | 39 | CARD_REWARD | Dash+ | Snakebite+ | act | +0.0000 (0.0000) n 32 | tie |
| 503 | 40 | RESTSITE | Smith | rest | act | +0.0000 (0.0000) n 32 | tie |
| 535 | 42 | CARD_REWARD | Escape Plan | Predator+ | act | +0.0000 (0.0000) n 32 | tie |
| 537 | 42 | CARD_REWARD | Serpent Form | Serpent Form | act |  | agree |
| 543 | 44 | RESTSITE | Rest | rest | act |  | agree |
| 577 | 45 | CARD_REWARD | Well-Laid Plans | Envenom | act | -0.0625 (0.0435) n 32 | tie |
| 579 | 45 | CARD_REWARD | Anticipate+ | Deadly Poison | act | +0.0000 (0.0000) n 32 | tie |
| 586 | 47 | RESTSITE | Smith | rest | act | -0.0312 (0.0312) n 32 | tie |
