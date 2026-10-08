# JapaneseExport (YouTube) STS2 expert notes: index
Channel: https://www.youtube.com/channel/UCYZwLfdwKJjIm_JFYCEWULw. A top StS player; A10 runs, coaching reviews (Spire Clinic) and panel podcasts (Spire Cast).
- Every claim is `[expert]`: a hypothesis with a proposed test, never a rule. Notes compare each claim with `.claude/skills/*` and `docs/research/evidence.md` and propose a test. No test has been run yet.
- One note per video, `<upload_date>_<id>.md`. Each claim has a key, a timestamp, a context read from frames, an assessment and a test; reconstructable fights are marked `sim state:` (candidate solver test states).
- Git-ignored, local only (creator content): `transcripts/` (deduped en auto-captions, 30 s paragraphs) and `frames/<id>/<mmss>.jpg` (the stills each claim's context was read from). Videos were not kept.
- Caveats: auto-captions mangle names (fixes listed per note); timestamps are 30 s paragraph starts. Our pinned build is v0.111.0 (2026-08-14), and older uploads may carry stale card numbers. Spire Cast is a panel whose speakers are not marked in the captions.

## Processed (11)
| date | id | title | char / build | claims | top 3 claims |
|---|---|---|---|---|---|
| 2026-10-06 | l6leE93iqPE | S-tier cards are killing your runs (Spire Clinic #2) | Silent A10, post-0.111 upload, review | 16 | scaling solved -> buy front/AoE, not a 2nd Accelerant; mass-remove only with an output loop, else fastest stabilizer; skip plan-diluting patches |
| 2026-09-19 | IqNkmsA0PLo | The hidden win conditions that most players miss | Ironclad, post-0.111 upload | 6 | rate decks on FD/SD/FB/SB/ACC vs upcoming fights; switch win condition by T2 when key cards are bottom-decked; play to the scaling's exact condition (Shuriken) |
| 2026-09-13 | 6T4bZkpdxbA | Reddit said this seed was impossible (Spire Clinic #1) | Ironclad A10, v0.111.0, review | 21 | skip attacks/filler once the plan works (~15-card deck); feed the strength (2nd Inflame over Shrug+); carry 300+ gold over early removals |
| 2026-09-08 | XGHSM0CVibo | Your deckbuilding isn't the problem - this is | Ironclad, post-0.111 upload | 6 | exhaust down to the 4-5 cards that win alone; take a big hit to set up the breakpoint turn; finding the fight plan matters more than macro |
| 2026-08-26 | BKo7uxVcMRg | Why Slay the Spire is so impossible to learn | Ironclad A10, post-0.111 upload | 8 | plan the whole fight (block + kill the next attacker); front-load risk (4 Act 1 elites when strong); skip cards/save resources for the end game |
| 2026-08-18 | p_pImjcVpug | Why pros win with bad cards (and you don't) | Necrobinder A10, post-0.111 upload | 14 | small cycling deck skips good-but-off-plan cards; hold Fairy for the end game, it licenses HP greed; souls fine vs Infested Prisms |
| 2026-06-30 | qB32y8nhMlk | The Act 1 blueprint for EVERY character (Spire Cast 3) | panel, pre-0.111 | 16 | Ironclad farms every Act 1 elite, deaths come from hallway HP bleed; Overgrowth asks damage, Underdocks block; unknown room fight odds 10% +10% per event |
| 2026-06-22 | qQFyhWqhX9E | Resting is riskier than you think | Defect A10, v0.107.1 | 13 | smith before a harmless-opening boss even at 13/75; smith at 12-20 HP unless the next fight can kill; play conservatively in Act 3 |
| 2026-06-07 | cjCUFSoQWks | Pathing matters more than you think | Ironclad A10, pre-0.111 | 18 | jump to a fire before a must-take elite; late game secured -> Acts 1-2 goal is arriving; early Act 1 take the strongest-now card |
| 2026-05-04 | 1MCm0JnR98g | Mastering Regent after the changes | Regent A10, v0.104.0 | 19 | skip a card that helps this act but hurts the next; removals compound (remove 3 even with an Innate curse); farm Act 1 elites |
| 2026-04-25 | bU-9JBsxQ_I | These Spire 2 "tips" are actually traps (Spire Cast 1) | panel, pre-0.111 | 17 | removal overvalued, gold buys relics; hallways chip, front block beats front damage there; Act 3 bosses are stat checks a good deck passes |

## Recurring claims (ranked by number of videos x strength). Test status: all untested
1. **skip-cards / conserve-for-endgame** (9 videos: 6T4bZkpdxbA, cjCUFSoQWks, l6leE93iqPE, p_pImjcVpug, 1MCm0JnR98g, qB32y8nhMlk, bU-9JBsxQ_I, qQFyhWqhX9E, BKo7uxVcMRg). Once the deck's plan works, skip attacks and filler even at 13-18 cards, because each added card weakens the Act 3 deck. **In tension** with the `sts2-deckbuilding` header (smooth greedy, "never skip unless all options are worse" `[sim]`, combat-only, near horizon) and with 2.7(a) (skip only past ~20 cards). Test: `eval --future --smooth --v "skip|" --v "x|add=<card>"` on recorded mid-deck screens, reading the Act 3 column; and paired run-model rollouts of a "skip off-plan past N cards" policy vs smooth greedy on P(clear 2 acts). The skip policy is supported if it wins the act-3 horizon by > 2 se at < 0.05 cost on the current boss.
2. **act1-decides-runs / front-load-risk / more-elites-act1 / elite-snowball** (9 videos). Act 1 is where runs are lost. Take risk early while strong: max-elite paths with bail-outs; Ironclad farms every Act 1 elite; relics compound into Act 2 elites and shops. **Consistent** with `sts2-pathing` Risk budget, deckbuilding 2.2 `[sim]` and E30. Test: `routes` / `price` on recorded Act 1 maps, 2- vs 4-elite lanes, reading P(clear act 1) and P(clear act 2); supported if the extra elites raise P(clear 2 acts).
3. **rest-vs-smith: smith almost always** (6 videos: qQFyhWqhX9E, bU-9JBsxQ_I, l6leE93iqPE, p_pImjcVpug, 1MCm0JnR98g, cjCUFSoQWks). Upgrade at nearly every fire; smith even at 13/75 before a boss whose opening turns are harmless. Exceptions they name: rest before a must-take elite, or when upgrades are marginal and the boss wants HP. **Consistent** with deckbuilding s6 (smith default). **Contradicts** `sts2-pathing` "Rest before a boss ... when HP binds" `[hyp]`. **In tension** with E30 (smithing cost 0.11-0.14 of P(clear act) on 3 recorded rests). Test: `eval --enc LAGAVULIN_MATRIARCH --hp 13 --v "smith|upgrade=BARRAGE"` vs `--hp 35` (sim state in 20260622_qQFyhWqhX9E C2), plus `price` on recorded rest screens; supported if smith is within 2 se on boss win and ahead on the next-act column.
4. **potion-use: hold potions for the problem fights** (9 videos). Buy potions as patches for a near-term weakness. Accept hallway damage to bank potions for elites and bosses. Carry 3-4 into the A10 double boss. Do the math before throwing. **Consistent** with `sts2-strategy` Potions and encounters.md Potions `[sim]`. Test: potion as an `eval` variant vs the named fight; `hindsight` on held vs spent potions.
5. **removal value: contested between videos**. Pro-removal: 1MCm0JnR98g, qQFyhWqhX9E, p_pImjcVpug ("removals compound"). Skeptical: bU-9JBsxQ_I (removal overvalued, big decks fine, gold buys relics), qB32y8nhMlk (Ironclad never takes Neow remove-1/2; removal pays only on Necrobinder), 6T4bZkpdxbA (carry gold, not early removals), l6leE93iqPE (mass-remove only with an output loop). The skeptical side is **in tension** with deckbuilding 2.7 `[sim]` (remove 8 starters +0.20 vs Kaiser Crab) and 2.7(c). Test: `rmcalc` / `eval --v "rm|remove=STRIKE"` vs `relics_add=<typical shop relic>` at equal gold, on Act 1 and Act 2 decks; price as removal-100g vs relic-~225g.
6. **scaling-vs-frontload: once scaling is solved, buy front damage/block** (5 videos: l6leE93iqPE, bU-9JBsxQ_I, qB32y8nhMlk, cjCUFSoQWks, 1MCm0JnR98g). Counter-example: 6T4bZkpdxbA ("feed the strength", 2nd Inflame over Shrug+). **Consistent** with the deckbuilding s3 bar. Test: `eval --smooth --boss --elites` with a scaling dup vs a front card, on a deck with one scaling source.
7. **plan-whole-fight** (6 videos). Leave no attacker for next turn; set up lethal a turn early; do not hit an enemy that dies anyway; trade HP for tempo on breakpoint turns. **Consistent** with `sts2-strategy` "plan 2-3 turns" `[hyp]` and E26. Test: `agent.hindsight` / paired sim runs on the listed sim states (e.g. 20260826_BKo7uxVcMRg C1). Also check whether the live solver finds his line.
8. **ancient-choice: immediate power over long-term access** (l6leE93iqPE: Velvet Choker over Black Star, Throwing Axe over Meat Cleaver; cjCUFSoQWks: Imbued Blood Wall over Driftwood). **In tension** with deckbuilding 2.2 "buy access" `[sim]`. Test: `eval` the relic or enchant variants vs `price` with access relics; read P(clear act) and P(clear next act).
9. **Act 3: play safe, skip elites, Act 3 bosses are passable stat checks** (qQFyhWqhX9E, p_pImjcVpug, 1MCm0JnR98g, cjCUFSoQWks, bU-9JBsxQ_I). **In tension** with the literal `sts2-pathing` Risk budget ("take extra elites while boss win >= ~0.9") and with encounters.md Act 3 sweeps (~0% for Act 2-clear decks, which E30 attributes to solver and predictor lower bounds). Test: `routes` in Act 3 with elite vs fire lanes, reading double-boss win; and boss-alone `eval` of the expert's Act 3 decks (cjCUFSoQWks C15 sim state).

## Direct conflicts with current skill rules (highest value)
- qQFyhWqhX9E C2: smithed Barrage+ at 13/75 at the last fire before Lagavulin Matriarch, won at 5/75 ("you definitely don't want to be resting here"). Conflicts with `sts2-pathing` "Rest before a boss or planned elite chain when HP binds" `[hyp]`. Cleanest sim state of the set.
- Skip at 13-18 cards (recurring #1). Conflicts with the deckbuilding header "never skip unless all options are worse". The header's `[sim]` support (0.70 vs 0.31) compares smooth greedy with "best win now, skip on no gain" on a near horizon; it does not test horizon-motivated skipping.
- qB32y8nhMlk: each unknown room has a 10% chance to be a fight, +10% for every event visited (panel claim: "10% chance increasing by 10% every time you go to an event"). Our `routes` weight is "unknown = 15% regular fight" `[hyp]`. A mechanic: check in decomp (room-type odds in the map/unknown-room code).
- qB32y8nhMlk: Skulking Colony rewards block and does not fold to Vulnerable. encounters.md says "FD to break Hardened Shell" `[hyp]`.
- bU-9JBsxQ_I / qB32y8nhMlk / 6T4bZkpdxbA: removal skepticism (recurring #5) vs deckbuilding 2.7(c) "removals before marginal cards".

## Not yet processed (STS2, most relevant first)
Coaching (closest to the brief):
- 2026-05-29 jJNithDo0rc I coached pChal in Spire 2 (2.7 h)
- 2026-05-17 r_17phea1Y0 I coached FrostPrime in Slay the Spire 2 (1.8 h)
- 2026-04-11 9Ru7UYkQZd8 Can I turn Rarran into a pro Spire 2 player? (1.9 h)
- 2026-09-22 6D4tmECdNEg Spire 2 but Frost Prime is trying to kill me (A10 Ironclad)

A10 runs with decision commentary:
- 2026-08-21 aFbSzqNtyes Can YOU play like the pros? (A10 Ironclad; likely the run BKo7uxVcMRg discusses, unverified)
- 2026-08-31 LKBhc87lAT0 I picked Hailstorm on floor 1 and it carried?? (A10 Defect)
- 2026-08-09 aIVoArZ7DxM Now THIS is why you speculate! (A10 Silent)
- 2026-08-03 _FeW9M94x-E This Defect Card BREAKS Silent (A10)
- 2026-06-13 GWVHY9oK2y0 Playing around Aeonglass from floor 1 (A10 Silent)
- 2026-04-23 4bddE1aXIRA How to master new Ironclad (A10)
- 2026-04-08 BW-BeX9IeDY Beyond archetypes: learn to play like a pro (A10 Defect)
- 2026-03-22 UVuZEJJILAE How to win after the WORST start (A10 Silent)
- 2026-03-14 f8IyOmLKzOQ Learn good deckbuilding with mid cards (A10)
- 2026-03-10 lEG0PbotjXA Learn regent in One run (A10)
- 2026-08-01 -jMDgUoJEjo / 2026-05-23 OrrtPhrPNwE blindfolded A10 wins (Ironclad / Necrobinder)
- other A10 runs: 2cfh_JLFMR8, bzyFnbxuxmM, PXfTFQY3oAA, YP7y_2MQtsE, A02RR0zUbCc, GrIVpFxLZF0, BxTfgj3gvm0, PxmrqsYifGA, eMP5eXuAwdk, PD8GVBSUkb0, 6kKKI_4g2eA, J6UsZdi4riU

Panels and reference (Spire Cast, tier lists, "Thoughts from a top STS player"):
- 2026-05-11 AX9ERWkvM0k The most misunderstood cards (Spire Cast 2)
- 2026-07-26 pD88cnmXGew Ranking EVERY Act 3 ancient relic (Spire Cast 5); 2026-07-18 _TurPkx5d34 Act 2 ancient relic tier list (Spire Cast 4)
- 2026-04-03 JIeOd1gL20o I ranked EVERY enemy; 2026-03-25 r_c0X5FJ_7g I analyzed EVERY event; 2026-03-21 26tjGCxjc-g Ranking every Ancient boon; 2026-03-18 GFYNErQ3vEs every shop relic; 2026-03-28 B5k1MHzeymE every colorless card
- tier lists: i70zlnQfayY (Ironclad, 2026-04-21), gZMdBV54B2s (Silent, 2026-08-15), woO58nlBSxE (Defect, 2026-07-23), bl-mt0hwU8U (Necrobinder, 2026-07-09), 8zXnLXaxwhc (Regent, 2026-05-01)
- essays: GFc8Bh2RYvI, -1gZiFN7heQ, _SMfBDBTyo0, ToKDj1lLeZ0, HSZ912N-75I, dcGa6DklLBU; 2026-06-17 O0RtoxdcPm8 Correlated RNG in Spire 2 (mechanics: check vs decomp); 22lAalqVWwk Spire Cast 7

Lowest priority: patch rundowns (kUqkDwtmrDM v0.111.0, stkG964y8D8, pQKMBeClOJM, d_cJ6WOy0pk, o9YSFOwfqwo, 6ItHuv_Bqa8, KthG4Wi0EFU, Nbg-WXsXHys, 6v2lCLhx9WU, elqtKPMp-BI, UCj4Eog_H7U, Uv6_eQD8B2k, aS6Tc67kIIg). Multiplayer and co-op: _cyvACqFL0I, VMCQpTfz1h0, kHF-y5Gj7rQ, tUFvtTzPyWM, y5fw6erBNfY, Fw0ws2k0e4I.
Out of scope: Slay the Spire 1 (A20), Slice & Dice, Guildrun, other.
