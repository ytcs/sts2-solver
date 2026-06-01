using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// The Regent's signature mechanic: Forge → Sovereign Blade.
//
// "Forge N" builds the Sovereign Blade — a generated, retained Token attack
// that accumulates forged damage over the combat. If no (non-exhausted) blade
// exists, Forge creates one in hand; then every blade's damage grows by N.
// The blade deals its accumulated damage when played, gains Block per Parry,
// hits all enemies under Seeking Edge, replays under Sword Sage, and deals ×2
// to a Conqueror-marked enemy.
// ===========================================================================

/// <summary>Builds/grows the Sovereign Blade. Mirrors MegaCrit ForgeCmd.Forge: if the player has no
/// Sovereign Blade outside the exhaust pile, create one in hand; then add <paramref name="amount"/> to the
/// damage of every Sovereign Blade the player owns (in any pile, exhausted included).</summary>
public static class RegentForge
{
    /// <summary>Every Sovereign Blade across the player's piles (hand/draw/discard/exhaust).</summary>
    private static IEnumerable<SovereignBlade> AllBlades(Player p) => p.Hand
        .Concat(p.DrawPile).Concat(p.DiscardPile).Concat(p.ExhaustPile).OfType<SovereignBlade>();

    public static void Forge(CombatState combat, int amount)
    {
        var p = combat.Player;
        // Create a blade in hand if none exists outside the exhaust pile.
        bool hasUsable = p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).OfType<SovereignBlade>().Any();
        if (!hasUsable) p.Hand.Add(new SovereignBlade());
        foreach (var blade in AllBlades(p).ToList()) blade.AddDamage(amount);
    }
}

/// <summary>The Sovereign Blade token: 2-energy Attack (1 upgraded), base 10 damage, Retain. Carries its
/// forged bonus damage as mutable per-combat state (Stateful). On play: deal (10 + forged) damage —
/// repeated 1 + Sword Sage times — to the target (or ALL enemies under Seeking Edge), as a powered attack
/// (so Strength / Vigor / Conqueror's ×2 apply); then, if the owner has Parry, gain that much Block.
/// (MegaCrit SovereignBlade.)</summary>
public sealed class SovereignBlade : CardModel
{
    public override string Name => "SovereignBlade";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);   // upgrade: cost -1
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Token;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Retain => true;
    public override bool Stateful => true;

    private int _bonusDamage;                       // forged damage accumulated this combat
    public int Damage => 10 + _bonusDamage;
    public void AddDamage(int amount) => _bonusDamage += amount;

    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int repeats = 1 + combat.Player.GetPowerAmount("SwordSage");
        if (combat.Player.HasPower("SeekingEdge"))
        {
            foreach (var m in combat.LivingMonsters.ToList())
                Cmd.AttackMulti(combat, combat.Player, m, Damage, repeats, ValueProp.Move, this);
        }
        else if (play.Target != null)
        {
            Cmd.AttackMulti(combat, combat.Player, play.Target, Damage, repeats, ValueProp.Move, this);
        }
        int parry = combat.Player.GetPowerAmount("Parry");
        if (parry > 0) Cmd.GainBlock(combat, combat.Player, parry, ValueProp.Move, this);
    }

    public override CardModel Clone()
    {
        var c = (SovereignBlade)base.Clone();
        c._bonusDamage = _bonusDamage;
        return c;
    }

    public override string StateKey() => _bonusDamage > 0 ? $"SovereignBlade#{_bonusDamage}" : "SovereignBlade";
}
