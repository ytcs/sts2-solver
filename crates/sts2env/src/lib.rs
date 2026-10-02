//! `BatchEnv`: thousands of independent fights stepped in parallel, writing observations / action masks / rewards
//! into flat, caller-owned buffers (zero-copy friendly for NumPy/PyTorch).
//!
//! Semantics follow the usual vector-env contract: `step` applies one action per env; envs whose episode ended are
//! automatically reset and `done[i] = 1` flags that the *returned* `reward[i]`/`outcome[i]` belong to the finished
//! episode while `obs`/`mask` already describe the first state of the new one.

use rayon::prelude::*;
use sts2sim::engine::ACTION_SPACE;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::{RngSet, Stage};
use sts2sim::types::Outcome;
use sts2sim::{Action, Combat, Scenario};

pub use sts2sim::engine::ACTION_SPACE as ACTIONS;
pub use sts2sim::observe::OBS_SIZE as OBS;

/// Produces the scenario of an episode (encounter, deck, relics, ...). `episode_seed` is unique per episode and
/// should drive every random choice (including the run-level RNG streams) so episodes are reproducible.
pub trait ScenarioSource: Send + Sync {
    fn sample(&self, env: usize, episode_seed: u64) -> Scenario;
}

/// Same scenario every episode; only the RNG streams change.
pub struct FixedScenario(pub Scenario);
impl ScenarioSource for FixedScenario {
    fn sample(&self, _env: usize, episode_seed: u64) -> Scenario {
        let mut s = self.0.clone();
        s.run_seed = episode_seed;
        s.rng = RngSet::from_run_seed(episode_seed);
        s
    }
}

/// Uniform choice among several scenarios per episode (e.g. different encounters / decks).
pub struct PoolScenario(Vec<Scenario>);
impl PoolScenario {
    pub fn new(v: Vec<Scenario>) -> PoolScenario {
        assert!(!v.is_empty());
        PoolScenario(v)
    }
}
impl ScenarioSource for PoolScenario {
    fn sample(&self, _env: usize, episode_seed: u64) -> Scenario {
        let mut s = self.0[(episode_seed >> 17) as usize % self.0.len()].clone();
        s.run_seed = episode_seed;
        s.rng = RngSet::from_run_seed(episode_seed);
        s
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RewardConfig {
    pub win: f32,
    pub loss: f32,
    /// Extra reward on victory: `hp_bonus * final_hp / max_hp`.
    pub hp_bonus: f32,
    /// Per-step reward (usually 0 or slightly negative).
    pub step: f32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.0, step: 0.0 }
    }
}

/// Episode outcome codes written to `outcome` when `done`.
pub const OUTCOME_ONGOING: i8 = 0;
pub const OUTCOME_WIN: i8 = 1;
pub const OUTCOME_LOSS: i8 = -1;
/// Episode aborted because it hit `max_steps`.
pub const OUTCOME_TRUNCATED: i8 = 2;
/// Episode aborted because it touched content that is not ported (the fight would not be faithful).
pub const OUTCOME_UNIMPLEMENTED: i8 = 3;

struct Slot {
    cx: Combat,
    steps: u32,
    episode: u64,
    rng: Rng,
}

pub struct BatchEnv {
    slots: Vec<Slot>,
    source: Box<dyn ScenarioSource>,
    reward_cfg: RewardConfig,
    max_steps: u32,
    base_seed: u64,
}

/// Per-step outputs (all slices have one entry per env; `obs` and `mask` are row-major `[n, OBS_SIZE]`/`[n, ACTION_SPACE]`).
pub struct StepOut<'a> {
    pub obs: &'a mut [f32],
    pub mask: &'a mut [u8],
    pub reward: &'a mut [f32],
    pub done: &'a mut [u8],
    pub outcome: &'a mut [i8],
    /// Number of illegal actions received (state unchanged for those envs).
    pub illegal: &'a mut [u8],
}

impl BatchEnv {
    pub fn new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> BatchEnv {
        let slots: Vec<Slot> = (0..n)
            .into_par_iter()
            .map(|i| {
                let episode = Self::episode_seed(base_seed, i, 0);
                Slot { cx: Combat::new(&source.sample(i, episode)), steps: 0, episode: 0, rng: Rng::new(base_seed ^ (i as u64).wrapping_mul(0x9E3779B97F4A7C15)) }
            })
            .collect();
        BatchEnv { slots, source, reward_cfg, max_steps, base_seed }
    }

    #[inline]
    fn episode_seed(base: u64, env: usize, episode: u64) -> u64 {
        // splitmix-style mixing so neighbouring envs/episodes get unrelated run seeds
        let mut z = base ^ (env as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ episode.wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Writes the current observation / mask of every env (e.g. after construction).
    pub fn observe_all(&self, obs: &mut [f32], mask: &mut [u8]) {
        self.slots.par_iter().zip(obs.par_chunks_mut(OBS_SIZE)).zip(mask.par_chunks_mut(ACTION_SPACE)).for_each(|((s, o), m)| {
            s.cx.observe(o);
            write_mask(&s.cx, m);
        });
    }

    /// Applies one action per env (`actions[i]` is a dense action index), auto-resetting finished episodes.
    pub fn step(&mut self, actions: &[i32], out: StepOut) {
        let cfg = self.reward_cfg;
        let max_steps = self.max_steps;
        let base = self.base_seed;
        let source = &*self.source;
        let n = self.slots.len();
        let envs: Vec<usize> = (0..n).collect();
        self.slots
            .par_iter_mut()
            .zip(actions.par_iter())
            .zip(out.obs.par_chunks_mut(OBS_SIZE))
            .zip(out.mask.par_chunks_mut(ACTION_SPACE))
            .zip(out.reward.par_iter_mut())
            .zip(out.done.par_iter_mut())
            .zip(out.outcome.par_iter_mut())
            .zip(out.illegal.par_iter_mut())
            .zip(envs.par_iter())
            .for_each(|((((((((slot, &a), obs), mask), reward), done), outcome), illegal), &env)| {
                *reward = cfg.step;
                *done = 0;
                *outcome = OUTCOME_ONGOING;
                *illegal = 0;
                let ok = match Action::from_index(a as usize) {
                    Some(act) => slot.cx.step(act),
                    None => false,
                };
                if !ok {
                    *illegal = 1;
                } else {
                    slot.steps += 1;
                }
                let mut end = None;
                if slot.cx.missing.is_some() {
                    end = Some((OUTCOME_UNIMPLEMENTED, 0.0));
                } else if slot.cx.stage == Stage::Over {
                    let c = &slot.cx;
                    let me = c.cr(0);
                    match c.outcome {
                        Outcome::Victory => end = Some((OUTCOME_WIN, cfg.win + cfg.hp_bonus * me.hp as f32 / me.max_hp.max(1) as f32)),
                        _ => end = Some((OUTCOME_LOSS, cfg.loss)),
                    }
                } else if slot.steps >= max_steps {
                    end = Some((OUTCOME_TRUNCATED, 0.0));
                }
                if let Some((oc, r)) = end {
                    *done = 1;
                    *outcome = oc;
                    *reward += r;
                    slot.episode += 1;
                    slot.steps = 0;
                    let seed = BatchEnv::episode_seed(base, env, slot.episode);
                    slot.cx = Combat::new(&source.sample(env, seed));
                }
                slot.cx.observe(obs);
                write_mask(&slot.cx, mask);
            });
    }
}

fn write_mask(cx: &Combat, mask: &mut [u8]) {
    for m in mask[..ACTION_SPACE].iter_mut() {
        *m = 0;
    }
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    for a in buf.iter() {
        mask[a.index()] = 1;
    }
}
