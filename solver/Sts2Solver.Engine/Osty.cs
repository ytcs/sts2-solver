namespace Sts2Solver.Engine;

/// <summary>
/// The Necrobinder's pet, Osty (MegaCrit.Sts2.Core.Models.Monsters.Osty). A player-side creature with
/// its own HP/MaxHp/block/powers. It never acts on its own turn (its move is a no-op), so it is NOT in
/// the monster list; instead it hangs off <see cref="Player.Osty"/> and is surfaced through
/// <see cref="CombatState.AllCreatures"/> so its powers (DieForYou) join the damage pipeline.
///
/// Faithful behaviour, all driven from Content (Necrobinder/):
///  • Summon grows MaxHp+heals an alive Osty, or (re)creates a missing/dead one at full HP.
///  • DieForYouPower redirects powered enemy attacks aimed at the player onto Osty.
///  • NecroMasteryPower reflects Osty's HP loss to all enemies.
///  • OstyAttack cards deal damage with Osty as the dealer (so Calcify/Osty-Strength apply, the
///    player's Strength does not), and fizzle while Osty is missing.
/// </summary>
public sealed class Osty : Creature
{
    public Osty() { Side = CombatSide.Player; Name = "Osty"; }

    /// <summary>"Missing" = no living Osty (never summoned, or dead). The game keeps a dead Osty around
    /// for revive bookkeeping; HP-faithfully that is indistinguishable from absent, so we collapse it to
    /// CurrentHp ≤ 0.</summary>
    public bool IsMissing => CurrentHp <= 0;

    public override Creature Clone()
    {
        var o = new Osty();
        CopyCreatureBaseTo(o);
        return o;
    }

    public override void Hash(ref StateHasher h)
    {
        base.Hash(ref h);
        h.Add(MaxHp);          // Osty's MaxHp feeds card damage (Protector/Sacrifice), so it is state
    }

    public override string StateKey() => $"O({base.StateKey()}|m{MaxHp})";
}
