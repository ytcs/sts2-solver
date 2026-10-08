# Resting is riskier than you think | Ascension 10 Defect | Slay the Spire 2
- url: https://www.youtube.com/watch?v=qQFyhWqhX9E | uploaded: 2026-06-22 | version: v0.107.1 (2026.06.18), read on the character-select screen (pinned build v0.111.0 = 2026-08-14; this run is 4 patches older) | char: Defect | asc: A10 | format: commentary run (won; Underdocks -> Hive -> Glory, bosses Lagavulin Matriarch, The Insatiable, Queen + Test Subject)
- source: en auto-captions + frames (frames/qQFyhWqhX9E/, git-ignored); @ = paragraph start (30 s) or frame time (mmss / hmmss); caption fixes: Fissure -> FTL, the bomb -> THE_BOMB, Loga / log -> LAGAVULIN_MATRIARCH, ship -> Ship in a Bottle (potion: 10 block now + 10 next turn, frame 4650), Tiny Forks -> Tuning Fork [caption?]; [frame?] = hard to read.
- numeric/card claims may be stale (v0.107.1); rest/smith logic is the transferable part.

## Rest-site record (frames; HP before -> choice -> next fight)
| floor | HP | choice | what followed |
|---|---|---|---|
| A1 F7 | 20/75 | smith Sunder | F8 hallway (52 HP enemy, then two more): 20 -> 8 |
| A1 F9-10 | 8/75 | rest (-> 30) | F11 Terror Eel elite: 30 -> 16 (Dexterity potion) |
| A1 F12 | 16/75 | smith Prowess | F13 elite (4 tentacles 29/32/27/28; PHANTASMAL_GARDENERS [frame?]) with Anchor 10 block: 16 -> 15 |
| A1 F16 | 13/75 | smith Barrage | F17 Lagavulin Matriarch (233 HP): 13 -> 5, won |
| A2 F28 | 26/75 | rest (-> 48) | after Infested Prism; "likely need HP into the boss" |
| A2 F32 | 39/75 | smith Fusion | F33 The Insatiable at 39/75, won (held a block potion) |
| A3 F47 | 61/71 | rest (-> 71) | Queen + Test Subject at 71/71, won |

## Claims
C1 `rest-vs-smith` [expert] @0:00:00, @0:07:11 Smith at a fire even at 12-20 HP unless the next fight can literally kill you; the upgrade usually saves more HP than the rest gives. ctx: A1 F7 rest, 20/75, 0 gold, deck 15 (Strike x2, Defend x4, Zap, Dualcast, Ascender's Bane, FTL, The Bomb, Coolheaded, Sunder, Prowess, Cold Snap), relic Cracked Core + Neow removal relic; next node a "?" already known to be a fight; smithed Sunder. The next fight took him 20 -> 8, and the next fire was a rest at 8/75. His own intro overstates it: he rested once in Act 1. q: "if you're not literally going to die here from 20 ... you should probably just upgrade"
- assess: in tension - sts2-deckbuilding s6 says smith by default (consistent), but sts2-pathing rule of thumb says "Rest before a boss or planned elite chain when HP binds", and E30 found 3 recorded rests where smithing cost 0.11-0.14 of P(clear act). His threshold (rest only if death is possible in the next fight) is far more aggressive than either.
- test: from the A1 F7 sim state: `price` on the rest screen (rest -> 42/75 vs smith Sunder at 20/75; P(clear act)); and `eval --enc <F8 encounter> --hp 20 --v "smith|upgrade=SUNDER"` vs the base deck at `--hp 42`, paired, with q90 HP lost. Supports: smith is within 2 se of rest on P(clear act), or ahead. Refutes: rest ahead by > 2 paired se (as on the E30 rests).
- sim state: Defect A10, A1 F7 -> F8, HP 20/75, deck as in ctx, relics CRACKED_CORE + Neow scissors relic [frame?], potions: 1 [frame?]; F8 enemy 52/52 HP, 8x2 intent on turn 1 [frame?].

C2 `rest-vs-smith` [expert] @0:15:55 Before a boss whose opening turns are harmless, smith instead of resting even at very low HP, when the deck is "strong enough". ctx: A1 F16, last fire before Lagavulin Matriarch (sleeps / Plating at the start), 13/75, 116 gold; smithed Barrage; entered the boss at 13/75 and won at 5/75. Deck 19: Strike x2, Defend x4, Zap, Dualcast, Ascender's Bane, FTL, Coolheaded, Cold Snap, Fusion, Barrage, The Bomb+, Sunder+, Prowess+, Darkness+, Hotfix; relics Cracked Core, Neow scissors relic, Anchor, two more [frame?]; potions Ship in a Bottle + one more [frame?]. q: "You definitely don't want to be resting here if you think you are strong enough"
- scope (user): conditional, not a categorical rule: it holds only when the boss is still winnable at the current HP (harmless opening turns: HP has little marginal value there). The decision rule is ours: smith iff P(win boss | HP now, smithed deck) >= P(win boss | rested HP, current deck) (then the next-act value breaks ties); price both, never default to smithing.
- assess: contradicted (by a [hyp] rule) - sts2-pathing "Rest before a boss ... when HP binds" and "The pre-boss rest sets boss HP"; encounters.md has Lagavulin Matriarch's "early turns harmless" (consistent with his reason). sts2-ironclad keeps "Smith vs rest before a boss by HP and boss" as an open test.
- test: from this sim state: `eval --enc LAGAVULIN_MATRIARCH --hp 13 --attempts 256 --v "smith|upgrade=BARRAGE"` vs base deck at `--hp 35` (paired seeds), plus `eval --smooth --boss --next` for the long horizon. Supports: smith at 13 HP is within 2 se of rest at 35 HP on boss win, and ahead on the next-act column. Refutes: rest ahead by > 2 se on boss win. Repeat with Ironclad's Act 1 bosses to see whether "harmless early turns" is the deciding feature (Vantom, Ceremonial Beast vs Lagavulin Matriarch).
- sim state: Defect A10, LAGAVULIN_MATRIARCH, HP 13/75, deck as in ctx (Barrage+ vs Barrage), Anchor (10 block turn 1), potions SHIP_IN_A_BOTTLE + 1 [frame?].

C3 `rest-vs-smith` [expert] @0:11:03 Relics and potions that cover the first turns (Anchor, a block potion) make smithing at low HP cheap: count them as HP. ctx: A1 F12 rest at 16/75 after the Terror Eel elite (reward: Anchor, Ship in a Bottle potion, Barrage); smithed Prowess; the next elite (4 enemies) cost 1 HP. q: "Anchor here and ship, so we can block for 30 over first two turns"
- assess: consistent - sts2-deckbuilding s6 and blind spot 4 (interim survival: "worse now" only if the HP gate is still reached).
- test: `eval --enc <elite> --hp 16 --v "smith|upgrade=PROWESS"` vs base at `--hp 38`, each with and without `relics_add=ANCHOR` / `potions=SHIP_IN_A_BOTTLE`. Supports: the rest-minus-smith gap shrinks by more than 2 se when Anchor + potion are present. Refutes: no interaction.
- sim state: Defect A10, A1 F13 elite, HP 16/75, deck 17 (F7 deck + Fusion, Barrage; Sunder+, Prowess+), relics CRACKED_CORE, Neow scissors relic, teapot icon [frame?], ANCHOR; enemies 4 x ~29 HP (PHANTASMAL_GARDENERS [frame?]).

C4 `rest-vs-smith` [expert] @0:42:25, @1:01:31, @1:03:17 Rest when the remaining upgrades are marginal and the next big fight wants HP. Smith when the upgrade is a core scaling or energy card. ctx: A2 F28 at 26/75 after Infested Prism ("hardest elite"; the boss is still ahead): rest. A3 F47 at 61/71 before the double boss: rest for 10 HP, "the upgrades are not that important here". A2 F32 at 39/75 before The Insatiable: smith Fusion, because the new orb slots made it "much more inclined". q: "I'll probably just rest at the last fire. The upgrades are not that important here."
- assess: consistent - sts2-deckbuilding s6 ("Rest only if the heal changes the rest of the act more than the best upgrade changes the run").
- test: `python -m agent.price <events.jsonl>` on recorded rest screens, split by the best upgrade's `eval --smooth --boss --next` gain (top vs bottom tercile). Supports: price prefers rest mostly where the best upgrade gain is small. Refutes: the preference is independent of upgrade value.

C5 `neow-choice` [expert] @0:00:35 Remove 2 (which costs HP) is right when the map is safe / low value: removing two Strikes keeps paying off later. Golden Pearl is a good Neow; Wing Boots almost never. ctx: A1 Neow, Defect A10 (75 max HP, 60 at the start); the options screen is cut from the video; deck 9 after (two Strikes removed). q: "if the map is low value, then we can take it since it is a very high value option for later"
- assess: consistent - sts2-pathing (score the Neow option and the route as one decision) and sts2-deckbuilding s2.7 ([sim] removals +0.03 each, eight +0.20).
- test: at a Neow screen, `eval --smooth --boss --next --v "rm2|remove=STRIKE_DEFECT,STRIKE_DEFECT"` against the other options priced by hand, plus `routes` on a safe vs an elite-heavy lane. Supports: rm2's gain is larger on the low-value lane, because the HP cost binds less there. Refutes: the ranking does not depend on the lane.

C6 `deck-density` [expert] @0:02:17, @0:06:06 Removals compound: each removal is worth more when you already have removals. With a small deck, take cards you play every time you draw them (FTL over Sweeping Beam). ctx: A1 F2 reward at 32/75, deck 9: FTL / Smokestack / Sweeping Beam; took FTL. q: "removes do get stronger with uh current removes already"
- assess: consistent - sts2-deckbuilding s2.7 (a single removal ~+0.03, eight +0.20: the gain is convex).
- test: `rmcalc` on the 9-card deck vs the 11-card starter (same remaining fights). Supports: the marginal value of the next removal is higher in the 9-card deck. Refutes: equal or lower.

C7 `skip-cards` [expert] @0:14:15, @0:40:10 Skip cards that do not fix the deck's actual problem, even strong or fun ones: here damage was solved and block density vs Lagavulin Matriarch was the gap. Later, skip to keep the deck small for Pocket Watch. ctx: A1 F14 reward Adaptive Strike / Hotfix / ... (took Hotfix: "if anything"); A2 skipped a reward to protect the Pocket Watch engine. q: "just don't need to pick it cuz the damage is not the problem"
- assess: consistent with the sts2-deckbuilding s3 bar (a card into solved buckets needs > max(3 se, 0.05)); in tension with the s1 header ("never skip unless all options are worse").
- test: at such a screen, `reward` and read the weakest-fight column: the damage card should gain < 2 se vs the boss while lowering block density. Supports: the skip / block option ties or beats it on the weakest fight. Refutes: the damage card is significantly ahead vs the boss.

C8 `potion-use` [expert] @0:18:15, @0:39:02 Save the strong potions (Focus, Potion of Binding) for the end game or the hardest fight; spend weak ones (Dexterity) on hallways and elites. Panic-using a potion in an easier fight leaves you without it for the dangerous one. ctx: A1 F11 Terror Eel: drank the Dexterity potion, kept Ship in a Bottle (held it through the F17 boss at 13 HP rather than use it early); A2 F27 Infested Prism at 40/75: Potion of Binding used there. q: "if you freak out on the last fight and use it there, then you don't have it for this"
- assess: consistent - sts2-strategy Potions ("keep the strongest for the act boss unless spending here is worth more").
- test: `agent.hindsight` on held/spent potions; `eval --enc INFESTED_PRISMS --hp 40 --v "bind|potions=POTION_OF_BINDING"` vs without, compared with the same potion's gain vs the act boss. Supports: the potion's gain is largest at the fight he used it in. Refutes: it gains more vs the boss.

C9 `ancient-choice` [expert] @0:20:57-0:23:06 Calling Bell (three relics, one of each rarity, plus a curse) beats Biased Cognition without frost / orb-slot support. The curse costs more in a small deck. Rare relics are run-warping. ctx: Act 2 ancient, 61/75, deck 19-20, options Biased Cognition / Calling Bell / Ectoplasm; took Calling Bell -> Puzzle-type relic, Tuning Fork [caption?], Pocket Watch (frame 2420 relic bar). q: "Usually just like being able to see a rare relic is just a really, really strong thing"
- assess: not covered - sts2-pathing Neow/ancient (relics priced by hand); sts2-strategy Events ("a curse costs one slot").
- test: `eval --smooth --boss --next --v "bell|relics_add=<3 sampled relics>|add=CURSE_OF_THE_BELL"` over several relic draws vs `--v "bc|add=BIASED_COGNITION"`. Supports: the mean over draws beats Biased Cognition by > 2 se. Refutes: Biased Cognition ahead.

C10 `shop-planning` [expert] @0:21:29 Character-specific relics no longer appear in chests (only shops and elites), so gold is worth more for Defect. ctx: same screen; reasoning for valuing money. q: "you cannot see character-specific relics in chests anymore"
- assess: not covered - a mechanics claim (sts2-mechanics adds only code- or sim-confirmed facts).
- test: decomp: treasure-room relic pool filter (character pool excluded?) at the pinned build. Supports: chest pool excludes character relics. Refutes: included.

C11 `path-flexibility` [expert] @0:24:10, @0:52:48, @0:59:43 Take every fight while the relic bar makes fights cheap (Act 2: also because "Act 2 events are the worst"). Once the run is clearly strong in Act 3, play conservatively: avoid elites ("the three guys"), keep rests, "we don't really need any relics". ctx: A2 at 61/75 with Anchor, Puzzle, Pocket Watch: a fight-heavy line with an early shop (frame 2420). A3: dodged an elite and rested before Queen + Test Subject.
- assess: in tension - sts2-pathing Risk budget: "while routes shows extra elites at >= ~0.9 boss win, take them" makes no exception for the final act, where no next-act pool exists to buy. The Act 2 part is consistent with it (and with blind spot 2.2).
- test: `routes` on an Act 3 map from a strong deck state: compare the 0-elite vs the 1-2 elite route on the double-boss win (q10 arrival HP). Supports: extra elites lower the double-boss win, or leave it unchanged. Refutes: they raise it by > 2 se.

C12 `act1-decides-runs` [expert] @1:00:53, @1:09:42 Spend thinking time early. Good early decisions make the end game "spew-proof", so later misplays are not punished. ctx: end of the run, very strong deck (Echo Form, Pocket Watch, 3 Strength / Dexterity). q: "do your homework in the early game essentially and that game looks easy almost always"
- assess: consistent - E30 (runs die at act-1/act-2 gates); sts2-deckbuilding s2.10 (horizon).
- test: none needed beyond E30's where-runs-die table (already measured for our policy).

C13 `scaling-vs-frontload` [expert] @0:18:15, @1:09:42 A relic like Anchor lets you draft scaling over front-loaded block, because it covers the set-up turns where this deck takes its damage. ctx: deck relying on Anchor + frost orbs; skipped front block. q: "seeing anchor allows you to draft in this sort of way because you're not going to get nearly as punished"
- assess: consistent - sts2-deckbuilding s3 (buckets: Anchor fills FB, so the open bucket shifts to SD).
- test: `reward` / `eval --v` on a scaling-vs-block card pair with and without `relics_add=ANCHOR`. Supports: the ranking flips toward the scaling card with Anchor. Refutes: no flip.

## Note for the index
The headline "rest is riskier" is shown on four smith-at-low-HP screens (20, 16, 13 and 39 HP). The run also contains three rests (8, 26 and 61 HP), so his practice is closer to s6 than his intro suggests. The pre-boss smith at 13/75 (C2) is the cleanest solver test state.
