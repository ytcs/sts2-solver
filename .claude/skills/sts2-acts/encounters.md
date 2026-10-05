# What each boss and elite asks of a deck

Used by `sts2-deckbuilding` section 4. Buckets: FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration. Test for any line: `eval --pool <Act>:boss|elite` or boss-alone `--v` variants.

## Bosses
- **Vantom** (A1 Overgrowth): FB for a ~30 hit every ~4th turn (pattern 14 / 30 / 0); many cheap hits or HP-loss triggers to strip Slippery 9 (each unblocked hit deals 1, removes a stack `[code]`); burst on buff turns. Win 0.46 at 60 HP, 0.11 at 45 `[sim]`.
- **Ceremonial Beast** (A1 Overgrowth, 262 HP at A10) `[code]` CeremonialBeast.cs, PlowPower.cs, RingingPower.cs: a burst check, then a race. T1 Stamp (no damage; gives itself Plow 160). Then every turn Plow: 20 damage and +2 Strength (20, 22, 24 ...). Plow breaks when an UNBLOCKED hit leaves it at or below 160 HP (102 damage from full): its Strength is removed, it is stunned (next turn lost) and phase 2 starts: Beast Cry (applies Ringing: on my next turn only ONE card may be played) -> Stomp 17 -> Crush 19 and +4 Strength -> Cry -> Stomp -> Crush. So: FD plus Vulnerable to reach 160 in the first two turns (Break, Uppercut, Bash, Rampage), FB for the Plow turns, one big card for each Ringing turn (draw and cheap-card chains are dead then), and SD or burst for phase 2 because Crush adds 4 Strength per cycle. A10 numbers assume both ascension tiers (HP and Plow values) are active: check the HP at the fight (262). `[hyp]` the target deck and potion numbers: test with `eval --boss --v` variants once a deck is built.
- **Waterfall Giant** (A1 Underdocks): sustained damage and block over 250 HP. 0.43-0.50 at 80 HP, weakest of its pool `[sim]`.
- **Lagavulin Matriarch** (A1 Underdocks): SD and vulnerability; early turns are not the threat. Inflame + Tremble 0.4% -> 74% `[sim]`.
- **Soul Fysh** (A1 Underdocks): low demand, 0.98 for a mid deck `[sim]`.
- **The Insatiable** (A2 Hive, 341 HP): hard clock. Liquify (T1) puts Sandpit 4 on me (-1 per enemy turn, 0 = death) and shuffles in 6 Frantic Escape (cost 1, +1 per play, each +1 Sandpit); then Thrash 9x2, Bite 31, Salivate (+3 Str), Thrash. Needs ~65 damage per turn for 5 turns: SD + burst + ACC `[code]` (TheInsatiable.cs, SandpitPower.cs, FranticEscape.cs). Won at 8 HP with Strength 5-7 `[played]`.
- **Aeonglass** (A3 Glory, 535 HP): Artifact 3 (wastes the first three debuffs), Withering Presence 6, Ebb 26 + 33 block, Eye Lasers 12x2, Increasing Intensity (+4 Str, Wither status); a Wither card every 6 cards played `[code]` Aeonglass.cs. Act 2-clear deck 7% at 76 HP `[played]`.
- **Queen** (A3 Glory, 419 HP): kill Torch Head Amalgam first; Execution 18, Off With Your Head 4x5, Enrage; applies Frail / Weak / Vulnerable 99 `[code]` Queen.cs. 0% for the Act 2-clear deck `[sim]`.
- **Test Subject** (A3 Glory): three forms (111 -> 212 -> 313 HP), respawns `[played]` run 4.
- Act 3 sweeps `[sim]` (run 4, Act 2-clear deck, 80 HP): baseline ~0% on Aeonglass and Queen; no single Ironclad card (88 tested) or relic (215) moves Aeonglass more than +4 or the Queen more than +3 (noise ~1-2); bundles of six strong cards 3-4% / 0%. Glory elites are fine (83-98%).

## Elites and hallways
- **Bygone Effigy** (A1): SD; sleeps first turn; starter deck predicted win 2% `[sim]`.
- **Phrog Parasite** (A1): AoE; dies into four Wrigglers (stunned first turn) plus Infection cards `[played]`.
- **Skulking Colony** (A1, 80 HP): FD to break Hardened Shell (20 per hit class); 16 attack; cost 37 HP at 68 HP `[played]`.
- **Terror Eel** (A1): 150 HP, Shriek at 75; SD and block `[played]`.
- **Spiny Toad** (A2 hallway): Thorns 5 while Spiked (each powered hit on it costs me 5), Explosion 25, Lash 19 `[code]` SpinyToad.cs; predicted 13-17% HP, lost 55% `[played]` (calibration: `evidence.md`).

## Target decks (check every reward, shop and event against them)
- Insatiable: Strength scaling + burst + ACC (Demon Form + Bludgeon + Tremble + Offering): 60% vs 2% baseline `[sim]`.
- Aeonglass: self-damage Strength engine (Inferno + Rupture + Demon Form + Fight Me + Spite + Stone Armor + Crimson Mantle), not exhaust: 0.61 vs 0.00-0.21 for hand-built exhaust decks `[sim]` (`evidence.md`).
- Queen: high sustained damage plus HP carry `[hyp]`; test: boss-alone `eval` of candidate decks.

## Potions `[sim]`
Against hard bosses a potion beats every card in the same shop: Powdered Demise +14..+15 and Flex Potion +9..+11 (The Insatiable, Act 2 elites), Block Potion +12. Test: the potion as an `eval` variant against the known boss.
