# What each boss and elite asks of a deck
For `sts2-deckbuilding` section 4 (buckets FD SD FB SB ACC). Test any line: `eval --pool <Act>:boss|elite` or boss-alone `--v`.

## Bosses
- Vantom (183 HP, Slippery 9) `[code]`: Ink Blot 8 -> Inky Lance 7x2 -> Dismember 30 + 3 Wounds -> +2 Str. Asks SD, FB timed for Dismember, big hit after Slippery. `[sim]` 0.47 at 60 HP, 0.26 at 53: HP-gated once SD is in.
- Ceremonial Beast (262 HP) `[code]`: Plow 160; an unblocked hit leaving it <= 160 breaks Plow (stun, phase 2: Ringing = one card next turn, Stomp 17, Crush 19 +4 Str). Asks FD + Vulnerable to break in two turns.
- Waterfall Giant: weakest of its pool, 0.43-0.50 at 80 HP `[sim]`.
- Lagavulin Matriarch: SD + Vulnerable; early turns harmless `[sim]`.
- Soul Fysh (221 HP) `[code]`: Beckon (6 HP loss if in hand at end of turn), Fade = Intangible 2, Scream 15 + Vulnerable. Asks 221 damage in ~3 non-Intangible turns.
- The Insatiable (341 HP) `[code]`: Sandpit 4 counts down to death; Frantic Escape cards push it back. ~65 damage/turn for 5 turns: SD + burst + ACC.
- Kaiser Crab (219 + 209 HP) `[code]`: the crab behind you deals x1.5; one dies -> other +6 Str, 99 block; Laser 35. Asks area SD, kill both close together.
- Aeonglass (535 HP) `[code]`: Artifact 3, Wither card every 6 cards played, Ebb 26 + 33 block.
- Queen (419 HP) `[code]`: kill Torch Head Amalgam first.
- Test Subject: forms 111 -> 212 -> 313 HP `[hyp]`; test: decomp.
- Act 3 sweeps `[sim]` (act-2-clear deck, 80 HP): ~0% vs Aeonglass and Queen; no single Ironclad card or relic adds > +4; Glory elites 83-98%.

## Elites and hallways
- Infested Prisms (171 HP) `[code]`: each Skill played gives you Tainted 3 vs its 9x3: win with Attacks/Powers/potions.
- Bygone Effigy: sleeps T1, SD; starter 2% `[sim]`. Phrog Parasite: area damage `[hyp]`. Skulking Colony: FD vs Hardened Shell `[hyp]`. Terror Eel (150 HP, Shriek at 75) `[hyp]`.
- Spiny Toad (hallway) `[code]`: Thorns 5 while Spiked, Explosion 25.

## Target decks `[sim]`
- Insatiable: Strength + burst + ACC (Demon Form, Bludgeon, Tremble, Offering) 0.60 vs 0.02.
- Aeonglass: self-damage Strength engine 0.61 (`sts2-deckbuilding/evidence.md`).
- Queen: sustained damage + HP carry `[hyp]`.

## Potions `[sim]`
Vs hard bosses a potion beats any card in the same shop: Powdered Demise +14, Flex +9..+11, Block +12. Test: potion as `eval` variant.
