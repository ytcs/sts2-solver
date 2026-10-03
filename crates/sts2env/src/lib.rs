//! `BatchEnv`: thousands of independent fights stepped in parallel, writing observations / action masks / rewards
//! into flat, caller-owned buffers (zero-copy friendly for NumPy/PyTorch).
//!
//! Semantics follow the usual vector-env contract: `step` applies one action per env; envs whose episode ended are
//! automatically reset and `done[i] = 1` flags that the *returned* `reward[i]`/`outcome[i]` belong to the finished
//! episode while `obs`/`mask` already describe the first state of the new one.
//!
//! Episodes whose fight can no longer be guaranteed faithful are aborted instead of continued: `OUTCOME_UNIMPLEMENTED`
//! (content that is not ported) and `OUTCOME_OVERFLOW` (a fixed capacity of the simulator was exceeded, see
//! `Combat::overflow`). Both end with reward 0 (+ the step reward); training code should treat them as truncations.

use rayon::prelude::*;
use sts2sim::engine::ACTION_SPACE;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::{RngSet, Stage};
use sts2sim::types::Outcome;
use sts2sim::{Action, Combat, Scenario};

pub use sts2sim::engine::ACTION_SPACE as ACTIONS;
pub use sts2sim::observe::OBS_SIZE as OBS;
pub use sts2sim::scenario::ScenarioError;

/// Produces the scenario of an episode (encounter, deck, relics, ...). `episode_seed` is unique per episode and
/// should drive every random choice (including the run-level RNG streams) so episodes are reproducible.
pub trait ScenarioSource: Send + Sync {
    fn sample(&self, env: usize, episode_seed: u64) -> Scenario;

    /// Allocation-free variant for sources whose episodes differ from a stored scenario only by the seed (`run_seed` and the
    /// RNG streams, which the env derives from `episode_seed` itself): return that stored scenario. `None` (the default)
    /// makes the env call [`ScenarioSource::sample`] each episode.
    fn pick(&self, _env: usize, _episode_seed: u64) -> Option<&Scenario> {
        None
    }

    /// Checks every scenario the source can produce (`BatchEnv::try_new` calls it once, so the per-episode reset can skip
    /// validation). The default accepts everything: sources built on `sample` are validated per episode instead.
    fn validate(&self) -> Result<(), ScenarioError> {
        Ok(())
    }
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
    fn pick(&self, _env: usize, _episode_seed: u64) -> Option<&Scenario> {
        Some(&self.0)
    }
    fn validate(&self) -> Result<(), ScenarioError> {
        self.0.validate()
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
    fn pick(&self, _env: usize, episode_seed: u64) -> Option<&Scenario> {
        Some(&self.0[(episode_seed >> 17) as usize % self.0.len()])
    }
    fn validate(&self) -> Result<(), ScenarioError> {
        self.0.iter().try_for_each(|s| s.validate())
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
/// Episode aborted because a fixed capacity of the simulator was exceeded (card arena, power list, history ring, decision
/// candidates, ... see `Combat::overflow` / `sts2sim::state::ov`): data was dropped, so the fight is no longer faithful.
pub const OUTCOME_OVERFLOW: i8 = 4;

struct Slot {
    cx: Combat,
    steps: u32,
    episode: u64,
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

/// (Re)starts `cx` on the scenario of `episode_seed`. Never panics: a scenario that cannot be started leaves the combat
/// flagged (`overflow`) so the env reports `OUTCOME_OVERFLOW` for it instead of aborting the process.
fn start_episode(source: &dyn ScenarioSource, env: usize, episode_seed: u64, cx: &mut Combat) {
    let ex = sts2sim::ScenarioExtras::default();
    let r = match source.pick(env, episode_seed) {
        Some(sc) => cx.reset_validated(sc, &ex, episode_seed, RngSet::from_run_seed_fast(episode_seed)),
        None => {
            let sc = source.sample(env, episode_seed);
            match sc.validate() {
                Ok(()) => cx.reset_validated(&sc, &ex, sc.run_seed, sc.rng),
                Err(e) => Err(e),
            }
        }
    };
    if r.is_err() {
        cx.overflow |= sts2sim::state::ov::SCENARIO;
    }
}

impl BatchEnv {
    /// Panics if a scenario of the source is invalid (construction time only); see [`BatchEnv::try_new`].
    pub fn new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> BatchEnv {
        Self::try_new(n, source, reward_cfg, max_steps, base_seed).expect("invalid scenario")
    }

    /// Builds `n` envs; fails (instead of panicking later) if the first scenario of any env cannot be started.
    pub fn try_new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> Result<BatchEnv, ScenarioError> {
        source.validate()?;
        let slots: Result<Vec<Slot>, ScenarioError> = (0..n)
            .into_par_iter()
            .map(|i| {
                let episode = Self::episode_seed(base_seed, i, 0);
                let sc = source.sample(i, episode);
                Ok(Slot { cx: Combat::try_new(&sc)?, steps: 0, episode: 0 })
            })
            .collect();
        Ok(BatchEnv { slots: slots?, source, reward_cfg, max_steps, base_seed })
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
    pub fn observe_all(&mut self, obs: &mut [f32], mask: &mut [u8]) {
        self.slots.par_iter_mut().zip(obs.par_chunks_mut(OBS_SIZE)).zip(mask.par_chunks_mut(ACTION_SPACE)).for_each(|((s, o), m)| {
            write_obs_mask(&mut s.cx, o, m);
        });
    }

    /// Applies one action per env (`actions[i]` is a dense action index), auto-resetting finished episodes.
    pub fn step(&mut self, actions: &[i32], out: StepOut) {
        let cfg = self.reward_cfg;
        let max_steps = self.max_steps;
        let base = self.base_seed;
        let source = &*self.source;
        self.slots
            .par_iter_mut()
            .enumerate()
            .zip(actions.par_iter())
            .zip(out.obs.par_chunks_mut(OBS_SIZE))
            .zip(out.mask.par_chunks_mut(ACTION_SPACE))
            .zip(out.reward.par_iter_mut())
            .zip(out.done.par_iter_mut())
            .zip(out.outcome.par_iter_mut())
            .zip(out.illegal.par_iter_mut())
            .for_each(|((((((((env, slot), &a), obs), mask), reward), done), outcome), illegal)| {
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
                } else if slot.cx.overflow != 0 {
                    end = Some((OUTCOME_OVERFLOW, 0.0));
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
                    start_episode(source, env, BatchEnv::episode_seed(base, env, slot.episode), &mut slot.cx);
                }
                write_obs_mask(&mut slot.cx, obs, mask);
            });
    }
}

/// Observation + action mask of one env. `can_play` of the hand is evaluated once (by `legal_actions_ex`) and shared with the
/// observation. Temporaries of both can overflow too: a flag raised here ends the episode on the next step.
fn write_obs_mask(cx: &mut Combat, obs: &mut [f32], mask: &mut [u8]) {
    let mut buf = sts2sim::engine::ActionBuf::new();
    let mut playable = 0u16;
    cx.legal_actions_ex(&mut buf, &mut playable);
    cx.observe_ex(obs, Some(playable));
    mask[..ACTION_SPACE].fill(0);
    for a in buf.iter() {
        mask[a.index()] = 1;
    }
    cx.sync_overflow();
}
