# Game-code facts: enemy moves, cross-character cards, run generation

Cite: `X/File.cs:N` = `decomp/MegaCrit.Sts2.Core.X/File.cs` line N. Rust paths without crate = `crates/sts2sim/src/`. Status words: done / open.

## A. Enemy move determination
Information contract: future enemy moves are public only as pattern knowledge (fixed where fixed, odds at random branches); realized branches under the live RNG are hidden.

### A1. When and how moves are rolled
- One run-wide `MonsterAi` stream (`Runs/RunRngSet.cs:77, 117-121, 139-143`: `Seed + xxHash64(snake_case(name))`), shared by every monster of every fight, never reset per combat (`Models/MonsterModel.cs:418`).
- Machine built in `SetUpForCombat` (`Models/MonsterModel.cs:410-414`). Every enemy rolls at the start of each player turn, after start-of-turn hooks (`Combat/CombatManager.cs:721, 739-744`; `Entities.Creatures/Creature.cs:547-553`); a monster added mid-turn rolls immediately (`CombatManager.cs:1131-1133`). So a shown intent never changes from player action except forced moves (A2).
- Roll (`MonsterMoves.MonsterMoveStateMachine/MonsterMoveStateMachine.cs:54-80`): no draw if the state cannot be left yet or is the unperformed opening move; else follow `GetNextState` to a `MoveState`; only the first loggable state enters `StateLog`.
- Node types (same dir): `MoveState` (fixed follow-up, no RNG; optional must-perform-once, `MoveState.cs:27-37, 67-70`); `ConditionalBranchState` (first true predicate, `:50-60`); `RandomBranchState` (always exactly one draw, `:115-128`; repeat rules `:130-167`, `MoveRepeatType.cs`: UseOnce, CannotRepeat / up to N, Cooldown K).

### A2. Future sequence = f(RNG, own state, live combat state)
Conditional branches on live state (`Models.Monsters/`):
| Monster | Predicate | Player lever | Cite |
|---|---|---|---|
| FrogKnight | own HP < 50% | damage | `FrogKnight.cs:75-76` |
| LagavulinMatriarch / SlumberingBeetle | Asleep / Slumber | unblocked damage wakes; else counts down | `LagavulinMatriarch.cs:173-174`, `SlumberingBeetle.cs:115-116` |
| BowlbugRock | Imbalanced | fully block its attack | `BowlbugRock.cs:76-77`, `Models.Powers/ImbalancedPower.cs:19-28` |
| LivingShield / Nibbit / Toadpole | ally count / alone / in front | kill allies | `LivingShield.cs:45-46`, `Nibbit.cs:77-82`, `Toadpole.cs:79-80` |
| Queen | Amalgam dead | kill Amalgam | `Queen.cs:145-149` |
| Fabricator / Ovicopter | alive count | kill bots / eggs | `Fabricator.cs:63-64`, `Ovicopter.cs:71-72` |
| TestSubject | respawn count | kill it | `TestSubject.cs:210-211` |
| KnowledgeDemon | own curse counter | none | `KnowledgeDemon.cs:139-140` |
- Slot branches (Exoskeleton, Myte, PhantasmalGardener, Wriggler) are static (`Exoskeleton.cs:62-65`). Only state-dependent weight: TwoTailedRat summon (countdown, call count, free slots, other rats' pending moves; `TwoTailedRat.cs:125-128, 194-218`).
- Forced moves: stun replaces the pending move, performed once (`Entities.Creatures/Creature.cs:525-544`), from Asleep/Slumber on unblocked damage, Burrowed block broken, Flutter on powered attacks, Plow/Shriek HP thresholds, Ravenous on ally death, Whistle (`Models.Powers/*Power.cs`, `Models.Cards/Whistle.cs:33`). Asleep/Slumber also wake at 0 by countdown (`AsleepPower.cs:46-55`, `SlumberPower.cs:40-45`). `SetMoveImmediate`: Queen -> Enrage (`Queen.cs:230-232`), TestSubject `:171`, WaterfallGiant `:308`, Illusion `:88`, Reattach `:62`, ToughEgg `:139`.

### A3. Other MonsterAi consumers
Fabricator's bot pick (`Fabricator.cs:115`; sim `content/monsters/glory_a.rs`). Flutter's `GetNextState` call draws nothing (`FlutterPower.cs:48`). Shared stream changes realized outcomes only, not pattern odds.

### A4. Simulator look-ahead (done, S1)
- `Combat::lookahead` projects the next `LOOK_H`=4 turns on a copy of the combat (own counters/powers tick, summons, joint encounter, buffs over the horizon), forks random nodes by weight, draws no RNG. Rows: per future turn move-slot probabilities + expected attack damage; `intent_plan` -> (move, p, text) measured by performing each move on a copy (live display `agent/live.py`). Observation carries pending node and stored follow-up (`enemy_moves`). Cached (`docs/solver.md`, performance invariants); `LOOK_LEGACY` switches back to the pre-S1 per-machine walk.
- `rng.rs` is a bit-exact port (xoshiro256**, splitmix, xxh64 names). Live play has no seed: the replayer resamples seeds until simulated intents match the game (`agent/fight.py`), so machine state is exact and the RNG a consistent sample.
- Open: player-controllable branches are projected as status quo (no "what-if" rows; `bound_enter` gives the all-branches superset).

### A5. Bridge export
Per enemy `next_move` id and intents with damage/hits (`mods/AgentBridge/src/Snap.cs`); seed is a placeholder by design. The game `Rng` is serialisable (`Random/Rng.cs:32-36, 356-364`) but exporting it would leak hidden state: never.

### A6. Representation rule
Keep per-monster pattern odds, project everything predictable without RNG; never realized branches.

## B. Cross-character mechanics
Exactly one character-dependent combat rule: Channel with no orb slots. Stars, Osty/Summon, Doom, Focus, Shiv/Sly are character-agnostic; simulator matches all.

### B1. Orbs on a non-Defect
`CharacterModel.BaseOrbSlotCount => 0` (`Models/CharacterModel.cs:101`), Defect 3 (`Models.Characters/Defect.cs:63`); combat start sets capacity (`Entities.Players/PlayerCombatState.cs:137-139`), cap 10 (`Entities.Orbs/OrbQueue.cs:15`). `OrbCmd.Channel` (`Commands/OrbCmd.cs:68-92`): character constant 0 and capacity 0 -> add 1 slot; full -> evoke front; capacity 0 otherwise -> orb lost silently (`OrbQueue.cs:52-55`; a Defect at 0 slots after Bulk Up loses orbs). Dualcast no-op without orbs; Essence of Darkness one Dark per slot; passives fire for every player (`Combat/CombatManager.cs:792, 1604`); Focus owner only; death clears queue. Sim: `engine/orbs.rs`, `content/powers/defect.rs`, `content/potions/defect.rs`, `engine/death.rs`; start slots from scenario (converter defaults missing `base_orb_slots` to the character's value: done). Edge: re-entrant full queue throws in game, sim drops the orb.

### B2. Stars on a non-Regent
Plain int on `PlayerCombatState` (`:23, 103-119`), clamped at 0, not reset per turn; `StarCostTooHigh` -> unplayable (`PlayerCombatState.cs:190-209`, `Models/CardModel.cs:1738`). Divine Right +3 on combat start (`Models.Relics/DivineRight.cs:16-21`). No character checks beyond UI/animation. Sim: `engine/energy.rs`, `engine/play.rs`, `engine/regent.rs`, `content/relics/shared_misc.rs`.

### B3. Osty / Summon / Doom without Necrobinder
`OstyCmd.Summon` works for anyone: alive -> max HP, dead -> revive, else new Osty with `DieForYouPower` (`Commands/OstyCmd.cs:35-91`). Osty attacks without Osty: playable, do nothing (`Models.Cards/Poke.cs:29-35`); Bone Shards block+kill inside the Osty branch; Snap still asks Retain; only High Five unplayable (`HighFive.cs:18`). Doom generic (`Models.Powers/DoomPower.cs:60-73, 146`). Sim: `engine/pets.rs`, `content/cards/necrobinder_osty.rs`, `content/powers/necrobinder.rs`.

### B4. Generators
Use the owner's pool (`Owner.Character.CardPool`): Infernal Blade, White Noise, Distraction, Discovery, Jackpot, Metamorphosis, Calamity, Creative AI, Attack/Skill/Power potions (`Models.Cards/InfernalBlade.cs:22`, `WhiteNoise.cs:23`). Splash takes the other pools (`Splash.cs:25-30`). Sim: `character_pool()` (`engine/cmds.rs`).

### B5-B6. Verification (done)
`tools/fuzz_gen_mix.py` draws 1-4 foreign-pool cards in 30% of decks (`--cross`) and `--focus cross` themed slices; oracle rounds (3,000 cross, 2,500 mixed, 700 Bulk Up / Capacitor / orb-potion) 0 mismatches; frozen as `oracle/regression/cross_*` and `defect_missing_base_orb_slots`. Diff compares `orbs`, `orb_capacity`, `stars` (`crates/sts2diff/src/snapshot.rs`). Run-level sources of foreign cards: C2.

## C. Run-level generation (for the run model, `agent/runmodel.py`)

### C0. RNG streams
`MegaRandom` = xoshiro256** via splitmix64 (`Random/MegaRandom.cs:48-66, 168-187`); `NextFloat(a,b)` = `(float)(NextDouble*(b-a)+a)`, `NextItem` = `NextInt(0,n)` (`Random/Rng.cs:170-177, 289-298`); `UnstableShuffle` back-to-front Fisher-Yates, `StableShuffle` sorts first (`Extensions/ListExtensions.cs:22-59`). Run seed `xxHash64(utf8)`; 12 run streams (`Entities.Rngs/RunRngType.cs`): up_front, shuffle, unknown_map_point, combat_card_generation, combat_potion_generation, combat_card_selection, combat_energy_costs, combat_targets, monster_ai, niche, combat_orbs, treasure_room_relics. Per player (seed `hash+slot`): rewards, shops, transformations (`Random/PlayerRngSet.cs:19-46`); rarity and potion odds use Rewards. Ad hoc: map `"act_{i+1}_map"` (`Map/StandardActMap.cs:113`), events `Seed + (IsShared?0:slot) + hash(id)` (`Models/EventModel.cs:234`). Up-front order: shared relic bag, player bags, `GenerateRooms` (`Runs/RunManager.cs:522-526, 743-766`; `Models/ActModel.cs:331-386`). Port done (`rng.rs`).

### C1. Map
7 columns x (rooms+1) rows (`Map/StandardActMap.cs:89-91`); rooms Overgrowth 15, Underdocks 15, Hive 14, Glory 13; ancient row 0, boss last, A10 second boss (`:94-99, 226-229`); 7 walks of shuffled {-1,0,+1} steps, no crossing (`:145-221`). Rests: Gaussian(7,1) clamp [6,7] (Overgrowth/Underdocks), Gaussian(6,1) clamp [6,7] (Hive), `NextInt(5,7)` (Glory). Unknowns Gaussian(12,1) clamp [10,14], -1 in Hive/Glory (`Map/MapPointTypeCounts.cs:516-519`). Shops 3; elites 5 (8 at A1). Fixed: row 1 Monster, Treasure at rooms+1-7, Rest on last row (`StandardActMap.cs:248-275`). Placement 3 passes; no Rest/Elite below row 6, no parent->child repeat of Elite/Rest/Treasure/Shop, no same-type siblings (`:405-485`); then path pruning (`Map/MapPathPruning.cs`). Golden Compass -> `GoldenPathActMap`. Seed-exact port hard; distribution clone medium.

### C2. Card rewards
Monster: gold, potion roll, 3 cards; Elite + relic; Boss: 100 gold, potion roll, 3 rare (`Rewards/RewardsSet.cs:206-246`). Rarity: one float u; rare if u < base+offset, uncommon below +baseUncommon (`Odds/CardRarityOdds.cs:96-111`).
| odds | rare (normal / A7+) | uncommon |
|---|---|---|
| regular | 3% / 1.49% | 37% |
| elite | 10% / 5% | 40% |
| shop | 9% / 4.5% | 37% |
| boss | 100% | - |
Offset starts -5%, resets after a rare, else +1% (+0.5% A7), cap +40%; advances only on encounter rewards; events/relics roll base odds; shop reads, never changes (`CardRarityOdds.cs:27-31, 69-81, 120-134`; `Factories/CardFactory.cs:50, 244-260`). Empty rarity -> next with wrap; no duplicates per screen. Upgrade roll non-rare actIndex x 0.25 (x0.125 A7), float drawn before upgradability check (`CardFactory.cs:283-305`). Order on Rewards: potion decision, gold, potion id, per card rarity -> item -> upgrade, relic. Gold: monster 10-20, elite 35-45, boss 100, x0.75 at A3 (`Models/EncounterModel.cs:64-99`).
Foreign-card sources (complete): Prismatic Gem (Orobas, all pools, `Models.Relics/PrismaticGem.cs:28-46`), Sea Glass (Orobas, 15 from one other pool; Orobas offers Gem 1/3 else Sea Glass, `Models.Events/Orobas.cs:188-211`), Kaleidoscope (Neow, all pools unlocked), Colorful Philosophers (Hive event, 3 rewards of one other colour), Splash (combat), CharacterCards modifier. Transforms keep the card's pool; Quest/Event/Ancient/Token -> Colorless (`CardFactory.cs:170-211`). Colorless: Dingy Rug, shop's 2 slots, transforms.

### C3. Shop
5 character cards (A, A, S, S, P; shop odds; one random slot half price), 2 colorless (U, R), 3 relics (rarity, rarity, Shop tier; rarity 50/33/17 on **Rewards**; back of player bag, `IsAllowedInShops`), 3 potions, removal (`Entities.Merchant/MerchantInventory.cs:15-148`, `Factories/RelicFactory.cs:80-94`). Prices: cards 50/75/150, x1.15 colorless, x U(0.95,1.05), halved on sale (extra float); relics C175/U225/R275/Shop200 x U(0.85,1.15) (`Models/RelicModel.cs:304-315`); potions 50/75/100 x U(0.95,1.05); removal 75+25/use, A6+ 100+50/use (`MerchantCardRemovalEntry.cs:20-32`); price hooks `MerchantEntry.cs:19-30`. Shop cards never upgrade but consume a Rewards float.

### C4. Potion drops
Chance starts 0.40, +0.125 on elites (code, not the comment's 25%), -0.10 after a drop, +0.10 after a miss, unclamped (`Odds/PotionRewardOdds.cs:54-72`); after every monster/elite/boss fight; White Beast Statue forces. Rarity rare <= 0.10, uncommon <= 0.35, else common; without replacement from character + shared pool (`Factories/PotionFactory.cs:76-95`). Stream Rewards.

### C5. Unknown rooms
Base Monster 0.10, Treasure 0.02, Shop 0.03, Elite disabled, Event remainder; one float walked cumulatively; rolled type resets to base, others gain their base (`Odds/UnknownMapPointOdds.cs:21-47, 97, 140-175`). Reset each act; Shop excluded after a shop or if all children are shops (`Runs/RunManager.cs:660-668, 1385`). Juzu Bracelet removes Monster; Golden Compass forces Event; DeadlyEvents enables Elite. Stream UnknownMapPoint.

### C6. Events
Act lists Overgrowth 13, Underdocks 10, Hive 10, Glory 7 + 18 shared (`Models/ModelDb.cs:157-176`); shuffled with UpFront at run start; a cursor skips events failing `IsAllowed` or visited anywhere in the run (no repeats across acts; `Rooms/RoomSet.cs:70-125`); `ModifyNextEvent` hook. ~36 `IsAllowed` overrides, e.g. Crystal Sphere act >= 2 and gold >= 100, Relic Trader act >= 2 and 5 tradable relics, Unrest Site HP <= 70%, Tea Master act < 3 and gold >= 150, Doll Room act 2. Catalog: `data/events.json` (`agent/events.py`); bodies are bespoke (hard).

### C7. Rest sites
Heal (30% max HP) and Smith (disabled if nothing upgradable), then `ModifyRestSiteOptions` (`Entities.RestSite/RestSiteOption.cs:53-74`). Relic options: Shovel Dig, Girya Lift (x3), Meat Cleaver Cook (remove 2, +5 max HP), Pumpkin Candle Kindle, Pael's Growth Clone, Byrdonis Egg Hatch. Midas modifier removes Smith.

### C8. Ancients
Act 1 Neow; Act 2 Orobas / Pael / Tezcatara; Act 3 Nonupeipe / Tanx / Vakuu; shared Darv by random prefix split (`Runs/RunManager.cs:745-752`); act's ancient `UpFront.NextItem` (`Models/ActModel.cs:385`). Neow: a curse option, coin-flip pairs (Lava Rock/Small Capsule, Oyster/Humidifier, Talisman/Pomander), 2 shuffled positives (`Models.Events/Neow.cs:220-284`). Options are a pure function of seed and ancient id. Data: `data/ancients.json`, `data/ancient_relics.json`.

### C9. Act transitions
`EnterAct`: act index, reset unknown odds, generate map (`Runs/RunManager.cs:1344-1388`). Ancient heals missing HP in full, or 80% at A2+ (`Models/AncientEventModel.cs:170-190`; Neow first sets HP to 0, so A2+ runs start at 80% max HP). After the final boss: `TheArchitect`. Encounters and bosses fixed at run start; A10 second boss differs from the first (`RunManager.cs:761-765`).

### C10. Boss rewards and treasure
No boss relic tier (`Entities.Relics/RelicRarity.cs`); the next act's ancient fills that role. Chests: rarity 50/33/17 on TreasureRoomRelics, front of the shared bag (no character relics), gold 42-52 x0.75 at A3 (`Multiplayer.Game/TreasureRoomRelicSynchronizer.cs:105-108`, `OneOffSynchronizer.cs:133-137`). Elite relics: front of the player bag, rarity on Rewards (`Runs/RelicGrabBag.cs:69-92`). Bag tiers shuffled once with UpFront; `IsAllowed` purged at pull; empty tier falls back Shop -> Common -> Uncommon -> Rare, Circlet last.

### C11. Port status
| system | difficulty | status |
|---|---|---|
| RNG | - | done (`rng.rs`) |
| map | hard exact / medium distribution | run model uses the real current map + template later acts |
| card rewards, potions, unknown rooms, rest, act transitions | easy-medium | done (`agent/runmodel.py`, `agent/tracker.py`) |
| shop | easy-medium | done (prices, removal) |
| event selection | easy | done (`data/events.json`) |
| event bodies | hard | partial (unknown ones stubbed) |
| ancients | easy | data in `data/ancients.json` |
| boss / treasure relics | medium | open (`IsAllowed`, bag order) |
| encounter tag rule (`AddWithoutRepeatingTags`) | easy | open (needs pool tags) |
