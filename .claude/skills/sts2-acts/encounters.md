# What each boss and elite asks of a deck
For `sts2-deckbuilding` section 4. FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration. Test any line: `eval --pool <Act>:boss|elite` or boss-alone `--v` variants.

## Bosses
- Vantom (A1 Overgrowth, 183 HP, Slippery 9) `[code]` (`monsters/overgrowth_a.rs`): cycle Ink Blot 8 -> Inky Lance 7x2 -> Dismember 30 + 3 Wounds to discard -> Prepare +2 Str. Asks: SD > FB timed for Dismember (Ice Cream banks energy) > big hit after Slippery > draw/exhaust for Wounds. `[sim]` at 60 HP, starter+5 cards + Ice Cream (base 0.47, se ~0.05): Demon Form +0.29, Rolling Boulder +0.26, Flame Barrier +0.17, Aggression/Tear Asunder +0.16, Uppercut/Primal Force +0.15, Shrug +0.12, Bludgeon/Inflame +0.08; Twin Strike/Anger/Sword Boomerang/Battle Trance ~0, Burning Pact -0.09. 0.47 at 60 HP, 0.26 at 53: HP-gated once SD is in.
- Ceremonial Beast (A1 Overgrowth, 262 HP) `[code]` (CeremonialBeast.cs, PlowPower.cs, RingingPower.cs): T1 Stamp (Plow 160); then Plow 20 +2 Str per turn. An unblocked hit leaving it <= 160 HP (102 from full) breaks Plow: Str removed, stunned, phase 2: Beast Cry (Ringing: one card next turn) -> Stomp 17 -> Crush 19 +4 Str, repeat. Asks: FD + Vulnerable to break in two turns, FB for Plow turns, one big card per Ringing turn, SD/burst for phase 2. `[hyp]` target deck + potions: `eval --boss --v`.
- Waterfall Giant (A1 Underdocks, 250+ HP): sustained damage + block. 0.43-0.50 at 80 HP, weakest of its pool `[sim]`.
- Lagavulin Matriarch (A1 Underdocks): SD + Vulnerable; early turns harmless. Inflame + Tremble 0.4% -> 74% `[sim]`.
- Soul Fysh (A1 Underdocks, 221 HP) `[code]` (`monsters/underdocks_b.rs`): cycle Beckon (1 to draw, 1 to discard) -> De-Gas 18 -> Gaze 8 + Beckon -> Fade (Intangible 2) -> Scream 15 + Vulnerable 3. Beckon: cost 1, 6 unblockable HP loss at end of turn in hand. Asks: FD + SD to deal 221 in ~3 non-Intangible turns per cycle, powers/block on Fade turns, energy/exhaust for Beckons. Damage-gated for starter, HP-gated once FD + SD are in.
- The Insatiable (A2 Hive, 341 HP) `[code]` (TheInsatiable.cs, SandpitPower.cs, FranticEscape.cs): T1 Liquify: Sandpit 4 (-1 per enemy turn, 0 = death) + 6 Frantic Escape (cost 1, +1 per play, each +1 Sandpit); then Thrash 9x2, Bite 31, Salivate +3 Str, Thrash. Needs ~65 damage/turn for 5 turns: SD + burst + ACC.
- Kaiser Crab (A2 Hive) `[code]` (`monsters/hive_b.rs`, `powers/hive_b.rs`): Crusher (left, 219 HP) + Rocket (right, 209). Surrounded: the crab behind me (not last targeted) deals x1.5; targeting turns me. Crab Rage: one dies -> other +6 Str, 99 block. Crusher: Thrash 14 -> Enlarging Strike 4 -> Bug Sting 7x2 + Weak 2 + Frail 2 -> Adapt +3 Str -> Guarded Strike 14 + 18 block. Rocket: Reticle 4 -> Precision Beam 20 -> Charge Up +3 Str -> Laser 35 -> Recharge. Asks: ~430 HP of SD + area damage (no turning), FB for Laser, kill both close together. `[hyp]` target deck: `eval --boss --v`.
- Aeonglass (A3 Glory, 535 HP) `[code]` Aeonglass.cs: Artifact 3, Withering Presence 6, Ebb 26 + 33 block, Eye Lasers 12x2, Increasing Intensity +4 Str + Wither; a Wither card every 6 cards played.
- Queen (A3 Glory, 419 HP) `[code]` Queen.cs: kill Torch Head Amalgam first; Execution 18, Off With Your Head 4x5, Enrage; Frail/Weak/Vulnerable 99. 0% for an Act 2-clear deck `[sim]`.
- Test Subject (A3 Glory): three forms 111 -> 212 -> 313 HP, respawns. `[hyp]` Test: decomp or sim port.
- Act 3 sweeps `[sim]` (Act 2-clear deck, 80 HP): ~0% vs Aeonglass and Queen; no single Ironclad card (88) or relic (215) moves Aeonglass > +4 or Queen > +3; six-card bundles 3-4% / 0%. Glory elites 83-98%.

## Elites and hallways
- Infested Prisms (A2 elite, 171 HP) `[code]` (`monsters/hive_b.rs` VitalSpark/Tainted): every Skill Tainted; playing one gives me Tainted 3 (each enemy hit +3, stacking, to end of enemy turn) vs its 9x3. Win with Attacks, Powers, potions; one turn faster ~20 HP.
- Bygone Effigy (A1): SD; sleeps T1; starter deck 2% `[sim]`.
- Phrog Parasite (A1): area damage; four Wrigglers + Infections `[hyp]`.
- Skulking Colony (A1, 80 HP): FD to break Hardened Shell (20 per hit class); attack 16 `[hyp]`.
- Terror Eel (A1): 150 HP, Shriek at 75; SD + block `[hyp]`.
- Spiny Toad (A2 hallway) `[code]` SpinyToad.cs: Thorns 5 while Spiked, Explosion 25, Lash 19.

## Target decks (check every reward, shop, event)
- Insatiable: Strength + burst + ACC (Demon Form, Bludgeon, Tremble, Offering): 60% vs 2% `[sim]`.
- Aeonglass: self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle): 0.61 vs 0.00-0.21 for exhaust decks `[sim]`.
- Queen: sustained damage + HP carry `[hyp]`. Test: boss-alone `eval` of candidate decks.

## Potions `[sim]`
Vs hard bosses a potion beats every card in the same shop: Powdered Demise +14..+15, Flex Potion +9..+11 (Insatiable, Act 2 elites), Block Potion +12. Test: potion as `eval` variant vs the known boss.
