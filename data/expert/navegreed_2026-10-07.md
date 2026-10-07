# Expert-data pilot: NaveGreed, A10 Ironclad, 2026-10-07

Data: `navegreed_2026-10-07.json` (meta, `macro` list, `fights` list).

Source: https://www.youtube.com/watch?v=rxMGDepYyy8, uploaded 2026-10-07. This is NaveGreed's most recent upload. It is an edited 74-minute cut of a Twitch stream, covering the whole run from Neow to the victory screen.

## Run
- **Character and outcome:** Ironclad, Ascension 10. Won on floor 49 at 47/95 HP. Acts: Underdocks, Hive, Glory. The bosses were Lagavulin Matriarch, Kaiser Crab, then Aeonglass + Queen.
- **Build:** the game shows `[v0.111.0] (2026.08.14)` in the top-right corner of every frame. That is exactly our pinned build. v0.111.0 (beta, 2026-08-13) is still the latest patch according to Steam news.
- **Mods:** the same corner shows `MODDED (3)`. The mods were not identified. Nothing visible suggests they change gameplay, but this is unverified.
- **Seed:** `3EC3BCK90DQX`, read at 720p. The 0/O and 8/B readings are uncertain.
- **Captions:** YouTube auto-captions exist and name most picks and reasons, but they mangle names: "Py" is Pyre, "packed" is Burning Pact, "Terascender" is Tear Asunder. There are no chapters.

## Time spent (wall clock, this session)
| part | time |
|---|---|
| Search: channel listing via yt-dlp, upload dates, build check against Steam news | ~4 min |
| Download: 720p60 mp4, 518 MB, plus captions | 2.5 min |
| Frame extraction: one ffmpeg pass, full-res frame every 2 s (2,210 frames) plus 3x3 contact sheets every 4 s | 8 min |
| Reading macro: 122 contact sheets plus ~10 full-res 2x2 quads at decision points | ~14 min |
| Reading combat: 1 fps 2x2 tiles, 3 fights, 27 turns | ~11 min |
| Spot check: re-reading 4 reward screens and the version comparison against the simulator's card table | ~5 min |
| Writing the JSON and this report | ~8 min |
| **Total** | **~55 min** |

About 200 images were read in total.

Per-act cost for the macro pass, roughly proportional to video length:
- Act 1: ~5 min (14.5 min of video).
- Act 2: ~5 min (25 min of video).
- Act 3: ~5 min (32 min of video). Act 3 has fewer frames needed per decision.

Per-fight cost: ~3-4 min for a 13-turn boss fight. Hallway fights are cheaper.

## What was extracted
**Macro: 52 entries, 41 decisions.**
- 3 ancients, each with all 3 options.
- 25 card rewards. 14 have all three options readable and a certain pick. The other 11 are missing options, the pick, or both, usually because the player clicked through faster than the 2 s sampling.
- 2 shops with full card stock and prices. Relic and potion stock is mostly unnamed (icons only).
- 8 rest sites, each with rest or smith and the smith target.
- 4 events with all options and the choice.
- 5 treasures, 2 act-boundary snapshots, the A10 double boss and the outcome.
- HP and gold appear on most entries.

**Combat: 3 fights, 27 turns, 103 actions.** Each fight has its start setup (deck, relics, potions, HP, enemy HP and powers). Each turn has the hand, energy, intents, the ordered plays with targets, and the result.
- **A (F2, Corpse Slugs, weak):** partial, read from 2 s frames.
- **B (F14, Terror Eel elite, 9 turns):** complete except Headbutt's top-deck choices.
- **C (F17, Lagavulin Matriarch boss, 13 turns):** complete except some exhaust and top-deck choices.

## Hard or impossible fields
- **Draw pile order:** never visible. Pile contents are visible only when the player opens the pile, which he did 3 times in fight C. Composition can be derived from deck minus hand minus discard, but order cannot.
- **Choices inside a card:** Headbutt and Burning Pact targets, and the shop removal target, appear for under a second. They are often missed even at 1 fps.
- **Relics from treasure, shop and some elites:** small icons with no name unless hovered. 4 relics are still `?`; a fifth (F10 treasure) is inferred as BAG_OF_MARBLES?.
- **Potion identity:** small icons; names were only readable on loot screens.
- **Relic counters:** Ornamental Fan and Fishing Rod counters are tiny digits, readable only on full-res frames.
- **Encounter ids:** a fight's name appears only when the cursor hovers the enemy. Eight hallway and elite encounters are inferred from enemy count, HP and art (marked `?`). The Act 1 elites are the weakest guesses.
- **Intents:** readable when the number is drawn above the enemy (e.g. `10x2`). Debuff and buff intents are icons only.
- **Card rewards clicked within 2 s:** missed. 5 of 25 have `card_options: null`, with the pick inferred from the next deck view.
- **Cuts in the edited video:** a few mid-fight cuts. An unedited Twitch VOD would avoid them.

## Accuracy (spot check)
- I re-read 4 reward screens at full res for picks I had inferred from the deck or captions (F7, F8, F13, F23). All 4 picks were confirmed and the missing options filled in.
- Fight C was cross-checked by HP arithmetic: damage per play against the boss HP bar, Weak and Vulnerable math, Ornamental Fan block. It was consistent turn by turn. A second pass over the player side (energy left, block amounts from Orichalcum and Ornamental Fan) corrected two plays in fight C and the Strike count (5, matching the HUD deck count of 21).
- Start-of-combat effects help identify relics: every fight after F10 opens with all enemies Vulnerable, which points to BAG_OF_MARBLES for the unnamed F10 treasure.
- Estimates:
  - Visible-option decisions (card rewards, ancients, events, rest targets): ~95% correct.
  - Encounter ids without a hover: ~60%.
  - Combat play order and targets at 1 fps: ~90%.
  - Exhaust and top-deck choices: ~50% readable.
  - Fight A (2 s sampling): ~75%.

## Build and version match
- The HUD build string matches v0.111.0 exactly.
- Card numbers shown in the video agree with `crates/sts2sim` (`gen_cards.rs`): Bash+ 10 / 3 Vulnerable, Anger+ 8, Blood Wall 2 HP / 16 (20 upgraded), Breakthrough 9 AoE, Evil Eye 8+8, Expect a Fight 15+5/Str, Fasten 4, Flame Barrier 12/4, Headbutt 9, Pommel Strike 9, Setup Strike 7 + 3 Str, Shrug It Off 8, Cinder+ 24, Uppercut 13 with 1 Weak and 1 Vulnerable (2/2 upgraded), Cruelty+ 50%, Not Yet heal 10, Pyre 1 energy, Rage 3, Bully 4+2.
- Costs match `data/catalog.json` for every card seen.
- Only open question: Tear Asunder showed "Deal 10 damage" in the deck view after the Tri-Boomerang Instinct enchant, while the sim has base 5 (+2). The likely cause is the enchant, not a version difference, but this is unverified.
- Events (Waterlogged Scriptorium, Field of Man-Sized Holes, Self-Help Book, The Future of Potions) and their options match `data/events.json` keys.

## Recommendation
**(a) Macro data for a plan library: worth scaling up.**
- One run gives ~40 decisions with the expert's reasons in the captions, in under 30 min of model time.
- The two big costs can be cut:
  1. Find decision screens automatically instead of reading contact sheets. The HUD is fixed, so the "Choose a Card" banner, shop, rest, event and map screens can be found by template matching or scene detection, then pulled at full res and 0.5 s spacing so fast clicks are not missed.
  2. OCR the card and option names: they are rendered text, so Tesseract or PaddleOCR on fixed crops would work. Keep the vision read only for verification and icons.
- Also align the captions to each decision to store the stated reason. They held useful reasoning, e.g. "Stoke is bad when your energy is Pyre", "path to hallways over a rest because Blood Vial + Not Yet heal".
- Prefer unedited VODs.
- Estimated cost with that tooling: ~5-10 min per run.

**(b) Combat test set: feasible, but extract turn-start snapshots, not whole fights.**
- A snapshot means hand, energy, HP/block, powers, enemy HP/intents, and draw pile composition derived from the deck.
- Exact replay is not possible from video alone: draw order and some in-card choices are hidden.
- The best lever: the build matches and the seed is on screen in every frame. If a run can be started from that seed in the real game through our bridge, transcribing only the decisions would let the game regenerate exact states and logs. Mods (3) and the seed reading need checking first; this is untested.
- Without seeded replay, select pivotal turns, e.g. where the commentary flags a hard choice ("could have played Bash there, would have saved 13 HP"). Expect ~2-3 min per snapshot with the current manual pipeline.

**Tooling wishlist, in priority order:**
1. A decision-screen detector with full-res frame dump.
2. OCR of card and option names with the catalog as a closed vocabulary.
3. Caption-to-decision alignment.
4. A test of seeded replay from the HUD seed.
5. A turn-banner detector ("Player Turn / Turn N") that dumps the frame after each banner, plus frames around every energy-orb change, to cut combat reading to the frames that matter.
