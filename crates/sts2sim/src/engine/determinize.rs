//! Determinization: resampling the part of the state a player cannot see, for search over the player's belief.
//!
//! The observation hides the order of the draw / discard / exhaust piles and every RNG stream (`observe::hidden_state_does_not_leak`).
//! A search that copies a state (`Clone`) and plays forward would otherwise "know" the true shuffle and the true future random
//! outcomes. `determinize` replaces exactly that hidden information by a fresh sample: the three piles are permuted uniformly
//! and all nine RNG streams are re-seeded. Everything visible (and therefore the observation and the legal actions) is unchanged.

use crate::rng::Rng;
use crate::state::*;

impl Combat {
    /// Resamples the hidden state from `seed`. Returns false (and does nothing) while a replayed Stratagem prompt is on screen: that
    /// state is a view of a step in progress, determinize the state before the action instead.
    ///
    /// Not tracked: a human remembers which card is on top after a put-on-top effect; the shuffle forgets it.
    pub fn determinize(&mut self, seed: u64) -> bool {
        if self.replay.is_some() {
            return false;
        }
        let mut r = Rng::new(seed ^ 0x5DEECE66D);
        r.shuffle(self.player.draw.as_mut_slice());
        r.shuffle(self.player.discard.as_mut_slice());
        r.shuffle(self.player.exhaust.as_mut_slice());
        self.rng = RngSet::from_run_seed(seed);
        true
    }

    /// Stratified determinization of a future that shares `shuffle_seed` with the others: the piles get the same uniform shuffle in every future and the future
    /// then rotates the draw pile by the fraction `frac` of its length, so the hands the futures draw next are disjoint parts of one shuffle (each future is still
    /// uniform on its own, the set of them covers the pile evenly: less variance in the mean over the futures). The RNG streams are seeded per future.
    pub fn determinize_strat(&mut self, shuffle_seed: u64, rng_seed: u64, frac: f32) -> bool {
        if self.replay.is_some() {
            return false;
        }
        let mut r = Rng::new(shuffle_seed ^ 0x5DEECE66D);
        r.shuffle(self.player.draw.as_mut_slice());
        r.shuffle(self.player.discard.as_mut_slice());
        r.shuffle(self.player.exhaust.as_mut_slice());
        let d = self.player.draw.as_mut_slice();
        if !d.is_empty() {
            let n = d.len();
            d.rotate_left(((frac * n as f32) as usize).min(n - 1));
        }
        self.rng = RngSet::from_run_seed(rng_seed);
        true
    }
}
