# What each boss and elite asks of a deck
For the plan's target deck (`sts2-plan`) and the buckets (`sts2-deckbuilding`). Test any line: `eval --pool <Act>:boss|elite` or boss-alone `--v`.

## Bosses
- Vantom (183 HP, Slippery 9) `[code]`: Ink Blot 8 -> Inky Lance 7x2 -> Dismember 30 + 3 Wounds -> +2 Str. Asks SD, FB timed for Dismember, big hit after Slippery. `[hyp]` HP-gated once SD is in (test: `eval --boss --hp 53/60/80`).
- Ceremonial Beast (262 HP) `[code]`: Plow 160; an unblocked hit leaving it <= 160 breaks Plow (stun, phase 2: Ringing = one card next turn, Stomp 17, Crush 19 +4 Str). Asks FD + Vulnerable to break in two turns.
- Waterfall Giant `[sim]` E82: Steam Eruption explodes after the kill (HP 999,999,999, a stunned turn): a deck-speed gate; search 0.52 vs live 0.50 on 16 starts.
- Lagavulin Matriarch: SD + Vulnerable; early turns harmless `[hyp]` (test: `eval --enc LAGAVULIN_MATRIARCH_BOSS`).
- Soul Fysh (221 HP) `[code]`: Beckon (6 HP loss if in hand at end of turn), Fade = Intangible 2, Scream 15 + Vulnerable. Asks 221 damage in ~3 non-Intangible turns.
- The Insatiable (341 HP) `[code]`: Sandpit 4 counts down to death; Frantic Escape cards push it back. ~65 damage/turn for 5 turns: SD + burst + ACC.
- Kaiser Crab (219 + 209 HP) `[code]`: the crab behind you deals x1.5; one dies -> other +6 Str, 99 block; Laser 35. Asks area SD, kill both close together.
- Aeonglass (535 HP) `[code]`: Artifact 3, Wither card every 6 cards played, Ebb 26 + 33 block.
- Queen (419 HP) `[code]`: kill Torch Head Amalgam first.
- Test Subject (111 -> 212 -> 313 HP) `[code]` (content/monsters/glory_b.rs ts_respawn): a death revives it on its next turn, which deals no damage; the revive strips its Strength / Enrage and the debuffs on it. Phase 2: Painful Stabs (your unblocked hits on it add Wounds), Multi Claw 11 x (3, 4, 5 ...) one more hit per turn. Phase 3: Nemesis (Intangible every other turn), Big Pounce 45, Burning Growl (+5 Burns, +3 Str). Asks ~636 damage over three phases plus block that scales with Multi Claw (run 3: 80 -> 8 HP on a 0.61 deck).
- Act 3 `[sim]` E84-E88: bot decks 0.24-0.33 vs the pair at full HP, expert decks 0.8-1.0; Demon Form +6 win points at act-3 bosses, front-damage commons -2 (E95 256-attempt re-measure).

## Elites and hallways
- Infested Prisms (171 HP) `[code]`: each Skill played gives you Tainted 3 vs its 9x3: win with Attacks/Powers/potions.
- Bygone Effigy: sleeps T1, SD; the starter deck loses it `[hyp]` (test: `eval --enc BYGONE_EFFIGY_ELITE`). Phrog Parasite: area damage `[hyp]`. Skulking Colony: FD vs Hardened Shell `[hyp]`. Terror Eel (150 HP, Shriek at 75) `[hyp]`.
- Spiny Toad (hallway) `[code]`: Thorns 5 while Spiked, Explosion 25.

## Target decks `[hyp]` (earlier models; test: `eval --boss --v` of the target deck)
- Insatiable: Strength + burst + ACC (Demon Form, Bludgeon, Tremble, Offering).
- Aeonglass: self-damage Strength engine (`sts2-deckbuilding/evidence.md`).
- Queen: sustained damage + HP carry `[hyp]`.

## Potions `[hyp]`
Vs hard bosses a potion can beat any card in the same shop (D8). Test: potion as an `eval` variant.
