use crate::state::*;
use crate::util::ArrayVec;

use super::Action;

pub enum ReplayAnswer {
    Cards(ArrayVec<CardIdx, 16>),
    Captured,
    Unavailable,
}

impl Combat {
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

    fn run_replayed(&mut self, a: Action, answers: ArrayVec<ArrayVec<u8, 16>, 6>) -> bool {
        debug_assert!(self.replay.is_none());
        self.replay = Some(Box::new(Replay { s0: self.clone(), action: a, answers, pos: 0, at_prompt: false, done: false, capture: None }));
        let ok = self.step_inner(a);
        let mut rp = self.replay.take().unwrap();
        match rp.capture.take() {
            Some(mut s1) => {
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

    fn replay_rerun(&mut self) -> bool {
        let mut rp = self.replay.take().unwrap();
        let sel = self.decision.as_ref().map(|d| d.selected).unwrap_or_default();
        rp.answers.push(sel);
        let Replay { s0, action, answers, .. } = *rp;
        *self = s0;
        self.run_replayed(action, answers)
    }

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
