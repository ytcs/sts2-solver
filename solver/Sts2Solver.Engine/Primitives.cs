namespace Sts2Solver.Engine;

public enum CombatSide { Player, Enemy }

public enum CardType { Attack, Skill, Power, Status, Curse }

public enum CardRarity { Basic, Common, Uncommon, Rare, Special, Curse, Status, Event, Ancient }

/// <summary>Who a card can target. v1 covers the subset the slice needs.</summary>
public enum TargetType { Self, AnyEnemy, AllEnemies, RandomEnemy, None }

public enum PowerType { Buff, Debuff }

/// <summary>
/// ValueProp flags mirror MegaCrit.Sts2.Core.ValueProps.ValueProp. They ride along with
/// every damage/block event and gate which power modifiers apply.
/// </summary>
[Flags]
public enum ValueProp
{
    None = 0,
    Unblockable = 1 << 0,
    Unpowered = 1 << 1,
    Move = 1 << 2,
    SkipHurtAnim = 1 << 3,
}

public static class ValuePropExtensions
{
    // Game semantics: a "powered" attack is one NOT flagged Unpowered. Strength/Weak/Vulnerable
    // only apply to powered attacks. Card/monster-move block likewise respects Unpowered.
    public static bool IsPoweredAttack(this ValueProp props) => !props.HasFlag(ValueProp.Unpowered);
    public static bool IsPoweredBlock(this ValueProp props) => !props.HasFlag(ValueProp.Unpowered);
}
