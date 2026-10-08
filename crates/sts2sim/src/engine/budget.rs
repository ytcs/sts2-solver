//! Runaway-work safeguard (`ov::LOOP`).
//!
//! Effects run as plain recursion inside one `step` (hooks firing hooks, auto-plays, damage passes), so a trigger chain that never
//! ends would hang the whole batch inside Rust (or overflow the stack), where Python cannot interrupt it. Three cheap counters bound
//! one step (and one look-ahead turn):
//! * `work`: hook passes that have listeners, card plays (every replay), attack hits, monster state transitions ([`WORK_LIMIT`]);
//! * `hook_depth`: hook passes nested inside each other, i.e. the recursion depth of a chain ([`HOOK_DEPTH_LIMIT`]);
//! * `step_turns`: player turns started by one step (an end of turn requested at every turn start recurses turn after turn,
//!   [`TURN_LIMIT`]).
//!
//! Past a limit, [`Combat::trip_loop`] sets `ov::LOOP` and puts the combat in the "combat ended" state the engine already unwinds
//! from (`in_progress = false`: guarded passes, plays, attacks and draws stop), and every further unit fails (`work_limit = 0`), so
//! the unguarded passes stop too. The rest of the call stack returns quickly; `step` then leaves the combat in `Stage::Over` with the
//! flag set. Such a combat is only safe to drop, not to continue. `BatchEnv` and the search score it as a LOSS (`sts2env::looped`), not as
//! a neutral overflow: a chain that never ends is a fight the real game never finishes either (a soft-lock, e.g. Pillage + Hellraiser
//! + Velvet Choker, `docs/research/evidence.md` E6), so a line into it must never look better than a loss. The limits therefore stay
//! far above every finite chain (corpus max 174 work units): a false trip would be scored as a loss.
//!
//! Replayed steps (`engine/replay.rs`) get a fresh budget on every run of the action (each rerun restores the snapshot taken after
//! `step` reset the counters); a loop in the thrown-away tail after a captured prompt is dropped with that tail, and trips again in
//! the rerun if the real line reaches it.

use crate::state::*;

impl Combat {
    /// Starts a fresh budget (`step`, a look-ahead turn). `work_limit` is left alone: 0 after a trip, so a tripped combat stays dead.
    #[inline(always)]
    pub(crate) fn budget_reset(&mut self) {
        self.work = 0;
        self.hook_depth = 0;
        self.step_turns = 0;
    }

    /// One unit of work (a card play, an attack hit, a monster transition). False = the safeguard tripped: stop what you are doing.
    #[inline(always)]
    pub(crate) fn tick(&mut self) -> bool {
        self.work += 1;
        if self.work > self.work_limit {
            self.trip_loop();
            return false;
        }
        true
    }

    /// Entering a hook pass that has listeners: one unit of work and one nesting level. False = the safeguard tripped: skip the pass
    /// (and do not call [`Combat::pass_exit`]).
    #[inline(always)]
    pub(crate) fn pass_enter(&mut self) -> bool {
        let (w, d) = (self.work + 1, self.hook_depth);
        self.work = w;
        // (`|`, not `||`: one well-predicted branch)
        if (w > self.work_limit) | (d >= HOOK_DEPTH_LIMIT) {
            self.trip_loop();
            return false;
        }
        self.hook_depth = d + 1;
        true
    }

    /// Leaving a pass entered by [`Combat::pass_enter`]. (Balanced: the counters are only reset where no pass of this combat is
    /// running, at the start of `step` / of a look-ahead turn on a fresh copy.)
    #[inline(always)]
    pub(crate) fn pass_exit(&mut self) {
        self.hook_depth = self.hook_depth.wrapping_sub(1);
    }

    /// A player turn starts. False = the safeguard tripped.
    #[inline]
    pub(crate) fn turn_enter(&mut self) -> bool {
        self.step_turns += 1;
        if self.step_turns > TURN_LIMIT {
            self.trip_loop();
            return false;
        }
        true
    }

    /// The safeguard tripped: flag the combat and wind it down (see the module doc). Idempotent.
    #[cold]
    #[inline(never)]
    pub(crate) fn trip_loop(&mut self) {
        self.overflow |= ov::LOOP;
        self.work_limit = 0;
        self.in_progress = false;
        self.stage = Stage::Over;
    }
}
