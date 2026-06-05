using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// CalcifiedCultist (MegaCrit): HP 38–41. Deterministic AI — Incantation (gain 2 Ritual) then
    /// Dark Strike (9 damage) forever. Dark Strike scales as Ritual feeds Strength.
    /// </summary>
    public static Monster CalcifiedCultist(int hp = -1, int ascension = 0)
        => Cultist("CalcifiedCultist",
            hp >= 0 ? hp : Asc.Tough(ascension, 42, 41),
            Asc.Deadly(ascension, 11, 9),
            ritualAmount: 2);

    /// <summary>
    /// DampCultist (MegaCrit): HP 51–53. Same Incantation→Dark Strike AI, but Ritual 5/turn and a
    /// weak base Dark Strike (1) — so its damage ramps fast via Strength.
    /// </summary>
    public static Monster DampCultist(int hp = -1, int ascension = 0)
        => Cultist("DampCultist",
            hp >= 0 ? hp : Asc.Tough(ascension, 54, 53),
            Asc.Deadly(ascension, 3, 1),
            Asc.Deadly(ascension, 6, 5));

    /// <summary>
    /// CorpseSlug (MegaCrit): HP 25–27. Cycles Whip Slap (3×2) → Glomp (8) → Goop (apply 2 Frail).
    /// Starts combat with Ravenous (4): devours a dead ally for +4 Strength (and a stun turn).
    /// </summary>
    public static Monster CorpseSlug(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 29, 27);
        int ravenousStr = Asc.Deadly(ascension, 5, 4), glompDamage = Asc.Deadly(ascension, 9, 8);
        var monster = new Monster { Name = "CorpseSlug", MaxHp = hp, CurrentHp = hp };

        var whipSlap = new MoveState("WHIP_SLAP_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, 3, 2, ValueProp.Move, null),
            intentDamage: 3, intentHits: 2);
        var glomp = new MoveState("GLOMP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, glompDamage, ValueProp.Move, null),
            intentDamage: glompDamage);
        var goop = new MoveState("GOOP_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new FrailPower(), 2, self));

        whipSlap.FollowUp = glomp;
        glomp.FollowUp = goop;
        goop.FollowUp = whipSlap;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { whipSlap, glomp, goop }, whipSlap.Id);
        monster.AddPower(new RavenousPower(), ravenousStr); // applied at combat start in-game
        return monster;
    }

    /// <summary>
    /// Byrdonis (Act 1 Overgrowth elite, MegaCrit): HP 81–84. Alternates Swoop (17) and Peck (3×3),
    /// starting on Swoop. Gains Territorial (1) at start → +1 Strength every turn end (ramps).
    /// </summary>
    public static Monster Byrdonis(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 90, 84);
        int peckDamage = Asc.Deadly(ascension, 4, 3), peckHits = 3;
        int swoopDamage = Asc.Deadly(ascension, 19, 17), territorial = 1;
        var monster = new Monster { Name = "Byrdonis", MaxHp = hp, CurrentHp = hp };

        var peck = new MoveState("PECK_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, peckDamage, peckHits, ValueProp.Move, null),
            intentDamage: peckDamage, intentHits: peckHits);
        var swoop = new MoveState("SWOOP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, swoopDamage, ValueProp.Move, null),
            intentDamage: swoopDamage);

        swoop.FollowUp = peck;
        peck.FollowUp = swoop;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { swoop, peck }, swoop.Id);
        monster.AddPower(new TerritorialPower(), territorial);
        return monster;
    }

    /// <summary>
    /// BygoneEffigy (Act 1 Overgrowth elite, MegaCrit): HP 127 (132 on Ascension ToughEnemies).
    /// Starts asleep with Slow (1): each card the player plays this turn makes the Effigy take +10%
    /// damage (resets each turn). Move chain: Sleep (no-op) → Wake (+10 Strength) → Slash (13) → Slash
    /// forever. Slash is 15 on Ascension DeadlyEnemies. (SLEEP_MOVE_2 in the game data is an unreachable
    /// dead state, so it is omitted.)
    /// </summary>
    public static Monster BygoneEffigy(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 132, 127);
        int slashDamage = Asc.Deadly(ascension, 15, 13), wakeStrength = 10;
        var monster = new Monster { Name = "BygoneEffigy", MaxHp = hp, CurrentHp = hp };

        var sleep = new MoveState("SLEEP_MOVE",
            (combat, self) => { /* no-op */ },
            intentDamage: null);
        var wake = new MoveState("WAKE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), wakeStrength, self),
            intentDamage: null);
        var slash = new MoveState("SLASHES_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, slashDamage, ValueProp.Move, null),
            intentDamage: slashDamage);

        sleep.FollowUp = wake;
        wake.FollowUp = slash;
        slash.FollowUp = slash;   // self-loop

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { sleep, wake, slash }, sleep.Id);
        monster.AddPower(new SlowPower(), 1); // applied at combat start in-game (AfterAddedToRoom)
        return monster;
    }

    /// <summary>
    /// PhrogParasite (Act 1 Overgrowth elite, MegaCrit): HP 61–64. Starts with Infested(4): when it dies
    /// it bursts into 4 Wrigglers (two-phase fight). Alternates Infect (adds 3 Infection status cards to
    /// the player's discard) and Lash (4×4), starting on Infect. Lash is 5 on Ascension DeadlyEnemies.
    /// </summary>
    public static Monster PhrogParasite(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 68, 61);
        int lashDamage = Asc.Deadly(ascension, 5, 4), lashHits = 4, infectCards = 3, infested = 4;
        var monster = new Monster { Name = "PhrogParasite", MaxHp = hp, CurrentHp = hp };

        var infect = new MoveState("INFECT_MOVE",
            (combat, self) => AddStatusToDiscard(combat, infectCards),
            intentDamage: null);
        var lash = new MoveState("LASH_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, lashDamage, lashHits, ValueProp.Move, null),
            intentDamage: lashDamage, intentHits: lashHits);

        infect.FollowUp = lash;
        lash.FollowUp = infect;   // deterministic alternation (game's RandomBranch both-CannotRepeat ⇒ alternate)

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { infect, lash }, infect.Id);
        monster.AddPower(new InfestedPower(), infested);  // applied at combat start in-game
        return monster;
    }

    /// <summary>
    /// Wriggler (spawned by Phrog Parasite, MegaCrit): HP 17–21. Bite 6 (7 on DeadlyEnemies). Wriggle adds
    /// 1 Infection to the player's discard and grants itself +2 Strength. Spawns stunned (a no-op first
    /// turn). Thereafter alternates; <paramref name="biteFirst"/> seeds whether the first real move is Bite
    /// (slots 1,3) or Wriggle (slots 2,4). When not stunned it opens directly on that first move.
    /// </summary>
    public static Monster Wriggler(int hp = -1, bool stunned = true, bool biteFirst = true, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 22, 17);
        int biteDamage = Asc.Deadly(ascension, 7, 6), wriggleStrength = 2;
        var monster = new Monster
        {
            Name = "Wriggler",
            MaxHp = hp,
            CurrentHp = hp,
            Variant = biteFirst ? "B" : "W",
        };

        var bite = new MoveState("NASTY_BITE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, biteDamage, ValueProp.Move, null),
            intentDamage: biteDamage);
        var wriggle = new MoveState("WRIGGLE_MOVE",
            (combat, self) =>
            {
                AddStatusToDiscard(combat, 1);
                Cmd.ApplyPower(combat, self, new StrengthPower(), wriggleStrength, self);
            },
            intentDamage: null);
        var spawned = new MoveState("SPAWNED_MOVE", (combat, self) => { /* stunned: no-op */ }, intentDamage: null);

        bite.FollowUp = wriggle;
        wriggle.FollowUp = bite;
        var firstReal = biteFirst ? bite : wriggle;
        spawned.FollowUp = firstReal;

        var initial = stunned ? spawned.Id : firstReal.Id;
        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { spawned, bite, wriggle }, initial);
        return monster;
    }

    /// <summary>
    /// TerrorEel (Act 1 elite, MegaCrit): HP 140 (150 on Ascension ToughEnemies). Loops Crash (16) and
    /// Thrash (3×3, then self-Vigor 6 so the next Crash hits 16→22). Starts with Shriek(70): the first
    /// unblocked hit that brings it to ≤70 HP stuns it into Terror (apply 99 Vulnerable to the player),
    /// then it resumes the Crash/Thrash loop. Crash 18 / Thrash 4 on Ascension DeadlyEnemies.
    /// </summary>
    public static Monster TerrorEel(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 150, 140);
        int crashDamage = Asc.Deadly(ascension, 18, 16), thrashDamage = Asc.Deadly(ascension, 4, 3), thrashHits = 3;
        int vigor = 6, shriek = Asc.Tough(ascension, 75, 70), terrorVulnerable = 99;
        var monster = new Monster { Name = "TerrorEel", MaxHp = hp, CurrentHp = hp };

        var crash = new MoveState("CRASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, crashDamage, ValueProp.Move, null),
            intentDamage: crashDamage);
        var thrash = new MoveState("THRASH_MOVE",
            (combat, self) =>
            {
                Cmd.AttackMulti(combat, self, combat.Player, thrashDamage, thrashHits, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new VigorPower(), vigor, self);
            },
            intentDamage: thrashDamage, intentHits: thrashHits);
        var stun = new MoveState(ShriekPower.StunStateId, (combat, self) => { /* stunned: no-op */ }, intentDamage: null);
        var terror = new MoveState("TERROR_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), terrorVulnerable, self),
            intentDamage: null);

        crash.FollowUp = thrash;
        thrash.FollowUp = crash;
        stun.FollowUp = terror;
        terror.FollowUp = crash;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { crash, thrash, stun, terror }, crash.Id);
        monster.AddPower(new ShriekPower(), shriek);   // applied to itself at combat start (AfterAddedToRoom)
        return monster;
    }

    /// <summary>
    /// SoulNexus (Act 1 elite, MegaCrit): HP 234 (254 on Ascension ToughEnemies). A single big body that
    /// opens on Soul Burn (29), then randomly cycles three moves that never repeat back-to-back (equal
    /// weight): Soul Burn (29), Maelstrom (6×4), Drain Life (18 + apply 2 Vulnerable and 2 Weak to the
    /// player). On Ascension DeadlyEnemies: Soul Burn 31, Maelstrom 7×4, Drain Life 19. First ported
    /// monster to use a genuine RandomBranchState.
    /// </summary>
    public static Monster SoulNexus(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 254, 234);
        int soulBurn = Asc.Deadly(ascension, 31, 29), maelstromDamage = Asc.Deadly(ascension, 7, 6), maelstromHits = 4;
        int drainDamage = Asc.Deadly(ascension, 19, 18), drainVulnerable = 2, drainWeak = 2;
        var monster = new Monster { Name = "SoulNexus", MaxHp = hp, CurrentHp = hp };

        var soulBurnMove = new MoveState("SOUL_BURN_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, soulBurn, ValueProp.Move, null),
            intentDamage: soulBurn);
        var maelstrom = new MoveState("MAELSTROM_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, maelstromDamage, maelstromHits, ValueProp.Move, null),
            intentDamage: maelstromDamage, intentHits: maelstromHits);
        var drainLife = new MoveState("DRAIN_LIFE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, drainDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), drainVulnerable, self);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), drainWeak, self);
            },
            intentDamage: drainDamage);

        var rand = new RandomBranchState("RAND")
            .Add(soulBurnMove.Id, 1f, MoveRepeatType.CannotRepeat)
            .Add(maelstrom.Id, 1f, MoveRepeatType.CannotRepeat)
            .Add(drainLife.Id, 1f, MoveRepeatType.CannotRepeat);

        soulBurnMove.FollowUp = rand;
        maelstrom.FollowUp = rand;
        drainLife.FollowUp = rand;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { soulBurnMove, maelstrom, drainLife, rand }, soulBurnMove.Id);
        return monster;
    }

    /// <summary>
    /// MechaKnight (Act 1 elite, MegaCrit): HP 300 (320 on Ascension ToughEnemies). Starts with
    /// Artifact(3) (negates the player's first 3 debuffs). Opens on Charge (25), then loops
    /// Flamethrower (adds 4 Burn to the player's hand) → Windup (gain 15 block, +5 Strength) →
    /// Heavy Cleave (35) → Flamethrower → … On Ascension DeadlyEnemies: Charge 30, Heavy Cleave 40.
    /// </summary>
    public static Monster MechaKnight(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 320, 300);
        int chargeDamage = Asc.Deadly(ascension, 30, 25), heavyCleaveDamage = Asc.Deadly(ascension, 40, 35);
        int windupBlock = 15, windupStrength = 5, burnCount = 4, artifact = 3;
        var monster = new Monster { Name = "MechaKnight", MaxHp = hp, CurrentHp = hp };

        var charge = new MoveState("CHARGE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, chargeDamage, ValueProp.Move, null),
            intentDamage: chargeDamage);
        var flamethrower = new MoveState("FLAMETHROWER_MOVE",
            (combat, self) => { for (int i = 0; i < burnCount; i++) combat.Player.Hand.Add(new Burn()); },
            intentDamage: null);
        var windup = new MoveState("WINDUP_MOVE",
            (combat, self) =>
            {
                Cmd.GainBlock(combat, self, windupBlock, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), windupStrength, self);
            },
            intentDamage: null);
        var heavyCleave = new MoveState("HEAVY_CLEAVE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, heavyCleaveDamage, ValueProp.Move, null),
            intentDamage: heavyCleaveDamage);

        charge.FollowUp = flamethrower;
        flamethrower.FollowUp = windup;
        windup.FollowUp = heavyCleave;
        heavyCleave.FollowUp = flamethrower;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { charge, flamethrower, windup, heavyCleave }, charge.Id);
        monster.AddPower(new ArtifactPower(), artifact);   // applied to itself at combat start
        return monster;
    }

    /// <summary>
    /// Entomancer (Act 1 elite, MegaCrit): HP 145 (155 on Ascension ToughEnemies). Starts with Personal
    /// Hive(1): whenever the player lands a powered attack, it shuffles that many Dazed into the player's
    /// draw pile. Opens on Bees (3×7), then loops Spear (18) → Pheromone Spit → Bees → … Pheromone Spit
    /// grows the hive (+1 Hive, +1 Strength) until Hive 3, thereafter +2 Strength. Deadly: Bees 3×8,
    /// Spear 20.
    /// </summary>
    public static Monster Entomancer(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 155, 145);
        int beesDamage = 3, beesHits = Asc.Deadly(ascension, 8, 7), spearDamage = Asc.Deadly(ascension, 20, 18);
        var monster = new Monster { Name = "Entomancer", MaxHp = hp, CurrentHp = hp };

        var bees = new MoveState("BEES_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, beesDamage, beesHits, ValueProp.Move, null),
            intentDamage: beesDamage, intentHits: beesHits);
        var spear = new MoveState("SPEAR_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, spearDamage, ValueProp.Move, null),
            intentDamage: spearDamage);
        var spit = new MoveState("PHEROMONE_SPIT_MOVE",
            (combat, self) =>
            {
                if (self.GetPowerAmount("PersonalHive") < 3)
                {
                    Cmd.ApplyPower(combat, self, new PersonalHivePower(), 1, self);
                    Cmd.ApplyPower(combat, self, new StrengthPower(), 1, self);
                }
                else Cmd.ApplyPower(combat, self, new StrengthPower(), 2, self);
            },
            intentDamage: null);

        bees.FollowUp = spear;
        spear.FollowUp = spit;
        spit.FollowUp = bees;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { bees, spear, spit }, bees.Id);
        monster.AddPower(new PersonalHivePower(), 1);   // applied to itself at combat start
        return monster;
    }

    /// <summary>
    /// SkulkingColony (Act 1 elite, MegaCrit): HP 75 (80 on Ascension ToughEnemies). Starts with
    /// HardenedShell(20): it can lose at most 20 HP per turn (excess negated, resets each turn). Loops
    /// Zoom (14) → Zoom (14) → Inertia (9 + self Strength 2) → Piercing Stabs (7×2) → … Deadly:
    /// Zoom 16, Inertia 11 (+3 Str), Piercing Stabs 8×2.
    /// </summary>
    public static Monster SkulkingColony(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 80, 75);
        int zoomDamage = Asc.Deadly(ascension, 16, 14), inertiaDamage = Asc.Deadly(ascension, 11, 9);
        int inertiaStrength = Asc.Deadly(ascension, 3, 2), stabsDamage = Asc.Deadly(ascension, 8, 7), stabsHits = 2, hardenedShell = 20;
        var monster = new Monster { Name = "SkulkingColony", MaxHp = hp, CurrentHp = hp };

        var zoom1 = new MoveState("ZOOM_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, zoomDamage, ValueProp.Move, null),
            intentDamage: zoomDamage);
        var zoom2 = new MoveState("ZOOM_MOVE_2",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, zoomDamage, ValueProp.Move, null),
            intentDamage: zoomDamage);
        var inertia = new MoveState("INERTIA_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, inertiaDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), inertiaStrength, self);
            },
            intentDamage: inertiaDamage);
        var stabs = new MoveState("PIERCING_STABS_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, stabsDamage, stabsHits, ValueProp.Move, null),
            intentDamage: stabsDamage, intentHits: stabsHits);

        zoom1.FollowUp = zoom2;
        zoom2.FollowUp = inertia;
        inertia.FollowUp = stabs;
        stabs.FollowUp = zoom1;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { zoom1, zoom2, inertia, stabs }, zoom1.Id);
        monster.AddPower(new HardenedShellPower(), hardenedShell);   // applied to itself at combat start
        return monster;
    }

    /// <summary>
    /// InfestedPrism (Act 1 elite, MegaCrit): HP 161 (171 on Ascension ToughEnemies). Starts with Vital
    /// Spark(2): each player Skill played applies Tainted(2) to the player (+2 damage taken per attack
    /// that turn). Loops Jab (15) → Radiate (11 + gain 11 block) → Whirlwind (5×3) → Pulsate (8 + gain 20
    /// block + self Vital Spark 2) → … Deadly: Jab 17, Radiate 13, Whirlwind 6×3, Pulsate 10, Vital Spark 3.
    /// </summary>
    public static Monster InfestedPrism(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 171, 161);
        int jabDamage = Asc.Deadly(ascension, 17, 15), radiateDamage = Asc.Deadly(ascension, 13, 11), radiateBlock = Asc.Deadly(ascension, 13, 11);
        int whirlwindDamage = Asc.Deadly(ascension, 6, 5), whirlwindHits = 3;
        int pulsateDamage = Asc.Deadly(ascension, 10, 8), pulsateBlock = Asc.Tough(ascension, 22, 20), vitalSpark = Asc.Deadly(ascension, 3, 2);
        var monster = new Monster { Name = "InfestedPrism", MaxHp = hp, CurrentHp = hp };

        var jab = new MoveState("JAB_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, jabDamage, ValueProp.Move, null),
            intentDamage: jabDamage);
        var radiate = new MoveState("RADIATE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, radiateDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, radiateBlock, ValueProp.Move, null);
            },
            intentDamage: radiateDamage);
        var whirlwind = new MoveState("WHIRLWIND_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, whirlwindDamage, whirlwindHits, ValueProp.Move, null),
            intentDamage: whirlwindDamage, intentHits: whirlwindHits);
        var pulsate = new MoveState("PULSATE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, pulsateDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, pulsateBlock, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new VitalSparkPower(), vitalSpark, self);
            },
            intentDamage: pulsateDamage);

        jab.FollowUp = radiate;
        radiate.FollowUp = whirlwind;
        whirlwind.FollowUp = pulsate;
        pulsate.FollowUp = jab;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { jab, radiate, whirlwind, pulsate }, jab.Id);
        monster.AddPower(new VitalSparkPower(), vitalSpark);   // applied to itself at combat start
        return monster;
    }

    /// <summary>
    /// PhantasmalGardener (Act 1 elite, MegaCrit): the elite fields FOUR of them (HP 26–31 each). Every
    /// gardener has Skittish(6): the first unblocked player hit each turn makes it gain 6 block. All four
    /// share one cycle — Bite (5) → Lash (7) → Flail (1×3) → Enlarge (+2 Strength) → … — but start at
    /// different points by slot (first→Flail, second→Bite, third→Lash, fourth→Enlarge), so their attacks
    /// stagger. Tough: HP 27–32, Skittish 7. Deadly: Enlarge +3 Strength.
    /// </summary>
    public static Monster PhantasmalGardener(int hp = -1, string slot = "second", int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 32, 28);
        int biteDamage = 5, lashDamage = 7, flailDamage = 1, flailHits = 3;
        int enlargeStrength = Asc.Deadly(ascension, 3, 2), skittish = Asc.Tough(ascension, 7, 6);
        var monster = new Monster { Name = "PhantasmalGardener", MaxHp = hp, CurrentHp = hp, Variant = slot };

        var bite = new MoveState("BITE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, biteDamage, ValueProp.Move, null),
            intentDamage: biteDamage);
        var lash = new MoveState("LASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, lashDamage, ValueProp.Move, null),
            intentDamage: lashDamage);
        var flail = new MoveState("FLAIL_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, flailDamage, flailHits, ValueProp.Move, null),
            intentDamage: flailDamage, intentHits: flailHits);
        var enlarge = new MoveState("ENLARGE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), enlargeStrength, self),
            intentDamage: null);

        bite.FollowUp = lash;
        lash.FollowUp = flail;
        flail.FollowUp = enlarge;
        enlarge.FollowUp = bite;

        var initial = slot switch
        {
            "first" => flail.Id,
            "third" => lash.Id,
            "fourth" => enlarge.Id,
            _ => bite.Id,            // "second" (default)
        };
        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { bite, lash, flail, enlarge }, initial);
        monster.AddPower(new SkittishPower(), skittish);
        return monster;
    }

    /// <summary>The four Phantasmal Gardeners, slot-staggered (first→Flail … fourth→Enlarge).</summary>
    public static IEnumerable<Monster> PhantasmalGardeners(int ascension = 0)
    {
        yield return PhantasmalGardener(slot: "first", ascension: ascension);
        yield return PhantasmalGardener(slot: "second", ascension: ascension);
        yield return PhantasmalGardener(slot: "third", ascension: ascension);
        yield return PhantasmalGardener(slot: "fourth", ascension: ascension);
    }

    /// <summary>
    /// FlailKnight (Act 1 Knights elite, MegaCrit): HP 101 (108 Tough). Opens on Ram (15), then randomly
    /// (no War Chant twice in a row; Flail and Ram weighted ×2): War Chant (pure buff: +3 Strength) /
    /// Flail (9×2) / Ram (15). Deadly: Flail 10, Ram 17.
    /// </summary>
    public static Monster FlailKnight(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 108, 101);
        int flailDamage = Asc.Deadly(ascension, 10, 9), ramDamage = Asc.Deadly(ascension, 17, 15), warChantStrength = 3;
        var monster = new Monster { Name = "FlailKnight", MaxHp = hp, CurrentHp = hp };

        var warChant = new MoveState("WAR_CHANT",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), warChantStrength, self),
            intentDamage: null);
        var flail = new MoveState("FLAIL_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, flailDamage, 2, ValueProp.Move, null),
            intentDamage: flailDamage, intentHits: 2);
        var ram = new MoveState("RAM_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, ramDamage, ValueProp.Move, null),
            intentDamage: ramDamage);

        var rand = new RandomBranchState("RAND")
            .Add(warChant.Id, 1f, MoveRepeatType.CannotRepeat)
            .Add(flail.Id, 2f)
            .Add(ram.Id, 2f);
        warChant.FollowUp = rand; flail.FollowUp = rand; ram.FollowUp = rand;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { warChant, flail, ram, rand }, ram.Id);
        return monster;
    }

    /// <summary>
    /// SpectralKnight (Act 1 Knights elite, MegaCrit): HP 93 (97 Tough). Opens Hex (makes the player's
    /// cards Ethereal — modelled as an inert marker; see HexPower) → Soul Slash (15), then randomly Soul
    /// Slash (×2) / Soul Flame (3×3, never twice in a row). Deadly: Soul Slash 17, Soul Flame 4×3.
    /// </summary>
    public static Monster SpectralKnight(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 97, 93);
        int soulSlashDamage = Asc.Deadly(ascension, 17, 15), soulFlameDamage = Asc.Deadly(ascension, 4, 3), hex = 2;
        var monster = new Monster { Name = "SpectralKnight", MaxHp = hp, CurrentHp = hp };

        var hexMove = new MoveState("HEX",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new HexPower(), hex, self),
            intentDamage: null);
        var soulSlash = new MoveState("SOUL_SLASH",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, soulSlashDamage, ValueProp.Move, null),
            intentDamage: soulSlashDamage);
        var soulFlame = new MoveState("SOUL_FLAME",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, soulFlameDamage, 3, ValueProp.Move, null),
            intentDamage: soulFlameDamage, intentHits: 3);

        var rand = new RandomBranchState("RAND")
            .Add(soulSlash.Id, 2f)
            .Add(soulFlame.Id, 1f, MoveRepeatType.CannotRepeat);
        hexMove.FollowUp = soulSlash;
        soulSlash.FollowUp = rand;
        soulFlame.FollowUp = rand;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { hexMove, soulSlash, soulFlame, rand }, hexMove.Id);
        return monster;
    }

    /// <summary>
    /// MagiKnight (Act 1 Knights elite, MegaCrit): HP 82 (89 Tough). Deterministic chain: Power Shield
    /// (6 + gain 5 block) → Dampen (downgrade upgraded cards — inert marker on a starter deck) → Spear
    /// (10) → Prep (gain 5 block) → Magic Bomb (35) → Spear → Prep → Bomb → … Tough block 9. Deadly:
    /// Power Shield 7, Spear 11, Bomb 40.
    /// </summary>
    public static Monster MagiKnight(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 89, 82);
        int powerShieldDamage = Asc.Deadly(ascension, 7, 6), powerShieldBlock = Asc.Tough(ascension, 9, 5);
        int spearDamage = Asc.Deadly(ascension, 11, 10), bombDamage = Asc.Deadly(ascension, 40, 35);
        var monster = new Monster { Name = "MagiKnight", MaxHp = hp, CurrentHp = hp };

        var powerShield = new MoveState("POWER_SHIELD_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, powerShieldDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, powerShieldBlock, ValueProp.Move, null);
            },
            intentDamage: powerShieldDamage);
        var dampen = new MoveState("DAMPEN_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new DampenPower(), 1, self),
            intentDamage: null);
        var spear = new MoveState("RAM_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, spearDamage, ValueProp.Move, null),
            intentDamage: spearDamage);
        var prep = new MoveState("PREP_MOVE",
            (combat, self) => Cmd.GainBlock(combat, self, powerShieldBlock, ValueProp.Move, null),
            intentDamage: null);
        var bomb = new MoveState("MAGIC_BOMB",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, bombDamage, ValueProp.Move, null),
            intentDamage: bombDamage);

        powerShield.FollowUp = dampen;
        dampen.FollowUp = spear;
        spear.FollowUp = prep;
        prep.FollowUp = bomb;
        bomb.FollowUp = spear;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { powerShield, dampen, spear, prep, bomb }, powerShield.Id);
        return monster;
    }

    /// <summary>The Knights elite pack: FlailKnight, SpectralKnight, MagiKnight (encounter order).</summary>
    public static IEnumerable<Monster> Knights(int ascension = 0)
    {
        yield return FlailKnight(ascension: ascension);
        yield return SpectralKnight(ascension: ascension);
        yield return MagiKnight(ascension: ascension);
    }

    /// <summary>
    /// A Decimillipede segment (Act 1 elite, MegaCrit): HP 40–46 (segments take distinct even HPs). Three
    /// of them form the elite. Each cycles Writhe (5×2) → Constrict (8 + 1 Weak to player) → Bulk (6 +
    /// self Strength 2) → … staggered by starter index. Each carries Reattach(25) — a downed segment skips one
    /// enemy turn then reattaches (heals to 25) unless all segments are downed together (see ReattachPower; modelled).
    /// Deadly: Writhe 6, Constrict 9, Bulk 7.
    /// </summary>
    public static Monster DecimillipedeSegment(string name = "DecimillipedeSegment", int starterIdx = 0,
        int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 52, 46);
        int writheDamage = Asc.Deadly(ascension, 6, 5), bulkDamage = Asc.Deadly(ascension, 7, 6);
        int bulkStrength = 2, constrictDamage = Asc.Deadly(ascension, 9, 8);
        var monster = new Monster { Name = name, MaxHp = hp, CurrentHp = hp, Variant = starterIdx.ToString() };

        var writhe = new MoveState("WRITHE_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, writheDamage, 2, ValueProp.Move, null),
            intentDamage: writheDamage, intentHits: 2);
        var bulk = new MoveState("BULK_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, bulkDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), bulkStrength, self);
            },
            intentDamage: bulkDamage);
        var constrict = new MoveState("CONSTRICT_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, constrictDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), 1, self);
            },
            intentDamage: constrictDamage);

        writhe.FollowUp = constrict;
        constrict.FollowUp = bulk;
        bulk.FollowUp = writhe;

        var initial = (starterIdx % 3) switch { 0 => writhe.Id, 1 => bulk.Id, _ => constrict.Id };
        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { writhe, bulk, constrict }, initial);
        monster.AddPower(new ReattachPower(), 25);
        return monster;
    }

    /// <summary>The Decimillipede elite: three segments (Front / Middle / Back), starter-staggered.</summary>
    public static IEnumerable<Monster> Decimillipede()
    {
        yield return DecimillipedeSegment("DecimillipedeSegmentFront", starterIdx: 0);
        yield return DecimillipedeSegment("DecimillipedeSegmentMiddle", starterIdx: 1);
        yield return DecimillipedeSegment("DecimillipedeSegmentBack", starterIdx: 2);
    }

    /// <summary>
    /// WaterfallGiant (Act-1 Underdocks BOSS, MegaCrit): HP 240 (250 on Ascension ToughEnemies); MinInitialHp ==
    /// MaxInitialHp, so the HP is fixed, not a roll. Opens on Pressurize (a pure self-buff: gains SteamEruption,
    /// no player effect), then loops a 5-move chain forever:
    /// Stomp (15, +1 Weak to the player) → Ram (10) → Siphon (heals itself 10×players = 10 in single-player; no
    /// player effect) → Pressure Gun (20, then permanently +5 — so it ramps each cycle) → Pressure Up (13) →
    /// (back to) Stomp → … On Ascension DeadlyEnemies: Stomp 16, Ram 11, Pressure Up 14, Pressure Gun base 23
    /// (Siphon heal 15 on Ascension ToughEnemies). Pressure Gun's +5 ramp is per-USE (CurrentPressureGunDamage),
    /// so the second Pressure Gun hits 25/28, the third 30/33, etc. — modelled by mutating the move's IntentDamage
    /// and the closure-captured counter each time it resolves (mirrors the BygoneEffigy/Cubex ramp style).
    ///
    /// STEAM ERUPTION (death-phase) — MODELLED FAITHFULLY. The boss accumulates a SteamEruptionPower counter as it
    /// acts (decompile: Pressurize +PressurizeAmount = 15, or 20 on DeadlyEnemies; EVERY other move — Stomp, Ram,
    /// Siphon, Pressure Gun, Pressure Up — +3). When brought to 0 HP it does NOT die (engine death-phase via
    /// <see cref="Monster.DeathPhaseEntryMove"/> + <see cref="Monster.InDeathPhase"/>): its AI jumps to ABOUT_TO_BLOW,
    /// a one-turn no-damage self-stun telegraph (game AboutToBlowMove: snapshot the counter, StunIntent,
    /// MustPerformOnceBeforeTransitioning), then on the NEXT enemy turn EXPLODE deals damage equal to the captured
    /// counter (a guaranteed retaliation; game ExplodeMove → DeathBlowIntent(SteamEruptionDamage)) and clears
    /// InDeathPhase so the boss truly dies. This is the SOUND direction for a deck-strength advisor: the explosion is
    /// a large, guaranteed hit, so modelling it faithfully (vs the prior pass's omission) ensures the boss is never
    /// UNDER-credited. The counter is captured at ABOUT_TO_BLOW and read at EXPLODE; we snapshot it into a one-shot
    /// EXPLODE-amount slot so further (no-op) hits on the 0-HP boss can't perturb the pending blow.
    /// </summary>
    public static Monster WaterfallGiant(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 250, 240);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int stompDamage = Asc.Deadly(ascension, 16, 15), ramDamage = Asc.Deadly(ascension, 11, 10);
        int pressureUpDamage = Asc.Deadly(ascension, 14, 13), pressureGunDamage = Asc.Deadly(ascension, 23, 20);
        int pressureGunIncrease = 5, siphonHeal = Asc.Tough(ascension, 15, 10), stompWeak = 1;
        int pressurizeAmount = Asc.Deadly(ascension, 20, 15);   // SteamEruption gained by Pressurize
        const int eruptionPerMove = 3;                          // every OTHER move accumulates +3 SteamEruption
        var monster = new Monster { Name = "WaterfallGiant", MaxHp = hp, CurrentHp = hp };

        var pressurize = new MoveState("PRESSURIZE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new SteamEruptionPower(), pressurizeAmount, self),
            intentDamage: null);
        var stomp = new MoveState("STOMP_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, stompDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), stompWeak, self);
                Cmd.ApplyPower(combat, self, new SteamEruptionPower(), eruptionPerMove, self);
            },
            intentDamage: stompDamage);
        var ram = new MoveState("RAM_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, ramDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new SteamEruptionPower(), eruptionPerMove, self);
            },
            intentDamage: ramDamage);
        var siphon = new MoveState("SIPHON_MOVE",
            (combat, self) =>
            {
                self.Heal(siphonHeal);   // SiphonHeal × players.Count == ×1 in single-player
                Cmd.ApplyPower(combat, self, new SteamEruptionPower(), eruptionPerMove, self);
            },
            intentDamage: null);

        // Pressure Gun ramps +5 per USE (the game's CurrentPressureGunDamage). The ramp is PER-CREATURE state, so it
        // must clone + hash with the monster — NOT a shared-closure counter (MoveState definitions are shared by
        // reference across every cloned search branch; mutating one would leak ramp between unrelated nodes). We ride
        // the ramp on a per-creature counter power (PressureGunPower): the move reads it for this hit's bonus, then
        // increments it — exactly the Entomancer/PersonalHive read-a-power-amount pattern. IntentDamage stays the
        // BASE telegraph (the displayed intent is informational; the resolved hit uses the live ramped value).
        var pressureGun = new MoveState("PRESSURE_GUN_MOVE",
            (combat, self) =>
            {
                int hit = pressureGunDamage + pressureGunIncrease * self.GetPowerAmount("PressureGun");
                Cmd.Attack(combat, self, combat.Player, hit, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new PressureGunPower(), 1, self);   // +1 use ⇒ next Pressure Gun +5
                Cmd.ApplyPower(combat, self, new SteamEruptionPower(), eruptionPerMove, self);
            },
            intentDamage: pressureGunDamage);
        var pressureUp = new MoveState("PRESSURE_UP_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, pressureUpDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new SteamEruptionPower(), eruptionPerMove, self);
            },
            intentDamage: pressureUpDamage);

        // Death phase (Steam Eruption). When brought to 0 HP the boss survives (engine InDeathPhase) and its AI is
        // forced to ABOUT_TO_BLOW: a one-turn no-damage self-stun telegraph (intentDamage null — it flags the impending
        // hit but deals nothing this turn), MustPerformOnceBeforeTransitioning in the game. Its FollowUp is EXPLODE,
        // which deals damage equal to the accumulated SteamEruption counter to the player, then clears InDeathPhase so
        // the boss truly dies. EXPLODE self-loops (game ExplodeMove.FollowUpState = itself) — once InDeathPhase is
        // cleared the monster is dead, so it never runs again.
        var aboutToBlow = new MoveState("ABOUT_TO_BLOW_MOVE",
            (combat, self) => { /* windup: self-stun, no damage; the SteamEruption counter stands captured */ },
            intentDamage: null);
        var explode = new MoveState("EXPLODE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, self.GetPowerAmount("SteamEruption"), ValueProp.Move, null);
                self.InDeathPhase = false;   // the blow has landed ⇒ the boss truly dies now (IsAlive → false)
            },
            intentDamage: null);

        pressurize.FollowUp = stomp;
        stomp.FollowUp = ram;
        ram.FollowUp = siphon;
        siphon.FollowUp = pressureGun;
        pressureGun.FollowUp = pressureUp;
        pressureUp.FollowUp = stomp;          // loop back to Stomp (not Pressurize — that fires only once, at start)
        aboutToBlow.FollowUp = explode;
        explode.FollowUp = explode;           // self-loop (game); inert once InDeathPhase clears (monster is dead)

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { pressurize, stomp, ram, siphon, pressureGun, pressureUp, aboutToBlow, explode },
            pressurize.Id);
        monster.DeathPhaseEntryMove = aboutToBlow.Id;   // 0 HP ⇒ jump to ABOUT_TO_BLOW instead of dying
        return monster;
    }

    // ----- Act-1 (Overgrowth) NORMAL / WEAK monsters. Stats from the decompiled game source. -----
    // UNIT-TESTED ONLY (move stats/intents/HP scaling) — no game traces exist for these yet, so they are
    // not trace-validated. Mechanics use only existing engine primitives + Common/MonsterPowers.

    /// <summary>
    /// SnappingJaxfruit (Act 1 Overgrowth normal, MegaCrit): HP 31–33 (Tough 34–36); factory defaults to the
    /// max roll (33 / 36 Tough), matching the project convention — trace replay injects the actual roll. One
    /// move forever — Energy Orb: 3 damage (4 Deadly) then self +2 Strength, so its hit ramps each turn.
    /// </summary>
    public static Monster SnappingJaxfruit(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 36, 33);
        int orbDamage = Asc.Deadly(ascension, 4, 3), orbStrength = 2;
        var monster = new Monster { Name = "SnappingJaxfruit", MaxHp = hp, CurrentHp = hp };

        var orb = new MoveState("ENERGY_ORB_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, orbDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), orbStrength, self);
            },
            intentDamage: orbDamage);
        orb.FollowUp = orb;   // self-loop

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { orb }, orb.Id);
        return monster;
    }

    /// <summary>
    /// Flyconid (Act 1 Overgrowth normal, MegaCrit): HP 47–49 (Tough 51–53), factory defaults to max roll
    /// (49 / 53 Tough). Opens randomly (no repeat):
    /// Frail Spores (8 + apply 2 Frail, weight 2) / Smash (11, weight 1). Then loops a random no-repeat
    /// branch: Vulnerable Spores (apply 2 Vulnerable, weight 3) / Frail Spores (8 + 2 Frail, weight 2) /
    /// Smash (11, weight 1). Deadly: Spores 9, Smash 12.
    /// </summary>
    public static Monster Flyconid(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 53, 49);
        int sporeDamage = Asc.Deadly(ascension, 9, 8), smashDamage = Asc.Deadly(ascension, 12, 11);
        int vulnerable = 2, frail = 2;
        var monster = new Monster { Name = "Flyconid", MaxHp = hp, CurrentHp = hp };

        var vulnSpores = new MoveState("VULNERABLE_SPORES_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), vulnerable, self),
            intentDamage: null);
        var frailSpores = new MoveState("FRAIL_SPORES_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, sporeDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new FrailPower(), frail, self);
            },
            intentDamage: sporeDamage);
        var smash = new MoveState("SMASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, smashDamage, ValueProp.Move, null),
            intentDamage: smashDamage);

        var rand = new RandomBranchState("RAND")
            .Add(vulnSpores.Id, 3f, MoveRepeatType.CannotRepeat)
            .Add(frailSpores.Id, 2f, MoveRepeatType.CannotRepeat)
            .Add(smash.Id, 1f, MoveRepeatType.CannotRepeat);
        var initial = new RandomBranchState("INITIAL")
            .Add(frailSpores.Id, 2f, MoveRepeatType.CannotRepeat)
            .Add(smash.Id, 1f, MoveRepeatType.CannotRepeat);
        vulnSpores.FollowUp = rand;
        frailSpores.FollowUp = rand;
        smash.FollowUp = rand;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { vulnSpores, frailSpores, smash, rand, initial }, initial.Id);
        return monster;
    }

    /// <summary>
    /// CubexConstruct (Act 1 Overgrowth normal, MegaCrit): HP 65 (Tough 70). Starts with 13 block and
    /// Artifact(1) (AfterAddedToRoom). Chain: Charge Up (+2 Strength) → Repeater Blast (7 + self +2 Str) →
    /// Repeater Blast (7 + self +2 Str) → Expel (5×2) → back to the first Repeater Blast. Deadly: Blast 8,
    /// Expel 6. (Blast ramps as Strength accrues.)
    /// </summary>
    public static Monster CubexConstruct(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 70, 65);
        int blastDamage = Asc.Deadly(ascension, 8, 7), expelDamage = Asc.Deadly(ascension, 6, 5);
        int chargeStrength = 2, blastStrength = 2, startBlock = 13, artifact = 1;
        var monster = new Monster { Name = "CubexConstruct", MaxHp = hp, CurrentHp = hp };

        var chargeUp = new MoveState("CHARGE_UP_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), chargeStrength, self),
            intentDamage: null);
        Action<CombatState, Monster> blastFn = (combat, self) =>
        {
            Cmd.Attack(combat, self, combat.Player, blastDamage, ValueProp.Move, null);
            Cmd.ApplyPower(combat, self, new StrengthPower(), blastStrength, self);
        };
        var blast1 = new MoveState("REPEATER_BLAST_MOVE", blastFn, intentDamage: blastDamage);
        var blast2 = new MoveState("REPEATER_BLAST_MOVE_2", blastFn, intentDamage: blastDamage);
        var expel = new MoveState("EXPEL_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, expelDamage, 2, ValueProp.Move, null),
            intentDamage: expelDamage, intentHits: 2);

        chargeUp.FollowUp = blast1;
        blast1.FollowUp = blast2;
        blast2.FollowUp = expel;
        expel.FollowUp = blast1;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { chargeUp, blast1, blast2, expel }, chargeUp.Id);
        monster.AddPower(new ArtifactPower(), artifact);   // applied at combat start
        monster.Block = startBlock;                        // 13 block gained AfterAddedToRoom
        return monster;
    }

    /// <summary>
    /// FuzzyWurmCrawler (Act 1 Overgrowth weak, MegaCrit): HP 55–57 (Tough 58–59), factory defaults to max
    /// roll (57 / 59 Tough). Cycle: Acid Goop (4) → Inhale (+7 Strength) → Acid Goop (4) → … i.e. a
    /// Goop/Inhale/Goop 3-cycle, so its Goop ramps hard. Deadly: Acid Goop 6.
    /// </summary>
    public static Monster FuzzyWurmCrawler(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 59, 57);
        int goopDamage = Asc.Deadly(ascension, 6, 4), inhaleStrength = 7;
        var monster = new Monster { Name = "FuzzyWurmCrawler", MaxHp = hp, CurrentHp = hp };

        Action<CombatState, Monster> goopFn = (combat, self) =>
            Cmd.Attack(combat, self, combat.Player, goopDamage, ValueProp.Move, null);
        var firstGoop = new MoveState("FIRST_ACID_GOOP", goopFn, intentDamage: goopDamage);
        var goop = new MoveState("ACID_GOOP", goopFn, intentDamage: goopDamage);
        var inhale = new MoveState("INHALE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), inhaleStrength, self),
            intentDamage: null);

        firstGoop.FollowUp = inhale;
        inhale.FollowUp = goop;
        goop.FollowUp = firstGoop;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { firstGoop, goop, inhale }, firstGoop.Id);
        return monster;
    }

    /// <summary>
    /// ShrinkerBeetle (Act 1 Overgrowth weak, MegaCrit): HP 38–40 (Tough 40–42), factory defaults to max
    /// roll (40 / 42 Tough). Opens on Shrink (apply Shrink to the player: its powered attacks deal ×0.7 for
    /// the rest of combat), then loops Chomp (7) → Stomp (13) → Chomp → … Deadly: Chomp 8, Stomp 14.
    /// </summary>
    public static Monster ShrinkerBeetle(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 42, 40);
        int chompDamage = Asc.Deadly(ascension, 8, 7), stompDamage = Asc.Deadly(ascension, 14, 13);
        var monster = new Monster { Name = "ShrinkerBeetle", MaxHp = hp, CurrentHp = hp };

        var shrink = new MoveState("SHRINKER_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new ShrinkPower(), -1, self),
            intentDamage: null);
        var chomp = new MoveState("CHOMP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, chompDamage, ValueProp.Move, null),
            intentDamage: chompDamage);
        var stomp = new MoveState("STOMP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, stompDamage, ValueProp.Move, null),
            intentDamage: stompDamage);

        shrink.FollowUp = chomp;
        chomp.FollowUp = stomp;
        stomp.FollowUp = chomp;   // loop (Shrink only at start)

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { shrink, chomp, stomp }, shrink.Id);
        return monster;
    }

    /// <summary>
    /// Mawler (Act 1 Overgrowth normal, MegaCrit): HP 72 (Tough 76). Opens on Claw (4×2), then a random
    /// no-repeat branch (equal weight): Rip and Tear (14) / Roar (apply 3 Vulnerable, only once per fight) /
    /// Claw (4×2). Deadly: Rip and Tear 16, Claw 5.
    /// </summary>
    public static Monster Mawler(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 76, 72);
        int ripDamage = Asc.Deadly(ascension, 16, 14), clawDamage = Asc.Deadly(ascension, 5, 4), vulnerable = 3;
        var monster = new Monster { Name = "Mawler", MaxHp = hp, CurrentHp = hp };

        var rip = new MoveState("RIP_AND_TEAR_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, ripDamage, ValueProp.Move, null),
            intentDamage: ripDamage);
        var roar = new MoveState("ROAR_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), vulnerable, self),
            intentDamage: null);
        var claw = new MoveState("CLAW_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, clawDamage, 2, ValueProp.Move, null),
            intentDamage: clawDamage, intentHits: 2);

        var rand = new RandomBranchState("RAND")
            .Add(rip.Id, 1f, MoveRepeatType.CannotRepeat)
            .Add(roar.Id, 1f, MoveRepeatType.UseOnlyOnce)
            .Add(claw.Id, 1f, MoveRepeatType.CannotRepeat);
        rip.FollowUp = rand; roar.FollowUp = rand; claw.FollowUp = rand;

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { rip, roar, claw, rand }, claw.Id);
        return monster;
    }

    /// <summary>
    /// Nibbit (Act 1 Overgrowth normal/weak, MegaCrit): HP 42–46 (Tough 44–48), factory defaults to max roll
    /// (46 / 48 Tough). Deterministic 3-cycle:
    /// Butt (12) → Slice (6 + gain 5 block) → Hiss (+2 Strength) → Butt → … The starting move depends on
    /// position (matching the game's conditional opener): alone → Butt; encounter front → Slice; encounter
    /// back → Hiss. Deadly: Butt 13, Slice 7, Hiss +3 Str. Tough: Slice block 6.
    /// </summary>
    public static Monster Nibbit(int hp = -1, string slot = "alone", int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 48, 46);
        int buttDamage = Asc.Deadly(ascension, 13, 12), sliceDamage = Asc.Deadly(ascension, 7, 6);
        int sliceBlock = Asc.Tough(ascension, 6, 5), hissStrength = Asc.Deadly(ascension, 3, 2);
        var monster = new Monster { Name = "Nibbit", MaxHp = hp, CurrentHp = hp, Variant = slot };

        var butt = new MoveState("BUTT_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, buttDamage, ValueProp.Move, null),
            intentDamage: buttDamage);
        var slice = new MoveState("SLICE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, sliceDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, sliceBlock, ValueProp.Move, null);
            },
            intentDamage: sliceDamage);
        var hiss = new MoveState("HISS_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), hissStrength, self),
            intentDamage: null);

        butt.FollowUp = slice;
        slice.FollowUp = hiss;
        hiss.FollowUp = butt;

        var initial = slot switch
        {
            "front" => slice.Id,
            "back" => hiss.Id,
            _ => butt.Id,     // "alone"
        };
        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { butt, slice, hiss }, initial);
        return monster;
    }

    /// <summary>
    /// Inklet (Act 1 Overgrowth normal, MegaCrit): HP 11–17 (Tough 12–18). Starts with Slippery(1) (its
    /// first incoming hit is capped to 1 HP, then it wears off). Side Inklets open on a random branch
    /// (no repeat): Jab (3, weight 2) / Whirlwind (2×3, weight 1); the middle Inklet opens on Whirlwind.
    /// After Jab → random (no repeat): Piercing Gaze (10) / Whirlwind (2×3); Whirlwind and Piercing Gaze
    /// both go back to Jab. Deadly: Jab 4, Whirlwind 3, Piercing Gaze 11.
    /// </summary>
    public static Monster Inklet(int hp = -1, bool middle = false, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 18, 17);   // max roll (range 11–17 / Tough 12–18); replay injects actual
        int jabDamage = Asc.Deadly(ascension, 4, 3), whirlwindDamage = Asc.Deadly(ascension, 3, 2);
        int gazeDamage = Asc.Deadly(ascension, 11, 10), slippery = 1;
        var monster = new Monster { Name = "Inklet", MaxHp = hp, CurrentHp = hp, Variant = middle ? "M" : "S" };

        var jab = new MoveState("JAB_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, jabDamage, ValueProp.Move, null),
            intentDamage: jabDamage);
        var whirlwind = new MoveState("WHIRLWIND_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, whirlwindDamage, 3, ValueProp.Move, null),
            intentDamage: whirlwindDamage, intentHits: 3);
        var gaze = new MoveState("PIERCING_GAZE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, gazeDamage, ValueProp.Move, null),
            intentDamage: gazeDamage);

        // After Jab: random no-repeat between Piercing Gaze and Whirlwind.
        var afterJab = new RandomBranchState("RAND")
            .Add(gaze.Id, 1f, MoveRepeatType.CannotRepeat)
            .Add(whirlwind.Id, 1f, MoveRepeatType.CannotRepeat);
        jab.FollowUp = afterJab;
        whirlwind.FollowUp = jab;
        gaze.FollowUp = jab;

        // Side Inklets' opener: random no-repeat Jab(2) / Whirlwind(1). Middle opens straight on Whirlwind.
        var initialRand = new RandomBranchState("INIT_RAND")
            .Add(jab.Id, 2f, MoveRepeatType.CannotRepeat)
            .Add(whirlwind.Id, 1f, MoveRepeatType.CannotRepeat);

        var states = new MonsterState[] { jab, whirlwind, gaze, afterJab, initialRand };
        var initial = middle ? whirlwind.Id : initialRand.Id;
        monster.Ai = new MonsterMoveStateMachine(states, initial);
        monster.AddPower(new SlipperyPower(), slippery);   // applied at combat start
        return monster;
    }

    /// <summary>The two Nibbits of NibbitsNormal: front (opens Slice) + back (opens Hiss).</summary>
    public static IEnumerable<Monster> Nibbits(int ascension = 0)
    {
        yield return Nibbit(slot: "front", ascension: ascension);
        yield return Nibbit(slot: "back", ascension: ascension);
    }

    /// <summary>The three Inklets of InkletsNormal: side, middle, side.</summary>
    public static IEnumerable<Monster> Inklets(int ascension = 0)
    {
        yield return Inklet(middle: false, ascension: ascension);
        yield return Inklet(middle: true, ascension: ascension);
        yield return Inklet(middle: false, ascension: ascension);
    }

    /// <summary>Inject n Infection status cards into the player's discard pile (Infect / Wriggle).</summary>
    private static void AddStatusToDiscard(CombatState combat, int n)
    {
        for (int i = 0; i < n; i++) combat.Player.DiscardPile.Add(new Infection());
    }

    private static Monster Cultist(string name, int hp, int darkStrikeDamage, int ritualAmount)
    {
        var monster = new Monster { Name = name, MaxHp = hp, CurrentHp = hp };

        var incantation = new MoveState("INCANTATION_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new RitualPower(), ritualAmount, self),
            intentDamage: null);

        var darkStrike = new MoveState("DARK_STRIKE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, darkStrikeDamage, ValueProp.Move, null),
            intentDamage: darkStrikeDamage);

        incantation.FollowUp = darkStrike;
        darkStrike.FollowUp = darkStrike;   // self-loop

        monster.Ai = new MonsterMoveStateMachine(new MonsterState[] { incantation, darkStrike }, incantation.Id);
        return monster;
    }
}
