using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Ironclad starter relic: heal 6 HP after each combat victory. (MegaCrit BurningBlood)</summary>
public sealed class BurningBlood : RelicModel
{
    public override string Id => "BurningBlood";
    public override void AfterCombatVictory(CombatState combat) => combat.Player.Heal(6);
}
