# Unresolved traces (excluded from the validated suite)

Traces here are **not** scanned by `TraceValidationTests` — they record real-game behavior
our engine does not (yet) reproduce. They are kept as evidence for future work, with the
discrepancy and its soundness direction characterized below.

---

## `decimillipede-longfight-reattach-90of124.jsonl`

Ironclad vs Decimillipede elite (A0), attack-heavy deck, played to a (lost) finish over 6
turns. Captured to oracle-confirm the **Reattach heal-to-25** — which it does: every
`REATTACH_MOVE` restores the segment to exactly 25 and the `ReattachPower` persists, and our
engine matches those checks ([ok] on `HP = 25`, `Reattach = 25`, and the post-heal `HP = 21`).

### Discovered, separate gap: the middle segment auto-buries on turn 1

`--validate` reports 90/124. Every failure traces to one previously-unknown mechanic:

> **The MIDDLE segment is downed (→ `DEAD_MOVE`) by the end of enemy turn 1 with no player
> damage dealt to it, then `REATTACH_MOVE`-heals to 25 and rejoins — and this bury/reattach
> cycle repeats.** The autopilot reports `enemiesAlive=2` from turn 1 onward.

This is **not** a recorder artifact and **not** player damage:

- In a control run with a *defensive* deck (3 Defends, **zero** damage dealt), MIDDLE still
  went `44 → 0/DEAD` by the end of enemy turn 1. A segment taking no damage cannot be killed
  by the player — so the down is scripted/deterministic.
- The recorder is accurate: in trace `combat-20260603-134817-569.jsonl` (#78, passes 82/82)
  the player cleanly focus-fired one segment with exact per-card damage and **no** segment was
  ever downed. The down only appears when a segment buries — i.e. the gap is specific to the
  bury/reattach cycle, not general Decimillipede recording.
- Observed across 3/3 runs where it triggered; segment [1] (MIDDLE) is the one that buries.
  Trace #78 (Front starter index = 2) showed no bury in 4 turns; the runs that buried had
  Front starter index ∈ {0,1}. Exact trigger/cadence is **not** determinable from the
  decompile — `DecimillipedeSegment.cs` / `DecimillipedeElite.cs` / `ReattachPower.cs` contain
  no combat-start or self-bury hook, and the rest of the assembly is name-obfuscated
  (every collided symbol decompiles to `ln`), so the driving code can't be located.

### Why we deliberately do NOT model it (soundness)

The game's auto-bury **helps the player**: a buried segment stops attacking, then reattaches
to only 25 HP. Our engine instead keeps that segment alive at full HP, attacking every turn and
ramping Strength via Bulk. So our model is **strictly pessimistic** in every check —
engine player HP ≤ game player HP throughout (e.g. 61 vs 67, 52 vs 58), and the engine faces
more enemy HP and more incoming damage. Under-crediting the player is the SOUND direction.

Modeling the auto-bury would move the solver **toward optimism** (crediting the player with
avoided damage), and without the exact in-game trigger/cadence any port would be speculative —
risking an over-bury that credits damage the real game still deals. Per project doctrine
(optimistic = dangerous; model RANDOM/unverified-beneficial mechanics conservatively, never
optimistically), the correct action is to keep the pessimistic model and leave this documented.

If a deeper/source-level view of the Decimillipede ever becomes available, resolve the exact
bury rule, model it, and promote a fresh full-fight trace back into `data/combat_traces/`.
