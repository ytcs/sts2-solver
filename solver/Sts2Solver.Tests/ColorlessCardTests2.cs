using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the second-wave Colorless cards (the 39 remaining pool entries + the
/// Ancient Wish) against the decompiled OnPlay logic. Same conventions as ColorlessCardTests: Cmd.Draw is a
/// no-op without an ambient Rng, so draw counts are asserted only where a concrete Rng is set.</summary>
public class ColorlessCardTests2
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80,
                                                                   IEnumerable<CardModel>? deck = null)
    {
        var player = Catalog.BuildPlayer(deck?.ToList() ?? new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static (CombatState combat, Player p, Monster m1, Monster m2) Fight2(int hp = 60)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: hp);
        var m2 = Monsters.CalcifiedCultist(hp: hp);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        player.ResetEnergy();
        return (combat, player, m1, m2);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target, int energy = 0)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, Math.Max(card.Cost, energy));
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    // ---------------- Catalog wiring / completeness ----------------

    [Fact]
    public void Catalog_Builds_All_SecondWave_Cards()
    {
        foreach (var name in new[]
        {
            "Bolas","Fisticuffs","GoldAxe","Jackpot","Omnislice","Rend","Salvo","SeekerStrike",
            "ThrummingHatchet","UltimateStrike","Volley","Alchemize","Anointed","BeatDown","Catastrophe",
            "Discovery","Equilibrium","HiddenGem","JackOfAllTrades","Production","Prolong","Restlessness",
            "Scrawl","SecretTechnique","SecretWeapon","Shockwave","Splash","TheGambit","UltimateDefend",
            "Wish","Automation","Calamity","Entropy","EternalArmor","Fasten","Nostalgia","PrepTime",
            "Prowess","RollingBoulder","Stratagem",
        })
            Assert.NotNull(Catalog.BuildCard(name));
    }

    // ---------------- Attacks ----------------

    [Theory]
    [InlineData(0, 3)]
    [InlineData(1, 4)]
    public void Bolas_Deals_3_Plus_Upgrade(int upg, int dmg)
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new Bolas().Upgraded(upg), m);
        Assert.Equal(60 - dmg, m.CurrentHp);
    }

    [Fact]
    public void Fisticuffs_Deals_7_And_Gains_7_Block()
    {
        var (c, p, m) = Fight();
        Play(c, new Fisticuffs(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
        Assert.Equal(7, p.Block);
    }

    [Fact]
    public void Fisticuffs_Upgraded_Deals_9_And_Gains_9_Block()
    {
        var (c, p, m) = Fight();
        Play(c, (CardModel)new Fisticuffs().Upgraded(1), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.Equal(9, p.Block);
    }

    [Fact]
    public void GoldAxe_Scales_With_Cards_Played_This_Combat()
    {
        // The deck must contain a GoldAxe so the gated CardsPlayedThisCombat counter is tracked.
        var (c, p, m) = Fight(deck: new CardModel[] { new GoldAxe() });
        Play(c, new Finesse(), null);   // play #1
        Play(c, new Finesse(), null);   // play #2
        Play(c, new GoldAxe(), m);      // 2 prior finished plays → 2 damage (this card not counted yet)
        Assert.Equal(60 - 2, m.CurrentHp);
    }

    [Fact]
    public void GoldAxe_Gains_Retain_On_Upgrade()
    {
        Assert.False(new GoldAxe().Retain);
        Assert.True(((CardModel)new GoldAxe().Upgraded(1)).Retain);
    }

    [Fact]
    public void Jackpot_Deals_25_At_Cost_3()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        var j = new Jackpot();
        Assert.Equal(3, j.Cost);
        Play(c, j, m);
        Assert.Equal(80 - 25, m.CurrentHp);
    }

    [Fact]
    public void Omnislice_Splashes_To_Other_Enemies()
    {
        var (c, p, m1, m2) = Fight2(60);
        Play(c, new Omnislice(), m1);
        Assert.Equal(60 - 8, m1.CurrentHp);   // primary
        Assert.Equal(60 - 8, m2.CurrentHp);   // splash = HP removed from primary
    }

    [Fact]
    public void Rend_Scales_With_Permanent_Debuffs()
    {
        var (c, _, m) = Fight(monsterHp: 99);
        m.AddPower(new WeakPower(), 2);
        m.AddPower(new VulnerablePower(), 1);   // 2 distinct permanent debuffs
        // base 10 + 5*2 = 20, then Vulnerable ×1.5 applies to the whole attack → 30 (floor).
        var before = m.CurrentHp;
        Play(c, new Rend(), m);
        Assert.Equal(before - (int)System.Math.Floor((10 + 5 * 2) * 1.5), m.CurrentHp);
    }

    [Fact]
    public void Rend_With_No_Debuffs_Deals_Base_10()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        Play(c, new Rend(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
    }

    [Fact]
    public void Salvo_Deals_12_And_Applies_RetainHand()
    {
        var (c, p, m) = Fight();
        Play(c, new Salvo(), m);
        Assert.Equal(60 - 12, m.CurrentHp);
        Assert.True(p.HasPower("RetainHand"));
    }

    [Fact]
    public void UltimateStrike_Deals_14_And_20_Upgraded()
    {
        var (c, _, m) = Fight(monsterHp: 99);
        Play(c, new UltimateStrike(), m);
        Assert.Equal(99 - 14, m.CurrentHp);
        var (c2, _, m2) = Fight(monsterHp: 99);
        Play(c2, (CardModel)new UltimateStrike().Upgraded(1), m2);
        Assert.Equal(99 - 20, m2.CurrentHp);
        Assert.True(new UltimateStrike().IsStrike);
    }

    [Fact]
    public void Volley_Hits_X_Times_For_10()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        var v = new Volley();
        Assert.True(v.IsXCost);
        Play(c, v, m, energy: 3);   // X = 3 → 3 × 10 = 30
        Assert.Equal(99 - 30, m.CurrentHp);
    }

    [Fact]
    public void SeekerStrike_Deals_9_And_Pulls_A_Card_To_Hand()
    {
        var (c, p, m) = Fight();
        var pull = new Finesse();
        p.DrawPile.Add(pull);
        Play(c, new SeekerStrike(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.Contains(pull, p.Hand);
        Assert.DoesNotContain(pull, p.DrawPile);
    }

    // ---------------- Skills ----------------

    [Fact]
    public void Production_Gains_2_Energy()
    {
        var (c, p, _) = Fight();
        p.MaxEnergy = 3; p.ResetEnergy();
        int before = p.Energy;
        Play(c, new Production(), null);
        Assert.Equal(before + 2, p.Energy);   // Production costs 0, then +2
        Assert.Contains(c.Player.ExhaustPile, x => x is Production);
    }

    [Fact]
    public void Equilibrium_Gains_13_Block_And_RetainHand()
    {
        var (c, p, _) = Fight();
        Play(c, new Equilibrium(), null);
        Assert.Equal(13, p.Block);
        Assert.True(p.HasPower("RetainHand"));
    }

    [Fact]
    public void UltimateDefend_Gains_11_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new UltimateDefend(), null);
        Assert.Equal(11, p.Block);
        Assert.True(new UltimateDefend().IsDefend);
    }

    [Fact]
    public void TheGambit_Gains_50_Block_And_Kills_On_Unblocked_Hit()
    {
        var (c, p, m) = Fight();
        Play(c, new TheGambit(), null);
        Assert.Equal(50, p.Block);
        Assert.True(p.HasPower("TheGambit"));
        p.Block = 0;                                   // strip block so the next hit is unblocked
        Cmd.Attack(c, m, p, 5, ValueProp.Move, null);  // unblocked powered attack → death
        Assert.False(p.IsAlive);
        Assert.False(p.HasPower("TheGambit"));
    }

    [Fact]
    public void Shockwave_Applies_Weak_And_Vulnerable_To_All()
    {
        var (c, p, m1, m2) = Fight2(60);
        Play(c, new Shockwave(), null);
        Assert.Equal(3, m1.GetPowerAmount("Weak"));
        Assert.Equal(3, m1.GetPowerAmount("Vulnerable"));
        Assert.Equal(3, m2.GetPowerAmount("Weak"));
        Assert.Equal(3, m2.GetPowerAmount("Vulnerable"));
        Assert.Contains(p.ExhaustPile, x => x is Shockwave);
    }

    [Fact]
    public void Restlessness_Draws_And_Gains_Energy_Only_When_Alone_In_Hand()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 4; i++) p.DrawPile.Add(new Finesse());
        p.MaxEnergy = 3; p.ResetEnergy();
        int before = p.Energy;
        // Restlessness is the only card; when played it is removed first, leaving an empty hand → fires.
        Play(c, new Restlessness(), null);
        Assert.Equal(before + 2, p.Energy);
        Assert.Equal(2, p.Hand.Count);   // drew 2
    }

    [Fact]
    public void Restlessness_Does_Nothing_With_Other_Cards_In_Hand()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 4; i++) p.DrawPile.Add(new Finesse());
        p.Hand.Add(new Finesse());   // a companion card → not alone
        p.MaxEnergy = 3; p.ResetEnergy();
        int before = p.Energy;
        Play(c, new Restlessness(), null);
        Assert.Equal(before, p.Energy);
        Assert.Single(p.Hand);   // only the companion Finesse remains; no draw
    }

    [Fact]
    public void SecretWeapon_Pulls_An_Attack_From_Draw()
    {
        var (c, p, _) = Fight();
        var skill = new Finesse();
        var atk = new Bolas();
        p.DrawPile.Add(skill);
        p.DrawPile.Add(atk);
        Play(c, new SecretWeapon(), null);
        Assert.Contains(atk, p.Hand);      // pulled the Attack
        Assert.DoesNotContain(skill, p.Hand);
    }

    [Fact]
    public void Prolong_Carries_Current_Block_To_Next_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Cmd.GainBlock(c, p, 12, ValueProp.Move, null);
        Play(c, new Prolong(), null);
        Assert.Equal(12, p.GetPowerAmount("BlockNextTurn"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(12, p.Block);   // regained next turn
        Assert.False(p.HasPower("BlockNextTurn"));
    }

    // ---------------- Powers ----------------

    [Fact]
    public void Prowess_Gains_Strength_And_Dexterity()
    {
        var (c, p, _) = Fight();
        Play(c, new Prowess(), null);
        Assert.Equal(1, p.GetPowerAmount("Strength"));
        Assert.Equal(1, p.GetPowerAmount("Dexterity"));
        var (c2, p2, _) = Fight();
        Play(c2, (CardModel)new Prowess().Upgraded(1), null);
        Assert.Equal(2, p2.GetPowerAmount("Strength"));
        Assert.Equal(2, p2.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void Fasten_Adds_Block_To_Defend_Cards_Only()
    {
        var (c, p, _) = Fight();
        Play(c, new Fasten(), null);
        Assert.Equal(4, p.GetPowerAmount("Fasten"));
        Play(c, new UltimateDefend(), null);   // Defend-tagged → 11 + 4 = 15
        Assert.Equal(15, p.Block);
        p.Block = 0;
        Play(c, new Finesse(), null);          // not a Defend → no bonus (4 block only)
        Assert.Equal(4, p.Block);
    }

    [Fact]
    public void PrepTime_Grants_Vigor_At_Turn_Start()
    {
        var (c, p, m) = Fight();
        Play(c, new PrepTime(), null);
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(4, p.GetPowerAmount("Vigor"));
    }

    [Fact]
    public void RollingBoulder_Escalates_Each_Turn()
    {
        var (c, p, m) = Fight(monsterHp: 99);
        Play(c, new RollingBoulder(), null);
        CombatManager.EndPlayerTurn(c); CombatManager.RunEnemyTurn(c); CombatManager.BeginPlayerTurn(c);
        Assert.Equal(99 - 5, m.CurrentHp);   // first tick: 5
        CombatManager.EndPlayerTurn(c); CombatManager.RunEnemyTurn(c); CombatManager.BeginPlayerTurn(c);
        Assert.Equal(99 - 5 - 10, m.CurrentHp);   // second tick: 10
    }

    [Fact]
    public void EternalArmor_Gains_Plating_Block_Each_Turn_End()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new EternalArmor(), null);
        Assert.Equal(9, p.GetPowerAmount("Plating"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(9, p.Block);   // gained at the owner's turn end
    }

    [Fact]
    public void Automation_Gains_Energy_Every_10_Draws()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 12; i++) p.DrawPile.Add(new Finesse());
        Play(c, new Automation(), null);
        Assert.True(p.HasPower("Automation"));
        p.MaxEnergy = 5; p.ResetEnergy();
        int before = p.Energy;
        Cmd.Draw(c, 10);   // 10th draw → +1 energy
        Assert.Equal(before + 1, p.Energy);
    }

    [Fact]
    public void Cost_Reductions_On_Upgrade()
    {
        Assert.Equal(0, new Automation().Upgraded(1).Cost);
        Assert.Equal(2, new Calamity().Upgraded(1).Cost);
        Assert.Equal(0, new Alchemize().Upgraded(1).Cost);
        Assert.Equal(0, new Nostalgia().Upgraded(1).Cost);
        Assert.Equal(0, new Stratagem().Upgraded(1).Cost);
    }

    [Fact]
    public void Exhaust_Removed_On_Upgrade()
    {
        Assert.Equal(CardResultPile.Exhaust, new Discovery().ResultPile);
        Assert.Equal(CardResultPile.Discard, ((CardModel)new Discovery().Upgraded(1)).ResultPile);
        Assert.Equal(CardResultPile.Discard, ((CardModel)new SecretTechnique().Upgraded(1)).ResultPile);
        Assert.Equal(CardResultPile.Discard, ((CardModel)new Prolong().Upgraded(1)).ResultPile);
    }

    [Fact]
    public void Inert_Markers_Apply_Their_Powers()
    {
        var (c, p, _) = Fight();
        Play(c, new Calamity(), null);  Assert.True(p.HasPower("Calamity"));
        Play(c, new Entropy(), null);   Assert.True(p.HasPower("Entropy"));
        Play(c, new Nostalgia(), null); Assert.True(p.HasPower("Nostalgia"));
        Play(c, new Stratagem(), null); Assert.True(p.HasPower("Stratagem"));
    }
}
