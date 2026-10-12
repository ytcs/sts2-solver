# Game-code facts: enemy moves, cross-character cards, run generation, event and ancient data
Cite `X/File.cs:N` = `decomp/MegaCrit.Sts2.Core.X/File.cs` line N. Rust paths = `crates/sts2sim/src/`.

## A. Enemy moves
Future moves are public only as pattern knowledge (fixed where fixed, odds at random branches); realized branches are hidden. Representation rule: keep per-monster pattern odds, project everything predictable without RNG, never realized branches; never export the game `Rng` (`Random/Rng.cs:32-36`).
- A1. One run-wide `MonsterAi` stream shared by every monster, never reset per combat (`Runs/RunRngSet.cs:77`, `Models/MonsterModel.cs:418`). Every enemy rolls at the start of each player turn after start-of-turn hooks (`Combat/CombatManager.cs:721, 739-744`); a mid-turn summon rolls at once (`:1131`). A shown intent changes only by forced moves. Nodes (`MonsterMoves.MonsterMoveStateMachine/`): `MoveState` (fixed follow-up), `ConditionalBranchState` (first true predicate), `RandomBranchState` (exactly one draw; repeat rules UseOnce / CannotRepeat N / Cooldown K).
- A2. Live-state branches (`Models.Monsters/`): FrogKnight own HP < 50% (`:75`); LagavulinMatriarch / SlumberingBeetle asleep until unblocked damage or countdown; BowlbugRock Imbalanced when its attack is fully blocked; LivingShield / Nibbit / Toadpole ally count; Queen Amalgam dead (`:145`); Fabricator / Ovicopter alive count; TestSubject respawns; KnowledgeDemon curse counter. Slot branches (Exoskeleton, Myte, PhantasmalGardener, Wriggler) are static; TwoTailedRat summon weight depends on state (`:125-218`). Forced moves: stun (`Entities.Creatures/Creature.cs:525`), wakes, Burrowed broken, Flutter, Plow/Shriek thresholds, Ravenous, Whistle; `SetMoveImmediate` (Queen Enrage `:230`, TestSubject, WaterfallGiant, Illusion, Reattach, ToughEgg).
- A3. Other `MonsterAi` draws: Fabricator's bot pick (`Fabricator.cs:115`; sim `content/monsters/glory_a.rs`); Flutter draws nothing.
- A4. Simulator: `Combat::lookahead` projects `LOOK_H` = 4 turns on a copy, forks random nodes by weight, draws no RNG; `intent_plan` -> (move, p, text) for live display (`agent/live.py`); cached (`docs/solver.md`). `rng.rs` is bit-exact; live play resamples seeds until simulated intents match the game (`agent/fight.py`). Open: player-controllable branches are projected as status quo (`bound_enter` gives the superset).
- A5. Bridge exports per enemy `next_move` and intents (`mods/AgentBridge/src/Snap.cs`); seed is a placeholder by design.

## B. Cross-character mechanics
One character-dependent combat rule: Channel with no orb slots. Stars, Osty/Summon, Doom, Focus, Shiv/Sly are agnostic; the simulator matches all.
- B1. Orbs: base slots 0, Defect 3 (`Models/CharacterModel.cs:101`), cap 10. `OrbCmd.Channel` (`Commands/OrbCmd.cs:68-92`): character constant 0 and capacity 0 -> +1 slot; full -> evoke front; capacity 0 otherwise -> orb lost. Passives fire for every player; Focus owner only. Sim `engine/orbs.rs`; converter defaults `base_orb_slots` to the character's.
- B2. Stars: plain int, clamped at 0, not reset per turn; `StarCostTooHigh` -> unplayable (`Entities.Players/PlayerCombatState.cs:190-209`). Sim `engine/energy.rs`, `engine/regent.rs`.
- B3. `OstyCmd.Summon` works for anyone (`Commands/OstyCmd.cs:35-91`); Osty attacks without Osty play and do nothing; only High Five unplayable. Doom generic. Sim `engine/pets.rs`.
- B4. Generators use the owner's pool (Infernal Blade, White Noise, Discovery, Jackpot, Metamorphosis, Calamity, Creative AI, Attack/Skill/Power potions); Splash takes other pools. Sim `character_pool()`.
- B5. Verified: `tools/fuzz_gen_mix.py --cross` / `--focus cross`; 6,200 oracle fights 0 mismatches; `oracle/regression/cross_*`, `defect_missing_base_orb_slots`.

## C. Run generation (run model `agent/runmodel.py`)
- C0. RNG: `MegaRandom` = xoshiro256** via splitmix64; `UnstableShuffle` back-to-front Fisher-Yates (`Extensions/ListExtensions.cs:22-59`). Run seed `xxHash64(utf8)`; 12 run streams (`Entities.Rngs/RunRngType.cs`); per player rewards, shops, transformations (`Random/PlayerRngSet.cs`); map `"act_{i+1}_map"`; events `Seed + hash(id)`. Port `rng.rs`.
- C1. Map (`Map/StandardActMap.cs`): 7 columns; rooms Overgrowth 15, Underdocks 15, Hive 14, Glory 13; ancient row 0, boss last, A10 second boss. Rests ~6-7 (Glory `NextInt(5,7)`); unknowns Gaussian(12,1) clamp [10,14], -1 in Hive/Glory; shops 3; elites 5 (8 at A1). Row 1 Monster, Treasure at rooms+1-7, Rest on the last row; no Rest/Elite below row 6, no parent->child repeat of Elite/Rest/Treasure/Shop.
- C2. Rewards (`Rewards/RewardsSet.cs:206-246`): monster gold + potion roll + 3 cards; elite + relic; boss 100 gold + 3 rares. Rare odds regular 3% (1.49% A7+), elite 10% (5%), shop 9% (4.5%); uncommon 37/40/37%. Offset starts -5%, +1% per non-rare card (+0.5% A7), reset on rare, cap +40%; only encounter rewards advance it; shop reads it (`Odds/CardRarityOdds.cs`). Upgrade roll per reward card (`Factories/CardFactory.cs:89-104, 288-305`): non-rare upgradable cards at actIndex x 0.25 (x0.125 from A7 Scarcity: A10 act 1 0, act 2 12.5%, act 3 25%); rares never; shop cards never (`CreateForMerchant` base -999999999); no model overrides `ModifyCardRewardUpgradeOdds`; `runmodel.UPGRADE_PER_ACT` matches. Relics upgrade offers after the roll, any rarity: Molten / Toxic / Frozen Egg (Attack / Skill / Power) upgrade reward and shop cards of their type and every such card added to the deck (`Models.Relics/MoltenEgg.cs:21-63`, `Helpers.Models/EggRelicHelper.cs`); Lava Lamp every combat reward after a fight with no unblocked damage taken (`LavaLamp.cs:64-88`); Silver Crucible (ancient) the first 3 card rewards (`SilverCrucible.cs:94-130`). The run model ignores all five (live offers are read upgraded from the screen). Gold monster 10-20, elite 35-45, x0.75 at A3. Foreign cards: Prismatic Gem, Sea Glass (Orobas), Kaleidoscope (Neow), Colorful Philosophers, Splash; transforms keep the pool.
- C3. Shop (`Entities.Merchant/MerchantInventory.cs`): 5 character cards (A, A, S, S, P; one half price), 2 colorless, 3 relics, 3 potions, removal. Cards 50/75/150 (x1.15 colorless) x U(0.95,1.05); relics 175/225/275/Shop 200 x U(0.85,1.15); potions 50/75/100; removal 75 +25/use, A6+ 100 +50/use.
- C4. Potion drop chance 0.40, -0.10 after a drop, +0.10 after a miss, +0.125 on elites, unclamped (`Odds/PotionRewardOdds.cs:54-72`); rarity rare <= 0.10, uncommon <= 0.35.
- C5. Unknown rooms (`Odds/UnknownMapPointOdds.cs`): base Monster 0.10, Treasure 0.02, Shop 0.03, Event rest; the rolled type resets to base, the others gain their base; reset each act; Shop excluded after a shop. Juzu Bracelet removes Monster; Golden Compass forces Event.
- C6. Events: act lists Overgrowth 13, Underdocks 10, Hive 10, Glory 7 + 18 shared (`Models/ModelDb.cs:157`), shuffled at run start, cursor skips `IsAllowed` failures and visited events (no repeats; `Rooms/RoomSet.cs:70-125`). Catalog `data/events.json`.
- C7. Rest: Heal 30% max HP, Smith; relic options (Shovel, Girya, Meat Cleaver, ...) via `ModifyRestSiteOptions`.
- C8. Ancients: act 1 Neow; act 2 Orobas / Pael / Tezcatara; act 3 Nonupeipe / Tanx / Vakuu; Darv shared. Options are a pure function of seed and ancient id. Data `data/ancients.json`, `data/ancient_relics.json`.
  - Character Ancient cards come only from two ancient relics, never rewards, shops or generators (`Factories/CardFactory.cs:161,223`): Archaic Tooth = Orobas option 3, uniform of {Touch of Orobas, Archaic Tooth} (`Models.Events/Orobas.cs:59-90,211`), transforms Bash / Neutralize / Unleash / Falling Star / Dualcast into Break / Suppress / Protector / Meteor Shower / Quadcast keeping upgrade and enchantment (`Models.Relics/ArchaicTooth.cs:26-50,149-172`); Dusty Tome = Darv's third option on `NextBool` (`Models.Events/Darv.cs:153-161`), the pool's other Ancient card, upgraded (`Models.Relics/DustyTome.cs:50-62`). Offer odds with `ancient_weights`: by act 2 Tooth 0.146, Tome 0.0625; by act 3 Tome 0.094 (one Darv per run). Taking it is 1 of 3 options, so the deck rate is lower. Generators deal at the offer odds (`tools/fuzz_gen_mix.py` `ANCIENT_OFFER`).
- C9. Act transition: ancient heals missing HP in full, 80% at A2+ (`Models/AncientEventModel.cs:170-190`; A2+ runs start at 80% max HP). Encounters and bosses fixed at run start; A10 second boss differs.
- C10. No boss relic tier. Chests: 50/33/17 from the front of the shared bag, gold 42-52; elite relics from the front of the player bag (`Runs/RelicGrabBag.cs:69-92`).
- C11. Port status: RNG, card rewards, potions, unknown rooms, rest, act transitions, shop, event selection done (`agent/runmodel.py`, `agent/tracker.py`); map = real current map + template later acts; event bodies partial; open: boss/treasure relic bag order, encounter tag rule (`AddWithoutRepeatingTags`, needs pool tags).

## D. Event and ancient-relic data vs decomp (`data/events.json`, `data/ancient_relics.json`, `agent/events.py`)
Rows only where the data is approximate or incomplete (ok and fixed rows omitted; history in git). Status: ok* = data matches, approximation or run-model gap noted; fixed = corrected; unmodelled = effect the vocabulary cannot express (an `unmodelled` note: -0.5 in the scripted event policy, skipped by `default_choose`; not shown in `price` output). Combat effects are the sim's (`content/relics/*.rs`). Run-model gaps not fixed: relic counters reset per fight (teas, Iron Club, Pumpkin Candle KindleCount), max HP gained in combat (Chosen Cheese) and in-combat gold are not carried back.

| event / relic | option | status | what was wrong | file:line |
|---|---|---|---|---|
| AbyssalBaths | IMMERSE > LINGER | unmodelled | further lingers (existing note) | data/events.json:11; Models.Events/AbyssalBaths.cs:11-106 |
| BattlewornDummy | SETTING_1 | unmodelled | turn limit (existing note) | data/events.json:160; Models.Events/BattlewornDummy.cs:20-119 |
| BrainLeech | RIP | ok* | colorless offer skips the rarity roll | data/events.json:230; Models.Events/BrainLeech.cs:19-73 |
| ByrdonisNest | TAKE | unmodelled | Byrdonis Egg's Hatch rest option (Byrdpip) not modelled; note added | data/events.json:312; Models.Events/ByrdonisNest.cs:15-51 |
| ColorfulPhilosophers | IRONCLAD | ok* | game offers 3 of the 4 other colours (own colour excluded); data lists all 5 | data/events.json:352; Models.Events/ColorfulPhilosophers.cs:16-67 |
| CrystalSphere | UNCOVER_FUTURE | unmodelled | minigame (existing note) | data/events.json:622; Models.Events/CrystalSphere.cs:16-82 |
| CrystalSphere | PAYMENT_PLAN | unmodelled | minigame (existing note); Debt's gold loss not simulated, added to note | data/events.json:622; Models.Events/CrystalSphere.cs:16-82 |
| DollRoom | TAKE_SOME_TIME > BING_BONG | unmodelled | Bing Bong clones every later card added to the deck; note added | data/events.json:716; Models.Events/DollRoom.cs:21-173 |
| DollRoom | EXAMINE > BING_BONG | unmodelled | Bing Bong clones every later card added to the deck; note added | data/events.json:716; Models.Events/DollRoom.cs:21-173 |
| DrowningBeacon | CLIMB | unmodelled | Fresnel Lens (Nimble 2 on later cards) not modelled; note added | data/events.json:881; Models.Events/DrowningBeacon.cs:14-57 |
| EndlessConveyor | CAVIAR > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | SPICY_SNAPPY > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | JELLY_LIVER > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | FRIED_EEL > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | SUSPICIOUS_CONDIMENT > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | CLAM_ROLL > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | GOLDEN_FYSH > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| EndlessConveyor | SEAPUNK_SALAD > GRAB_AGAIN | unmodelled | further grabs (existing note) | data/events.json:921; Models.Events/EndlessConveyor.cs:25-289 |
| FakeMerchant | SHOP | unmodelled | fake-relic shop and Foul Potion fight (existing note); condition stricter than the game (Foul Potion also allows < 100 gold) | data/events.json:1205; Models.Events/FakeMerchant.cs:22-166 |
| RoomFullOfCheese | SEARCH | ok* | data ok; Chosen Cheese's +1 max HP per combat is lost: runmodel fight() writes only hp (not fixed) | data/events.json:1862; Models.Events/RoomFullOfCheese.cs:19-53 |
| SlipperyBridge | HOLD_ON_0 > HOLD_ON_1 | unmodelled | further hold-ons (existing note) | data/events.json:2075; Models.Events/SlipperyBridge.cs:19-162 |
| SunkenStatue | GRAB_SWORD | unmodelled | Sword of Stone -> Sword of Jade after 5 elites; note added | data/events.json:2286; Models.Events/SunkenStatue.cs:13-54 |
| TeaMaster | BONE_TEA | ok* | data ok; runmodel rebuilds relics per fight, so the tea's one-fight effect repeats every fight (code gap, not fixed) | data/events.json:2547; Models.Events/TeaMaster.cs:15-85 |
| TeaMaster | EMBER_TEA | ok* | same per-fight counter reset (not fixed) | data/events.json:2547; Models.Events/TeaMaster.cs:15-85 |
| TeaMaster | TEA_OF_DISCOURTESY | ok* | same per-fight counter reset (not fixed) | data/events.json:2547; Models.Events/TeaMaster.cs:15-85 |
| TheLanternKey | KEEP_THE_KEY | unmodelled | Lantern Key's act-3 ?-room / Repy effect; note added | data/events.json:2695; Models.Events/TheLanternKey.cs:14-57 |
| TheLegendsWereTrue | NAB_THE_MAP | unmodelled | Spoils Map (+600 gold act-2 treasure map); note added | data/events.json:2739; Models.Events/TheLegendsWereTrue.cs:19-63 |
| TheLegendsWereTrue | SLOWLY_FIND_AN_EXIT | ok* | game draws the potion uniformly; draws.potion() weights by rarity | data/events.json:2739; Models.Events/TheLegendsWereTrue.cs:19-63 |
| TinkerTime | CHOOSE_CARD_TYPE | unmodelled | card crafting (existing note) | data/events.json:2826; Models.Events/TinkerTime.cs:17-205 |
| TrashHeap | DIVE_IN | unmodelled | Darkstone Periapt / Dream Catcher / Maw Bank ongoing effects; note added | data/events.json:2854; Models.Events/TrashHeap.cs:17-77 |
| Trial | REJECT > DOUBLE_DOWN | unmodelled | abandons the run (existing note) | data/events.json:2917; Models.Events/Trial.cs:26-218 |
| WelcomeToWongos | BARGAIN_BIN | ok* | game restricts to shop-allowed relics | data/events.json:3352; Models.Events/WelcomeToWongos.cs:18-167 |
| WelcomeToWongos | FEATURED_ITEM | ok* | game restricts to shop-allowed relics | data/events.json:3352; Models.Events/WelcomeToWongos.cs:18-167 |
| WelcomeToWongos | MYSTERY_BOX | unmodelled | Wongo's Mystery Ticket (3 relics after 5 combats); note added | data/events.json:3352; Models.Events/WelcomeToWongos.cs:18-167 |
| Wellspring | BOTTLE | ok* | game draws the potion uniformly; draws.potion() weights by rarity | data/events.json:3444; Models.Events/Wellspring.cs:15-56 |
| Wellspring | BATHE | unmodelled | Guilty leaves after 5 combats; note added | data/events.json:3444; Models.Events/Wellspring.cs:15-56 |
| ancient BLACK_STAR |  | unmodelled | existing note: elite fights drop an extra relic reward | data/ancient_relics.json:115; Models.Relics/BlackStar.cs:13-24 |
| ancient DOWSING_ROD |  | unmodelled | Dowsing -> Abundance after 5 unknown rooms; note added | data/ancient_relics.json:307; Models.Relics/DowsingRod.cs:19-22 |
| ancient DRIFTWOOD |  | unmodelled | existing note: card rewards can be rerolled | data/ancient_relics.json:322; Models.Relics/Driftwood.cs:14-24 |
| ancient ECTOPLASM |  | unmodelled | existing note: gold can no longer be gained | data/ancient_relics.json:361; Models.Relics/Ectoplasm.cs:18-39 |
| ancient FISHING_ROD |  | unmodelled | existing note: every 3rd monster fight upgrades a random deck card | data/ancient_relics.json:413; Models.Relics/FishingRod.cs:40-59 |
| ancient FUR_COAT |  | unmodelled | existing note: marks 8 monster / elite rooms of the current act; their enemies start at 1 HP | data/ancient_relics.json:425; Models.Relics/FurCoat.cs:62-157 |
| ancient GLITTER |  | unmodelled | existing note: card reward cards come Glam-enchanted | data/ancient_relics.json:492; Models.Relics/Glitter.cs:18-35 |
| ancient GOLDEN_COMPASS |  | unmodelled | existing note: regenerates the current act's map as the Golden Path; its unknown rooms are all events | data/ancient_relics.json:504; Models.Relics/GoldenCompass.cs:33-54 |
| ancient IRON_CLUB |  | ok* | CardsPlayed resets per fight in rollouts | data/ancient_relics.json:550; Models.Relics/IronClub.cs:83-92 |
| ancient KALEIDOSCOPE |  | ok* | offer adds rarity pity; game uses base odds | data/ancient_relics.json:580; Models.Relics/Kaleidoscope.cs:22-49 |
| ancient LAVA_ROCK |  | unmodelled | existing note: the act 1 boss drops 2 extra relic rewards | data/ancient_relics.json:626; Models.Relics/LavaRock.cs:36-63 |
| ancient LORDS_PARASOL |  | unmodelled | existing note: entering a merchant buys its whole stock (cards, relics, potions, removal) for free | data/ancient_relics.json:695; Models.Relics/LordsParasol.cs:21-110 |
| ancient LOST_COFFER |  | ok* | offer adds rarity pity; game uses base odds | data/ancient_relics.json:707; Models.Relics/LostCoffer.cs:16-22 |
| ancient MASSIVE_SCROLL |  | unmodelled | existing note: multiplayer only: choose 1 of 3 multiplayer-only cards | data/ancient_relics.json:727; Models.Relics/MassiveScroll.cs:19-45 |
| ancient MEAT_CLEAVER |  | unmodelled | existing note: rest sites gain a Cook option | data/ancient_relics.json:741; Models.Relics/MeatCleaver.cs:15-22 |
| ancient NEOWS_SACRIFICE |  | unmodelled | Guilty leaves after 5 combats; note added | data/ancient_relics.json:820; Models.Relics/NeowsSacrifice.cs:29-33 |
| ancient PAELS_GROWTH |  | unmodelled | existing note: rest sites gain a Clone option | data/ancient_relics.json:961; Models.Relics/PaelsGrowth.cs:22-38 |
| ancient PAELS_TOOTH |  | unmodelled | existing note: the 5 chosen upgradable cards leave the deck and come back upgraded, one after each combat | data/ancient_relics.json:1014; Models.Relics/PaelsTooth.cs:81-112 |
| ancient PAELS_WING |  | unmodelled | existing note: a card reward can be sacrificed instead; every 2nd sacrifice gives a relic | data/ancient_relics.json:1030; Models.Relics/PaelsWing.cs:73-90 |
| ancient PRISMATIC_GEM |  | unmodelled | existing note: card rewards draw from every character's pool | data/ancient_relics.json:1141; Models.Relics/PrismaticGem.cs:19-46 |
| ancient PUMPKIN_CANDLE |  | unmodelled | run model passes no KindleCount, so sim energy is 0 in rollouts (code gap, not fixed); note extended | data/ancient_relics.json:1154; Models.Relics/PumpkinCandle.cs:52-90 |
| ancient SCROLL_BOXES |  | ok* | approximation: random bundle vs 1 of 2 chosen (note) | data/ancient_relics.json:1205; Models.Relics/ScrollBoxes.cs:32-103 |
| ancient SEAL_OF_GOLD |  | ok* | gold spent in combat not carried back to the run model | data/ancient_relics.json:1233; Models.Relics/SealOfGold.cs:25-31 |
| ancient SEA_GLASS |  | ok* | approximation: 1 card per rarity vs any of 15 (note) | data/ancient_relics.json:1245; Models.Relics/SeaGlass.cs:74-91 |
| ancient SILKEN_TRESS |  | unmodelled | existing note: the next card reward's cards come Glam-enchanted | data/ancient_relics.json:1315; Models.Relics/SilkenTress.cs:42-82 |
| ancient SILVER_CRUCIBLE |  | unmodelled | existing note: the next 3 card rewards come upgraded; the first treasure room holds no chest | data/ancient_relics.json:1330; Models.Relics/SilverCrucible.cs:94-146 |
| ancient SMALL_CAPSULE |  | ok* | relic_random skips the drawn relic's pickup effect | data/ancient_relics.json:1343; Models.Relics/SmallCapsule.cs:13-18 |
| ancient SOZU |  | ok* | run model rewards() still grants potions | data/ancient_relics.json:1365; Models.Relics/Sozu.cs:17-28 |
| ancient STONE_HUMIDIFIER |  | unmodelled | rest +5 max HP (existing note) | data/ancient_relics.json:1387; Models.Relics/StoneHumidifier.cs:18-24 |
| ancient TOY_BOX |  | ok* | relic_random skips the drawn relic's pickup effect | data/ancient_relics.json:1460; Models.Relics/ToyBox.cs:87-116 |
| ancient WAR_HAMMER |  | unmodelled | existing note: each elite win upgrades 4 random deck cards | data/ancient_relics.json:1524; Models.Relics/WarHammer.cs:19-32 |
| ancient WINGED_BOOTS |  | unmodelled | existing note: free travel: may ignore map paths 3 times | data/ancient_relics.json:1545; Models.Relics/WingedBoots.cs:47-92 |
