using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>One row of the elites view: either a computed <see cref="Stats"/> or a <see cref="Skipped"/>
/// reason (e.g. no playable deck cards). Rendered by <see cref="Dashboard"/>.</summary>
public sealed record EliteResult(string Elite, string Composition, CombatStats? Stats, string? Skipped);
