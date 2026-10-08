# Baalorlord "New Poison Deck!" (A10 Silent): Act 1 fight lines vs the solver
- url: https://youtu.be/hMrQSndDvPc | uploaded 2026-10-08 | 47 min edited stream | Silent A10 (double-boss badge) | run won (final boss dead @46:30).
- **build: `v0.111.0 (2026.08.14)` = our pinned build** (stamp top right of every gameplay frame) -> real-game re-enactment on the seed is permitted. Stamp also shows `MODDED (2)` (mods not identified; nothing visible changes gameplay, unverified) and `HASH [1568834832]`.
- **run seed: `YMY1KELG18SC`** (read at 1080p; all characters are in the game's seed alphabet `0-9A-HJ-NP-Z`, so no I/O ambiguity; 8/B read clearly).
- Act 1 = Underdocks (records use `act: 0`, the run's act index, as live records do; the simulator does not read `act`); boss Lagavulin Matriarch.
- source: frames at 1080p60 + YouTube auto-transcript (yt-dlp subtitle fetch got HTTP 429; fetched with `youtube-transcript-api`). Git-ignored, local: `transcripts/20261008_hMrQSndDvPc.txt` (30 s paragraphs), `frames/hMrQSndDvPc/<mmss>.jpg` (122 stills, one per decision used). Video deleted.
- caption fixes: "Tess/truss" = Silken Tress, "cal gamble" = Calculated Gamble, "penib/pen nib" = Pen Nib, "panagramraph/pentagramraph" = Pantograph, "Yumes Humes" = Noxious Fumes, "lag of / Lagulan" = Lagavulin Matriarch, "gardenals/gardener reels" = Phantasmal Gardeners, "coral thing" = Skulking Colony, "decks potion" = Dexterity Potion, "ship potion" = Cunning Potion (3 Shiv+), "terror" = Terror Eel.
- fight records (committed): `fights/hMrQSndDvPc_F<floor>_<ENCOUNTER>.json`, harness schema (`id, encounter, hp_start, hp_end, scenario, fight{scenario, log, states, state}`) plus `source`, `times` (video time per logged action), `observed` (which fields each state took from frames), `replay_check`. `scenario` carries `run_seed`, `game_build`, `build_hash`, `modded`, `video`, `video_span` (the simulator accepts the extra keys).

## Method (what each number is)
- **State reconstruction.** Per decision I read HP, block, energy, powers, hand (order and upgrades), shown piles, enemy HP/block/powers/intents, relic counters (Pen Nib), potions. A builder starts `sts2.Sim` on the scenario with a seed whose opening intents (and, for duplicate monsters, HPs) match the frame, applies his logged actions, and syncs each observed state (`Sim.sync`); end turns are re-rolled over determinizations until the next turn's shown intents match. The draw pile is the simulator's remainder (sorted contents synced where the video opened the pile). `agent.fight.Replayer` replays every record (`replay_check`).
- **Live player** = `agent.engine.Engine()` (models/current.json at run time: policy `solver_gen2.pt`, adopted in 548826f; all R1-R3 searches and playouts use the same network; M=5 K=32, cover) `decide(budget 2 s, tol_hp 0.5, keep_potions=True)` on CPU. Differences from the harness: fixed 2 s instead of the danger-scaled budget, CPU, linear objective in all fights (the harness uses win-only for an act-1 boss; fight E was re-run with it, below). Potions are held by design (operator commits them), so his potion throws are "held" rows, not disagreements.
- **R1: K=256 search** (`FastSearch`, cover, 16 seeds, common random numbers per seed): q per action class (identical cards merged). his-live = paired mean over seeds, se over seeds. Weakness found here: the rest of the current turn after the candidate is played by the rollout policy, so a line that needs exact sequencing can score as a loss (fight C T4).
- **R2: turn-exhaustive check** (`turncheck`): enumerates every distinct line to end of turn (choices included), values each end-of-turn state by terminal utility or by a K=8 search at the next decision (2 determinizations to select the best line per first action, 8 fresh determinizations to re-value it; paired se vs his line). Exact on current-turn sequencing; leaf values are a weak search.
- **R3: paired full-fight playouts** (`playout`): his action vs the live player's, then the live player (0.3 s/decision for A, 0.15 s for D/E; potions free) finishes the fight; paired determinization seeds (n per row: 24 / 16 / 12, limited by CPU on a shared box).
- units: q = win (+1 + 0.5 x end HP/max HP) or loss (-1). 1 HP at 70 max = 0.0071 q. **Verdict rule:** a disagreement is called only when the decisive reference shows |gap| > 2 se **and** >= 1 HP-eq (0.0071); Which reference decides: an exact (terminal) enumeration overrides everything; otherwise R3 when it was run with se below ~0.005 (resolves 1 HP-eq); otherwise R2; R1 never decides alone. "tie (order only)" = exactly equal q under common random numbers (commuting card orders reach the same end-of-turn state).

## Act 1 macro (transcribed; floor = in-game floor counter)
| floor | room | state | options seen | his pick (@time, reason) |
|---|---|---|---|---|
| 0 | Neow | 56/70, 99 g | Lead Paperweight / Booming Conch / Silken Tress | Silken Tress (@0:31; lose all gold, first card reward Glam) |
| 1 | Sludge Spinner (weak) | 56 -> 56 | reward: Dagger Spray / Calculated Gamble / Prepared (all Glam) | Dagger Spray (@1:24 "I suppose dagger spray will suffice") |
| 2 | Toadpoles (weak) | 56 -> 52 | Sucker Punch / Dash / Acrobatics | Dash |
| 3 | Seapunk | 52 -> 52 | Predator / Dodge and Roll / Hidden Daggers | Hidden Daggers ("good right now") |
| 4 | ? event This or That | 52 | lose 6 HP +51 g / Clumsy + random relic | That: Clumsy + Centennial Puzzle |
| 5 | Haunted Ship | 52 -> 35 | Expose / Blade Dance / Snakebite | Snakebite ("for our boss we kind of have to") |
| 6 | treasure | 35 | Pen Nib | - |
| 7 | rest | 35/70 | rest / smith | smith Dagger Spray -> + ("against my own better judgment") |
| 9 | **elite Terror Eel** (fight A) | 35 -> 18 | 30 g, White Star; Strangle / Storm of Steel / Flick-Flack | Strangle |
| 10 | treasure | 18 | Pantograph | - |
| 11 | Gremlin Merc (fight B) | 18 -> 17 | 10 g + 20 g stolen back; Noxious Fumes / Deflect / Flick-Flack | Noxious Fumes |
| 12 | **elite Skulking Colony** (fight C) | 17 -> 9 | 26 g, Lasting Candy; Abrasive / Outbreak / Burst; Tactician / Grand Finale / Deflect | Outbreak; Deflect |
| 13 | rest | 9 -> 30 | rest / smith | rest (+21) |
| 14 | ? Spiraling Whirlpool | 30 | Observe (Spiral on a Strike/Defend) / Drink (heal 23) | Observe -> Defend (Spiral) ("I have pantograph coming") |
| 15 | **elite Phantasmal Gardeners** (fight D) | 30 -> 28 | 32 g, Energy Potion, Festive Popper; Corrosive Wave / Assassinate / Outbreak | Outbreak (2nd). A second card reward (he names Noxious Fumes / Echoing Slash) was skipped [unverified: options not on a readable frame] |
| 16 | rest | 28/70 | rest / smith | smith Snakebite -> + (previewed Outbreak+) |
| 17 | **boss Lagavulin Matriarch** (fight E) | 28 -> 53 (Pantograph +25) | - | won @15:21 (video cut inside turn 4) |
Not transcribed: map node positions (floor sequence only), shop/potion choices outside fights, gold spent. Path after the boss (Acts 2-3) not in scope.

## Fights
Card codes: S Strike, D Defend (D* = Spiral Defend), N Neutralize, SV Survivor, AB Ascender's Bane, DS+ Dagger Spray+ (Glam: replays once per combat), DA Dash, HD Hidden Daggers, CL Clumsy, SB Snakebite, ST Strangle, NF Noxious Fumes, OB Outbreak, DF Deflect, SH Shiv. Relics everywhere: Ring of the Snake, Silken Tress (used), Centennial Puzzle, Pen Nib (+ White Star, Pantograph from fight B; + Lasting Candy from D; + Festive Popper in E).

### A. F9 elite Terror Eel (5:08-7:06) - his line wins at 18/70
- state T1: 35/70, 3 E, hand N AB S D D DA D, draw 11; deck 19 (S5 D5 N SV AB DS+ DA HD CL SB); Pen Nib 0; potions Dexterity, Cunning. Terror Eel 150/150, Shriek 75 (first time HP <= 75: stunned), intent 18.
- his line: T1 Dex potion, N, DA (12 block), S, end (35->34) | T2 SB, SV (discard S), end (->32) | T3 eel 124, poison 6, vigor 6, intent 24: DS+ (Glam replay, ->100), S (94), SV (discard S, 10 block), HD (hand empty), SH, SH (86), end (->18) | T4 N, DA, S (Pen Nib 10th, x2) (80->51), end | T5 SB, end (eel Debuff turn) | T6 Cunning potion (3 Shiv+), SH+ x3, S, S, HD, SH, SH: dead.
- his words: "Take one on purpose." (T1, 5:10); "if you stun now, you get hit by the 36 damage. So, I don't want to do that now." (T3-T4, 6:00); "We only need to do 25. I get there with a potion." (T6, 6:36).
- live player: same line on 27/32 decisions. Disagreements:

| # | t | his | live | R1 his-live (se) | R2 (turn-exhaustive) | R3 playouts his-live (se) | verdict |
|---|---|---|---|---|---|---|---|
| 0 | 5:20 | Dex potion | N | 0 (order only) | - | - | held by design; tie (N and potion commute) |
| 5 | 5:36 | SB | SV | -0.0145 (0.0077) | tie: SB-first = SV-first 1.0367 | - | tie |
| 10 | 5:53 | S | SV | +0.0066 (0.0181) | - | - | within noise |
| 11 | 5:57 | SV (8 block) | S | **-0.0979 (0.0027)** | +0.013 (0.023) for live, noise | -0.0110 (0.0053), n 24; both win 100%, end HP 15.3 vs 16.8 | **expert error, small**: R3 1.5 HP-eq at 2.1 se; R1 overstates it ~9x; R2 inconclusive. Low-moderate confidence |
| 23 | 6:58 | Cunning potion | SV | +0.0045 (0.0045) | - | - | held by design |

### B. F11 Gremlin Merc (7:29-8:19) - 18 -> 17/70
- state T1: 18/70, hand D D S SB D D HD, draw 12; deck 20 (+ST); White Star, Pantograph; Pen Nib 8; no potions. Gremlin Merc 53/53 (Surprise 1, Thievery 20), intent 8x2.
- his line: T1 D, D, D (15 block), HD (discard S, D), SH (49), end (->17) | T2 (Centennial Puzzle drew 3) ST (33), N (28), DS+ (the merc is replaced by Sneaky Gremlin 12/12 + Fat Gremlin 18/18, both hit by the replay), S>Fat Gremlin, end | T3 DA>Fat (12->2), S>Fat, end | T4 SH, S, S: win.
- his words: "Only going to play one shiv. I don't want to pen nib this shiv. A nib strangle." (7:39) - he spent the Pen Nib 10th attack on Strangle by holding the 2nd Shiv.
- live player: 10/16 agree (2 forced). Disagreements: #7 (T2 first card) ST vs DS+: R1 +0.0030 (0.0009) for his, R2 **+0.0170 (0.0036) for live** (best DS+-first line: SV discard S, DS+, N, D), R3 not run: on a fresh 2 s decision the live player chose ST, i.e. agreed with him -> **unresolved**: R1 and R2 disagree in sign, and the live player's own choice flips between runs; no call. #8, #9, #10, #13, #15 (merc already split, fight decided): |R1| <= 0.0009 q (< 0.15 HP) -> immaterial.

### C. F12 elite Skulking Colony (8:39-10:03) - 17 -> 9/70, poison kills it on its turn
- state T1: 17/70, hand ST S SV S S DA HD, draw 13; deck 21 (+NF); Pen Nib 9; no potions. Skulking Colony 80/80, Hardened Shell 20 (takes at most 20 per turn), intent 16.
- his line: T1 DA (60, 10 block), HD (discard S, S), SV (discard S, 18 block), SH, SH (shell cap), end (17->17) | T2 N, S, S (45), D, end (->10) | T3 DS+ (25), D, D, end (->9) | T4 intent 12x2 at 9 HP: N (22), HD (discard D, S), SH (18), S (Pen Nib x2, 6), SH (5, cap reached), SB (7 poison), end: poison kills it at its turn start.
- his words: "I think that draw kills me. Although, wait, I can do more damage, right?" (8:47); "the poison hits on its turn, not on my turn. So, we win thanks to snake bite." (9:50).
- live player: 13/21 agree (4 forced), 4 more are order-only ties (#8-10: N S S D all get played either way; #21). Disagreements:

| # | t | his | live | R1 his-live (se) | R2 (turn-exhaustive) | verdict |
|---|---|---|---|---|---|---|
| 1 | 8:50 | HD | SV | -0.0852 (0.0582) | tie: HD-first = SV-first 0.0519 | tie |
| 2 | 8:53 | discard S | discard ST | -0.0414 (0.0414) | - | within noise |
| 17 | 9:31 | N | SB | **-1.0321 (0.2665)** | exact: N-, S-, SB-first all have a forced win (1.0643); D-first / end turn lose | **tie; R1 artifact** (rollout misses the forced line after N) |
| 18 | 9:48 | HD (after N) | **D** | 0 (all scored -1) | exact: every D-first line loses (-1.0); HD/S/SB-first win (1.0643) | **solver gap, decisive** (2.06 q): the live player plays a losing card in a forced-win position; R1 cannot rank it (all options -1) |

### D. F15 elite Phantasmal Gardeners (11:07-13:41) - 30 -> 28/70
- state T1: 30/70, hand HD SB OB D AB DF D (one Defend is Spiral: replay once), draw 15; deck 23 (+OB, DF, Spiral D); Pen Nib 1; Lasting Candy; no potions. 4 Phantasmal Gardeners 29/31/30/32, Skittish 7 each (block 7 when first hit each turn), intents 1x3 / 5 / 7 / Buff.
- his line: T1 DF (4), D* (Spiral, 14 block), SB>Buff gardener, HD (discard OB, D), SH>e3, SH>e0, end (30->29) | T2 ST>e3, DA>e3 (8), N>e0, end (->29) | T3 D, D, S>e0, end (->28) | T4 S>e2, DS+ (Glam: two die, one at 2), end | T5 S: win.
- his words: "Question is, do I just play Outbreak on turn one?" / "I think I'm going to play snake bite instead." (11:26); "we still have taken a lot less damage than we would have if I played outbreak. So I feel very much correct in not having played outbreak." (12:44-13:14).
- live player: 12/17 agree (3 forced). Disagreements:

| # | t | his | live | R1 his-live (se) | R2 (turn-exhaustive) | R3 playouts | verdict |
|---|---|---|---|---|---|---|---|
| 0 | 11:13 | DF | HD | +0.0017 (0.0025) | - | - | within noise |
| 1 | 11:40 | D* (no Outbreak T1) | **OB** | -0.0183 (0.0064) | **his +0.0387 (0.0181)** over the best OB-first line (2651 lines, 1202 leaves) | **his +0.0620 (0.0048)**, n 16; both win, end HP 27.7 vs 19.0 | **solver gap**: R2 and R3 agree (R3 8.7 HP-eq, 13 se); R1 had the sign wrong. High confidence; matches his stated reason |
| 8 | 12:26 | ST>e3 | N>e0 | -0.0105 (0.0156) | - | - | within noise |
| 12 | 12:47 | D | NF | 0 (order only) | - | - | tie |
| 16 | 13:32 | S>e2 | DS+ | -0.0082 (0.0008) | live +0.0035 (0.0013) (0.5 HP) | not run | immaterial (< 1 HP-eq by R2) |

### E. F17 boss Lagavulin Matriarch, turns 1-3 (14:30-15:08; video cut in T4; won @15:21)
- state T1: 53/70 (28 + Pantograph 25), hand S D N D NF S DS+, draw 16; deck 24 (+2nd OB, SB upgraded); Pen Nib 1; Festive Popper (fired at combat start), Energy Potion. Matriarch 233/233, block 3 (Plating 12), Asleep 3 (wakes when it loses HP or after 3 turns), intent Sleep.
- his line: T1 NF, N, end | T2 SB+ (10 poison), S, end (poison wakes it at its turn: stunned) | T3 intent 21: OB (9 poison, triggers), HD (discard S, OB), SH, SH, end (53->32).
- his words: "Would have liked to retain the snake bite" (14:46); "if I can't really play any block cards anyway, then I'm very happy to just play outbreak." (14:46-15:21).
- live player (linear objective): 9/12 agree. Disagreements:

| # | t | his | live | R1 his-live (se) | R2 (turn-exhaustive) | R3 playouts | verdict |
|---|---|---|---|---|---|---|---|
| 0 | 14:37 | NF | **DS+** | -0.0058 (0.0040) | **his +0.1607 (0.0506)** over the best DS+-first line (attacking wakes the boss early) | his +0.185 (0.178), n 12; win 92% vs 83%, end HP 19.3 vs 16.7 | **solver gap on R2 alone** (3.2 se) plus the win-only rerun (DS+ at #0, #1, #2); R3 under-powered (n 12, 1.0 se, same sign). Moderate confidence |
| 1 | 14:38 | N | end turn | **+0.0482 (0.0115)** | his +0.0018 (0.0003) (0.25 HP) | not run | immaterial by R2 (0.25 HP-eq; R1's 6.7 HP-eq not confirmed) |
| 3 | 14:49 | SB+ | S | -0.0003 (0.0002) | tie: SB+-first = S-first | - | tie (order only) |
- live player with the harness boss objective (win only): `proposal.fight_objective` -> win only. The live player then plays **DS+ at #0, #1 and #2** (attacks the sleeping boss on turn 1 whatever he has already played) and S before SB+ at #3 (order only); 8/12 agree. Same finding as #0.

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

## Caveats
- Hidden information: draw order (simulator remainder; sorted contents only where the pile was opened: A T3, B T2, D T3, E T2), Survivor/Hidden Daggers discard targets were read from the selection screen (all readable except where noted), enemy move RNG (end turns re-rolled to the shown intent).
- Reconstruction fidelity: start states matched exactly (enemy ids, HP, intents) in all five fights; Pen Nib counter synced from the relic digit each turn. Sim-vs-frame diffs before sync (hand contents excluded, they differ by draw RNG and are synced): only B T2, where the simulator spawned Sneaky Gremlin at 15/15 vs 12/12 shown (synced to the frame), and D T3, draw-pile count 14 vs 15 shown. A, C, E: none beyond the relic counters set at the start.
- Replayer finding (separate from the evaluation): `Sim.sync`/`sync_enemies` pairs same-id monsters by nearest HP, so on the 4x Phantasmal Gardener record the Replayer attaches moves to the wrong slots (4 unmatched end turns, ~200 intent residuals). `agent.hindsight` on that record would be wrong; my evaluation used the builder's simulator (seeded so ids, HPs and intents match slot by slot). Also `sync_hand` ignores enchantments (Spiral Defend residuals in D/E).
- Fight E covers turns 1-3 only (stream cut inside turn 4, "stream got interrupted"; actions after the cut not transcribed).
- MODDED (2): mods unidentified; if they changed rules, states would diverge in re-enactment (none of the reconstructed states needed an unexplained correction).
- Live-player column is a 2 s CPU proxy for the harness player (see Method); R1-R3 are lower bounds on line values, not verdicts on plans (claims discipline).

## Appendix: every decision (live player vs his action; R1 his-live where they differ; R1 best)
Columns: log index | video time | his action (p play, c choose/discard, pot potion slot, e end turn; card codes as above) | live player | agree | R1 his-live (se) | R1 best class. "forced" = one legal action.
```
a_terror_eel: i | t | his | live | ag | K256 his-live (se) | K256 best
0 | 5:20 | pot 0 | NEUTRALIZE>e0 | N | +0.0000 (0.0000) | DEFEND
1 | 5:21 | p N | NEUTRALIZE>e0 | Y |  | DEFEND
2 | 5:22 | p DA | DASH>e0 | Y |  | DEFEND
3 | 5:24 | p S | STRIKE>e0 | Y |  | DEFEND
4 | 5:25 | e  | end turn | Y |  | end turn
5 | 5:36 | p SB | SURVIVOR | N | -0.0145 (0.0077) | STRIKE>e0
6 | 5:37 | p SV | SURVIVOR | Y |  | SURVIVOR
7 | 5:38 | c S | discard 2 (STRIKE) | Y |  | discard 2 (STRIKE)
8 | 5:39 | e  | end turn | Y |  | end turn
9 | 5:47 | p DS+ | DAGGER_SPRAY | Y |  | DAGGER_SPRAY
10 | 5:53 | p S | SURVIVOR | N | +0.0066 (0.0181) | STRIKE>e0
11 | 5:57 | p SV | STRIKE>e0 | N | -0.0979 (0.0027) | STRIKE>e0
12 | 6:06 | c S | discard 1 (STRIKE) | Y |  | discard 1 (STRIKE)
13 | 6:08 | p HD | HIDDEN_DAGGERS | Y |  | HIDDEN_DAGGERS
14 | 6:14 | p SH | SHIV>e0 | Y |  | SHIV>e0
15 | 6:15 | p SH | SHIV>e0 | Y |  | end turn
16 | 6:17 | e  | end turn | Y |  | end turn
17 | 6:25 | p N | NEUTRALIZE>e0 | Y |  | NEUTRALIZE>e0
18 | 6:26 | p DA | DASH>e0 | Y |  | potion slot1
19 | 6:27 | p S@2 | STRIKE>e0 | Y |  | potion slot1
20 | 6:28 | e  | end turn | Y |  | potion slot1
21 | 6:41 | p SB | SNAKEBITE>e0 | Y |  | potion slot1
22 | 6:44 | e  | end turn | Y |  | potion slot1
23 | 6:58 | pot 1 | SURVIVOR | N | +0.0045 (0.0045) | potion slot1
24 | 6:59 | p SH+ | SHIV>e0 | Y |  | SHIV>e0
25 | 7:00 | p SH+ | SHIV>e0 | Y |  | SHIV>e0
26 | 7:01 | p SH+ | SHIV>e0 | Y |  | STRIKE>e0
27 | 7:02 | p S | STRIKE>e0 | Y |  | STRIKE>e0
28 | 7:03 | p S | STRIKE>e0 | Y |  | STRIKE>e0
29 | 7:04 | p HD | HIDDEN_DAGGERS | Y |  | HIDDEN_DAGGERS
30 | 7:05 | p SH | SHIV>e0 | Y |  | SHIV>e0
31 | 7:06 | p SH | SHIV>e0 | Y |  | SHIV>e0
b_gremlin_merc: i | t | his | live | ag | K256 his-live (se) | K256 best
0 | 7:36 | p D@0 | DEFEND | Y |  | DEFEND
1 | 7:37 | p D | DEFEND | Y |  | DEFEND
2 | 7:38 | p D | DEFEND | Y |  | HIDDEN_DAGGERS
3 | 7:39 | p HD | HIDDEN_DAGGERS | Y |  | HIDDEN_DAGGERS
4 | 7:39 | c S | discard 0 (STRIKE) | Y |  | discard 0 (STRIKE)
5 | 7:41 | p SH | SHIV>e0 | Y |  | SHIV>e0
6 | 7:44 | e  | end turn | Y |  | end turn
7 | 7:49 | p ST | DAGGER_SPRAY | N | +0.0030 (0.0009) | STRANGLE>e0
8 | 7:51 | p N | DEFEND | N | -0.0002 (0.0000) | DEFEND
9 | 7:52 | p DS+ | SURVIVOR | N | -0.0001 (0.0000) | DEFEND
10 | 8:07 | p S | SURVIVOR | N | +0.0001 (0.0000) | STRIKE>e1
11 | 8:08 | e  | forced
12 | 8:11 | p DA | DASH>e1 | Y |  | DASH>e1
13 | 8:13 | p S | DEFEND | N | -0.0000 (0.0000) | end turn
14 | 8:14 | e  | forced
15 | 8:17 | p SH | SURVIVOR | N | -0.0009 (0.0009) | SURVIVOR
16 | 8:18 | p S | STRIKE>e0 | Y |  | STRIKE>e0
17 | 8:18 | p S | STRIKE>e0 | Y |  | STRIKE>e0
c_skulking_colony: i | t | his | live | ag | K256 his-live (se) | K256 best
0 | 8:46 | p DA | DASH>e0 | Y |  | DASH>e0
1 | 8:50 | p HD | SURVIVOR | N | -0.0852 (0.0582) | SURVIVOR
2 | 8:53 | c S | discard 0 (STRANGLE) | N | -0.0414 (0.0414) | discard 0 (STRANGLE)
3 | 8:57 | p SV | SURVIVOR | Y |  | SHIV>e0
4 | 8:58 | c S | discard 1 (STRIKE) | Y |  | discard 1 (STRIKE)
5 | 8:59 | p SH | SHIV>e0 | Y |  | SHIV>e0
6 | 8:59 | p SH | SHIV>e0 | Y |  | SHIV>e0
7 | 9:00 | e  | forced
8 | 9:03 | p N | DEFEND | N | +0.0000 (0.0000) | DEFEND
9 | 9:05 | p S | DEFEND | N | +0.0000 (0.0000) | DEFEND
10 | 9:06 | p S | DEFEND | N | +0.0000 (0.0000) | DEFEND
11 | 9:07 | p D | DEFEND | Y |  | DEFEND
12 | 9:08 | e  | forced
13 | 9:22 | p DS+ | DAGGER_SPRAY | Y |  | DAGGER_SPRAY
14 | 9:24 | p D | DEFEND | Y |  | DEFEND
15 | 9:25 | p D | DEFEND | Y |  | DEFEND
16 | 9:26 | e  | forced
17 | 9:31 | p N | SNAKEBITE>e0 | N | -1.0321 (0.2665) | SNAKEBITE>e0
18 | 9:48 | p HD | DEFEND | N | +0.0000 (0.0000) | SNAKEBITE>e0
19 | 9:50 | c D | discard 1 (DEFEND) | Y |  | discard 1 (DEFEND)
20 | 9:52 | p SH | SHIV>e0 | Y |  | SHIV>e0
21 | 9:53 | p S | SHIV>e0 | N | +0.0000 (0.0000) | SHIV>e0
22 | 9:55 | p SH | SHIV>e0 | Y |  | SHIV>e0
23 | 9:58 | p SB | SNAKEBITE>e0 | Y |  | SNAKEBITE>e0
24 | 10:01 | e  | forced
d_gardeners: i | t | his | live | ag | K256 his-live (se) | K256 best
0 | 11:13 | p DF | HIDDEN_DAGGERS | N | +0.0017 (0.0025) | OUTBREAK
1 | 11:40 | p D* | OUTBREAK | N | -0.0183 (0.0064) | HIDDEN_DAGGERS
2 | 11:44 | p SB | SNAKEBITE>e3 | Y |  | SNAKEBITE>e3
3 | 11:46 | p HD | HIDDEN_DAGGERS | Y |  | HIDDEN_DAGGERS
4 | 11:47 | c OB | discard 0 (OUTBREAK) | Y |  | discard 0 (OUTBREAK)
5 | 11:49 | p SH | SHIV>e3 | Y |  | SHIV>e3
6 | 11:51 | p SH | SHIV>e0 | Y |  | SHIV>e0
7 | 11:52 | e  | forced
8 | 12:26 | p ST | NEUTRALIZE>e0 | N | -0.0105 (0.0156) | NEUTRALIZE>e0
9 | 12:28 | p DA | DASH>e3 | Y |  | DASH>e3
10 | 12:39 | p N | NEUTRALIZE>e0 | Y |  | NEUTRALIZE>e0
11 | 12:41 | e  | forced
12 | 12:47 | p D | NOXIOUS_FUMES | N | -0.0000 (0.0000) | NOXIOUS_FUMES
13 | 12:49 | p D | DEFEND | Y |  | DEFEND
14 | 13:01 | p S | STRIKE>e0 | Y |  | STRIKE>e0
15 | 13:03 | e  | forced
16 | 13:32 | p S | DAGGER_SPRAY | N | -0.0082 (0.0008) | DEFLECT
17 | 13:33 | p DS+ | DAGGER_SPRAY | Y |  | DEFEND
18 | 13:37 | e  | end turn | Y |  | DEFLECT
19 | 13:40 | p S | STRIKE>e0 | Y |  | STRIKE>e0
e_lagavulin: i | t | his | live | ag | K256 his-live (se) | K256 best
0 | 14:37 | p NF | DAGGER_SPRAY | N | -0.0058 (0.0040) | DAGGER_SPRAY
1 | 14:38 | p N | end turn | N | +0.0482 (0.0115) | DAGGER_SPRAY
2 | 14:40 | e  | end turn | Y |  | DAGGER_SPRAY
3 | 14:49 | p SB+ | STRIKE>e0 | N | -0.0003 (0.0002) | STRIKE>e0
4 | 14:51 | p S | STRIKE>e0 | Y |  | STRIKE>e0
5 | 14:52 | e  | end turn | Y |  | end turn
6 | 15:00 | p OB | OUTBREAK | Y |  | STRIKE>e0
7 | 15:02 | p HD | HIDDEN_DAGGERS | Y |  | potion slot0
8 | 15:03 | c S | discard 1 (OUTBREAK) | Y |  | discard 1 (OUTBREAK)
9 | 15:04 | p SH | SHIV>e0 | Y |  | end turn
10 | 15:06 | p SH | SHIV>e0 | Y |  | end turn
11 | 15:07 | e  | end turn | Y |  | end turn
```
