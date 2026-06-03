using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// The Defect's powers. Focus is the signature orb modifier; other orb-reactive
// powers (Electrodynamics, Loop, Storm, Echo Form, …) are added as their cards
// are ported.
// ===========================================================================

/// <summary>Focus: adds its amount to every focus-affected orb's passive/evoke value (Lightning / Frost /
/// Dark / Glass — NOT Plasma), clamped ≥0 per value. A signed counter (BiasedCognition can drive it negative,
/// hence <see cref="AllowNegative"/>). Read directly by <see cref="OrbModel"/>.Focused. (Game FocusPower.)</summary>
public sealed class FocusPower : PowerModel
{
    public override string Id => "Focus";
    public override PowerType Type => PowerType.Buff;
    public override bool AllowNegative => true;
}
