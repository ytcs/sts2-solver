---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the five-bucket deck audit, what each boss and elite asks, the gap-then-eval pick procedure, plan switching, and the per-pick ledger.
---

# Deck building as an audit, not a feeling

Why: picks made on intuition cannot be compared run to run. Every pick below is the same four steps, and each step leaves a number in the `-- why` and in the run record, so a later review can ask which rule predicted the outcome. The five-bucket model and the "answer the fight's questions" idea are from an expert player's analysis `[expert]` (JapaneseExport, "The hidden win conditions that most players miss", youtube IqNkmsA0PLo, 10 min): he reads decks in five buckets and found a run-losing mistake by asking what the boss demands of the deck.

## 1. The five buckets (classify every card and relic in the deck)
- **Front-loaded damage:** kills things quickly (hallway fights, a boss's early window).
- **Scaling damage:** more damage per turn as the fight goes on (Strength, Inferno, poison-like, Rampage, Rolling Boulder).
- **Front-loaded block:** blocks now (Defend, Blood Wall, Body Slam as a finisher).
- **Scaling block:** block that grows or repeats (Dexterity, Feel No Pain, Metallicize, Barricade-with-block).
- **Acceleration:** energy, draw, cycling, and things that make the other four happen on time (Pyre, Bag of Preparation, Shrug, Pommel; Barricade belongs here: it saves block from turns it is not needed, it does not add block).
A card can sit in several buckets; a card that sits in none is a candidate to skip. Write the deck's count per bucket (a line of five numbers) after every pick and note the empty ones.

## 2. The questions ahead (what each fight asks)
For the act's boss first (it is known: `m`), then the elites that can still appear (`macro.narrow`), then hallways and events. For each: front-loaded damage need (does it threaten early, or give free turns?), scaling need (long fight, enrage, Strength?), block need (one huge hit every N turns, many small hits, multi-hit?), and any condition (Slippery, Hard to Kill, Artifact, stun). Source: `sts2-acts`, `sts2-mechanics`, the first turns of the fight, and `eval` against that encounter alone. Record the answers in the act's skill as a table (encounter -> what it asks).

## 3. Pick procedure (card reward, shop, relic, upgrade, removal)
1. List the unanswered questions (bucket gaps against the boss/elites ahead).
2. Shortlist the options that fill a gap; skipping is an option with its own cost (a diluted deck).
3. `eval` the shortlist against the **known boss alone** and the elites that can still appear, at the HP I expect to arrive with, 256 attempts; the gain must clear ~2 SE; otherwise skip. Remember a card's value depends on the deck (Blood Wall -5 -> +10 once Inferno was in the deck).
4. Check conditions: does the plan need a specific draw (three attacks in a turn, a power early)? Prefer picks that make the plan robust to a bad draw (acceleration, cheap cards, redundancy) over picks that add one more condition.
5. Write the ledger line: `bucket line; gap; options with eval deltas (win, HP); choice`.

## 3b. Horizon rule (long-term plan, not only the next boss)
- Evaluate every pick at three horizons: (1) the current act's known boss and the elites that can still appear, (2) the NEXT act's pools (all elites, all bosses: `--pool Hive:elite`, `--pool Hive:boss`; the boss only becomes visible on entering the act), (3) the final act's pools (`Glory:*`; at A10 the final act has two bosses back to back). Write the three deltas in the `-- why`.
- Before the current boss is solved the first horizon dominates; once the current boss is above ~85% (or after the Act 1 boss), the later horizons decide: a pick that is +3 now and +12 later beats one that is +8 now and -3 later. Prefer carry-over power: scaling (Strength, energy, draw), rares, relics, potions for later bosses, upgrades of the cards that carry.
- Keep a written target deck for each later boss (in `encounters.md` and the act skill): for The Insatiable it is Strength scaling + burst + acceleration (Demon Form, Bludgeon, Tremble, Offering = 60% vs 2%); for Act 3's two bosses (Aeonglass 535 HP with Artifact 3, Queen 419) it is high sustained damage per turn plus HP carry. Every reward screen, shop and event is checked against that list first: look for the cards on it, not only for incremental gains.
- Keep gold and potion slots for later acts when the current act is solved; shops in Acts 2-3 have the cards the target deck needs.
- Audit at each act boundary: the five-bucket line, the HP-gate numbers for each later boss (`eval --hp`), and which target cards are still missing.

## 4. Plan and switch
- State the fight plan before the boss and each elite (what wins it, in which bucket it lives, what condition it needs).
- During the fight, after turn 1-2, ask whether the opening draws killed the plan. If so, switch to the plan that uses what is in hand (the expert's example: with the powers bottom-decked, play the 0-cost attacks to enable a three-attack relic instead of blocking).
- A solver line that contradicts the plan is data: trust the solver for the micro, change the plan for the macro.

## 5. Macro audit at every rest, route and relic choice
- **Rest or smith:** compute `eval` at the HP after rest versus the HP now with the upgrade (Lagavulin at 44 HP 42% vs at 68 HP 86%, best smith +8): rest unless the HP gate is already cleared.
- **Route:** work back from the boss's HP gate (`eval --hp` at 34/45/60/80), then pick the path that arrives above it with the buckets filled (`sts2-pathing`).
- **Relics and Neow:** score by the bucket they fill and by the route they enable.

## 6. Making it iterative
- Each run's `events.jsonl` holds every eval (spec and result) and every action with its `-- why`. After a run: for each pick, compare the eval gain to what happened in the fights it was meant to fix (`python -m agent.improve review`); mark each rule that held or failed in the character/act skill with `[played]`.
- Keep two lists in the character skill: rules that predicted well (promote to `[sim]`/`[played]`) and rules that failed (delete or revise with the numbers).
- Change one thing per run when possible (a pick threshold, a route shape) so the effect can be read.

## Lessons from run 3 (reached Act 3, died to the first Act 3 boss) `[played]`
- Early game is where power is added: Act 1 picks (Inferno, Anger, Setup Strike, Inflame+Tremble vs Lagavulin) moved win rates by 20-70 points; by Act 2-3 almost every offered card was below the 2 SE bar (12 skips in Act 2), and the deck stalled. When a boss pool is at ~0%, a +5 point pick will not save the run: look for outliers (rares, relics, potions, shops, transforms, upgrades) and build the target deck for the boss, not incremental picks; consider taking rare picks (Demon Form +12/+9) over marginal elite-HP picks (Tear Asunder +22 elites, +0 boss) when the boss is the gate.
- Pre-check the next act's boss on the Act N boss reward: it is on the map only after entering the act, but the pool is known (`sts2-acts`): evaluate against all three bosses.
- Potions and consumables are cheap power: against hard bosses Powdered Demise (+14), Flex (+11) and Block Potion (+12) beat every card in the same shop.

## Balance across the five buckets (expert baseline, JapaneseExport) and the feasibility bound (user's design)
- Do not over-index into one bucket: a deck that is excellent at one question and empty in the others fails the fights that ask the others (run 4's deck had strong front damage and Strength scaling but no scaling block, and lost to the long boss fights). Check the five-number line after every pick and prefer the pick that raises the lowest bucket that a known upcoming fight asks about, unless `eval` shows a large gain elsewhere.
- **Demand vs capacity radar: built, measured, rejected `[sim]`.** A Python prototype (enemy schedule measured by passing turns in the simulator, per-card effects measured by probing, perfect-order planner, bisection on the efficiency needed to win) took 0.33 s per fight, against 0.27 s for `eval` at 64 attempts (0.72 s at 256), and agreed with the solver only moderately (Spearman -0.55 against win rate over 88 recorded fights; 10 of 80 fights the solver wins were called impossible: it cannot value triggered damage, engines like Primal Force + Demon Form, or boss reactions). Since `eval` is the solver and is as fast, the radar added nothing; it was deleted. What survives: `sts2.provably_unwinnable` (sound, narrow: pure cards only), `eval --future` (elites and bosses of this act and every later act in one call), and reading the enemy's passive schedule by hand when planning a boss (the simulator exposes it: pass turns with a huge-HP player and read the intents). For "which resource binds" use `eval` sensitivity variants instead (add Strength / Block / energy to the deck and see which moves the win rate).

## Bucket tags and mock builds (user's idea, built)
- `data/card_buckets_ironclad.json` (regenerate: `python -m agent.card_tags`) tags every Ironclad card with its buckets from simulator measurements, with an override table for state-dependent cards. `card_tags.deck_line(deck, tags)` gives the five-number line, `deficiencies(line)` the buckets below a minimum, `candidates(tags, bucket)` the cards that fill one. Run 4's final deck read FD 12, SD 3, FB 8, SB 0, ACC 3: the empty bucket was scaling block, which matches what lost the long fights. Tags only propose; `eval` decides.
- Search literature (for replacing the greedy "next best card"): evolutionary deck building with a simulator as fitness (Garcia-Sanchez et al., Hearthstone), whose heuristic mutation operator mimics what a human does (swap in a card that fills a weakness), and quality-diversity search (MAP-Elites) with a learned surrogate when evaluations are expensive (Zhang et al., arXiv 2112.03534: behavior descriptors keep a diverse archive, the surrogate screens candidates, the simulator confirms the best). Here `eval` costs 0.3-0.7 s per (deck, encounter), so an offline search with `eval` as fitness is affordable without a surrogate.
