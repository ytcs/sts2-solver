# Boss and elite gimmicks (questions, not answers)
What makes each fight hard. The operator reasons out whether the deck answers each gimmick and what is missing (`sts2-plan` Answers), then measures (`eval --enc ID --v`, `eval --pool <Act>:boss|elite`).

## Bosses
- Vantom (183 HP) `[code]`: Slippery 9 (each unblocked hit deals 1 and removes a stack); Ink Blot 8 -> Inky Lance 7x2 -> Dismember 30 + 3 Wounds -> +2 Str, repeating.
- Ceremonial Beast (262 HP) `[code]`: Plow 160; an unblocked hit leaving it <= 160 breaks Plow (stun, phase 2: Ringing = one card next turn, Stomp 17, Crush 19 +4 Str).
- Waterfall Giant `[code]`: Steam Eruption explodes after the kill (a stunned turn, then the blast).
- Lagavulin Matriarch `[hyp]`: sleeps early, then scales.
- Soul Fysh (221 HP) `[code]`: Beckon cards (6 HP loss if in hand at end of turn), Fade = Intangible 2, Scream 15 + Vulnerable.
- The Insatiable (341 HP) `[code]`: Sandpit 4 counts down to your death; Frantic Escape cards push it back (`[hyp]` each costs 1 more per play).
- Kaiser Crab (219 + 209 HP) `[code]`: the crab behind you deals x1.5; when one dies the other gains +6 Str and 99 block; Laser 35.
- Aeonglass (535 HP) `[code]`: Artifact 3, Wither card every 6 cards played, Ebb 26 + 33 block; `[hyp]` Wither deals 6 if held at end of turn, Increasing Intensity +4 Str.
- Queen (419 HP) `[code]`: Torch Head Amalgam beside her.
- Test Subject (111 -> 212 -> 313 HP) `[code]` (content/monsters/glory_b.rs ts_respawn): a death revives it on its next turn, which deals no damage; the revive strips its Strength / Enrage and its debuffs. Phase 2: Painful Stabs (your unblocked hits on it add Wounds), Multi Claw 11 x (3, 4, 5 ...) one more hit per turn. Phase 3: Nemesis (Intangible every other turn), Big Pounce 45, Burning Growl (+5 Burns, +3 Str).

## Elites and hallways
- Infested Prisms (171 HP) `[code]`: each Skill you play gives you Tainted 3; Vital Spark; 9x3 attacks.
- Skulking Colony `[hyp]`: Hardened Shell (damage it can take per turn is capped); gains Strength.
- Terror Eel (150 HP) `[hyp]`: Shriek at 75 HP; Crash / Thrash alternate with Vigor.
- Bygone Effigy `[hyp]`: sleeps turn 1. Phrog Parasite `[hyp]`: spawns Wrigglers, area pressure.
- Scrolls of Biting (Glory weak) `[hyp]`: Paper Cuts (each unblocked hit costs max HP). MANUAL (`data/drive_manual.json`).
- Spiny Toad (hallway) `[code]`: Thorns 5 while Spiked, Explosion 25.
- Thieving Hopper `[hyp]`: steals a card, flees after a few turns (Escape Artist).
- Gremlin Merc `[hyp]`: Thievery (steals gold), splits into gremlins on death.
