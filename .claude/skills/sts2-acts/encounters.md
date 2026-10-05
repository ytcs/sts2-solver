# What each boss and elite asks of a deck (fill as learned; one row per encounter)

Use with `sts2-deckbuilding` step 2. Tags: `[code]`, `[sim]`, `[played]`, `[hyp]`. Buckets: FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration.

| encounter | asks (needs) | notes and evidence |
|---|---|---|
| Vantom (Act 1 boss, Overgrowth) | FB for a ~30 hit about every 4th turn (14 / 30 / 0 expected damage pattern); cheap many-hit damage or HP-loss triggers to strip Slippery 9; burst on buff turns | Slippery: every unblocked hit deals 1 and removes one stack `[code]` `[played]`; with Inferno+ Blood Wall stripped a stack per HP loss `[played]`; win 0.46 at 60 HP, 0.11 at 45 `[sim]` |
| Waterfall Giant (Act 1 boss, Underdocks) | sustained damage and block over a long fight (250 HP) | weakest boss for a Strike/Hemokinesis/Dismantle deck, ~0.43-0.5 at 80 HP `[sim]` |
| Lagavulin Matriarch (Act 1 boss, Underdocks) | SD and vulnerability (Inflame + Tremble took win from 0.4% to 74%) | a scaling fight; early turns are not the threat `[sim]`; verify mechanics when first seen |
| Soul Fysh (Act 1 boss, Underdocks) | low demand: 0.98 for a mid-strength deck | `[sim]` |
| Skulking Colony (elite) | FD to break Hardened Shell 20 per hit class; 16 attack | 80 HP; cost 37 HP at 68 HP with a mid deck `[played]` |
| Bygone Effigy (elite, Overgrowth) | SD; sleeps first turn | `[played]` |
| Phrog Parasite (elite, Overgrowth) | AoE for the wrigglers | `[played]`, see `sts2-mechanics` |
| Terror Eel (elite, Underdocks) | 150 HP, Shriek at 75; SD and block | `[played]` |
| The Insatiable (Act 2 boss, Hive) | 341 HP, a hard clock: Liquify (T1) puts Sandpit 4 on me (-1 per enemy turn, 0 = instant death) and shuffles in 6 Frantic Escape cards (cost 1, +1 per play, each +1 Sandpit); then Thrash 9x2, Bite 31, Salivate (+3 Str), Thrash. Needs ~65 damage per turn for 5 turns: SD + burst + acceleration | `[code]` TheInsatiable.cs, SandpitPower.cs, FranticEscape.cs. Eval target deck Demon Form + Bludgeon + Tremble + Offering 60% vs 2% baseline; Powdered Demise +14, Flex +11; won at 8 HP with Strength 5-7, Pyre+, Tea Set `[played]` |
| Aeonglass (Act 3 boss 1, Glory) | 535 HP, Artifact 3, Withering Presence 6; Ebb 26 + 33 block, Eye Lasers 12x2, Increasing Intensity (+4 Str, Wither status) | `[code]` Aeonglass.cs; this deck 7% at 76 HP, died `[played]` |
| Queen (Act 3 boss 2, Glory) | 419 HP, Torch Head Amalgam, Execution 18, Off With Your Head 4x5, Enrage | `[code]` Queen.cs; 0% for this deck `[sim]` |
| Spiny Toad (Act 2 hallway) | Thorns 5 while Spiked (each powered hit on it costs me 5), Explosion 25, Lash 19 | `[code]` SpinyToad.cs; predicted 13-17% HP, lost 55% `[played]` |
