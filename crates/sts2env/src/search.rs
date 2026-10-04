//! `SearchEngine`: the determinized play-out search of `rl/search.py` as a continuous-batching state machine.
//!
//! The solver plays many fights at once. Every fight (a "root") repeatedly asks the policy for its likeliest actions, copies the fight
//! `M x K` times (the `M` best actions, each on `K` determinized futures, see [`Combat::determinize`]), lets every copy play its action
//! and then the policy's own moves to the end of the player turn, and plays the action whose copies got the best mean estimate
//! (final reward if the fight ended, else the value network's opinion of the next turn's first state).
//!
//! The simulator never calls a network. It stops wherever a decision is needed and hands out one observation row per request:
//! policy rows (a root's decision or a play-out's next move) and value rows (a finished play-out). The caller evaluates the rows in one
//! batch of its choice (any mix of roots and play-outs at different phases, so the batch stays large however the fights are distributed)
//! and passes the answers back to [`SearchEngine::advance`]. Moves with a single legal action are played without a request, only slots
//! that need an answer are stepped or observed, a finished fight is replaced by the next job at once, and the whole state machine runs on
//! the rayon pool (one task per root, so a root and its play-outs never share data with another).

use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use sts2sim::engine::{ActionBuf, ACTION_SPACE};
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::{RngSet, Stage};
use sts2sim::types::Outcome;
use sts2sim::{Action, Combat, Scenario, ScenarioExtras};

use crate::{EnvError, OUTCOME_LOSS, OUTCOME_OVERFLOW, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_WIN, WORKER_STACK};

const NONE: u32 = u32::MAX;
/// Largest number of options per decision.
pub const MAX_M: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct SearchCfg {
    /// Options (the policy's most probable actions) tried per decision.
    pub m: usize,
    /// Determinized futures per option.
    pub k: usize,
    /// A decision whose top action has at least this probability is not searched.
    pub conf: f32,
    /// Options below this probability are not tried (the top one always is).
    pub pmin: f32,
    /// An option must beat the top one by more than this to replace it.
    pub margin: f32,
    /// Play-outs stop after this many steps (no bootstrap).
    pub roll_cap: u32,
    /// A play-out asks the value network for the state it has reached after this many policy decisions (instead of playing on to the end of the turn).
    pub depth: u32,
    /// A fight is truncated after this many steps of the real fight.
    pub max_steps: u32,
    pub win: f32,
    pub loss: f32,
    pub hp_bonus: f32,
}

/// What the engine reports per job (fight).
#[derive(Clone, Copy, Debug, Default)]
pub struct JobResult {
    pub scen: u32,
    pub outcome: i8,
    pub hp_lost: f32,
    pub hp_end: f32,
    pub len: u32,
    pub done: bool,
}

/// One move of a recorded fight: the action played, and for searched decisions the options considered with their policy probabilities
/// and estimated returns.
#[derive(Clone, Copy, Debug)]
pub struct MoveRec {
    pub action: u16,
    pub searched: bool,
    pub opts: [u16; MAX_M],
    pub p: [f32; MAX_M],
    pub q: [f32; MAX_M],
    pub legal: [bool; MAX_M],
}

impl MoveRec {
    fn forced(action: u16) -> MoveRec {
        MoveRec { action, searched: false, opts: [0; MAX_M], p: [0.0; MAX_M], q: [0.0; MAX_M], legal: [false; MAX_M] }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStats {
    pub root_decisions: u64,
    pub searched: u64,
    pub forced_root: u64,
    pub sim_steps: u64,
    pub forced_sim: u64,
    pub policy_rows: u64,
    pub value_rows: u64,
    pub forks: u64,
    pub illegal: u64,
    pub end_turn: u64,
    pub end_term: u64,
    pub end_cap: u64,
    pub end_stuck: u64,
    pub end_depth: u64,
    /// time stamp counter ticks spent in: `step`, legal actions, observation rows (policy / value), forks (clone + determinize), the rest of the real fight's moves
    pub cy_step: u64,
    pub cy_legal: u64,
    pub cy_obs: u64,
    pub cy_fork: u64,
    pub cy_main: u64,
}

#[inline(always)]
fn tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: rdtsc has no preconditions.
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SimSt {
    /// not part of the current decision
    Idle,
    /// waiting for the policy's move (row of the policy request)
    Pol(u32),
    /// waiting for the value of the next turn's first state (row of the value request)
    Val(u32),
    Done,
}

struct Sim {
    cx: Combat,
    st: SimSt,
    start_turn: i32,
    est: f32,
    steps: u32,
    /// policy decisions taken after the first action
    pdec: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RootSt {
    Idle,
    /// waiting for the policy's ranking of the actions (row)
    Pol(u32),
    Searching,
}

struct Block {
    main: Combat,
    sims: Vec<Sim>,
    st: RootSt,
    job: u32,
    steps: u32,
    hp0: f32,
    scen: u32,
    rng: u64,
    opts: [i32; MAX_M],
    ok: [bool; MAX_M],
    /// probabilities of the options of the current decision and the estimates of the finished search
    probs: [f32; MAX_M],
    qs: [f32; MAX_M],
    log: Vec<MoveRec>,
    stats: SearchStats,
}

struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}
impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        SendPtr(self.0)
    }
}
impl<T> Copy for SendPtr<T> {}

/// Request buffers (caller-owned, row-major) and the per-call row counters.
struct Out {
    pol_obs: SendPtr<f32>,
    pol_mask: SendPtr<u8>,
    /// per policy row: bit 0 = a move inside a play-out (0: a decision of the real fight), bit 1 = a card selection is pending
    pol_kind: SendPtr<u8>,
    val_obs: SendPtr<f32>,
    /// per value row: bit 1 = a card selection is pending
    val_kind: SendPtr<u8>,
    pol_cap: usize,
    val_cap: usize,
    n_pol: AtomicUsize,
    n_val: AtomicUsize,
}

/// The caller's answers to the previous call's requests.
struct Inputs<'a> {
    /// `[rows, 2M + 1]`: the M best action indices, their probabilities, the action to play in a play-out.
    pol: &'a [f32],
    val: &'a [f32],
}

struct Shared<'a> {
    cfg: SearchCfg,
    scen: &'a [(Scenario, ScenarioExtras)],
    jobs: &'a [(u32, u64)],
    next_job: AtomicUsize,
    results: SendPtr<JobResult>,
    /// per-job move logs (empty unless the engine records)
    logs: SendPtr<Vec<MoveRec>>,
    record: bool,
}

fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Terminal state of a combat as the batch env reports it: `(outcome, reward)`.
fn terminal(cx: &Combat, steps: u32, max_steps: u32, cfg: &SearchCfg) -> Option<(i8, f32)> {
    if cx.missing.is_some() {
        Some((OUTCOME_UNIMPLEMENTED, 0.0))
    } else if cx.overflow != 0 {
        Some((OUTCOME_OVERFLOW, 0.0))
    } else if cx.stage == Stage::Over {
        let me = cx.cr(0);
        match cx.outcome {
            Outcome::Victory => Some((OUTCOME_WIN, cfg.win + cfg.hp_bonus * me.hp as f32 / me.max_hp.max(1) as f32)),
            _ => Some((OUTCOME_LOSS, cfg.loss)),
        }
    } else if steps >= max_steps {
        Some((OUTCOME_TRUNCATED, 0.0))
    } else {
        None
    }
}

/// The move that needs no decision: the only legal action, or confirming a selection that is full (picking another card at `max` only
/// swaps the latest pick, which the policy could have chosen directly; a sampled policy otherwise wanders between picks for dozens of steps).
fn forced_action(cx: &Combat, buf: &ActionBuf) -> Option<Action> {
    if buf.len() == 1 {
        return Some(buf[0]);
    }
    if cx.stage == Stage::AwaitChoice {
        if let Some(d) = &cx.decision {
            if d.selected.len() == d.max as usize && buf.iter().any(|a| matches!(a, Action::Confirm)) {
                return Some(Action::Confirm);
            }
        }
    }
    None
}

/// Writes the observation (and optionally the action mask) of `cx` into row `row` of the request buffers.
fn write_row(cx: &mut Combat, buf: &ActionBuf, playable: Option<u16>, obs: SendPtr<f32>, mask: Option<SendPtr<u8>>, row: usize) {
    // SAFETY: `row` was handed out once by the atomic counter and is below the buffer's capacity (checked by the caller), so no other
    // task touches these bytes; the buffers have `OBS_SIZE` / `ACTION_SPACE` entries per row.
    let o = unsafe { std::slice::from_raw_parts_mut(obs.0.add(row * OBS_SIZE), OBS_SIZE) };
    cx.observe_ex(o, playable);
    if let Some(m) = mask {
        let m = unsafe { std::slice::from_raw_parts_mut(m.0.add(row * ACTION_SPACE), ACTION_SPACE) };
        m.fill(0);
        for a in buf.iter() {
            m[a.index()] = 1;
        }
    }
    cx.sync_overflow();
}

fn pol_row(out: &Out, sim: bool, cx: &Combat) -> usize {
    let kind = sim as u8 | (cx.decision.is_some() as u8) << 1;
    let r = out.n_pol.fetch_add(1, Ordering::Relaxed);
    assert!(r < out.pol_cap, "policy request buffer too small");
    // SAFETY: row `r` is owned by the caller (unique counter value) and below the capacity.
    unsafe { *out.pol_kind.0.add(r) = kind };
    r
}

fn val_row(out: &Out, cx: &Combat) -> usize {
    let r = out.n_val.fetch_add(1, Ordering::Relaxed);
    assert!(r < out.val_cap, "value request buffer too small");
    // SAFETY: as in `pol_row`.
    unsafe { *out.val_kind.0.add(r) = (cx.decision.is_some() as u8) << 1 };
    r
}

/// Plays `act` on a play-out copy and then every forced move, until it needs the policy / the value network or ends.
#[inline(never)]
fn sim_run(sim: &mut Sim, mut act: Action, cfg: &SearchCfg, out: &Out, st: &mut SearchStats) {
    loop {
        let t0 = tsc();
        let ok = sim.cx.step(act);
        st.cy_step += tsc() - t0;
        sim.steps += 1;
        st.sim_steps += 1;
        if !ok {
            // cannot happen for an action the policy took from the legal set; count it and score the copy as lost
            st.illegal += 1;
            sim.est += cfg.loss;
            sim.st = SimSt::Done;
            return;
        }
        if let Some((_, r)) = terminal(&sim.cx, 0, u32::MAX, cfg) {
            sim.est += r;
            sim.st = SimSt::Done;
            st.end_term += 1;
            return;
        }
        if sim.cx.player.turn_number > sim.start_turn {
            let t0 = tsc();
            let row = val_row(out, &sim.cx);
            let buf = ActionBuf::new();
            write_row(&mut sim.cx, &buf, None, out.val_obs, None, row);
            st.cy_obs += tsc() - t0;
            st.value_rows += 1;
            st.end_turn += 1;
            sim.st = SimSt::Val(row as u32);
            return;
        }
        if sim.steps >= cfg.roll_cap {
            sim.st = SimSt::Done;
            st.end_cap += 1;
            return;
        }
        let mut buf = ActionBuf::new();
        let mut playable = 0u16;
        let t0 = tsc();
        sim.cx.legal_actions_ex(&mut buf, &mut playable);
        st.cy_legal += tsc() - t0;
        if buf.is_empty() {
            sim.st = SimSt::Done;
            st.end_stuck += 1;
            return;
        }
        if let Some(a) = forced_action(&sim.cx, &buf) {
            act = a;
            st.forced_sim += 1;
        } else if sim.pdec >= cfg.depth {
            // deep enough: the value network judges the state as it stands
            let t0 = tsc();
            let row = val_row(out, &sim.cx);
            let buf = ActionBuf::new();
            write_row(&mut sim.cx, &buf, None, out.val_obs, None, row);
            st.cy_obs += tsc() - t0;
            st.value_rows += 1;
            st.end_depth += 1;
            sim.st = SimSt::Val(row as u32);
            return;
        } else {
            sim.pdec += 1;
            let t0 = tsc();
            let row = pol_row(out, true, &sim.cx);
            write_row(&mut sim.cx, &buf, Some(playable), out.pol_obs, Some(out.pol_mask), row);
            st.cy_obs += tsc() - t0;
            st.policy_rows += 1;
            sim.st = SimSt::Pol(row as u32);
            return;
        }
    }
}

impl Block {
    fn new(sc: &Scenario, ex: &ScenarioExtras, n_sims: usize) -> Result<Block, EnvError> {
        let main = Combat::try_new_with(sc, ex)?;
        let sims = (0..n_sims).map(|_| Sim { cx: main.clone(), st: SimSt::Idle, start_turn: 0, est: 0.0, steps: 0, pdec: 0 }).collect();
        Ok(Block { main, sims, st: RootSt::Idle, job: NONE, steps: 0, hp0: 0.0, scen: 0, rng: 0, opts: [0; MAX_M], ok: [false; MAX_M], probs: [0.0; MAX_M], qs: [0.0; MAX_M], log: Vec::new(), stats: SearchStats::default() })
    }

    /// Takes the next job (if any) and sets the real fight up. Returns false when the queue is empty.
    fn start_job(&mut self, sh: &Shared) -> bool {
        let j = sh.next_job.fetch_add(1, Ordering::Relaxed);
        if j >= sh.jobs.len() {
            self.st = RootSt::Idle;
            self.job = NONE;
            return false;
        }
        let (si, seed) = sh.jobs[j];
        let (sc, ex) = &sh.scen[si as usize];
        if self.main.reset_validated(sc, ex, seed, RngSet::from_run_seed_fast(seed)).is_err() {
            self.main.overflow |= sts2sim::state::ov::SCENARIO;
        }
        self.job = j as u32;
        self.steps = 0;
        self.scen = si;
        let me = self.main.cr(0);
        self.hp0 = me.hp as f32 / me.max_hp.max(1) as f32;
        self.rng = seed ^ 0xA5A5_5A5A_1234_8765;
        true
    }

    fn record(&mut self, sh: &Shared, outcome: i8) {
        let me = self.main.cr(0);
        let end_frac = if outcome == OUTCOME_WIN { me.hp as f32 / me.max_hp.max(1) as f32 } else { 0.0 };
        let r = JobResult { scen: self.scen, outcome, hp_lost: self.hp0 - end_frac, hp_end: end_frac, len: self.steps, done: true };
        // SAFETY: every job index is taken by exactly one block (atomic counter), so this entry is written once and by this task only.
        unsafe { *sh.results.0.add(self.job as usize) = r };
        if sh.record {
            // SAFETY: as above, one writer per job.
            unsafe { *sh.logs.0.add(self.job as usize) = std::mem::take(&mut self.log) };
        }
    }

    /// Plays `act` in the real fight. Returns true if the job ended (and the next one could not be started: the block is idle).
    fn main_act(&mut self, act: Action, sh: &Shared, rec: Option<MoveRec>) -> bool {
        if sh.record {
            self.log.push(rec.unwrap_or_else(|| MoveRec::forced(act.index() as u16)));
        }
        let t0 = tsc();
        let ok = self.main.step(act);
        self.stats.cy_main += tsc() - t0;
        self.steps += 1;
        let mut end = terminal(&self.main, self.steps, sh.cfg.max_steps, &sh.cfg).map(|t| t.0);
        if !ok {
            self.stats.illegal += 1;
            end = Some(OUTCOME_TRUNCATED);
        }
        if let Some(oc) = end {
            self.record(sh, oc);
            return !self.start_job(sh);
        }
        false
    }

    /// The real fight stands at a state: plays forced moves, then asks the policy about the first real decision (or goes idle).
    fn root_next(&mut self, sh: &Shared, out: &Out) {
        if self.job == NONE && !self.start_job(sh) {
            self.st = RootSt::Idle;
            return;
        }
        loop {
            let mut buf = ActionBuf::new();
            let mut playable = 0u16;
            self.main.legal_actions_ex(&mut buf, &mut playable);
            if buf.is_empty() {
                self.record(sh, OUTCOME_TRUNCATED);
                if !self.start_job(sh) {
                    self.st = RootSt::Idle;
                    return;
                }
            } else if let Some(a) = forced_action(&self.main, &buf) {
                self.stats.forced_root += 1;
                if self.main_act(a, sh, None) {
                    self.st = RootSt::Idle;
                    return;
                }
            } else {
                let row = pol_row(out, false, &self.main);
                write_row(&mut self.main, &buf, Some(playable), out.pol_obs, Some(out.pol_mask), row);
                self.stats.policy_rows += 1;
                self.st = RootSt::Pol(row as u32);
                return;
            }
        }
    }

    /// The record of the decision being played (None when the engine does not record).
    fn move_rec(&self, action: i32, searched: bool, sh: &Shared) -> Option<MoveRec> {
        if !sh.record {
            return None;
        }
        let mut r = MoveRec::forced(action as u16);
        r.searched = searched;
        for j in 0..sh.cfg.m {
            r.opts[j] = self.opts[j] as u16;
            r.p[j] = self.probs[j];
            r.legal[j] = self.ok[j];
            r.q[j] = if searched && self.ok[j] { self.qs[j] } else { f32::NAN };
        }
        Some(r)
    }

    /// The policy ranked the actions of the real fight's decision: play the best one or search.
    fn on_root_policy(&mut self, inp: &Inputs, row: usize, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        let (m, k) = (cfg.m, cfg.k);
        let stride = 2 * m + 1;
        let r = &inp.pol[row * stride..(row + 1) * stride];
        let mut n_legal = 0;
        for j in 0..m {
            self.opts[j] = r[j] as i32;
            let p = r[m + j];
            self.probs[j] = p;
            self.ok[j] = p > 0.0 && (p >= cfg.pmin || j == 0);
            n_legal += self.ok[j] as usize;
        }
        self.stats.root_decisions += 1;
        if n_legal <= 1 || r[m] >= cfg.conf {
            let a = Action::from_index(self.opts[0] as usize);
            let rec = self.move_rec(self.opts[0], false, sh);
            let finished = match a {
                Some(a) => self.main_act(a, sh, rec),
                None => {
                    self.stats.illegal += 1;
                    self.record(sh, OUTCOME_TRUNCATED);
                    !self.start_job(sh)
                }
            };
            if finished {
                self.st = RootSt::Idle;
            } else {
                self.root_next(sh, out);
            }
            return;
        }
        self.stats.searched += 1;
        let ks: Vec<u64> = (0..k).map(|_| splitmix(&mut self.rng)).collect();
        let turn = self.main.player.turn_number;
        for j in 0..m {
            for kk in 0..k {
                let sim = &mut self.sims[j * k + kk];
                if !self.ok[j] {
                    sim.st = SimSt::Idle;
                    continue;
                }
                let t0 = tsc();
                sim.cx.clone_from(&self.main);
                sim.cx.determinize(ks[kk]);
                self.stats.cy_fork += tsc() - t0;
                sim.start_turn = turn;
                sim.est = 0.0;
                sim.steps = 0;
                sim.pdec = 0;
                self.stats.forks += 1;
                match Action::from_index(self.opts[j] as usize) {
                    Some(a) => sim_run(sim, a, cfg, out, &mut self.stats),
                    None => {
                        sim.st = SimSt::Done;
                        sim.est = cfg.loss;
                    }
                }
            }
        }
        self.st = RootSt::Searching;
        self.try_finish_search(sh, out);
    }

    /// If every play-out of the current decision is done: plays the best option for real and moves on.
    fn try_finish_search(&mut self, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        let (m, k) = (cfg.m, cfg.k);
        if self.sims[..m * k].iter().any(|s| matches!(s.st, SimSt::Pol(_) | SimSt::Val(_))) {
            return;
        }
        let mut best = 0usize;
        let mut best_q = f32::NEG_INFINITY;
        let mut q0 = 0.0f32;
        for j in 0..m {
            if !self.ok[j] {
                continue;
            }
            let q = self.sims[j * k..(j + 1) * k].iter().map(|s| s.est).sum::<f32>() / k as f32;
            self.qs[j] = q;
            if j == 0 {
                q0 = q;
            }
            if q > best_q {
                best_q = q;
                best = j;
            }
        }
        if best != 0 && best_q - q0 <= cfg.margin {
            best = 0;
        }
        for s in self.sims[..m * k].iter_mut() {
            s.st = SimSt::Idle;
        }
        let rec = self.move_rec(self.opts[best], true, sh);
        let finished = match Action::from_index(self.opts[best] as usize) {
            Some(a) => self.main_act(a, sh, rec),
            None => {
                self.stats.illegal += 1;
                self.record(sh, OUTCOME_TRUNCATED);
                !self.start_job(sh)
            }
        };
        if finished {
            self.st = RootSt::Idle;
        } else {
            self.root_next(sh, out);
        }
    }

    #[inline(never)]
    fn advance(&mut self, inp: Option<&Inputs>, sh: &Shared, out: &Out) {
        let m = sh.cfg.m;
        let stride = 2 * m + 1;
        match self.st {
            RootSt::Idle => {
                if inp.is_none() && self.job == NONE {
                    self.root_next(sh, out); // first call: take a job
                }
            }
            RootSt::Pol(row) => {
                if let Some(inp) = inp {
                    self.on_root_policy(inp, row as usize, sh, out);
                }
            }
            RootSt::Searching => {
                let Some(inp) = inp else { return };
                let cfg = sh.cfg;
                let mut stats = self.stats;
                for sim in self.sims[..cfg.m * cfg.k].iter_mut() {
                    match sim.st {
                        SimSt::Pol(row) => {
                            let a = inp.pol[row as usize * stride + 2 * m] as i32;
                            match Action::from_index(a as usize) {
                                Some(act) => sim_run(sim, act, &cfg, out, &mut stats),
                                None => {
                                    stats.illegal += 1;
                                    sim.st = SimSt::Done;
                                }
                            }
                        }
                        SimSt::Val(row) => {
                            sim.est += inp.val[row as usize];
                            sim.st = SimSt::Done;
                        }
                        _ => {}
                    }
                }
                self.stats = stats;
                self.try_finish_search(sh, out);
            }
        }
    }
}

pub struct SearchEngine {
    cfg: SearchCfg,
    scen: Vec<(Scenario, ScenarioExtras)>,
    jobs: Vec<(u32, u64)>,
    blocks: Vec<Block>,
    results: Vec<JobResult>,
    logs: Vec<Vec<MoveRec>>,
    record: bool,
    next_job: usize,
    pool: rayon::ThreadPool,
    started: bool,
}

impl SearchEngine {
    /// `scen`: the distinct scenarios; `jobs`: one `(scenario index, seed)` per fight to play; `n_roots`: fights played at the same time.
    pub fn new(scen: Vec<(Scenario, ScenarioExtras)>, jobs: Vec<(u32, u64)>, n_roots: usize, cfg: SearchCfg, threads: usize, record: bool) -> Result<SearchEngine, EnvError> {
        if scen.is_empty() || cfg.m == 0 || cfg.m > MAX_M || cfg.k == 0 {
            return Err(EnvError::Buffer("bad search configuration"));
        }
        if jobs.iter().any(|&(s, _)| s as usize >= scen.len()) {
            return Err(EnvError::Buffer("job refers to an unknown scenario"));
        }
        for (s, _) in &scen {
            s.validate()?;
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .stack_size(WORKER_STACK)
            .thread_name(|i| format!("sts2-search-{i}"))
            .build()
            .map_err(|e| EnvError::Pool(e.to_string()))?;
        let n = n_roots.min(jobs.len()).max(1);
        let blocks: Result<Vec<Block>, EnvError> = pool.install(|| (0..n).into_par_iter().map(|_| Block::new(&scen[0].0, &scen[0].1, cfg.m * cfg.k)).collect());
        let results = vec![JobResult::default(); jobs.len()];
        let logs = if record { vec![Vec::new(); jobs.len()] } else { Vec::new() };
        Ok(SearchEngine { cfg, scen, jobs, blocks: blocks?, results, logs, record, next_job: 0, pool, started: false })
    }

    pub fn n_roots(&self) -> usize {
        self.blocks.len()
    }

    pub fn n_jobs(&self) -> usize {
        self.jobs.len()
    }

    /// Largest number of policy / value rows one call can request (size the buffers with it).
    pub fn max_rows(&self) -> (usize, usize) {
        (self.blocks.len() * (self.cfg.m * self.cfg.k + 1), self.blocks.len() * self.cfg.m * self.cfg.k)
    }

    /// The recorded moves of a finished job (empty unless the engine was created with `record`).
    pub fn moves(&self, job: usize) -> &[MoveRec] {
        self.logs.get(job).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn results(&self) -> &[JobResult] {
        &self.results
    }

    pub fn stats(&self) -> SearchStats {
        let mut t = SearchStats::default();
        for b in &self.blocks {
            let s = b.stats;
            t.root_decisions += s.root_decisions;
            t.searched += s.searched;
            t.forced_root += s.forced_root;
            t.sim_steps += s.sim_steps;
            t.forced_sim += s.forced_sim;
            t.policy_rows += s.policy_rows;
            t.value_rows += s.value_rows;
            t.forks += s.forks;
            t.illegal += s.illegal;
            t.end_turn += s.end_turn;
            t.end_term += s.end_term;
            t.end_cap += s.end_cap;
            t.end_stuck += s.end_stuck;
            t.end_depth += s.end_depth;
            t.cy_step += s.cy_step;
            t.cy_legal += s.cy_legal;
            t.cy_obs += s.cy_obs;
            t.cy_fork += s.cy_fork;
            t.cy_main += s.cy_main;
        }
        t
    }

    /// True when every job is finished.
    pub fn finished(&self) -> bool {
        self.started && self.blocks.iter().all(|b| b.st == RootSt::Idle)
    }

    /// One cycle. The first call (`pol` / `val` = `None`) starts the roots; every later call passes the answers to the rows the previous call
    /// returned: `pol` = `[rows, 2M + 1]` f32 (the `M` best action indices, their probabilities, the action a play-out plays), `val` = `[rows]`.
    /// Writes the next requests into the buffers and returns `(policy rows, value rows)`; `(0, 0)` with [`SearchEngine::finished`] ends the run.
    pub fn advance(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, pol_obs: &mut [f32], pol_mask: &mut [u8], pol_kind: &mut [u8], val_obs: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        let (pc, vc) = self.max_rows();
        if pol_obs.len() < pc * OBS_SIZE || pol_mask.len() < pc * ACTION_SPACE || pol_kind.len() < pc || val_obs.len() < vc * OBS_SIZE || val_kind.len() < vc {
            return Err(EnvError::Buffer("request buffers too small, see SearchEngine::max_rows"));
        }
        let first = !self.started;
        if !first && (pol.is_none() || val.is_none()) {
            return Err(EnvError::Buffer("answers missing"));
        }
        self.started = true;
        let out = Out {
            pol_obs: SendPtr(pol_obs.as_mut_ptr()),
            pol_mask: SendPtr(pol_mask.as_mut_ptr()),
            pol_kind: SendPtr(pol_kind.as_mut_ptr()),
            val_obs: SendPtr(val_obs.as_mut_ptr()),
            val_kind: SendPtr(val_kind.as_mut_ptr()),
            pol_cap: pc,
            val_cap: vc,
            n_pol: AtomicUsize::new(0),
            n_val: AtomicUsize::new(0),
        };
        let sh = Shared { cfg: self.cfg, scen: &self.scen, jobs: &self.jobs, next_job: AtomicUsize::new(self.next_job), results: SendPtr(self.results.as_mut_ptr()), logs: SendPtr(self.logs.as_mut_ptr()), record: self.record };
        let inp = if first { None } else { Some(Inputs { pol: pol.unwrap(), val: val.unwrap() }) };
        let blocks = &mut self.blocks;
        self.pool.install(|| {
            blocks.par_iter_mut().for_each(|b| b.advance(inp.as_ref(), &sh, &out));
        });
        self.next_job = sh.next_job.load(Ordering::Relaxed).min(self.jobs.len());
        Ok((out.n_pol.load(Ordering::Relaxed), out.n_val.load(Ordering::Relaxed)))
    }
}

/// Replays a recorded fight: the observation and action mask before every action (`actions.len() + 1` rows, the last one after the final action).
/// The fight depends only on the scenario, the job seed and the actions (the real fight is never determinized).
pub fn replay(scen: &(Scenario, ScenarioExtras), seed: u64, actions: &[u16], obs: &mut [f32], mask: &mut [u8]) -> Result<usize, EnvError> {
    scen.0.validate()?;
    let mut cx = Combat::try_new_with(&scen.0, &scen.1)?;
    cx.reset_validated(&scen.0, &scen.1, seed, RngSet::from_run_seed_fast(seed)).map_err(EnvError::Scenario)?;
    let n = actions.len() + 1;
    if obs.len() < n * OBS_SIZE || mask.len() < n * ACTION_SPACE {
        return Err(EnvError::Buffer("replay buffers too small"));
    }
    for i in 0..n {
        crate::write_obs_mask(&mut cx, &mut obs[i * OBS_SIZE..(i + 1) * OBS_SIZE], &mut mask[i * ACTION_SPACE..(i + 1) * ACTION_SPACE]);
        if i < actions.len() {
            match Action::from_index(actions[i] as usize) {
                Some(a) if cx.step(a) => {}
                _ => return Err(EnvError::Buffer("the recorded action is not legal: the replay diverged")),
            }
        }
    }
    Ok(n)
}
