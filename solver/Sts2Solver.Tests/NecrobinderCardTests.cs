using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the ported Necrobinder cards/powers against the game's decompiled
/// logic: damage/block/summon/Doom plus the Osty interactions (DieForYou redirection, NecroMastery
/// reflection, Osty attacks) and the per-turn counters they rely on.</summary>
public class NecrobinderCardTests
{
    private static readonly string[] PoolNames =
    {
        "StrikeNecrobinder","DefendNecrobinder","Bodyguard","Unleash","Afterlife","BansheesCry","BlightStrike",
        "BoneShards","BorrowedTime","Bury","Calcify","CallOfTheVoid","CaptureSpirit","Cleanse","Countdown",
        "DanseMacabre","DeathMarch","Deathbringer","DeathsDoor","Debilitate","Defile","Defy","Delay","Demesne",
        "DevourLife","Dirge","DrainPower","Dredge","Eidolon","EndOfDays","EnfeeblingTouch","Eradicate","Fear",
        "Fetch","Flatten","ForbiddenGrimoire","Friendship","GlimpseBeyond","GraveWarden","Graveblast","Hang",
        "Haunt","HighFive","Invoke","LegionOfBone","Lethality","Melancholy","Misery","NecroMastery",
        "NegativePulse","Neurosurge","NoEscape","Oblivion","Pagestorm","Parse","Poke","Protector","PullAggro",
        "PullFromBelow","Putrefy","Rattle","Reanimate","Reap","ReaperForm","Reave","RightHandHand","Sacrifice",
        "Scourge","SculptingStrike","Seance","SentryMode","Severance","SharedFate","Shroud","SicEm",
        "SleightOfFlesh","Snap","SoulStorm","Sow","SpiritOfAsh","Spur","Squeeze","TheScythe","TimesUp",
        "Transfigure","Undeath","Veilpiercer","Wisp",
    };

    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80, string[]? relics = null)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp, relics: relics);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static (CombatState combat, Player p, Monster a, Monster b) Fight2(int hp = 60)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var a = Monsters.CalcifiedCultist(hp: hp);
        var b = Monsters.CalcifiedCultist(hp: hp);
        var combat = Catalog.SetupCombat(player, new[] { a, b });
        player.ResetEnergy();
        return (combat, player, a, b);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target, int energy = 3)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(energy, card.Cost);
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    /// <summary>Summon Osty at exactly <paramref name="hp"/> HP for a clean fixture.</summary>
    private static void GiveOsty(CombatState combat, int hp) => NecroOsty.Summon(combat, hp);

    // ─────────────── Catalog / wiring ───────────────

    [Fact]
    public void All_88_Cards_Build_And_Are_In_Pool()
    {
        Assert.Equal(88, PoolNames.Length);
        foreach (var name in PoolNames)
        {
            var card = Catalog.BuildCard(name);
            Assert.Equal(name, card.Name);
            Assert.Contains(name, Catalog.CardPool);
        }
        // Tokens build but stay out of the pool.
        Assert.Equal("Soul", Catalog.BuildCard("Soul").Name);
        Assert.Equal("SweepingGaze", Catalog.BuildCard("SweepingGaze").Name);
        Assert.DoesNotContain("Soul", Catalog.CardPool);
    }

    [Fact]
    public void Starter_Deck_Is_4_Strike_4_Defend_Bodyguard_Unleash()
    {
        var deck = Catalog.NecrobinderStarterDeck();
        Assert.Equal(10, deck.Count);
        Assert.Equal(4, deck.Count(c => c.Name == "StrikeNecrobinder"));
        Assert.Equal(4, deck.Count(c => c.Name == "DefendNecrobinder"));
        Assert.Equal(1, deck.Count(c => c.Name == "Bodyguard"));
        Assert.Equal(1, deck.Count(c => c.Name == "Unleash"));
    }

    [Fact]
    public void BoundPhylactery_Builds() => Assert.Equal("BoundPhylactery", Catalog.BuildRelic("BoundPhylactery").Id);

    // ─────────────── Basics ───────────────

    [Fact]
    public void Strike_Deals_6_Defend_Blocks_5()
    {
        var (c, p, m) = Fight();
        Play(c, new StrikeNecrobinder(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Play(c, new DefendNecrobinder(), null);
        Assert.Equal(5, p.Block);
    }

    [Fact]
    public void Bury_Deals_52() { var (c, _, m) = Fight(monsterHp: 99); Play(c, new Bury(), m, energy: 4); Assert.Equal(99 - 52, m.CurrentHp); }

    // ─────────────── Summon / Osty growth ───────────────

    [Fact]
    public void Bodyguard_Summons_Osty_At_5()
    {
        var (c, p, _) = Fight();
        Play(c, new Bodyguard(), null);
        Assert.True(p.IsOstyAlive);
        Assert.Equal(5, p.Osty!.CurrentHp);
        Assert.Equal(5, p.Osty.MaxHp);
    }

    [Fact]
    public void Summon_Grows_Alive_Osty_MaxHp_And_Heals()
    {
        var (c, p, _) = Fight();
        GiveOsty(c, 5);
        p.Osty!.CurrentHp = 2;                 // damaged
        Play(c, new Bodyguard(), null);        // summon 5 more
        Assert.Equal(10, p.Osty.MaxHp);
        Assert.Equal(7, p.Osty.CurrentHp);     // 2 + 5
    }

    [Fact]
    public void Summon_Recreates_Dead_Osty_At_Full()
    {
        var (c, p, _) = Fight();
        GiveOsty(c, 5);
        p.Osty!.CurrentHp = 0;                 // dead
        Play(c, new Reanimate(), null, energy: 3);
        Assert.Equal(20, p.Osty.MaxHp);
        Assert.Equal(20, p.Osty.CurrentHp);
    }

    // ─────────────── DieForYou redirection ───────────────

    [Fact]
    public void DieForYou_Redirects_Powered_Enemy_Attack_Onto_Osty()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 10);
        Cmd.Attack(c, m, p, 7, ValueProp.Move, null);   // enemy hits the player
        Assert.Equal(80, p.CurrentHp);                  // player untouched
        Assert.Equal(3, p.Osty!.CurrentHp);             // Osty took 7
    }

    [Fact]
    public void DieForYou_Does_Not_Redirect_When_Osty_Dead()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 5);
        p.Osty!.CurrentHp = 0;
        Cmd.Attack(c, m, p, 7, ValueProp.Move, null);
        Assert.Equal(80 - 7, p.CurrentHp);
    }

    // ─────────────── NecroMastery reflection ───────────────

    [Fact]
    public void NecroMastery_Reflects_Osty_HP_Loss_To_All_Enemies()
    {
        var (c, p, a, b) = Fight2();
        Play(c, new NecroMastery(), null, energy: 2);   // summon 5 + NecroMastery 1
        Cmd.Attack(c, a, p, 4, ValueProp.Move, null);   // redirected to Osty -> loses 4 -> reflect 4 to all
        Assert.Equal(60 - 4, a.CurrentHp);
        Assert.Equal(60 - 4, b.CurrentHp);
        Assert.Equal(1, p.Osty!.CurrentHp);             // 5 - 4
    }

    // ─────────────── Osty attacks ───────────────

    [Fact]
    public void Poke_Deals_6_From_Osty()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 5);
        Play(c, new Poke(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Assert.Equal(1, c.OstyAttacksThisTurn);
    }

    [Fact]
    public void Osty_Attack_Fizzles_With_No_Osty()
    {
        var (c, _, m) = Fight();
        Play(c, new Poke(), m);
        Assert.Equal(60, m.CurrentHp);
        Assert.Equal(0, c.OstyAttacksThisTurn);
    }

    [Fact]
    public void Unleash_Deals_6_Plus_Osty_CurrentHp()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 8);
        Play(c, new Unleash(), m);
        Assert.Equal(60 - (6 + 8), m.CurrentHp);
    }

    [Fact]
    public void Protector_Deals_10_Plus_Osty_MaxHp()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        GiveOsty(c, 12);
        p.Osty!.CurrentHp = 3;                  // damaged: uses MAX hp, not current
        Play(c, new Protector(), m);
        Assert.Equal(99 - (10 + 12), m.CurrentHp);
    }

    [Fact]
    public void Calcify_Adds_4_To_Osty_Attacks()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 5);
        Play(c, new Calcify(), null);
        Play(c, new Poke(), m);
        Assert.Equal(60 - (6 + 4), m.CurrentHp);
    }

    [Fact]
    public void Rattle_Hits_Once_Plus_Prior_Osty_Attacks()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        GiveOsty(c, 5);
        Play(c, new Poke(), m);                 // 1 prior osty attack
        int hpAfterPoke = m.CurrentHp;
        Play(c, new Rattle(), m);               // hits 1 + 1 = 2 times, 7 each
        Assert.Equal(hpAfterPoke - 14, m.CurrentHp);
    }

    [Fact]
    public void Flatten_Costs_Zero_After_An_Osty_Attack()
    {
        var (c, p, m) = Fight();
        GiveOsty(c, 5);
        Assert.Equal(2, new Flatten().EffectiveCost(c));
        c.OstyAttacksThisTurn = 1;
        Assert.Equal(0, new Flatten().EffectiveCost(c));
    }

    [Fact]
    public void Squeeze_Scales_With_Other_Osty_Attacks_In_Deck()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        GiveOsty(c, 5);
        p.DrawPile.Add(new Poke());
        p.DiscardPile.Add(new Snap());          // 2 other OstyAttack cards
        Play(c, new Squeeze(), m, energy: 3);
        Assert.Equal(99 - (25 + 5 * 2), m.CurrentHp);
    }

    [Fact]
    public void BoneShards_Hits_All_Blocks_And_Kills_Osty()
    {
        var (c, p, a, b) = Fight2();
        GiveOsty(c, 5);
        Play(c, new BoneShards(), null);
        Assert.Equal(60 - 9, a.CurrentHp);
        Assert.Equal(60 - 9, b.CurrentHp);
        Assert.Equal(9, p.Block);
        Assert.True(p.IsOstyMissing);
    }

    [Fact]
    public void Sacrifice_Blocks_Twice_Osty_MaxHp_And_Kills_It()
    {
        var (c, p, _) = Fight();
        GiveOsty(c, 7);
        Play(c, new Sacrifice(), null);
        Assert.Equal(14, p.Block);
        Assert.True(p.IsOstyMissing);
    }

    [Fact]
    public void Sacrifice_With_NecroMastery_Reflects_Osty_HP()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Play(c, new NecroMastery(), null, energy: 2);   // Osty 5
        Play(c, new Sacrifice(), null);                 // kill 5-HP Osty -> reflect 5
        Assert.Equal(99 - 5, m.CurrentHp);
    }

    [Fact]
    public void Spur_Summons_And_Heals_Osty()
    {
        var (c, p, _) = Fight();
        GiveOsty(c, 5);
        p.Osty!.CurrentHp = 1;
        Play(c, new Spur(), null);                      // +3 max (now 8, cur 4), heal 5 -> cur 8 (capped at 8)
        Assert.Equal(8, p.Osty.MaxHp);
        Assert.Equal(8, p.Osty.CurrentHp);
    }

    // ─────────────── Doom ───────────────

    [Fact]
    public void Scourge_Applies_13_Doom()
    {
        var (c, _, m) = Fight();
        Play(c, new Scourge(), m);
        Assert.Equal(13, m.GetPowerAmount("Doom"));
        Assert.True(c.DoomAppliedThisTurn);
    }

    [Fact]
    public void EndOfDays_Kills_Doomed_Enemies_Immediately()
    {
        var (c, p, a, b) = Fight2(hp: 20);
        Play(c, new EndOfDays(), null, energy: 3);      // 29 Doom >= 20 HP -> both die now
        Assert.False(a.IsAlive);
        Assert.False(b.IsAlive);
    }

    [Fact]
    public void Doom_Executes_At_Enemy_Turn_End()
    {
        var (c, p, m) = Fight(monsterHp: 8);
        Cmd.ApplyPower(c, m, new DoomPower(), 10, p);   // 8 <= 10
        CombatManager.RunEnemyTurn(c);                  // at enemy turn end, doomed -> dies
        Assert.False(m.IsAlive);
    }

    [Fact]
    public void BlightStrike_Applies_Doom_Equal_To_Damage()
    {
        var (c, _, m) = Fight();
        Play(c, new BlightStrike(), m);
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Equal(8, m.GetPowerAmount("Doom"));
    }

    [Fact]
    public void TimesUp_Deals_Damage_Equal_To_Target_Doom()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, m, new DoomPower(), 15, p);
        Play(c, new TimesUp(), m, energy: 2);
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void ReaperForm_Applies_Doom_From_Powered_Attacks()
    {
        var (c, _, m) = Fight();
        Play(c, new ReaperForm(), null, energy: 3);
        Play(c, new StrikeNecrobinder(), m);            // deals 6 -> 6 Doom
        Assert.Equal(6, m.GetPowerAmount("Doom"));
    }

    [Fact]
    public void NoEscape_Scales_With_Existing_Doom()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, m, new DoomPower(), 25, p);   // floor(25/10)=2
        Play(c, new NoEscape(), m);                     // +10 + 5*2 = 20
        Assert.Equal(25 + 20, m.GetPowerAmount("Doom"));
    }

    [Fact]
    public void Shroud_Gains_Block_When_Doom_Applied()
    {
        var (c, p, m) = Fight();
        Play(c, new Shroud(), null);                    // 2 block per Doom application
        Play(c, new Scourge(), m);                      // applies Doom
        Assert.Equal(2, p.Block);
    }

    [Fact]
    public void DeathsDoor_Triples_Block_After_Applying_Doom()
    {
        var (c, p, m) = Fight();
        Play(c, new Scourge(), m);                       // doom applied this turn
        Play(c, new DeathsDoor(), null);                 // 3 × 6
        Assert.Equal(18, p.Block);
    }

    // ─────────────── debuffs / scaling ───────────────

    [Fact]
    public void Debilitate_Doubles_Vulnerable_Effect()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Cmd.ApplyPower(c, m, new VulnerablePower(), 5, p);
        Cmd.ApplyPower(c, m, new DebilitatePower(), 1, p);
        Play(c, new StrikeNecrobinder(), m);             // 6 × 2.0 = 12 (instead of ×1.5 = 9)
        Assert.Equal(99 - 12, m.CurrentHp);
    }

    [Fact]
    public void Lethality_Boosts_Only_The_First_Attack()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Play(c, new Lethality(), null);                  // +50% to first attack
        Play(c, new StrikeNecrobinder(), m);             // 6 ×1.5 = 9
        int afterFirst = m.CurrentHp;
        Assert.Equal(99 - 9, afterFirst);
        Play(c, new StrikeNecrobinder(), m);             // 6 (no boost)
        Assert.Equal(afterFirst - 6, m.CurrentHp);
    }

    [Fact]
    public void Hang_Damage_Doubles_Each_Play()
    {
        var (c, p, m) = Fight(monsterHp: 200);
        Play(c, new Hang(), m);                          // 10, then Hang 2 applied
        Assert.Equal(200 - 10, m.CurrentHp);
        Play(c, new Hang(), m);                          // 10 × 2 = 20, then Hang -> 4
        Assert.Equal(200 - 10 - 20, m.CurrentHp);
        Assert.Equal(4, m.GetPowerAmount("Hang"));
    }

    [Fact]
    public void TheScythe_Grows_Three_Each_Play()
    {
        var (c, _, m) = Fight(monsterHp: 99);
        var scythe = new TheScythe();
        Play(c, scythe, m);                              // 13
        Assert.Equal(99 - 13, m.CurrentHp);
        Assert.Equal(16, scythe.Damage);                 // 13 + 3
    }

    [Fact]
    public void SoulStorm_Scales_With_Souls_In_Exhaust()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        p.ExhaustPile.Add(new Soul());
        p.ExhaustPile.Add(new Soul());                   // 2 souls
        Play(c, new SoulStorm(), m);                     // 9 + 2*2 = 13
        Assert.Equal(99 - 13, m.CurrentHp);
    }

    [Fact]
    public void Eradicate_Hits_Once_Per_Energy_Spent()
    {
        var (c, _, m) = Fight(monsterHp: 99);
        Play(c, new Eradicate(), m, energy: 2);          // X = 2 -> 11 × 2
        Assert.Equal(99 - 22, m.CurrentHp);
    }

    [Fact]
    public void PullFromBelow_Hits_Per_Ethereal_Played()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Play(c, new Defile(), m);                        // an Ethereal attack (also deals 13)
        int afterDefile = m.CurrentHp;
        Assert.Equal(1, c.EtherealPlayedThisCombat);
        Play(c, new PullFromBelow(), m);                 // 5 × 1 hit
        Assert.Equal(afterDefile - 5, m.CurrentHp);
    }

    // ─────────────── powers / energy / next-turn ───────────────

    [Fact]
    public void Friendship_Loses_Strength_And_Adds_Max_Energy()
    {
        var (c, p, _) = Fight();
        Play(c, new Friendship(), null);
        Assert.Equal(-2, p.GetPowerAmount("Strength"));
        Assert.Equal(4, p.EffectiveMaxEnergy);           // 3 + 1
    }

    [Fact]
    public void EnfeeblingTouch_Reduces_Enemy_Strength_This_Turn()
    {
        var (c, p, m) = Fight();
        Play(c, new EnfeeblingTouch(), m);
        Assert.Equal(-8, m.GetPowerAmount("Strength"));  // pushed immediately, undone at enemy turn end
    }

    [Fact]
    public void SleightOfFlesh_Damages_On_Debuff_Application()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Play(c, new SleightOfFlesh(), null);             // 9 damage per debuff applied
        Play(c, new Scourge(), m);                       // applies Doom (a debuff)
        Assert.Equal(99 - 9, m.CurrentHp);
    }

    [Fact]
    public void SicEm_Summons_When_Osty_Hits_The_Marked_Enemy()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        GiveOsty(c, 5);
        Play(c, new SicEm(), m);                         // Osty deals 5, applies 2 SicEm
        int ostyHpBefore = p.Osty!.CurrentHp;
        Play(c, new Poke(), m);                          // Osty hits marked enemy -> summon 2
        Assert.Equal(ostyHpBefore + 2, p.Osty.CurrentHp);
    }

    [Fact]
    public void Eidolon_Gains_Intangible_When_Exhausting_9_Plus()
    {
        var (c, p, _) = Fight();
        for (int i = 0; i < 9; i++) p.Hand.Add(new DefendNecrobinder());
        Play(c, new Eidolon(), null, energy: 2);
        Assert.Equal(1, p.GetPowerAmount("Intangible"));
        Assert.Equal(10, p.ExhaustPile.Count);            // 9 hand cards + Eidolon itself
    }

    [Fact]
    public void Intangible_Clamps_Incoming_Damage_To_One()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new IntangiblePower(), 1, p);
        Cmd.Attack(c, m, p, 30, ValueProp.Move, null);
        Assert.Equal(80 - 1, p.CurrentHp);
    }

    [Fact]
    public void Veilpiercer_Makes_The_Next_Ethereal_Card_Free()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new VeilpiercerPower(), 1, p);
        // The discount is applied through the power pipeline in PlayCard, not the static EffectiveCost.
        var defile = new Defile();                        // Ethereal, normally costs 1
        p.Hand.Add(defile);
        p.MaxEnergy = 0; p.ResetEnergy();                 // 0 energy: only the Veilpiercer discount makes it playable
        CombatManager.PlayCard(c, defile, m);
        Assert.Equal(60 - 13, m.CurrentHp);
        Assert.Equal(0, p.GetPowerAmount("Veilpiercer")); // one charge consumed
    }

    [Fact]
    public void BansheesCry_Cost_Drops_Per_Ethereal_Played()
    {
        var (c, p, m) = Fight();
        Assert.Equal(9, new BansheesCry().EffectiveCost(c));
        Play(c, new Defile(), m);                         // +1 ethereal
        Play(c, new Defile(), m);                         // +2 ethereal
        Assert.Equal(9 - 4, new BansheesCry().EffectiveCost(c));
    }

    [Fact]
    public void Delay_Grants_Energy_Next_Turn()
    {
        var (c, p, m) = Fight();
        Play(c, new Delay(), null);
        Assert.Equal(11, p.Block);
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(4, p.Energy);                        // 3 + 1 next-turn energy
    }

    [Fact]
    public void Invoke_Summons_And_Energizes_Next_Turn()
    {
        var (c, p, m) = Fight();
        Play(c, new Invoke(), null);
        Assert.True(p.IsOstyMissing);                     // nothing yet this turn
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.True(p.IsOstyAlive);                       // summoned 2 at turn start
        Assert.Equal(2, p.Osty!.MaxHp);
        Assert.Equal(5, p.Energy);                        // 3 + 2
    }

    // ─────────────── relic ───────────────

    [Fact]
    public void BoundPhylactery_Summons_One_At_Combat_Start_And_Grows_Each_Turn()
    {
        var (c, p, m) = Fight(relics: new[] { "BoundPhylactery" });
        Assert.True(p.IsOstyAlive);
        Assert.Equal(1, p.Osty!.MaxHp);
        CombatManager.BeginPlayerTurn(c);                 // turn 1: no extra summon
        Assert.Equal(1, p.Osty.MaxHp);
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);                 // turn 2: +1
        Assert.Equal(2, p.Osty.MaxHp);
    }

    // ─────────────── inert cards don't throw ───────────────

    [Fact]
    public void Inert_Cards_Resolve_Without_Error()
    {
        var (c, p, m) = Fight();
        Play(c, new Parse(), null);
        Play(c, new Seance(), null);
        Play(c, new Dredge(), null);
        Play(c, new Transfigure(), null);
        Play(c, new Pagestorm(), null);
        Play(c, new SentryMode(), null);
        Play(c, new CallOfTheVoid(), null);
        Play(c, new ForbiddenGrimoire(), null, energy: 2);
        Play(c, new CaptureSpirit(), m);                  // deals 3 unblockable
        Assert.Equal(60 - 3, m.CurrentHp);
    }
}
