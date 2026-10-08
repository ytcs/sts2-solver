use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use sts2sim::engine::{with_look_cache, ActionBuf, LookCache, ACTION_SPACE, LOOK_CACHE_ENTRIES};
use sts2sim::observe::{obs_size, obs_version};
use sts2sim::state::{RngSet, Stage};
use sts2sim::types::Outcome;
use sts2sim::util::ArrayVec;
use sts2sim::{Action, Combat, Scenario, ScenarioExtras};

use crate::{EnvError, OUTCOME_LOSS, OUTCOME_OVERFLOW, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_WIN, WORKER_STACK};

const NONE: u32 = u32::MAX;
pub const MAX_M: usize = 16;

pub const HEAD_BIN: i32 = 2;
pub const HEAD_NB: usize = 75;
pub const HEAD_NC: usize = HEAD_NB + 1;
pub const POT: usize = sts2sim::state::MAX_POTIONS;

#[derive(Clone, Copy, Debug)]
pub struct Worth {
    pub table: bool,
    pub u: [f32; HEAD_NC],
}

impl Worth {
    pub fn linear() -> Worth {
        Worth { table: false, u: [0.0; HEAD_NC] }
    }
}

fn end_class(hp: i32) -> usize {
    ((hp.max(0) + HEAD_BIN - 1) / HEAD_BIN).clamp(1, HEAD_NB as i32) as usize
}

fn pot_ids(cx: &Combat) -> [u16; POT] {
    let mut o = [u16::MAX; POT];
    for (k, p) in cx.player.potions.iter().enumerate().take(POT) {
        if let Some(p) = p {
            o[k] = p.id;
        }
    }
    o
}

fn end_score(cx: &Combat, oc: i8, r: f32, w: &Worth) -> f32 {
    if !w.table {
        return r;
    }
    match oc {
        OUTCOME_WIN => w.u[end_class(cx.cr(0).hp)],
        OUTCOME_LOSS => w.u[0],
        _ => 0.0,
    }
}

fn leaf_value(r: &[f32], max_hp: i32, cfg: &SearchCfg, w: &Worth) -> f32 {
    if r.len() == 1 {
        return r[0];
    }
    let p = &r[..HEAD_NC];
    if w.table {
        let mut v = 0.0f32;
        for b in 0..HEAD_NC {
            v += p[b] * w.u[b];
        }
        v
    } else {
        let mx = max_hp.max(1) as f32;
        let half = (HEAD_BIN as f32 - 1.0) / 2.0;
        let mut v = p[0] * cfg.loss;
        for (b, &pb) in p.iter().enumerate().skip(1) {
            let c = b as f32 * HEAD_BIN as f32 - half;
            v += pb * (cfg.win + cfg.hp_bonus * (c / mx).min(1.0));
        }
        v
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SearchCfg {
    pub m: usize,
    pub k: usize,
    pub conf: f32,
    pub roll_cap: u32,
    pub leaf_turns: u32,
    pub lead: bool,
    pub strat: bool,
    pub carry: bool,
    pub max_steps: u32,
    pub win: f32,
    pub loss: f32,
    pub hp_bonus: f32,
    pub turn_cap: u32,
    pub val_w: usize,
    pub cover: bool,
    // Total futures per searched decision, split evenly over the candidates and capped at k each; 0 = k per candidate.
    pub futures: usize,
    // DIAGNOSTIC ONLY: futures copy the true state (hidden information); never set for live play.
    pub clairvoyant: bool,
    pub exact: ExactCfg,
}

// Exact turn search: when the searched values are blind (best <= loss, or best - second <= tie; linear units, rescaled under a worth
// table), every distinct line to the end of the turn is enumerated and its end state valued (value net after the enemy turn, `dets`
// paired determinizations); the best line's first move is played. Over `cap` states or `MAX_DEPTH` moves: the searched move.
#[derive(Clone, Copy, Debug)]
pub struct ExactCfg {
    pub on: bool,
    pub loss: f32,
    pub tie: f32,
    pub dets: usize,
    pub cap: usize,
    pub potions: bool,
}

impl Default for ExactCfg {
    fn default() -> ExactCfg {
        ExactCfg { on: false, loss: -0.9, tie: 0.0, dets: 8, cap: 5000, potions: true }
    }
}

impl Default for SearchCfg {
    fn default() -> SearchCfg {
        SearchCfg {
            m: 3,
            k: 8,
            conf: 1.01,
            roll_cap: 120,
            leaf_turns: 2,
            lead: true,
            strat: true,
            carry: true,
            max_steps: 300,
            win: 1.0,
            loss: -1.0,
            hp_bonus: 0.5,
            turn_cap: 0,
            val_w: 1,
            cover: false,
            futures: 0,
            clairvoyant: false,
            exact: ExactCfg::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JobResult {
    pub scen: u32,
    pub outcome: i8,
    pub hp_lost: f32,
    pub hp_end: f32,
    pub len: u32,
    pub done: bool,
    pub hp_end_abs: i32,
    pub pot_kept: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct MoveRec {
    pub action: u16,
    pub searched: bool,
    pub opts: [u16; MAX_M],
    pub p: [f32; MAX_M],
    pub q: [f32; MAX_M],
    pub legal: [bool; MAX_M],
    pub exact: bool,
}

impl MoveRec {
    fn forced(action: u16) -> MoveRec {
        MoveRec { action, searched: false, opts: [0; MAX_M], p: [0.0; MAX_M], q: [0.0; MAX_M], legal: [false; MAX_M], exact: false }
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
    pub panics: u64,
    pub end_turn: u64,
    pub end_term: u64,
    pub end_cap: u64,
    pub end_stuck: u64,
    pub end_loop: u64,
    pub fight_loops: u64,
    pub lead_branch: u64,
    pub lead_clean: u64,
    pub lead_prefix_steps: u64,
    pub carried: u64,
    pub lead_first_unclean: u64,
    pub cy_step: u64,
    pub cy_legal: u64,
    pub cy_obs: u64,
    pub cy_fork: u64,
    pub cy_main: u64,
    pub cy_endturn: u64,
    pub n_endturn: u64,
    pub cover_actions: u64,
    pub cover_classes: u64,
    pub cover_capped: u64,
    pub ex_triggered: u64,
    pub ex_done: u64,
    pub ex_capped: u64,
    pub ex_changed: u64,
    pub ex_states: u64,
    pub ex_rows: u64,
    pub cy_exact: u64,
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
    Idle,
    Pol(u32),
    Val(u32),
    Done,
}

struct Sim {
    cx: Combat,
    st: SimSt,
    start_turn: i32,
    est: f32,
    steps: u32,
    rng: u64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RootSt {
    Idle,
    Pol(u32),
    Searching,
    Exact,
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
    pot_start: [u16; POT],
    opts: [i32; MAX_M],
    ok: [bool; MAX_M],
    probs: [f32; MAX_M],
    qs: [f32; MAX_M],
    lead: [bool; MAX_M],
    lead_acts: [Vec<u16>; MAX_M],
    carry: Option<(Vec<u16>, f32)>,
    known: [bool; MAX_M],
    kc: usize,
    ks: Vec<u64>,
    todo: Vec<(usize, SimSt)>,
    log: Vec<MoveRec>,
    stats: SearchStats,
    look: Option<Box<LookCache>>,
    ex: Option<Box<Exact>>,
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

struct Out {
    obs: SendPtr<f32>,
    pol_mask: SendPtr<u8>,
    pol_kind: SendPtr<u8>,
    // Play-out moves are sampled with this uniform from the play-out's own stream: job seeds fix every play-out.
    pol_u: SendPtr<f32>,
    val_kind: SendPtr<u8>,
    n_pol: AtomicUsize,
    n_val: AtomicUsize,
    shared: usize,
    used: AtomicUsize,
    ver: u8,
}

impl Out {
    fn val_obs_row(&self, r: usize) -> usize {
        self.shared - 1 - r
    }

    fn take_shared(&self) {
        let u = self.used.fetch_add(1, Ordering::Relaxed);
        assert!(u < self.shared, "shared request buffer too small");
    }
}

struct Inputs<'a> {
    pol: &'a [f32],
    val: &'a [f32],
}

struct Shared<'a> {
    cfg: SearchCfg,
    scen: &'a [(Scenario, ScenarioExtras)],
    worth: &'a [Worth],
    starts: &'a [Option<Combat>],
    jobs: &'a [(u32, u64)],
    next_job: AtomicUsize,
    results: SendPtr<JobResult>,
    logs: SendPtr<Vec<MoveRec>>,
    record: bool,
}

fn slots(cfg: &SearchCfg) -> usize {
    let mk = cfg.m * cfg.k;
    if cfg.futures > 0 {
        mk.min(cfg.futures.max(cfg.m))
    } else {
        mk
    }
}

fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn terminal(cx: &Combat, steps: u32, max_steps: u32, cfg: &SearchCfg) -> Option<(i8, f32)> {
    if cx.missing.is_some() {
        Some((OUTCOME_UNIMPLEMENTED, 0.0))
    } else if crate::looped(cx) {
        Some((OUTCOME_LOSS, cfg.loss))
    } else if cx.overflow != 0 {
        Some((OUTCOME_OVERFLOW, 0.0))
    } else if cx.stage == Stage::Over {
        let me = cx.cr(0);
        match cx.outcome {
            Outcome::Victory => {
                let frac = me.hp as f32 / me.max_hp.max(1) as f32;
                Some((OUTCOME_WIN, cfg.win + cfg.hp_bonus * frac))
            }
            _ => Some((OUTCOME_LOSS, cfg.loss)),
        }
    } else if cfg.turn_cap > 0 && cx.player.turn_number > cfg.turn_cap as i32 {
        Some((OUTCOME_LOSS, cfg.loss))
    } else if steps >= max_steps {
        Some((OUTCOME_TRUNCATED, 0.0))
    } else {
        None
    }
}

pub fn forced_action(cx: &Combat, buf: &ActionBuf) -> Option<Action> {
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

#[allow(clippy::too_many_arguments)]
fn write_row(cx: &mut Combat, buf: &ActionBuf, playable: Option<u16>, obs: SendPtr<f32>, mask: Option<SendPtr<u8>>, row: usize, ver: u8) {
    // SAFETY: `row` is unique (atomic counter) and below capacity; rows hold `obs_size(ver)` / `ACTION_SPACE` entries.
    let osz = obs_size(ver);
    let o = unsafe { std::slice::from_raw_parts_mut(obs.0.add(row * osz), osz) };
    cx.observe_v(o, playable, ver);
    if let Some(m) = mask {
        let m = unsafe { std::slice::from_raw_parts_mut(m.0.add(row * ACTION_SPACE), ACTION_SPACE) };
        m.fill(0);
        for a in buf.iter() {
            m[a.index()] = 1;
        }
    }
    cx.sync_overflow();
}

fn pol_row(out: &Out, sim: bool, cx: &Combat, u: f32) -> usize {
    let kind = sim as u8 | (cx.decision.is_some() as u8) << 1;
    out.take_shared();
    let r = out.n_pol.fetch_add(1, Ordering::Relaxed);
    // SAFETY: row `r` is owned by the caller (unique counter value) and below the capacity.
    unsafe {
        *out.pol_kind.0.add(r) = kind;
        *out.pol_u.0.add(r) = u;
    }
    r
}

fn unit(s: &mut u64) -> f32 {
    ((splitmix(s) >> 40) as f32 + 0.5) / (1u64 << 24) as f32
}

const ROLL_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;

fn val_row(out: &Out, cx: &Combat) -> usize {
    out.take_shared();
    let r = out.n_val.fetch_add(1, Ordering::Relaxed);
    // SAFETY: as in `pol_row`.
    unsafe { *out.val_kind.0.add(r) = (cx.decision.is_some() as u8) << 1 };
    r
}

// Changes whenever a step consumes hidden information: a shared prefix must branch before such a step.
fn hidden_sig(cx: &Combat) -> (i64, u64) {
    let r = &cx.rng;
    let c = r.shuffle.counter as i64
        + r.combat_card_generation.counter as i64
        + r.combat_potion_generation.counter as i64
        + r.combat_card_selection.counter as i64
        + r.combat_energy_costs.counter as i64
        + r.combat_targets.counter as i64
        + r.monster_ai.counter as i64
        + r.niche.counter as i64
        + r.combat_orbs.counter as i64;
    let mut h = 0xcbf29ce484222325u64;
    for &x in cx.player.draw.as_slice() {
        h = (h ^ x as u64).wrapping_mul(0x100000001b3);
    }
    (c, h)
}

fn shows_draw_pile(cx: &Combat) -> bool {
    matches!(&cx.decision, Some(d) if matches!(d.source, sts2sim::state::DecisionSource::Pile(sts2sim::types::PileType::Draw)))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn sim_run(sim: &mut Sim, mut act: Action, cfg: &SearchCfg, w: &Worth, out: &Out, st: &mut SearchStats, mut scratch: Option<&mut Combat>, mut acts: Option<&mut Vec<u16>>) -> Option<Action> {
    loop {
        let sig0 = if let Some(sc) = scratch.as_deref_mut() {
            sc.clone_from(&sim.cx);
            Some(hidden_sig(&sim.cx))
        } else {
            None
        };
        let t0 = tsc();
        let ok = sim.cx.step(act);
        let dt = tsc() - t0;
        st.cy_step += dt;
        if matches!(act, Action::EndTurn) {
            st.cy_endturn += dt;
            st.n_endturn += 1;
        }
        sim.steps += 1;
        st.sim_steps += 1;
        if let Some(sig0) = sig0 {
            if ok && (hidden_sig(&sim.cx) != sig0 || shows_draw_pile(&sim.cx)) {
                sim.steps -= 1;
                return Some(act);
            }
        }
        if let Some(v) = acts.as_deref_mut() {
            v.push(act.index() as u16);
        }
        if !ok {
            st.illegal += 1;
            sim.est += if w.table { w.u[0] } else { cfg.loss };
            sim.st = SimSt::Done;
            return None;
        }
        if let Some((oc, r)) = terminal(&sim.cx, 0, u32::MAX, cfg) {
            sim.est += end_score(&sim.cx, oc, r, w);
            sim.st = SimSt::Done;
            st.end_term += 1;
            st.end_loop += crate::looped(&sim.cx) as u64;
            return None;
        }
        let capped = sim.steps >= cfg.roll_cap;
        let at_leaf = (sim.cx.player.turn_number - sim.start_turn) as i64 >= cfg.leaf_turns as i64;
        if at_leaf || (capped && w.table) {
            let t0 = tsc();
            let row = val_row(out, &sim.cx);
            let buf = ActionBuf::new();
            write_row(&mut sim.cx, &buf, None, out.obs, None, out.val_obs_row(row), out.ver);
            st.cy_obs += tsc() - t0;
            st.value_rows += 1;
            if at_leaf {
                st.end_turn += 1;
            } else {
                st.end_cap += 1;
            }
            sim.st = SimSt::Val(row as u32);
            return None;
        }
        if capped {
            sim.st = SimSt::Done;
            st.end_cap += 1;
            return None;
        }
        let mut buf = ActionBuf::new();
        let mut playable = 0u16;
        let t0 = tsc();
        sim.cx.legal_actions_ex(&mut buf, &mut playable);
        st.cy_legal += tsc() - t0;
        if buf.is_empty() {
            sim.st = SimSt::Done;
            st.end_stuck += 1;
            return None;
        }
        if let Some(a) = forced_action(&sim.cx, &buf) {
            act = a;
            st.forced_sim += 1;
        } else {
            let t0 = tsc();
            let u = unit(&mut sim.rng);
            let row = pol_row(out, true, &sim.cx, u);
            write_row(&mut sim.cx, &buf, Some(playable), out.obs, Some(out.pol_mask), row, out.ver);
            st.cy_obs += tsc() - t0;
            st.policy_rows += 1;
            sim.st = SimSt::Pol(row as u32);
            return None;
        }
    }
}

fn idle_sim(cx: &Combat) -> Sim {
    Sim { cx: cx.clone(), st: SimSt::Idle, start_turn: 0, est: 0.0, steps: 0, rng: 0 }
}

#[inline]
fn det_future(cx: &mut Combat, cfg: &SearchCfg, ks: &[u64], f: usize) {
    if cfg.clairvoyant {
        return;
    }
    if cfg.strat {
        cx.determinize_strat(ks[0], ks[f], f, ks.len());
    } else {
        cx.determinize(ks[f]);
    }
}

impl Block {
    fn new(sc: &Scenario, ex: &ScenarioExtras, cfg: &SearchCfg) -> Result<Block, EnvError> {
        let main = Combat::try_new_with(sc, ex)?;
        let eager = if cfg.cover || cfg.futures > 0 { 0 } else { cfg.m * cfg.k };
        let sims = (0..eager).map(|_| idle_sim(&main)).collect();
        Ok(Block {
            main,
            sims,
            st: RootSt::Idle,
            job: NONE,
            steps: 0,
            hp0: 0.0,
            scen: 0,
            rng: 0,
            pot_start: [u16::MAX; POT],
            opts: [0; MAX_M],
            ok: [false; MAX_M],
            probs: [0.0; MAX_M],
            qs: [0.0; MAX_M],
            lead: [false; MAX_M],
            lead_acts: Default::default(),
            carry: None,
            known: [false; MAX_M],
            kc: cfg.k,
            ks: Vec::new(),
            todo: Vec::new(),
            log: Vec::new(),
            stats: SearchStats::default(),
            look: Some(Box::new(LookCache::new(LOOK_CACHE_ENTRIES))),
            ex: None,
        })
    }

    fn start_job(&mut self, sh: &Shared) -> bool {
        let j = sh.next_job.fetch_add(1, Ordering::Relaxed);
        if j >= sh.jobs.len() {
            self.st = RootSt::Idle;
            self.job = NONE;
            return false;
        }
        let (si, seed) = sh.jobs[j];
        let (sc, ex) = &sh.scen[si as usize];
        if let Some(Some(c)) = sh.starts.get(si as usize) {
            self.main.clone_from(c);
        } else if self.main.reset_validated(sc, ex, seed, RngSet::from_run_seed_fast(seed)).is_err() {
            self.main.overflow |= sts2sim::state::ov::SCENARIO;
        }
        self.job = j as u32;
        self.carry = None;
        self.steps = 0;
        self.scen = si;
        let me = self.main.cr(0);
        self.hp0 = me.hp as f32 / me.max_hp.max(1) as f32;
        self.pot_start = pot_ids(&self.main);
        self.rng = seed ^ 0xA5A5_5A5A_1234_8765;
        true
    }

    fn record(&mut self, sh: &Shared, outcome: i8) {
        let me = self.main.cr(0);
        let end_frac = if outcome == OUTCOME_WIN { me.hp as f32 / me.max_hp.max(1) as f32 } else { 0.0 };
        let hp_end_abs = if outcome == OUTCOME_WIN { me.hp } else { 0 };
        let now = pot_ids(&self.main);
        let pot_kept = (0..POT).filter(|&k| self.pot_start[k] != u16::MAX && now[k] == self.pot_start[k]).fold(0u8, |b, k| b | 1 << k);
        let r = JobResult { scen: self.scen, outcome, hp_lost: self.hp0 - end_frac, hp_end: end_frac, len: self.steps, done: true, hp_end_abs, pot_kept };
        // SAFETY: every job index is taken by exactly one block (atomic counter), so this entry is written once and by this task only.
        unsafe { *sh.results.0.add(self.job as usize) = r };
        if sh.record {
            // SAFETY: as above, one writer per job.
            unsafe { *sh.logs.0.add(self.job as usize) = std::mem::take(&mut self.log) };
        }
    }

    fn main_act(&mut self, act: Action, sh: &Shared, rec: Option<MoveRec>) -> bool {
        if sh.record {
            self.log.push(rec.unwrap_or_else(|| MoveRec::forced(act.index() as u16)));
        }
        if let Some((v, _)) = self.carry.as_mut() {
            if v.first() == Some(&(act.index() as u16)) {
                v.remove(0);
                if v.is_empty() {
                    self.carry = None;
                }
            } else {
                self.carry = None;
            }
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
            self.stats.fight_loops += crate::looped(&self.main) as u64;
            self.record(sh, oc);
            return !self.start_job(sh);
        }
        false
    }

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
                let looped = crate::looped(&self.main);
                self.stats.fight_loops += looped as u64;
                self.record(sh, if looped { OUTCOME_LOSS } else { OUTCOME_TRUNCATED });
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
                let row = pol_row(out, false, &self.main, 0.5);
                write_row(&mut self.main, &buf, Some(playable), out.obs, Some(out.pol_mask), row, out.ver);
                self.stats.policy_rows += 1;
                self.st = RootSt::Pol(row as u32);
                return;
            }
        }
    }

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

    fn on_root_policy(&mut self, inp: &Inputs, row: usize, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        let m = cfg.m;
        let stride = 2 * m + 1;
        let r = &inp.pol[row * stride..(row + 1) * stride];
        let n_legal = if cfg.cover {
            self.cover_opts(r, m)
        } else {
            let mut n = 0;
            for j in 0..m {
                self.opts[j] = r[j] as i32;
                let p = r[m + j];
                self.probs[j] = p;
                self.ok[j] = p > 0.0;
                n += self.ok[j] as usize;
            }
            n
        };
        self.stats.root_decisions += 1;
        if n_legal <= 1 || self.probs[0] >= cfg.conf {
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
        let k = if cfg.futures > 0 { (cfg.futures / n_legal).clamp(1, cfg.k) } else { cfg.k };
        self.kc = k;
        let need = (0..m).filter(|&j| self.ok[j]).last().map_or(0, |j| j + 1) * k;
        while self.sims.len() < need {
            self.sims.push(idle_sim(&self.main));
        }
        self.ks.clear();
        for _ in 0..k {
            let x = splitmix(&mut self.rng);
            self.ks.push(x);
        }
        let turn = self.main.player.turn_number;
        let lead = cfg.lead && k >= 2;
        let w = &sh.worth[self.scen as usize];
        for j in 0..m {
            self.lead[j] = false;
            self.known[j] = false;
            self.lead_acts[j].clear();
            if cfg.carry && self.ok[j] {
                if let Some((v, q)) = &self.carry {
                    if v.first() == Some(&(self.opts[j] as u16)) {
                        self.known[j] = true;
                        self.qs[j] = *q;
                        self.stats.carried += 1;
                    }
                }
            }
            if !self.ok[j] || self.known[j] {
                for s in self.sims.iter_mut().skip(j * k).take(k) {
                    s.st = SimSt::Idle;
                }
                continue;
            }
            let Some(first) = Action::from_index(self.opts[j] as usize) else {
                for kk in 0..k {
                    let sim = &mut self.sims[j * k + kk];
                    sim.st = SimSt::Done;
                    sim.est = cfg.loss;
                }
                continue;
            };
            self.start_option(j, first, lead, cfg, w, out, turn);
        }
        self.st = RootSt::Searching;
        self.try_finish_search(sh, out);
    }

    fn cover_opts(&mut self, r: &[f32], m: usize) -> usize {
        let mut buf = ActionBuf::new();
        self.main.legal_actions(&mut buf);
        let ranked = (0..m).filter(|&j| r[m + j] > 0.0).filter_map(|j| Action::from_index(r[j] as usize).map(|a| (a, r[m + j])));
        let mut cls: ArrayVec<(Action, f32), 256> = ArrayVec::new();
        // Discarding is never a candidate: live play never discards a potion in combat.
        for (a, p) in ranked.chain(buf.iter().map(|&a| (a, 0.0))) {
            if matches!(a, Action::DiscardPotion { .. }) || !buf.contains(a) {
                continue;
            }
            match cls.as_mut_slice().iter_mut().find(|c| self.main.interchangeable(c.0, a)) {
                Some(c) => c.1 += p,
                None => cls.push((a, p)),
            }
        }
        cls.as_mut_slice().sort_by(|x, y| y.1.total_cmp(&x.1));
        self.stats.cover_actions += buf.iter().filter(|a| !matches!(a, Action::DiscardPotion { .. })).count() as u64;
        self.stats.cover_classes += cls.len() as u64;
        self.stats.cover_capped += (cls.len() > m) as u64;
        let n = cls.len().min(m);
        for j in 0..m {
            let (a, p) = if j < n { (cls[j].0.index() as i32, cls[j].1) } else { (0, 0.0) };
            self.opts[j] = a;
            self.probs[j] = p;
            self.ok[j] = j < n;
        }
        n
    }

    #[allow(clippy::too_many_arguments)]
    fn start_option(&mut self, j: usize, first: Action, lead: bool, cfg: &SearchCfg, w: &Worth, out: &Out, turn: i32) {
        let (base, c) = (j * self.kc, self.kc);
        let n_now = if lead { 1 } else { c };
        for kk in 0..c {
            let sim = &mut self.sims[base + kk];
            if kk >= n_now {
                sim.st = SimSt::Idle;
                continue;
            }
            let t0 = tsc();
            sim.cx.clone_from(&self.main);
            det_future(&mut sim.cx, cfg, &self.ks, kk);
            self.stats.cy_fork += tsc() - t0;
            sim.start_turn = turn;
            sim.est = 0.0;
            sim.steps = 0;
            sim.rng = self.ks[kk] ^ ROLL_SALT;
            self.stats.forks += 1;
        }
        if lead {
            self.lead[j] = true;
            self.lead_run(j, first, cfg, w, out);
        } else {
            for kk in 0..c {
                sim_run(&mut self.sims[base + kk], first, cfg, w, out, &mut self.stats, None, None);
            }
        }
    }

    fn lead_run(&mut self, j: usize, act: Action, cfg: &SearchCfg, w: &Worth, out: &Out) {
        let (base, k) = (j * self.kc, self.kc);
        let branch = {
            let (head, tail) = self.sims.split_at_mut(base + k - 1);
            let scratch = &mut tail[0];
            let lead = &mut head[base];
            sim_run(lead, act, cfg, w, out, &mut self.stats, Some(&mut scratch.cx), Some(&mut self.lead_acts[j]))
        };
        match branch {
            Some(a) => {
                self.stats.lead_branch += 1;
                self.lead_acts[j].push(a.index() as u16);
                self.stats.lead_prefix_steps += self.sims[base].steps as u64;
                self.stats.lead_first_unclean += (self.sims[base].steps == 0) as u64;
                let (est, steps, start_turn) = {
                    let l = &self.sims[base];
                    (l.est, l.steps, l.start_turn)
                };
                for kk in 0..k - 1 {
                    let (head, tail) = self.sims.split_at_mut(base + k - 1);
                    head[base + kk].cx.clone_from(&tail[0].cx);
                }
                for kk in 0..k {
                    let sim = &mut self.sims[base + kk];
                    let t0 = tsc();
                    det_future(&mut sim.cx, cfg, &self.ks, kk);
                    self.stats.cy_fork += tsc() - t0;
                    self.stats.forks += 1;
                    sim.est = est;
                    sim.steps = steps;
                    sim.start_turn = start_turn;
                    sim.rng = self.ks[kk] ^ ROLL_SALT ^ (steps as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    sim_run(sim, a, cfg, w, out, &mut self.stats, None, None);
                }
                self.lead[j] = false;
            }
            None => {
                if self.sims[base].st == SimSt::Done {
                    self.lead_finish(j, k);
                }
            }
        }
    }

    fn lead_finish(&mut self, j: usize, k: usize) {
        let base = j * k;
        let est = self.sims[base].est;
        for kk in 1..k {
            let sim = &mut self.sims[base + kk];
            sim.est = est;
            sim.st = SimSt::Done;
        }
        self.lead[j] = false;
        self.stats.lead_clean += 1;
    }

    fn try_finish_search(&mut self, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        if self.sims.iter().any(|s| matches!(s.st, SimSt::Pol(_) | SimSt::Val(_))) {
            return;
        }
        let (m, k) = (cfg.m, self.kc);
        let mut best = 0usize;
        let mut best_q = f32::NEG_INFINITY;
        let mut q0 = 0.0f32;
        for j in 0..m {
            if !self.ok[j] {
                continue;
            }
            let q = if self.known[j] { self.qs[j] } else { self.sims[j * k..(j + 1) * k].iter().map(|s| s.est).sum::<f32>() / k as f32 };
            self.qs[j] = q;
            if j == 0 {
                q0 = q;
            }
            if q > best_q {
                best_q = q;
                best = j;
            }
        }
        if best != 0 && best_q - q0 <= 0.0 {
            best = 0;
        }
        for s in self.sims.iter_mut() {
            s.st = SimSt::Idle;
        }
        if cfg.exact.on && self.blind(best, &sh.cfg, &sh.worth[self.scen as usize]) {
            self.ex_start(best, sh, out);
            return;
        }
        self.play_searched(best, sh, out);
    }

    fn play_searched(&mut self, best: usize, sh: &Shared, out: &Out) {
        let rec = self.move_rec(self.opts[best], true, sh);
        if sh.cfg.carry && !self.known[best] {
            self.carry = if self.lead_acts[best].first() == Some(&(self.opts[best] as u16)) { Some((std::mem::take(&mut self.lead_acts[best]), self.qs[best])) } else { None };
        }
        self.play_root(Action::from_index(self.opts[best] as usize), rec, sh, out);
    }

    fn play_root(&mut self, act: Option<Action>, rec: Option<MoveRec>, sh: &Shared, out: &Out) {
        let finished = match act {
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

    fn recover(&mut self, sh: &Shared, out: &Out) {
        eprintln!("search engine: panic while playing job {} (scenario {}, seed {:#x}, {} moves played); the fight is aborted", self.job, self.scen, sh.jobs.get(self.job as usize).map(|j| j.1).unwrap_or(0), self.steps);
        if sh.record {
            eprintln!("  moves so far: {:?}", self.log.iter().map(|m| m.action).collect::<Vec<_>>());
        }
        self.stats.panics += 1;
        for s in self.sims.iter_mut() {
            s.st = SimSt::Idle;
        }
        self.lead = [false; MAX_M];
        self.ex = None;
        if self.job != NONE {
            self.record(sh, OUTCOME_OVERFLOW);
            self.job = NONE;
        }
        self.st = RootSt::Idle;
        if self.start_job(sh) {
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
                    self.root_next(sh, out);
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
                let w = &sh.worth[self.scen as usize];
                let (vw, k) = (cfg.val_w, self.kc);
                let mut todo = std::mem::take(&mut self.todo);
                todo.clear();
                todo.extend((0..self.sims.len()).filter(|&i| matches!(self.sims[i].st, SimSt::Pol(_) | SimSt::Val(_))).map(|i| (i, self.sims[i].st)));
                for &(idx, waiting) in todo.iter() {
                    match waiting {
                        SimSt::Pol(row) => {
                            let r = row as usize * stride;
                            let j = idx / k;
                            let is_lead = self.lead[j] && idx == j * k;
                            let a = inp.pol[r + 2 * m] as i32;
                            match Action::from_index(a as usize) {
                                Some(act) => {
                                    if is_lead {
                                        self.lead_run(j, act, &cfg, w, out);
                                    } else {
                                        sim_run(&mut self.sims[idx], act, &cfg, w, out, &mut self.stats, None, None);
                                    }
                                }
                                None => {
                                    self.stats.illegal += 1;
                                    self.sims[idx].st = SimSt::Done;
                                }
                            }
                        }
                        SimSt::Val(row) => {
                            let sim = &mut self.sims[idx];
                            let r = row as usize * vw;
                            sim.est += leaf_value(&inp.val[r..r + vw], sim.cx.cr(0).max_hp, &cfg, w);
                            sim.st = SimSt::Done;
                            let j = idx / k;
                            if self.lead[j] && idx == j * k {
                                self.lead_finish(j, k);
                            }
                        }
                        _ => {}
                    }
                }
                self.todo = todo;
                self.try_finish_search(sh, out);
            }
            RootSt::Exact => {
                if let Some(inp) = inp {
                    self.ex_answer(inp, sh);
                    self.ex_continue(sh, out);
                }
            }
        }
    }
}

const MAX_DEPTH: usize = 64;
const EXACT_SALT: u64 = 0x3C6E_F372_FE94_F82B;
const X_LEAF: u8 = 0;
const X_INNER: u8 = 1;
const X_CHANCE: u8 = 2;

// Leaf: v sums n samples. Inner: max over kids. Chance (a move that reveals hidden information): mean over the determinized worlds.
struct XNode {
    kind: u8,
    v: f32,
    n: u32,
    kids: Vec<u32>,
}

struct XFrame {
    node: u32,
    world: u32,
    cx: Combat,
    acts: Vec<Action>,
    next: usize,
}

#[derive(Default)]
struct Exact {
    nodes: Vec<XNode>,
    index: HashMap<u64, u32>,
    stack: Vec<XFrame>,
    pending: Vec<(u32, u32, i32)>,
    root: Vec<(Action, u32)>,
    ks: Vec<u64>,
    obs: Vec<f32>,
    turn: i32,
    best: usize,
}

struct Fx(u64);

impl Fx {
    fn add(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
}

fn det_world(ks: &[u64], cx: &mut Combat, cfg: &SearchCfg, d: usize) {
    if cfg.clairvoyant {
        return;
    }
    if cfg.strat {
        cx.determinize_strat(ks[0], ks[d], d, ks.len());
    } else {
        cx.determinize(ks[d]);
    }
}

impl Exact {
    fn reset(&mut self) {
        self.nodes.clear();
        self.index.clear();
        self.stack.clear();
        self.pending.clear();
        self.root.clear();
        self.ks.clear();
    }

    fn leaf(&mut self, v: f32, n: u32) -> u32 {
        self.nodes.push(XNode { kind: X_LEAF, v, n, kids: Vec::new() });
        (self.nodes.len() - 1) as u32
    }

    fn score(&mut self, node: u32, cx: &mut Combat, cfg: &SearchCfg, w: &Worth, out: &Out, st: &mut SearchStats) {
        if let Some((oc, r)) = terminal(cx, 0, u32::MAX, cfg) {
            self.nodes[node as usize].v += end_score(cx, oc, r, w);
            return;
        }
        let row = val_row(out, cx);
        write_row(cx, &ActionBuf::new(), None, out.obs, None, out.val_obs_row(row), out.ver);
        self.pending.push((row as u32, node, cx.cr(0).max_hp));
        st.value_rows += 1;
        st.ex_rows += 1;
    }

    fn end_turn(&mut self, node: u32, mut cx: Combat, cfg: &SearchCfg, w: &Worth, out: &Out, st: &mut SearchStats) {
        if cx.step(Action::EndTurn) {
            self.score(node, &mut cx, cfg, w, out, st);
        } else {
            st.illegal += 1;
            self.nodes[node as usize].v += if w.table { w.u[0] } else { cfg.loss };
        }
    }

    fn key(&mut self, cx: &Combat, world: u32, ver: u8) -> u64 {
        cx.observe_v(&mut self.obs, None, ver);
        let mut h = Fx(world as u64);
        for x in self.obs.iter() {
            h.add(x.to_bits() as u64);
        }
        let (c, d) = hidden_sig(cx);
        h.add(c as u64);
        h.add(d);
        let t = &cx.hist;
        for x in [t.cards_played_this_turn, t.attacks_played_this_turn, t.skills_played_this_turn, t.cards_exhausted_this_turn, t.cards_finished_this_turn, t.attacks_finished_this_turn, t.skills_finished_this_turn, t.shivs_finished_this_turn] {
            h.add(x as u16 as u64);
        }
        let mut fin = 0u64;
        for (w, &bits) in t.finished_cards.iter().enumerate() {
            let mut b = bits;
            while b != 0 {
                let k = &cx.cards[w * 64 + b.trailing_zeros() as usize];
                b &= b - 1;
                let mut s = k.id as u64 | (k.pile as u64) << 16 | (k.upgrade as u64) << 24 | (k.enchant as u64) << 32 | (k.flags as u64) << 40;
                fin = fin.wrapping_add(splitmix(&mut s));
            }
        }
        h.add(fin);
        for e in t.play_amounts.as_slice() {
            h.add(e.uid as u64 | (e.card as u64) << 16 | (e.amount as u32 as u64) << 32);
        }
        let l = &cx.hist_log;
        for x in l.total {
            h.add(x as u64);
        }
        h.add(l.ethereal_finished as u64 | (l.player_hits_taken as u64) << 16 | (l.generated_by_player as u64) << 32 | (l.lightning_channeled as u64) << 48);
        if let Some(dc) = &cx.decision {
            h.add(0xDEC0 | (dc.min as u64) << 16 | (dc.max as u64) << 24);
            for &x in dc.selected.as_slice() {
                h.add(x as u64);
            }
            h.add(u64::MAX);
            for &x in dc.cands.as_slice() {
                h.add(x as u64);
            }
        }
        h.0
    }

    // None: over the state cap or the depth cap.
    fn add(&mut self, mut cx: Combat, world: u32, cfg: &SearchCfg, w: &Worth, out: &Out, st: &mut SearchStats) -> Option<u32> {
        if terminal(&cx, 0, u32::MAX, cfg).is_some() || cx.player.turn_number != self.turn {
            let id = self.leaf(0.0, 1);
            self.score(id, &mut cx, cfg, w, out, st);
            return Some(id);
        }
        let k = self.key(&cx, world, out.ver);
        if let Some(&id) = self.index.get(&k) {
            return Some(id);
        }
        if self.index.len() >= cfg.exact.cap || self.stack.len() >= MAX_DEPTH {
            return None;
        }
        let mut buf = ActionBuf::new();
        cx.legal_actions(&mut buf);
        let view = cx.decision.as_ref().map(|d| (cx.decision_view(d), d));
        // A selection is enumerated as a set: picks only add candidates, in increasing order.
        let adds = |idx: u8| {
            view.as_ref().is_none_or(|(v, d)| v.get(idx as usize).is_some_and(|i| d.selected.len() < d.max as usize && d.selected.iter().all(|&s| s < i)))
        };
        let mut acts: Vec<Action> = Vec::new();
        for &a in buf.iter() {
            let skip = match a {
                Action::DiscardPotion { .. } => true,
                Action::UsePotion { .. } => !cfg.exact.potions,
                Action::Pick { idx } => !adds(idx),
                _ => false,
            };
            if !skip && !acts.iter().any(|&b| cx.interchangeable(b, a)) {
                acts.push(a);
            }
        }
        let id = self.nodes.len() as u32;
        self.index.insert(k, id);
        st.ex_states += 1;
        if acts.is_empty() && !buf.is_empty() {
            self.nodes.push(XNode { kind: X_INNER, v: f32::NEG_INFINITY, n: 0, kids: Vec::new() });
        } else if acts.is_empty() {
            self.nodes.push(XNode { kind: X_LEAF, v: 0.0, n: 1, kids: Vec::new() });
        } else {
            self.nodes.push(XNode { kind: X_INNER, v: f32::NEG_INFINITY, n: 0, kids: Vec::new() });
            self.stack.push(XFrame { node: id, world, cx, acts, next: 0 });
        }
        Some(id)
    }

    // Some(true): every line enumerated; Some(false): paused at the row budget; None: capped.
    fn run(&mut self, cfg: &SearchCfg, w: &Worth, out: &Out, st: &mut SearchStats, budget: usize) -> Option<bool> {
        let dets = self.ks.len();
        loop {
            let Some(f) = self.stack.last_mut() else { return Some(true) };
            let chance = self.nodes[f.node as usize].kind == X_CHANCE;
            if f.next == if chance { dets } else { f.acts.len() } {
                self.stack.pop();
                continue;
            }
            if self.pending.len() + dets > budget {
                return Some(false);
            }
            let (node, world, i) = (f.node, f.world, f.next);
            f.next += 1;
            let a = f.acts[if chance { 0 } else { i }];
            let mut c = f.cx.clone();
            if chance {
                det_world(&self.ks, &mut c, cfg, i);
                if c.step(a) {
                    let kid = self.add(c, i as u32, cfg, w, out, st)?;
                    self.nodes[node as usize].kids.push(kid);
                }
                continue;
            }
            let kid = if matches!(a, Action::EndTurn) {
                let leaf = self.leaf(0.0, if world == NONE { dets as u32 } else { 1 });
                if world == NONE {
                    for d in 0..dets {
                        let mut e = c.clone();
                        det_world(&self.ks, &mut e, cfg, d);
                        self.end_turn(leaf, e, cfg, w, out, st);
                    }
                } else {
                    self.end_turn(leaf, c, cfg, w, out, st);
                }
                leaf
            } else {
                let sig = hidden_sig(&c);
                if !c.step(a) {
                    continue;
                }
                if world == NONE && (hidden_sig(&c) != sig || shows_draw_pile(&c)) {
                    let id = self.nodes.len() as u32;
                    self.nodes.push(XNode { kind: X_CHANCE, v: f32::NEG_INFINITY, n: 0, kids: Vec::new() });
                    let pre = self.stack.last().expect("the expanding frame").cx.clone();
                    self.stack.push(XFrame { node: id, world: NONE, cx: pre, acts: vec![a], next: 0 });
                    id
                } else {
                    self.add(c, world, cfg, w, out, st)?
                }
            };
            self.nodes[node as usize].kids.push(kid);
            if node == 0 {
                self.root.push((a, kid));
            }
        }
    }

    fn values(&mut self) {
        for n in self.nodes.iter_mut().filter(|n| n.kind == X_LEAF) {
            n.v /= n.n.max(1) as f32;
        }
        for _ in 0..=self.nodes.len() {
            let mut changed = false;
            for i in (0..self.nodes.len()).rev() {
                let n = &self.nodes[i];
                if n.kind == X_LEAF || n.kids.is_empty() {
                    continue;
                }
                let it = n.kids.iter().map(|&k| self.nodes[k as usize].v);
                let v = if n.kind == X_INNER { it.fold(f32::NEG_INFINITY, f32::max) } else { it.sum::<f32>() / n.kids.len() as f32 };
                if v > self.nodes[i].v {
                    self.nodes[i].v = v;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
}

impl Block {
    fn blind(&self, best: usize, cfg: &SearchCfg, w: &Worth) -> bool {
        let (b, mut second, mut n) = (self.qs[best], f32::NEG_INFINITY, 0);
        for j in (0..cfg.m).filter(|&j| self.ok[j]) {
            n += 1;
            if j != best {
                second = second.max(self.qs[j]);
            }
        }
        let (lo, scale) = if w.table {
            let mx = w.u.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            (w.u[0], (mx - w.u[0]) / (cfg.win + cfg.hp_bonus - cfg.loss))
        } else {
            (cfg.loss, 1.0)
        };
        n >= 2 && (b <= lo + (cfg.exact.loss - cfg.loss) * scale || b - second <= cfg.exact.tie * scale)
    }

    fn ex_start(&mut self, best: usize, sh: &Shared, out: &Out) {
        let t0 = tsc();
        let cfg = &sh.cfg;
        self.stats.ex_triggered += 1;
        let mut ex = self.ex.take().unwrap_or_default();
        ex.reset();
        let mut s = self.ks.first().copied().unwrap_or(self.rng) ^ EXACT_SALT;
        let dets = cfg.exact.dets.clamp(1, slots(cfg));
        ex.ks.extend((0..dets).map(|_| splitmix(&mut s)));
        ex.obs.resize(obs_size(out.ver), 0.0);
        ex.turn = self.main.player.turn_number;
        ex.best = best;
        let root = self.main.clone();
        let ok = ex.add(root, NONE, cfg, &sh.worth[self.scen as usize], out, &mut self.stats) == Some(0);
        self.stats.cy_exact += tsc() - t0;
        self.ex = Some(ex);
        if !ok {
            self.stats.ex_capped += 1;
            self.play_searched(best, sh, out);
            return;
        }
        self.st = RootSt::Exact;
        self.ex_continue(sh, out);
    }

    fn ex_answer(&mut self, inp: &Inputs, sh: &Shared) {
        let Some(ex) = self.ex.as_mut() else { return };
        let (vw, w) = (sh.cfg.val_w, &sh.worth[self.scen as usize]);
        for &(row, node, max_hp) in ex.pending.iter() {
            let r = row as usize * vw;
            ex.nodes[node as usize].v += leaf_value(&inp.val[r..r + vw], max_hp, &sh.cfg, w);
        }
        ex.pending.clear();
    }

    fn ex_continue(&mut self, sh: &Shared, out: &Out) {
        let t0 = tsc();
        let cfg = &sh.cfg;
        let mut ex = self.ex.take().expect("exact search state");
        let r = ex.run(cfg, &sh.worth[self.scen as usize], out, &mut self.stats, slots(cfg));
        if r == Some(false) || (r == Some(true) && !ex.pending.is_empty()) {
            self.stats.cy_exact += tsc() - t0;
            self.ex = Some(ex);
            return;
        }
        let best = ex.best;
        let pick = if r.is_some() { self.ex_pick(&mut ex, best, cfg.m) } else { None };
        ex.pending.clear();
        self.stats.cy_exact += tsc() - t0;
        self.ex = Some(ex);
        let Some((a, v, rank)) = pick else {
            self.stats.ex_capped += 1;
            self.play_searched(best, sh, out);
            return;
        };
        self.stats.ex_done += 1;
        self.stats.ex_changed += (rank != 0) as u64;
        if rank > MAX_M {
            let j = (0..cfg.m).find(|&j| !self.ok[j]).unwrap_or(cfg.m - 1);
            self.opts[j] = a.index() as i32;
            self.ok[j] = true;
            self.probs[j] = 0.0;
            self.qs[j] = v;
        }
        let rec = self.move_rec(a.index() as i32, true, sh).map(|mut r| {
            r.exact = true;
            r
        });
        self.carry = None;
        self.play_root(Some(a), rec, sh, out);
    }

    // The best first move (ties: the searched choice, then the candidates' order); writes each candidate's exact value into `qs`.
    fn ex_pick(&mut self, ex: &mut Exact, best: usize, m: usize) -> Option<(Action, f32, usize)> {
        ex.values();
        let main = &self.main;
        let opt = |j: usize| if self.ok[j] { Action::from_index(self.opts[j] as usize) } else { None };
        let same = |j: usize, a: Action| opt(j).is_some_and(|o| main.interchangeable(o, a));
        let rank = |a: Action| if same(best, a) { 0 } else { (0..m).find(|&j| same(j, a)).map_or(MAX_M + 1, |j| j + 1) };
        let mut pick: Option<(Action, f32, usize)> = None;
        for &(a, k) in ex.root.iter() {
            let (v, r) = (ex.nodes[k as usize].v, rank(a));
            if pick.is_none_or(|(_, pv, pr)| v > pv || (v == pv && r < pr)) {
                pick = Some((a, v, r));
            }
        }
        let qs: Vec<f32> = (0..m).map(|j| ex.root.iter().find(|&&(a, _)| same(j, a)).map_or(f32::NAN, |&(_, k)| ex.nodes[k as usize].v)).collect();
        for (j, q) in qs.into_iter().enumerate() {
            if self.ok[j] {
                self.qs[j] = q;
            }
        }
        pick
    }
}

pub struct SearchEngine {
    cfg: SearchCfg,
    scen: Vec<(Scenario, ScenarioExtras)>,
    worth: Vec<Worth>,
    starts: Vec<Option<Combat>>,
    jobs: Vec<(u32, u64)>,
    blocks: Vec<Block>,
    results: Vec<JobResult>,
    logs: Vec<Vec<MoveRec>>,
    record: bool,
    next_job: usize,
    pool: rayon::ThreadPool,
    started: bool,
    obs_version: u8,
}

impl SearchEngine {
    pub fn new(scen: Vec<(Scenario, ScenarioExtras)>, jobs: Vec<(u32, u64)>, n_roots: usize, cfg: SearchCfg, threads: usize, record: bool) -> Result<SearchEngine, EnvError> {
        Self::new_with_starts(scen, Vec::new(), jobs, n_roots, cfg, threads, record)
    }

    pub fn new_with_starts(scen: Vec<(Scenario, ScenarioExtras)>, starts: Vec<Option<Combat>>, jobs: Vec<(u32, u64)>, n_roots: usize, cfg: SearchCfg, threads: usize, record: bool) -> Result<SearchEngine, EnvError> {
        if scen.is_empty() || cfg.m == 0 || cfg.m > MAX_M || cfg.k == 0 || !(cfg.val_w == 1 || cfg.val_w == HEAD_NC) {
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
        let blocks: Result<Vec<Block>, EnvError> = pool.install(|| (0..n).into_par_iter().map(|_| Block::new(&scen[0].0, &scen[0].1, &cfg)).collect());
        let results = vec![JobResult::default(); jobs.len()];
        let logs = if record { vec![Vec::new(); jobs.len()] } else { Vec::new() };
        let worth = vec![Worth::linear(); scen.len()];
        Ok(SearchEngine { cfg, scen, worth, starts, jobs, blocks: blocks?, results, logs, record, next_job: 0, pool, started: false, obs_version: obs_version() })
    }

    pub fn set_worth(&mut self, worth: Vec<Worth>) -> Result<(), EnvError> {
        if worth.len() != self.scen.len() {
            return Err(EnvError::Buffer("one worth per scenario"));
        }
        if self.started {
            return Err(EnvError::Buffer("set_worth after the first advance"));
        }
        if self.cfg.val_w == 1 && worth.iter().any(|w| w.table) {
            return Err(EnvError::Buffer("a worth table needs the outcome head's value rows (val_w > 1)"));
        }
        self.worth = worth;
        Ok(())
    }

    pub fn obs_version(&self) -> u8 {
        self.obs_version
    }

    pub fn obs_size(&self) -> usize {
        obs_size(self.obs_version)
    }

    pub fn set_obs_version(&mut self, version: u8) -> Result<(), EnvError> {
        if obs_size(version) == 0 || self.started {
            return Err(EnvError::Buffer("unknown observation version, or the search has started"));
        }
        self.obs_version = version;
        Ok(())
    }

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
            t.panics += s.panics;
            t.end_turn += s.end_turn;
            t.end_term += s.end_term;
            t.end_cap += s.end_cap;
            t.end_stuck += s.end_stuck;
            t.end_loop += s.end_loop;
            t.fight_loops += s.fight_loops;
            t.lead_branch += s.lead_branch;
            t.lead_clean += s.lead_clean;
            t.lead_prefix_steps += s.lead_prefix_steps;
            t.carried += s.carried;
            t.lead_first_unclean += s.lead_first_unclean;
            t.cy_step += s.cy_step;
            t.cy_legal += s.cy_legal;
            t.cy_obs += s.cy_obs;
            t.cy_fork += s.cy_fork;
            t.cy_main += s.cy_main;
            t.cy_endturn += s.cy_endturn;
            t.n_endturn += s.n_endturn;
            t.cover_actions += s.cover_actions;
            t.cover_classes += s.cover_classes;
            t.cover_capped += s.cover_capped;
            t.ex_triggered += s.ex_triggered;
            t.ex_done += s.ex_done;
            t.ex_capped += s.ex_capped;
            t.ex_changed += s.ex_changed;
            t.ex_states += s.ex_states;
            t.ex_rows += s.ex_rows;
            t.cy_exact += s.cy_exact;
        }
        t
    }

    pub fn finished(&self) -> bool {
        self.started && self.blocks.iter().all(|b| b.st == RootSt::Idle)
    }

    pub fn shared_rows(&self) -> usize {
        self.blocks.len() * (slots(&self.cfg) + 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn advance_shared(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, obs: &mut [f32], mask: &mut [u8], pol_kind: &mut [u8], pol_u: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        let cap = self.shared_rows();
        if obs.len() < cap * self.obs_size() || mask.len() < cap * ACTION_SPACE || pol_kind.len() < cap || pol_u.len() < cap || val_kind.len() < cap {
            return Err(EnvError::Buffer("request buffers too small, see SearchEngine::shared_rows"));
        }
        let out = Out {
            obs: SendPtr(obs.as_mut_ptr()),
            pol_mask: SendPtr(mask.as_mut_ptr()),
            pol_kind: SendPtr(pol_kind.as_mut_ptr()),
            pol_u: SendPtr(pol_u.as_mut_ptr()),
            val_kind: SendPtr(val_kind.as_mut_ptr()),
            n_pol: AtomicUsize::new(0),
            n_val: AtomicUsize::new(0),
            shared: cap,
            used: AtomicUsize::new(0),
            ver: self.obs_version,
        };
        if let Some(v) = val {
            if v.len() % self.cfg.val_w != 0 {
                return Err(EnvError::Buffer("value answers are not a whole number of rows of val_w floats"));
            }
        }
        let first = !self.started;
        if !first && (pol.is_none() || val.is_none()) {
            return Err(EnvError::Buffer("answers missing"));
        }
        self.started = true;
        let sh = Shared { cfg: self.cfg, scen: &self.scen, worth: &self.worth, starts: &self.starts, jobs: &self.jobs, next_job: AtomicUsize::new(self.next_job), results: SendPtr(self.results.as_mut_ptr()), logs: SendPtr(self.logs.as_mut_ptr()), record: self.record };
        let inp = if first { None } else { Some(Inputs { pol: pol.unwrap(), val: val.unwrap() }) };
        let blocks = &mut self.blocks;
        self.pool.install(|| {
            blocks.par_iter_mut().for_each(|b| {
                let mut look = b.look.take().expect("the root's look-ahead cache");
                with_look_cache(&mut look, || {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.advance(inp.as_ref(), &sh, &out))).is_err() {
                        b.recover(&sh, &out);
                    }
                });
                b.look = Some(look);
            });
        });
        self.next_job = sh.next_job.load(Ordering::Relaxed).min(self.jobs.len());
        Ok((out.n_pol.load(Ordering::Relaxed), out.n_val.load(Ordering::Relaxed)))
    }
}

pub fn replay(scen: &(Scenario, ScenarioExtras), seed: u64, actions: &[u16], obs: &mut [f32], mask: &mut [u8], ver: u8) -> Result<usize, EnvError> {
    let osz = obs_size(ver);
    if osz == 0 {
        return Err(EnvError::Buffer("unknown observation version"));
    }
    scen.0.validate()?;
    let mut cx = Combat::try_new_with(&scen.0, &scen.1)?;
    cx.reset_validated(&scen.0, &scen.1, seed, RngSet::from_run_seed_fast(seed)).map_err(EnvError::Scenario)?;
    let n = actions.len() + 1;
    if obs.len() < n * osz || mask.len() < n * ACTION_SPACE {
        return Err(EnvError::Buffer("replay buffers too small"));
    }
    for i in 0..n {
        crate::write_obs_mask(&mut cx, &mut obs[i * osz..(i + 1) * osz], &mut mask[i * ACTION_SPACE..(i + 1) * ACTION_SPACE], ver);
        if i < actions.len() {
            match Action::from_index(actions[i] as usize) {
                Some(a) if cx.step(a) => {}
                _ => return Err(EnvError::Buffer("the recorded action is not legal: the replay diverged")),
            }
        }
    }
    Ok(n)
}

pub fn replay_steps(scen: &(Scenario, ScenarioExtras), seed: u64, actions: &[u16], steps: &[u32], obs: &mut [f32], mask: &mut [u8], ver: u8) -> Result<(), EnvError> {
    let osz = obs_size(ver);
    if osz == 0 {
        return Err(EnvError::Buffer("unknown observation version"));
    }
    if obs.len() < steps.len() * osz || mask.len() < steps.len() * ACTION_SPACE {
        return Err(EnvError::Buffer("replay buffers too small"));
    }
    if steps.iter().any(|&t| t as usize > actions.len()) {
        return Err(EnvError::Buffer("a requested step is past the fight's end"));
    }
    scen.0.validate()?;
    let mut cx = Combat::try_new_with(&scen.0, &scen.1)?;
    cx.reset_validated(&scen.0, &scen.1, seed, RngSet::from_run_seed_fast(seed)).map_err(EnvError::Scenario)?;
    let mut order: Vec<usize> = (0..steps.len()).collect();
    order.sort_by_key(|&k| steps[k]);
    let mut next = 0;
    for i in 0..=actions.len() {
        while next < order.len() && steps[order[next]] as usize == i {
            let k = order[next];
            crate::write_obs_mask(&mut cx, &mut obs[k * osz..(k + 1) * osz], &mut mask[k * ACTION_SPACE..(k + 1) * ACTION_SPACE], ver);
            next += 1;
        }
        if i < actions.len() {
            match Action::from_index(actions[i] as usize) {
                Some(a) if cx.step(a) => {}
                _ => return Err(EnvError::Buffer("the recorded action is not legal: the replay diverged")),
            }
        }
    }
    Ok(())
}
