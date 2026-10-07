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

pub mod search;

use rayon::prelude::*;
use sts2sim::engine::ACTION_SPACE;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::{RngSet, Stage};
use sts2sim::ScenarioExtras;
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

    /// Optional per-card inputs (enchantments, saved properties) of the scenario `pick` returns for the same episode.
    fn extras(&self, _env: usize, _episode_seed: u64) -> Option<&ScenarioExtras> {
        None
    }

    /// Index of the scenario an episode runs (for per-scenario statistics); 0 for sources with a single scenario.
    fn index(&self, _env: usize, _episode_seed: u64) -> u32 {
        0
    }

    /// Checks every scenario the source can produce (`BatchEnv::try_new` calls it once, so the per-episode reset can skip
    /// validation). The default accepts everything: sources built on `sample` are validated per episode instead.
    fn validate(&self) -> Result<(), ScenarioError> {
        Ok(())
    }

    /// Sampling weights of the scenarios (one per scenario; the next episodes draw scenario i with probability w_i / sum w). Returns false when the
    /// source does not sample from a list (the weights are ignored).
    fn set_weights(&mut self, _w: &[f32]) -> bool {
        false
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
pub struct PoolScenario(Vec<Scenario>, Vec<ScenarioExtras>, Option<Vec<f64>>);
impl PoolScenario {
    pub fn new(v: Vec<Scenario>) -> PoolScenario {
        assert!(!v.is_empty());
        let ex = v.iter().map(|_| ScenarioExtras::default()).collect();
        PoolScenario(v, ex, None)
    }

    /// Scenarios with their deck enchantments / saved card properties (what the oracle JSON can carry).
    pub fn with_extras(v: Vec<(Scenario, ScenarioExtras)>) -> PoolScenario {
        assert!(!v.is_empty());
        let (s, e) = v.into_iter().unzip();
        PoolScenario(s, e, None)
    }

    /// The scenario of an episode: uniform, or by the cumulative weights (`set_weights`).
    fn idx(&self, episode_seed: u64) -> usize {
        match &self.2 {
            None => (episode_seed >> 17) as usize % self.0.len(),
            Some(cdf) => {
                let u = (episode_seed >> 11) as f64 / (1u64 << 53) as f64 * cdf[cdf.len() - 1];
                cdf.partition_point(|&c| c <= u).min(self.0.len() - 1)
            }
        }
    }
}
impl ScenarioSource for PoolScenario {
    fn sample(&self, _env: usize, episode_seed: u64) -> Scenario {
        let mut s = self.0[self.idx(episode_seed)].clone();
        s.run_seed = episode_seed;
        s.rng = RngSet::from_run_seed(episode_seed);
        s
    }
    fn pick(&self, _env: usize, episode_seed: u64) -> Option<&Scenario> {
        Some(&self.0[self.idx(episode_seed)])
    }
    fn index(&self, _env: usize, episode_seed: u64) -> u32 {
        self.idx(episode_seed) as u32
    }
    fn extras(&self, _env: usize, episode_seed: u64) -> Option<&ScenarioExtras> {
        Some(&self.1[self.idx(episode_seed)])
    }
    fn validate(&self) -> Result<(), ScenarioError> {
        self.0.iter().try_for_each(|s| s.validate())
    }
    fn set_weights(&mut self, w: &[f32]) -> bool {
        if w.len() != self.0.len() {
            return false;
        }
        let mut acc = 0.0f64;
        let cdf: Vec<f64> = w.iter().map(|&x| {
            acc += x.max(0.0) as f64;
            acc
        }).collect();
        self.2 = if acc > 0.0 { Some(cdf) } else { None };
        true
    }
}

/// Env `i` always plays scenario `i % len` (every episode, only the RNG streams change): `n_envs = n x len` gives `n` attempts of every scenario.
pub struct RoundRobinScenario(Vec<Scenario>, Vec<ScenarioExtras>);
impl RoundRobinScenario {
    pub fn with_extras(v: Vec<(Scenario, ScenarioExtras)>) -> RoundRobinScenario {
        assert!(!v.is_empty());
        let (s, e) = v.into_iter().unzip();
        RoundRobinScenario(s, e)
    }
}
impl ScenarioSource for RoundRobinScenario {
    fn sample(&self, env: usize, episode_seed: u64) -> Scenario {
        let mut s = self.0[env % self.0.len()].clone();
        s.run_seed = episode_seed;
        s.rng = RngSet::from_run_seed(episode_seed);
        s
    }
    fn pick(&self, env: usize, _episode_seed: u64) -> Option<&Scenario> {
        Some(&self.0[env % self.0.len()])
    }
    fn extras(&self, env: usize, _episode_seed: u64) -> Option<&ScenarioExtras> {
        Some(&self.1[env % self.0.len()])
    }
    fn index(&self, env: usize, _episode_seed: u64) -> u32 {
        (env % self.0.len()) as u32
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
    /// A fight still running after this many player turns ends as a loss (0 = no cap). Stalls are losses, as in the search (`SearchCfg::turn_cap`).
    pub turn_cap: u32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.0, step: 0.0, turn_cap: 0 }
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
    /// Scenario index and starting HP fraction of the running episode.
    scen: u32,
    hp0: f32,
    /// Summary of the episode that ended last.
    last: EpisodeInfo,
    /// Set when the episode ended and the env does not auto-reset (`BatchEnv::set_autoreset(false)`): the outcome code.
    frozen: Option<i8>,
    /// Belt slots whose potion the latest step used up or lost (bit k = slot k; thrown, discarded or consumed by a relic / power such as Fairy in a Bottle),
    /// computed before an auto-reset: the potion-use head's target (`docs/rl_redesign.md` 3.3).
    pot_used: u8,
}

/// Potion id per belt slot (`u16::MAX` = empty).
fn belt(cx: &Combat) -> [u16; sts2sim::state::MAX_POTIONS] {
    let mut o = [u16::MAX; sts2sim::state::MAX_POTIONS];
    for (k, p) in cx.player.potions.iter().enumerate() {
        if let Some(p) = p {
            o[k] = p.id;
        }
    }
    o
}

/// What `BatchEnv::episode_info` reports per env about the episode that ended most recently.
#[derive(Clone, Copy, Default, Debug)]
pub struct EpisodeInfo {
    /// Index of the scenario in the source.
    pub scen: u32,
    /// Player HP lost during the fight as a fraction of max HP (a loss counts the HP the player had left as lost).
    pub hp_lost: f32,
    /// HP left at the end as a fraction of max HP (0 on a loss).
    pub hp_end: f32,
    /// Agent steps the episode took.
    pub len: u32,
    /// HP left at the end (absolute; 0 on a loss), the max HP at the end, and the player turns the fight took.
    pub hp_end_abs: i32,
    pub max_hp_end: i32,
    pub turns: i32,
}

/// Why a `BatchEnv` call failed (always a caller / scenario error: stepping itself never fails or panics).
#[derive(Debug)]
pub enum EnvError {
    /// A scenario of the source cannot be started (unported content, does not fit the fixed capacities, ...).
    Scenario(ScenarioError),
    /// The worker thread pool could not be created.
    Pool(String),
    /// An output / input buffer is shorter than `n_envs` rows.
    Buffer(&'static str),
}
impl From<ScenarioError> for EnvError {
    fn from(e: ScenarioError) -> EnvError {
        EnvError::Scenario(e)
    }
}

/// Stack size of the env worker threads. The per-env work (a full `Combat::step` + observation, with hooks calling hooks)
/// is a deep call tree with kilobyte frames; rayon nests such tasks on the stack while it work-steals, which overflowed the
/// 2 MB default of the global pool now and then. (Virtual memory only: pages are committed when touched.)
const WORKER_STACK: usize = 32 << 20;

pub struct BatchEnv {
    slots: Vec<Slot>,
    source: Box<dyn ScenarioSource>,
    reward_cfg: RewardConfig,
    max_steps: u32,
    base_seed: u64,
    pool: rayon::ThreadPool,
    /// Finished episodes restart on the next scenario (default). Search environments turn this off: a finished slot keeps its final
    /// state, reports `done` with the same outcome every step and ignores actions.
    autoreset: bool,
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
    let default_ex = sts2sim::ScenarioExtras::default();
    let ex = source.extras(env, episode_seed).unwrap_or(&default_ex);
    let r = match source.pick(env, episode_seed) {
        Some(sc) => cx.reset_validated(sc, ex, episode_seed, RngSet::from_run_seed_fast(episode_seed)),
        None => {
            let sc = source.sample(env, episode_seed);
            match sc.validate() {
                Ok(()) => cx.reset_validated(&sc, ex, sc.run_seed, sc.rng),
                Err(e) => Err(e),
            }
        }
    };
    if r.is_err() {
        cx.overflow |= sts2sim::state::ov::SCENARIO;
    }
}

/// One env, one step. Deliberately NOT inlined into the rayon closure: the engine inlined here has a frame of many kilobytes,
/// and rayon's recursive splitting would otherwise stack one such frame per recursion level.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn step_one(
    cfg: RewardConfig,
    max_steps: u32,
    base: u64,
    autoreset: bool,
    source: &dyn ScenarioSource,
    env: usize,
    slot: &mut Slot,
    a: i32,
    obs: &mut [f32],
    mask: &mut [u8],
    reward: &mut f32,
    done: &mut u8,
    outcome: &mut i8,
    illegal: &mut u8,
) {
    *reward = cfg.step;
    *done = 0;
    *outcome = OUTCOME_ONGOING;
    *illegal = 0;
    slot.pot_used = 0;
    if let Some(oc) = slot.frozen {
        // a finished search slot: nothing happens, the outcome is reported again
        *done = 1;
        *outcome = oc;
        *reward = 0.0;
        write_obs_mask(&mut slot.cx, obs, mask);
        return;
    }
    let before = belt(&slot.cx);
    let ok = match Action::from_index(a as usize) {
        Some(act) => slot.cx.step(act),
        None => false,
    };
    if !ok {
        *illegal = 1;
    } else {
        slot.steps += 1;
        let after = belt(&slot.cx);
        for k in 0..before.len() {
            if before[k] != u16::MAX && after[k] != before[k] {
                slot.pot_used |= 1 << k;
            }
        }
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
    } else if cfg.turn_cap > 0 && slot.cx.player.turn_number > cfg.turn_cap as i32 {
        end = Some((OUTCOME_LOSS, cfg.loss));
    } else if slot.steps >= max_steps {
        end = Some((OUTCOME_TRUNCATED, 0.0));
    }
    if let Some((oc, r)) = end {
        *done = 1;
        *outcome = oc;
        *reward += r;
        let me = slot.cx.cr(0);
        let end_frac = if oc == OUTCOME_WIN { me.hp as f32 / me.max_hp.max(1) as f32 } else { 0.0 };
        slot.last = EpisodeInfo { scen: slot.scen, hp_lost: slot.hp0 - end_frac, hp_end: end_frac, len: slot.steps, hp_end_abs: if oc == OUTCOME_WIN { me.hp } else { 0 },
                                  max_hp_end: me.max_hp, turns: slot.cx.player.turn_number };
        if !autoreset {
            slot.frozen = Some(oc);
            write_obs_mask(&mut slot.cx, obs, mask);
            return;
        }
        slot.episode += 1;
        slot.steps = 0;
        let seed = BatchEnv::episode_seed(base, env, slot.episode);
        start_episode(source, env, seed, &mut slot.cx);
        slot.scen = source.index(env, seed);
        slot.hp0 = slot.cx.cr(0).hp as f32 / slot.cx.cr(0).max_hp.max(1) as f32;
    }
    write_obs_mask(&mut slot.cx, obs, mask);
}

impl BatchEnv {
    /// Panics if the env cannot be created (construction time only); see [`BatchEnv::try_new`].
    pub fn new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> BatchEnv {
        Self::try_new(n, source, reward_cfg, max_steps, base_seed).expect("cannot create the batch env")
    }

    /// Builds `n` envs; fails (instead of panicking later) if the first scenario of any env cannot be started.
    pub fn try_new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> Result<BatchEnv, EnvError> {
        source.validate()?;
        let pool = rayon::ThreadPoolBuilder::new()
            .stack_size(WORKER_STACK)
            .thread_name(|i| format!("sts2-env-{i}"))
            .build()
            .map_err(|e| EnvError::Pool(e.to_string()))?;
        let slots: Result<Vec<Slot>, ScenarioError> = pool.install(|| {
            (0..n)
                .into_par_iter()
                .map(|i| {
                    let episode = Self::episode_seed(base_seed, i, 0);
                    let sc = source.sample(i, episode);
                    let default_ex = ScenarioExtras::default();
                    let cx = Combat::try_new_with(&sc, source.extras(i, episode).unwrap_or(&default_ex))?;
                    let hp0 = cx.cr(0).hp as f32 / cx.cr(0).max_hp.max(1) as f32;
                    Ok(Slot { cx, steps: 0, episode: 0, scen: source.index(i, episode), hp0, last: EpisodeInfo::default(), frozen: None, pot_used: 0 })
                })
                .collect()
        });
        Ok(BatchEnv { slots: slots?, source, reward_cfg, max_steps, base_seed, pool, autoreset: true })
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

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The player-turn cap (a fight still running after it ends as a loss; 0 = none) from the next step on: a curriculum relaxes it stage by stage.
    pub fn set_turn_cap(&mut self, cap: u32) {
        self.reward_cfg.turn_cap = cap;
    }

    /// Sampling weights of the source's scenarios for the episodes that start from now on (see `ScenarioSource::set_weights`).
    pub fn set_weights(&mut self, w: &[f32]) -> bool {
        self.source.set_weights(w)
    }

    /// Finished episodes restart on the next scenario (`true`, the default) or stay finished (`false`, for search).
    pub fn set_autoreset(&mut self, on: bool) {
        self.autoreset = on;
    }

    /// Copies the combat of `src` slot `src_idx[k]` into this env's slot `dst_idx[k]` and resamples everything a player cannot see
    /// (`Combat::determinize(seeds[k])`: pile orders and RNG streams). The copy continues as a fresh episode of the same scenario
    /// (step counter restarts, finished state is cleared). Used to evaluate actions by simulated play-outs.
    pub fn fork_from(&mut self, src: &BatchEnv, src_idx: &[u32], dst_idx: &[u32], seeds: &[u64]) -> Result<(), EnvError> {
        if src_idx.len() != dst_idx.len() || seeds.len() != dst_idx.len() {
            return Err(EnvError::Buffer("src / dst / seeds lengths differ"));
        }
        for k in 0..dst_idx.len() {
            let (si, di) = (src_idx[k] as usize, dst_idx[k] as usize);
            if si >= src.slots.len() || di >= self.slots.len() {
                return Err(EnvError::Buffer("slot index out of range"));
            }
            let from = &src.slots[si];
            let to = &mut self.slots[di];
            to.cx = from.cx.clone();
            to.cx.determinize(seeds[k]);
            to.steps = 0;
            to.scen = from.scen;
            to.hp0 = from.hp0;
            to.frozen = None;
        }
        Ok(())
    }

    /// Per env, the belt slots whose potion the latest `step` used up (bit k = slot k), measured before an auto-reset.
    pub fn potion_used(&self, out: &mut [u8]) {
        for (o, s) in out.iter_mut().zip(self.slots.iter()) {
            *o = s.pot_used;
        }
    }

    /// Summary of the episode each env finished last (valid where `done` was set by the latest `step`).
    pub fn episode_info(&self, out: &mut [EpisodeInfo]) {
        for (o, s) in out.iter_mut().zip(self.slots.iter()) {
            *o = s.last;
        }
    }

    /// Writes the current observation / mask of every env (e.g. after construction).
    pub fn observe_all(&mut self, obs: &mut [f32], mask: &mut [u8]) -> Result<(), EnvError> {
        let n = self.slots.len();
        if obs.len() < n * OBS_SIZE {
            return Err(EnvError::Buffer("obs buffer shorter than n_envs * OBS_SIZE"));
        }
        if mask.len() < n * ACTION_SPACE {
            return Err(EnvError::Buffer("mask buffer shorter than n_envs * ACTION_SPACE"));
        }
        let slots = &mut self.slots;
        self.pool.install(|| {
            slots
                .par_iter_mut()
                .zip(obs[..n * OBS_SIZE].par_chunks_mut(OBS_SIZE))
                .zip(mask[..n * ACTION_SPACE].par_chunks_mut(ACTION_SPACE))
                .for_each(|((s, o), m)| observe_one(s, o, m));
        });
        Ok(())
    }

    /// Applies one action per env (`actions[i]` is a dense action index), auto-resetting finished episodes.
    pub fn step(&mut self, actions: &[i32], out: StepOut) -> Result<(), EnvError> {
        let n = self.slots.len();
        if actions.len() < n {
            return Err(EnvError::Buffer("actions shorter than n_envs"));
        }
        if out.obs.len() < n * OBS_SIZE {
            return Err(EnvError::Buffer("obs buffer shorter than n_envs * OBS_SIZE"));
        }
        if out.mask.len() < n * ACTION_SPACE {
            return Err(EnvError::Buffer("mask buffer shorter than n_envs * ACTION_SPACE"));
        }
        if out.reward.len() < n || out.done.len() < n || out.outcome.len() < n || out.illegal.len() < n {
            return Err(EnvError::Buffer("reward / done / outcome / illegal shorter than n_envs"));
        }
        let cfg = self.reward_cfg;
        let max_steps = self.max_steps;
        let base = self.base_seed;
        let autoreset = self.autoreset;
        let source = &*self.source;
        let slots = &mut self.slots;
        self.pool.install(|| {
            slots
                .par_iter_mut()
                .enumerate()
                .zip(actions[..n].par_iter())
                .zip(out.obs[..n * OBS_SIZE].par_chunks_mut(OBS_SIZE))
                .zip(out.mask[..n * ACTION_SPACE].par_chunks_mut(ACTION_SPACE))
                .zip(out.reward[..n].par_iter_mut())
                .zip(out.done[..n].par_iter_mut())
                .zip(out.outcome[..n].par_iter_mut())
                .zip(out.illegal[..n].par_iter_mut())
                .for_each(|((((((((env, slot), &a), obs), mask), reward), done), outcome), illegal)| {
                    step_one(cfg, max_steps, base, autoreset, source, env, slot, a, obs, mask, reward, done, outcome, illegal)
                });
        });
        Ok(())
    }
}

#[inline(never)]
fn observe_one(s: &mut Slot, obs: &mut [f32], mask: &mut [u8]) {
    write_obs_mask(&mut s.cx, obs, mask);
}

/// Observation + action mask of one env. `can_play` of the hand is evaluated once (by `legal_actions_ex`) and shared with the
/// observation. Temporaries of both can overflow too: a flag raised here ends the episode on the next step.
pub(crate) fn write_obs_mask(cx: &mut Combat, obs: &mut [f32], mask: &mut [u8]) {
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
