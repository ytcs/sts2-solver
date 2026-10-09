pub mod search;

use rayon::prelude::*;
use sts2sim::engine::ACTION_SPACE;
use sts2sim::state::{RngSet, Stage};
use sts2sim::ScenarioExtras;
use sts2sim::types::Outcome;
use sts2sim::{Action, Combat, Scenario};

pub use sts2sim::engine::ACTION_SPACE as ACTIONS;
pub use sts2sim::observe::OBS_SIZE as OBS;
pub use sts2sim::scenario::ScenarioError;

pub trait ScenarioSource: Send + Sync {
    fn sample(&self, env: usize, episode_seed: u64) -> Scenario;

    fn pick(&self, _env: usize, _episode_seed: u64) -> Option<&Scenario> {
        None
    }

    fn extras(&self, _env: usize, _episode_seed: u64) -> Option<&ScenarioExtras> {
        None
    }

    fn index(&self, _env: usize, _episode_seed: u64) -> u32 {
        0
    }

    fn validate(&self) -> Result<(), ScenarioError> {
        Ok(())
    }

    fn set_weights(&mut self, _w: &[f32]) -> bool {
        false
    }
}

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

pub struct PoolScenario(Vec<Scenario>, Vec<ScenarioExtras>, Option<Vec<f64>>);
impl PoolScenario {
    pub fn new(v: Vec<Scenario>) -> PoolScenario {
        assert!(!v.is_empty());
        let ex = v.iter().map(|_| ScenarioExtras::default()).collect();
        PoolScenario(v, ex, None)
    }

    pub fn with_extras(v: Vec<(Scenario, ScenarioExtras)>) -> PoolScenario {
        assert!(!v.is_empty());
        let (s, e) = v.into_iter().unzip();
        PoolScenario(s, e, None)
    }

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
    pub hp_bonus: f32,
    pub turn_cap: u32,
}

impl Default for RewardConfig {
    fn default() -> Self {
        RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.0, turn_cap: 0 }
    }
}

pub const OUTCOME_ONGOING: i8 = 0;
pub const OUTCOME_WIN: i8 = 1;
pub const OUTCOME_LOSS: i8 = -1;
pub const OUTCOME_TRUNCATED: i8 = 2;
pub const OUTCOME_UNIMPLEMENTED: i8 = 3;
pub const OUTCOME_OVERFLOW: i8 = 4;

#[inline]
pub fn looped(cx: &Combat) -> bool {
    cx.overflow & sts2sim::state::ov::LOOP != 0
}

struct Slot {
    cx: Combat,
    steps: u32,
    episode: u64,
    scen: u32,
    hp0: f32,
    last: EpisodeInfo,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct EpisodeInfo {
    pub scen: u32,
    pub hp_lost: f32,
    pub hp_end: f32,
    pub len: u32,
    pub hp_end_abs: i32,
    pub max_hp_end: i32,
    pub turns: i32,
}

#[derive(Debug)]
pub enum EnvError {
    Scenario(ScenarioError),
    Pool(String),
    Buffer(&'static str),
}
impl From<ScenarioError> for EnvError {
    fn from(e: ScenarioError) -> EnvError {
        EnvError::Scenario(e)
    }
}

const WORKER_STACK: usize = 32 << 20;

pub struct BatchEnv {
    slots: Vec<Slot>,
    source: Box<dyn ScenarioSource>,
    reward_cfg: RewardConfig,
    max_steps: u32,
    base_seed: u64,
    pool: rayon::ThreadPool,
}

pub struct StepOut<'a> {
    pub obs: &'a mut [f32],
    pub mask: &'a mut [u8],
    pub reward: &'a mut [f32],
    pub done: &'a mut [u8],
    pub outcome: &'a mut [i8],
    pub illegal: &'a mut [u8],
}

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

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn step_one(
    cfg: RewardConfig,
    max_steps: u32,
    base: u64,
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
    *reward = 0.0;
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
    } else if looped(&slot.cx) {
        end = Some((OUTCOME_LOSS, cfg.loss));
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
    pub fn new(n: usize, source: Box<dyn ScenarioSource>, reward_cfg: RewardConfig, max_steps: u32, base_seed: u64) -> BatchEnv {
        Self::try_new(n, source, reward_cfg, max_steps, base_seed).expect("cannot create the batch env")
    }

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
                    Ok(Slot { cx, steps: 0, episode: 0, scen: source.index(i, episode), hp0, last: EpisodeInfo::default() })
                })
                .collect()
        });
        Ok(BatchEnv { slots: slots?, source, reward_cfg, max_steps, base_seed, pool })
    }

    #[inline]
    fn episode_seed(base: u64, env: usize, episode: u64) -> u64 {
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

    pub fn set_weights(&mut self, w: &[f32]) -> bool {
        self.source.set_weights(w)
    }

    pub fn episode_info(&self, out: &mut [EpisodeInfo]) {
        for (o, s) in out.iter_mut().zip(self.slots.iter()) {
            *o = s.last;
        }
    }

    pub fn observe_all(&mut self, obs: &mut [f32], mask: &mut [u8]) -> Result<(), EnvError> {
        let n = self.slots.len();
        let osz = OBS;
        if obs.len() < n * osz {
            return Err(EnvError::Buffer("obs buffer shorter than n_envs * obs_size"));
        }
        if mask.len() < n * ACTION_SPACE {
            return Err(EnvError::Buffer("mask buffer shorter than n_envs * ACTION_SPACE"));
        }
        let slots = &mut self.slots;
        self.pool.install(|| {
            slots
                .par_iter_mut()
                .zip(obs[..n * osz].par_chunks_mut(osz))
                .zip(mask[..n * ACTION_SPACE].par_chunks_mut(ACTION_SPACE))
                .for_each(|((s, o), m)| observe_one(s, o, m));
        });
        Ok(())
    }

    pub fn step(&mut self, actions: &[i32], out: StepOut) -> Result<(), EnvError> {
        let n = self.slots.len();
        if actions.len() < n {
            return Err(EnvError::Buffer("actions shorter than n_envs"));
        }
        let osz = OBS;
        if out.obs.len() < n * osz {
            return Err(EnvError::Buffer("obs buffer shorter than n_envs * obs_size"));
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
        let source = &*self.source;
        let slots = &mut self.slots;
        self.pool.install(|| {
            slots
                .par_iter_mut()
                .enumerate()
                .zip(actions[..n].par_iter())
                .zip(out.obs[..n * osz].par_chunks_mut(osz))
                .zip(out.mask[..n * ACTION_SPACE].par_chunks_mut(ACTION_SPACE))
                .zip(out.reward[..n].par_iter_mut())
                .zip(out.done[..n].par_iter_mut())
                .zip(out.outcome[..n].par_iter_mut())
                .zip(out.illegal[..n].par_iter_mut())
                .for_each(|((((((((env, slot), &a), obs), mask), reward), done), outcome), illegal)| {
                    step_one(cfg, max_steps, base, source, env, slot, a, obs, mask, reward, done, outcome, illegal)
                });
        });
        Ok(())
    }
}

/// The first observation of every scenario under a seed, equal to `BatchEnv` with a round-robin source of n = len envs, then `observe_all`.
pub struct FightStarts {
    source: RoundRobinScenario,
    n: usize,
    pool: &'static rayon::ThreadPool,
}

static STARTS_POOL: std::sync::OnceLock<Result<rayon::ThreadPool, String>> = std::sync::OnceLock::new();

impl FightStarts {
    pub fn try_new(scs: Vec<(Scenario, ScenarioExtras)>) -> Result<FightStarts, EnvError> {
        if scs.is_empty() {
            return Err(EnvError::Buffer("no scenarios"));
        }
        let n = scs.len();
        let source = RoundRobinScenario::with_extras(scs);
        source.validate()?;
        let pool = STARTS_POOL
            .get_or_init(|| {
                rayon::ThreadPoolBuilder::new().stack_size(WORKER_STACK).thread_name(|i| format!("sts2-starts-{i}")).build().map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(|e| EnvError::Pool(e.clone()))?;
        Ok(FightStarts { source, n, pool })
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn observe(&self, seed: u64, obs: &mut [f32], mask: &mut [u8]) -> Result<(), EnvError> {
        let (n, osz) = (self.n, OBS);
        if obs.len() < n * osz || mask.len() < n * ACTION_SPACE {
            return Err(EnvError::Buffer("obs / mask buffer shorter than n * size"));
        }
        let source = &self.source;
        self.pool.install(|| {
            obs[..n * osz]
                .par_chunks_mut(osz)
                .zip(mask[..n * ACTION_SPACE].par_chunks_mut(ACTION_SPACE))
                .enumerate()
                .try_for_each(|(i, (o, m))| {
                    let episode = BatchEnv::episode_seed(seed, i, 0);
                    let sc = source.sample(i, episode);
                    let mut cx = Combat::try_new_with(&sc, &source.1[i])?;
                    write_obs_mask(&mut cx, o, m);
                    Ok(())
                })
        })
    }
}

#[inline(never)]
fn observe_one(s: &mut Slot, obs: &mut [f32], mask: &mut [u8]) {
    write_obs_mask(&mut s.cx, obs, mask);
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use sts2sim::dec::Dec;
    use sts2sim::state::{CardIdx, PLAYER};
    use sts2sim::types::{CardPilePosition, PileType, NO};
    use sts2sim::{ids, DeckCard, RelicInit};

    fn scenario() -> Scenario {
        Scenario {
            run_seed: 3,
            total_floor: 1,
            character: 0,
            ascension: 10,
            encounter: ids::encounter::NIBBITS_WEAK,
            max_hp: 80,
            hp: 80,
            max_energy: 3,
            orb_slots: 0,
            potion_slots: 2,
            deck: (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect(),
            relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }, RelicInit { id: ids::relic::VELVET_CHOKER, ..Default::default() }],
            potions: vec![],
            rng: RngSet::from_run_seed(3),
        }
    }

    fn arm(cx: &mut Combat) {
        let all: Vec<CardIdx> = cx.player.hand.iter().chain(cx.player.draw.iter()).chain(cx.player.discard.iter()).copied().collect();
        for (k, &c) in all.iter().enumerate() {
            cx.move_card(c, if k < 2 { PileType::Discard } else { PileType::Exhaust }, CardPilePosition::Bottom);
        }
        let p = cx.new_card(ids::card::PILLAGE, 0).unwrap();
        cx.move_card(p, PileType::Hand, CardPilePosition::Bottom);
        cx.apply_power(ids::power::HELLRAISER_POWER, PLAYER, Dec::int(1), PLAYER, NO);
        let r = cx.player.relics.iter().position(|r| r.id == ids::relic::VELVET_CHOKER).unwrap();
        cx.player.relics[r].counter = 5;
        cx.sync_overflow();
        assert_eq!(cx.overflow, 0);
    }

    #[test]
    fn an_episode_the_loop_guard_ends_is_a_loss_with_the_loss_reward() {
        let cfg = RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.5, turn_cap: 0 };
        let mut env = BatchEnv::new(1, Box::new(PoolScenario::new(vec![scenario()])), cfg, 1000, 5);
        arm(&mut env.slots[0].cx);
        let e = env.slots[0].cx.enemies[0];
        let (mut obs, mut mask) = (vec![0f32; OBS], vec![0u8; ACTION_SPACE]);
        let (mut reward, mut done, mut outcome, mut illegal) = ([0f32], [0u8], [0i8], [0u8]);
        let a = [Action::PlayCard { hand_pos: 0, target: e }.index() as i32];
        env.step(&a, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal }).unwrap();
        assert_eq!((done[0], outcome[0], illegal[0]), (1, OUTCOME_LOSS, 0));
        assert!((reward[0] + 1.0).abs() < 1e-6, "the loss reward, not 0: {}", reward[0]);
        let mut info = [EpisodeInfo::default()];
        env.episode_info(&mut info);
        assert_eq!((info[0].hp_end, info[0].hp_end_abs), (0.0, 0), "scored like any loss");
    }
}
