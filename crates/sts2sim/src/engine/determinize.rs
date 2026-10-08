// Resamples exactly the hidden state (pile order, RNG streams); everything visible, hence the observation and legal actions, stays unchanged.
use crate::rng::Rng;
use crate::state::*;

impl Combat {
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

    pub fn determinize_strat(&mut self, shuffle_seed: u64, rng_seed: u64, i: usize, k: usize) -> bool {
        if self.replay.is_some() {
            return false;
        }
        let mut r = Rng::new(shuffle_seed ^ 0x5DEECE66D);
        r.shuffle(self.player.draw.as_mut_slice());
        r.shuffle(self.player.discard.as_mut_slice());
        r.shuffle(self.player.exhaust.as_mut_slice());
        let d = self.player.draw.as_mut_slice();
        if !d.is_empty() && k > 0 {
            let n = d.len();
            d.rotate_left(i * n / k);
        }
        self.rng = RngSet::from_run_seed(rng_seed);
        true
    }
}
