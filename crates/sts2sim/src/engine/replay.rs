//! Replay-based continuation for decisions raised where the engine cannot suspend.
//!
//! Hooks and effects run as plain recursion, and a reshuffle (hence Stratagem's "choose cards from the draw pile" prompt) can
//! happen at any call depth: in the middle of Battle Trance, inside an auto-played card, under another hook. Instead of a
//! bespoke continuation per context, a step that could hit such a prompt is run under replay:
//!
//! 1. `step` snapshots the state (`S0`) and runs the action. The first unanswered prompt clones the state right there (`S1`: the
//!    cards drawn so far, energy spent, partial effects: exactly what a human sees) and lets the rest of the call stack unwind with
//!    an empty answer; that tail is thrown away.
//! 2. The agent now sees `S1` and answers by `Pick` / `Confirm` as on any selection screen.
//! 3. When the selection is complete the answer is recorded, `S0` is restored and the same action runs again. The engine is
//!    deterministic, so it reaches the same prompt, which is now answered from the record. A further prompt in the same step
//!    captures again.
//!
//! Only steps of combats that contain a Stratagem card run under replay (`Combat::strat_possible`); everything else pays nothing.

use crate::state::*;
use crate::util::ArrayVec;

use super::Action;

/// What a prompt that cannot suspend got from the replay machinery.
pub enum ReplayAnswer {
    /// Answered from the record of the agent's earlier selection.
    Cards(ArrayVec<CardIdx, 16>),
    /// The agent will see this state (the rest of the running step is discarded): do nothing.
    Captured,
    /// Not running under replay: the caller flags the content as not ported.
    Unavailable,
}

impl Combat {
    /// `step` of a combat that can have replayed prompts.
    pub(crate) fn step_replayed(&mut self, a: Action) -> bool {
        if self.replay.as_ref().is_some_and(|r| r.at_prompt) {
            if !matches!(a, Action::Pick { .. } | Action::Confirm) {
                return false;
            }
            let ok = self.step_inner(a);
            if ok && self.replay.as_ref().is_some_and(|r| r.done) {
                return self.replay_rerun();
            }
            return ok;
        }
        self.run_replayed(a, ArrayVec::new())
    }

    /// Runs `a` from the current state; `answers` are the agent's selections for the prompts of this step so far.
    fn run_replayed(&mut self, a: Action, answers: ArrayVec<ArrayVec<u8, 16>, 6>) -> bool {
        debug_assert!(self.replay.is_none());
        self.replay = Some(Box::new(Replay { s0: self.clone(), action: a, answers, pos: 0, at_prompt: false, done: false, capture: None }));
        let ok = self.step_inner(a);
        let mut rp = self.replay.take().unwrap();
        match rp.capture.take() {
            Some(mut s1) => {
                // the unwound tail ran on a state that is now discarded: drop what it raised (S1 already folded its own bits)
                crate::util::take_overflow();
                rp.at_prompt = true;
                rp.pos = 0;
                s1.replay = Some(rp);
                *self = *s1;
                true
            }
            None => ok,
        }
    }

    /// The prompt on screen was answered: restore the state the step began in and run it again with the answer recorded.
    fn replay_rerun(&mut self) -> bool {
        let mut rp = self.replay.take().unwrap();
        let sel = self.decision.as_ref().map(|d| d.selected).unwrap_or_default();
        rp.answers.push(sel);
        let Replay { s0, action, answers, .. } = *rp;
        *self = s0;
        self.run_replayed(action, answers)
    }

    /// Called by a listener whose `Ask::Pending` decision sits in a context that cannot suspend.
    pub fn replay_prompt(&mut self) -> ReplayAnswer {
        let Some(rp) = self.replay.as_mut() else { return ReplayAnswer::Unavailable };
        if rp.at_prompt {
            return ReplayAnswer::Unavailable;
        }
        let Some(d) = self.decision.as_ref() else { return ReplayAnswer::Unavailable };
        if (rp.pos as usize) < rp.answers.len() {
            let ans = rp.answers[rp.pos as usize];
            rp.pos += 1;
            let mut cards = ArrayVec::new();
            for &i in ans.iter() {
                let Some(&c) = d.cands.as_slice().get(i as usize) else { return ReplayAnswer::Unavailable };
                cards.push(c);
            }
            self.decision = None;
            return ReplayAnswer::Cards(cards);
        }
        if rp.capture.is_none() {
            self.sync_overflow();
            let held = self.replay.take();
            let mut s1 = self.clone();
            self.replay = held;
            s1.stage = Stage::AwaitChoice;
            self.replay.as_mut().unwrap().capture = Some(Box::new(s1));
        }
        self.decision = None;
        ReplayAnswer::Captured
    }
}
