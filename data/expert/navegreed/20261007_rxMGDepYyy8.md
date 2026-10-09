# NaveGreed "Most Underrated and Most Overrated" (A10 Ironclad): full run record
- url https://www.youtube.com/watch?v=rxMGDepYyy8 | uploaded 2026-10-07 | 74 min edited stream | build `v0.111.0 (2026.08.14)`, `HASH [1568834832]`, `MODDED (3)` (no gameplay effect seen) | seed `3EC3BCK90DQX` (1440p read; 0 not O by alphabet, B not 8 confirmed by the headless replay) | won: Queen (F49) dead, 47/95 HP, 197 gold.
- Run record: `rxMGDepYyy8.compact.jsonl`: 209 steps, 30 fights, 920 combat actions. Pilot extraction (3 fights, macro list): `../navegreed_2026-10-07.*`.
- Headless check (oracle `run-replay`, game run code): the record replays from the seed to the end, 47/95 HP and 197 gold as in the video; `tools/expert.py check-trace`: every fight turn's frame hand, HP and enemy HP match the game's state except F48 T5 (frames list a Wither the game adds after the first decision). Live real-game replay: not run yet.

## Transcription (2026-10-09)
- Frames from the 1440p60 stream (av01, git-ignored), read at 1-2 fps with zooms; Act 1 by hand, Acts 2-3 by four parallel transcribers, merged in game order; each block checked by the simulator build, then the oracle.
- Inferred by lookahead (3): F17 T3 Headbutt pick (Strike; read as the hovered Defend, the turn-4 hand needs Strike), F40 T5 Headbutt pick (Burning Pact, hidden selection), F48 T7 Burning Pact exhaust (the start-of-turn Wither, from pile counts). No gaps.
- Fixed by the oracle: F39 map node is a Monster node (r5c4), not Unknown; F49 T7 he plays Armaments+ last (69:57); hands drawn by Cruelty (Swift) at F35/F49 T1 belong after the play, not at turn start.
- Pilot corrections: F3 SEAPUNK_WEAK, F8 SEAPUNK_NORMAL, F13 PUNCH_CONSTRUCT_NORMAL, F44 GLOBE_HEAD_NORMAL; Anger is not upgraded at the end of Act 1; slot 1 from F14 is a Skill Potion.

## Record format additions
- Out-of-combat potion: `{potion: slot, id}` on a room screen (F26 Fruit Juice on the treasure) or the rewards screen (F44 Blood Potion). Oracle: `RunReplay.DrinkPotion`; live: `macro_command` picks the `potion <name>` option. The bridge lists AnyTime potions on room screens; the rewards-overlay listing is added in `mods/AgentBridge/src/Decisions.cs` (needs a mod rebuild before the live replay reaches F44).
- Thieving Hopper's returned card: reward pick by card name (`Burning Pact`).
- The Future of Potions: event pick `Lose <Potion>`; its card reward is scripted by the REWARDS/CARD_REWARD steps after the event.
- SELECT and in-card picks honor `+` (upgraded vs plain copy) in the oracle, as the live harness does.

## Simulator fidelity found (frame builds)
- Bound (Chains of Binding, F49): not in snapshot or sync; the seed-1 Replayer binds other cards (F49 replay check fails at Rage+).
- Headbutt's discard pick: `legal()` label index and the moved card disagree after sync (F17, F40).
- Decimillipede: dead segments keep their index (targets stay 0/1/2); opening HP and Writhe/Constrict order differ before sync (F25, F31).
- Tear Asunder hit count: the combat's HP-loss counter is not synced (F48, F49).
- Enemy HP rolls and summon HP (Ovicopter eggs, Bowlbugs, Seapunk) differ before sync; Kaiser Crab facing intents synced only.
