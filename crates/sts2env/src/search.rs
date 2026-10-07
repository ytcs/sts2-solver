//! `SearchEngine`: the determinized play-out search of `rl/search.py` as a continuous-batching state machine.
//!
//! The solver plays many fights at once. Every fight (a "root") repeatedly asks the policy for its likeliest actions, copies the fight
//! `M x K` times (the `M` best actions, each on `K` determinized futures, see [`Combat::determinize`]), lets every copy play its action
//! and then the policy's own moves to the end of the player turn, and plays the action whose copies got the best mean estimate
//! (final reward if the fight ended, else the value network's opinion of the next turn's first state). That is the default root ([`RootMode::TopM`]);
//! [`RootMode::Gumbel`] samples its candidates over every legal action instead and spends the futures by sequential halving ([`Halving`]).
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
/// Largest number of options per decision (top-M options, or Gumbel candidates).
pub const MAX_M: usize = 16;

/// Classes of the fight-outcome head (`rl/heads.py`): 0 = a loss, b = 1..HEAD_NB = a win with end HP in ((b - 1) * HEAD_BIN, b * HEAD_BIN] (the last bin open).
pub const HEAD_BIN: i32 = 2;
pub const HEAD_NB: usize = 75;
pub const HEAD_NC: usize = HEAD_NB + 1;
/// Belt slots the potion prices and the potion-use head cover.
pub const POT: usize = sts2sim::state::MAX_POTIONS;

/// What a job's endings are worth (`docs/rl_redesign.md` 3.2, the decision layer): with `table`, a finished play-out scores `u[class of its ending]` and a leaf
/// `sum_b P(b) u[b]` from the outcome head's distribution, minus `price[k]` for every root potion the play-out used (at a leaf, plus `price[k]` times the potion-use
/// head's probability for a root potion still in its slot). Without `table` (the default): today's linear return (`win + hp_bonus x HP fraction` / `loss`).
#[derive(Clone, Copy, Debug)]
pub struct Worth {
    pub table: bool,
    pub u: [f32; HEAD_NC],
    pub price: [f32; POT],
}

impl Worth {
    pub fn linear() -> Worth {
        Worth { table: false, u: [0.0; HEAD_NC], price: [0.0; POT] }
    }
}

/// Class of a won fight that ends at `hp` (`rl/heads.py` `end_class`).
fn end_class(hp: i32) -> usize {
    ((hp.max(0) + HEAD_BIN - 1) / HEAD_BIN).clamp(1, HEAD_NB as i32) as usize
}

/// Potion ids per belt slot (`u16::MAX` = empty).
fn pot_ids(cx: &Combat) -> [u16; POT] {
    let mut o = [u16::MAX; POT];
    for (k, p) in cx.player.potions.iter().enumerate().take(POT) {
        if let Some(p) = p {
            o[k] = p.id;
        }
    }
    o
}

/// The price of every root potion a play-out used, plus (at a leaf, `p_use` = the potion-use head) the price times the probability that a root potion still in its slot is used.
fn potion_charge(w: &Worth, used: u8, p_use: Option<&[f32]>) -> f32 {
    let mut c = 0.0;
    for k in 0..POT {
        if w.price[k] == 0.0 {
            continue;
        }
        if (used >> k) & 1 == 1 {
            c += w.price[k];
        } else if let Some(p) = p_use {
            c += w.price[k] * p[k];
        }
    }
    c
}

/// Score of a finished play-out (`terminal`'s reward, or with a worth table the worth of its class minus its potion charge).
fn end_score(cx: &Combat, oc: i8, r: f32, w: &Worth, used: u8) -> f32 {
    if !w.table {
        return r;
    }
    let base = match oc {
        OUTCOME_WIN => w.u[end_class(cx.cr(0).hp)],
        OUTCOME_LOSS => w.u[0],
        _ => return 0.0,
    };
    base - potion_charge(w, used, None)
}

/// A value row's answer at a leaf: the scalar value (`val_w` = 1), or the outcome head's class probabilities (and the potion-use head's per-slot probabilities)
/// combined with the job's worth; without a table, today's linear return of each class from the leaf's max HP (`rl/heads.py` `value`).
fn leaf_value(r: &[f32], cx: &Combat, cfg: &SearchCfg, w: &Worth, used: u8) -> f32 {
    if r.len() == 1 {
        return r[0];
    }
    let p = &r[..HEAD_NC];
    if w.table {
        let mut v = 0.0f32;
        for b in 0..HEAD_NC {
            v += p[b] * w.u[b];
        }
        let pu = if r.len() >= HEAD_NC + POT { Some(&r[HEAD_NC..HEAD_NC + POT]) } else { None };
        v - potion_charge(w, used, pu)
    } else {
        let mx = cx.cr(0).max_hp.max(1) as f32;
        let half = (HEAD_BIN as f32 - 1.0) / 2.0;
        let mut v = p[0] * cfg.loss;
        for (b, &pb) in p.iter().enumerate().skip(1) {
            let c = b as f32 * HEAD_BIN as f32 - half;
            v += pb * (cfg.win + cfg.hp_bonus * (c / mx).min(1.0));
        }
        v
    }
}

/// How a searched decision chooses the actions it tries (`docs/solver.md`, "Root modes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootMode {
    /// The policy's `m` likeliest actions, each on the same `k` futures; the best mean wins (the default, live play).
    TopM,
    /// Gumbel MuZero's root (Danihelka et al. 2022): `gm` candidates sampled without replacement over every legal action (Gumbel-top-k on the prior's
    /// logits), `gn` futures allocated by sequential halving, the action played = argmax of `g + logits + sigma(completed Q)` over the last survivors.
    Gumbel,
}

#[derive(Clone, Copy, Debug)]
pub struct SearchCfg {
    /// Options (the policy's most probable actions) tried per decision. In Gumbel mode only the width of the policy answers (`[rows, 2m + 1]`).
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
    /// Player turns a play-out runs before the value network takes over: 1 = to the end of the current turn (the original search), k = through k turn
    /// boundaries, `u32::MAX` = to the end of the fight (still capped by `roll_cap`; a capped play-out scores its running estimate, no bootstrap).
    pub leaf_turns: u32,
    /// Share the in-turn play of an option between its futures: one play-out per option runs on a scratch copy and the `k` futures branch (each with its own
    /// determinization) at the first step that touches hidden information (a draw, a shuffle, a random choice, the enemy turn).
    pub lead: bool,
    /// Futures are determinized with rotations of one shuffle (`Combat::determinize_strat`).
    pub strat: bool,
    /// An option whose line (the shared prefix of the option chosen at the previous decision) is still being followed is not searched again: its estimate is carried over.
    pub carry: bool,
    /// A fight is truncated after this many steps of the real fight.
    pub max_steps: u32,
    pub win: f32,
    pub loss: f32,
    pub hp_bonus: f32,
    /// With `use_util`: the return of a finished play-out is `util[0]` for a loss and `util[1 + round(100 * hp fraction)]` for a win (`rl/utility.py`
    /// `table`: the fight's HP-worth curve at 1 % steps), instead of `win + hp_bonus * fraction` / `loss`.
    pub util: [f32; 102],
    pub use_util: bool,
    /// A fight (real or play-out) still running after this many player turns is a loss (0 = no cap), as in the training env (`RewardConfig::turn_cap`).
    pub turn_cap: u32,
    /// Floats per value row the caller answers: 1 (a scalar value), `HEAD_NC` (the outcome head's class probabilities) or `HEAD_NC + POT` (and the potion-use
    /// head's per-slot probabilities). Rows wider than 1 are combined in Rust with the job's [`Worth`].
    pub val_w: usize,
    /// Root mode. Gumbel mode ignores `k`, `pmin`, `margin` and `carry`, never tries a potion discard, and needs the root's logits over the whole action
    /// space with every answer ([`SearchEngine::advance_root`]).
    pub root: RootMode,
    /// Gumbel: candidates sampled per searched decision (2..=`MAX_M`; fewer when fewer actions are legal).
    pub gm: usize,
    /// Gumbel: futures per searched decision, over all candidates and phases (top-M's equivalent is `m * k`).
    pub gn: usize,
    /// Gumbel: `sigma(q) = (c_visit + max_a N(a)) * c_scale * qn`, with `qn` the completed Q scaled to [0, 1] by the return's known range
    /// (`win + hp_bonus` .. `loss`, the HP-worth curve's or the worth table's range), not by the decision's own min-max (that blows noise-level gaps up to
    /// the full scale, E9). `c_visit` 50 as in the paper, `c_scale` 0.1 as in mctx's `qtransform_completed_by_mix_value`.
    pub c_visit: f32,
    pub c_scale: f32,
    /// **DIAGNOSTIC ONLY: SEES HIDDEN INFORMATION. NEVER SET FOR LIVE PLAY.** The `k` futures of a decision are NOT determinized: each is an exact copy of the
    /// true current state (the same RNG streams and pile orders), so a play-out meets the real future the fight would meet under the same actions (they
    /// still differ in the play-out policy's sampling stream, so `k` futures are `k` policy samples on the one true future). It gives an upper bound on how
    /// winnable a fight set is (`tools/headroom.py`), not a player: the real game hides exactly what this reads. Default `false`; the live engine never sets it.
    pub clairvoyant: bool,
}

impl Default for SearchCfg {
    /// The batch solver's defaults (`rl/fastsearch.py`): top-M root, 3 x 8, play-outs of 2 player turns, `lead`, `strat`, `carry`, linear return.
    fn default() -> SearchCfg {
        SearchCfg {
            m: 3,
            k: 8,
            conf: 1.01,
            pmin: 0.0,
            margin: 0.0,
            roll_cap: 120,
            leaf_turns: 2,
            lead: true,
            strat: true,
            carry: true,
            max_steps: 300,
            win: 1.0,
            loss: -1.0,
            hp_bonus: 0.5,
            util: [0.0; 102],
            use_util: false,
            turn_cap: 0,
            val_w: 1,
            root: RootMode::TopM,
            gm: 16,
            gn: 160,
            c_visit: 50.0,
            c_scale: 0.1,
            clairvoyant: false,
        }
    }
}

impl SearchCfg {
    /// Play-out slots one root needs at once: `m * k` (top-M), or the largest phase of any halving plan with up to `gm` candidates (Gumbel).
    pub fn slots(&self) -> usize {
        match self.root {
            RootMode::TopM => self.m * self.k,
            RootMode::Gumbel => (2..=self.gm.max(2)).flat_map(|m| halving_plan(m, self.gn)).map(|(s, v)| s * v).max().unwrap_or(1).max(1),
        }
    }
}

/// Phases of sequential halving with `m` candidates and `n` futures: `(survivors, futures per survivor)` per phase. `ceil(log2 m)` phases, survivors halved
/// (rounded up) each phase, each survivor gets `max(1, n / (phases * survivors))` fresh futures; the last phase also takes what the rounding left over
/// (the paper leaves it unused), so the total is `n` whenever `n >= m * phases`.
pub fn halving_plan(m: usize, n: usize) -> Vec<(usize, usize)> {
    if m < 2 {
        return Vec::new();
    }
    let phases = (usize::BITS - (m - 1).leading_zeros()) as usize; // ceil(log2 m)
    let (mut s, mut used, mut out) = (m, 0usize, Vec::with_capacity(phases));
    for p in 0..phases {
        let v = if p + 1 == phases { (n.saturating_sub(used) / s).max(1) } else { (n / (phases * s)).max(1) };
        out.push((s, v));
        used += s * v;
        s = s.div_ceil(2);
    }
    out
}

/// The indices of the `k` largest entries of `score` (Gumbel-top-k when `score = g + logits`: a sample of `k` actions without replacement from the
/// softmax of the logits), best first; ties go to the lower index.
pub fn top_k(score: &[f32], k: usize) -> Vec<usize> {
    let mut ix: Vec<usize> = (0..score.len()).collect();
    ix.sort_by(|&a, &b| score[b].partial_cmp(&score[a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    ix.truncate(k);
    ix
}

/// A standard Gumbel sample from the uniform `u` in (0, 1).
pub fn gumbel(u: f32) -> f32 {
    -(-(u as f64).ln()).ln() as f32
}

/// Gumbel MuZero's monotone transform of a completed Q value: `(c_visit + max_n) * c_scale * qn`, `qn` = `q` scaled from `[lo, hi]` to [0, 1] (clamped).
#[derive(Clone, Copy, Debug)]
pub struct Sigma {
    pub lo: f32,
    pub hi: f32,
    pub c_visit: f32,
    pub c_scale: f32,
}

impl Sigma {
    pub fn of(&self, q: f32, max_n: u32) -> f32 {
        let qn = ((q - self.lo) / (self.hi - self.lo).max(1e-6)).clamp(0.0, 1.0);
        (self.c_visit + max_n as f32) * self.c_scale * qn
    }

    /// The transform for a job: the return's range under its worth table, HP-worth curve or linear return.
    fn for_job(cfg: &SearchCfg, w: &Worth) -> Sigma {
        let (lo, hi) = if w.table {
            w.u.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)))
        } else if cfg.use_util {
            cfg.util.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)))
        } else {
            let xs = [cfg.loss, cfg.win, cfg.win + cfg.hp_bonus];
            xs.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)))
        };
        Sigma { lo, hi, c_visit: cfg.c_visit, c_scale: cfg.c_scale }
    }
}

/// Sequential halving over the sampled candidates of one decision (Gumbel mode). Candidate `j` has `gl[j] = g(j) + logp(j)`; every phase each survivor is
/// played on the same fresh futures (common random numbers: future `f` of every candidate is the same determinization), its completed Q is the mean of all
/// its futures so far, and the phase keeps the better half by `gl + sigma(q)`.
#[derive(Clone, Debug, Default)]
pub struct Halving {
    pub gl: Vec<f32>,
    pub sum: Vec<f32>,
    pub n: Vec<u32>,
    pub alive: Vec<bool>,
    pub plan: Vec<(usize, usize)>,
    pub phase: usize,
}

impl Halving {
    pub fn new(gl: Vec<f32>, n_total: usize) -> Halving {
        let m = gl.len();
        Halving { plan: halving_plan(m, n_total), sum: vec![0.0; m], n: vec![0; m], alive: vec![true; m], gl, phase: 0 }
    }

    /// Futures each survivor gets in the current phase, and the index of the first one (the futures before it were played in earlier phases).
    pub fn phase_futures(&self) -> (usize, usize) {
        (self.plan[self.phase].1, self.plan[..self.phase].iter().map(|p| p.1).sum())
    }

    /// Futures the last survivors end up with (the number of distinct futures of the decision).
    pub fn total_futures(&self) -> usize {
        self.plan.iter().map(|p| p.1).sum()
    }

    pub fn add(&mut self, j: usize, est_sum: f32, count: u32) {
        self.sum[j] += est_sum;
        self.n[j] += count;
    }

    /// Completed Q of a tried candidate (the mean of its futures; NaN if untried).
    pub fn q(&self, j: usize) -> f32 {
        if self.n[j] == 0 {
            f32::NAN
        } else {
            self.sum[j] / self.n[j] as f32
        }
    }

    pub fn max_n(&self) -> u32 {
        self.n.iter().copied().max().unwrap_or(0)
    }

    pub fn score(&self, j: usize, sig: &Sigma) -> f32 {
        let q = self.q(j);
        self.gl[j] + if q.is_nan() { 0.0 } else { sig.of(q, self.max_n()) }
    }

    fn ranked(&self, sig: &Sigma) -> Vec<usize> {
        let mut ix: Vec<usize> = (0..self.gl.len()).filter(|&j| self.alive[j]).collect();
        ix.sort_by(|&a, &b| self.score(b, sig).partial_cmp(&self.score(a, sig)).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
        ix
    }

    /// Ends the current phase: returns true (and keeps the best survivors) when another phase follows, false after the last.
    pub fn next_phase(&mut self, sig: &Sigma) -> bool {
        self.phase += 1;
        if self.phase >= self.plan.len() {
            return false;
        }
        let keep = self.plan[self.phase].0;
        for &j in self.ranked(sig).iter().skip(keep) {
            self.alive[j] = false;
        }
        true
    }

    /// The action to play: the best survivor by `gl + sigma(q)`.
    pub fn best(&self, sig: &Sigma) -> usize {
        self.ranked(sig)[0]
    }
}

/// Van der Corput order of `f` among `n` (a power of two) rotations: any prefix of the futures of a decision is spread over the shuffle (`strat`), as
/// later phases of sequential halving add futures to the earlier ones.
fn spread(f: usize, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let bits = n.trailing_zeros();
    (f.reverse_bits() >> (usize::BITS - bits)) & (n - 1)
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
    /// end HP (absolute; 0 unless won)
    pub hp_end_abs: i32,
    /// belt slots whose starting potion is still there at the end (bit k = slot k)
    pub pot_kept: u8,
}

/// One move of a recorded fight: the action played, and for searched decisions the options considered with their policy probabilities
/// and estimated returns.
///
/// Gumbel mode (`gumbel`) also records Gumbel MuZero's improved policy `pi' = softmax(logits + sigma(completed Q))` over every legal action, sparsely:
/// candidate j has `n[j]` futures, completed Q `q[j]` (the mean of its futures), `adv[j] = sigma(q[j]) - sigma(v)` and `pi[j] = pi'(opts[j])`; an action
/// that was not sampled has completed Q `v` (the paper's `v_mix`: the root value mixed with the prior-weighted mean of the candidates' Q), so its
/// shift is 0. Hence `pi' = softmax(logits + adv)` with `adv` = 0 off the candidates, from the prior's logits at the state (`rl/exit.py --target gumbel`),
/// and the mass of the actions that were not sampled is `1 - sum(pi)`.
#[derive(Clone, Copy, Debug)]
pub struct MoveRec {
    pub action: u16,
    pub searched: bool,
    pub opts: [u16; MAX_M],
    pub p: [f32; MAX_M],
    pub q: [f32; MAX_M],
    pub legal: [bool; MAX_M],
    pub gumbel: bool,
    pub n: [u16; MAX_M],
    pub pi: [f32; MAX_M],
    pub adv: [f32; MAX_M],
    pub v: f32,
}

impl MoveRec {
    fn forced(action: u16) -> MoveRec {
        MoveRec { action, searched: false, opts: [0; MAX_M], p: [0.0; MAX_M], q: [0.0; MAX_M], legal: [false; MAX_M], gumbel: false, n: [0; MAX_M], pi: [0.0; MAX_M], adv: [0.0; MAX_M], v: f32::NAN }
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
    /// play-outs the loop guard ended (scored as losses, part of `end_term`), and real fights it ended (recorded as `OUTCOME_LOSS`)
    pub end_loop: u64,
    pub fight_loops: u64,
    /// options that branched at a hidden-information step / finished before needing one
    pub lead_branch: u64,
    pub lead_clean: u64,
    pub lead_prefix_steps: u64,
    pub carried: u64,
    pub lead_first_unclean: u64,
    /// time stamp counter ticks spent in: `step`, legal actions, observation rows (policy / value), forks (clone + determinize), the rest of the real fight's moves
    pub cy_step: u64,
    pub cy_legal: u64,
    pub cy_obs: u64,
    pub cy_fork: u64,
    pub cy_main: u64,
    /// share of `cy_step` taken by turn-ending steps and their number
    pub cy_endturn: u64,
    pub n_endturn: u64,
    /// Gumbel mode: candidates sampled, and the prior rank (among the legal actions) of the action each searched decision played: 1st, 2nd-5th,
    /// 6th-8th, 9th or lower (the top-M root can only play ranks up to M)
    pub g_cand: u64,
    pub g_rank: [u64; 4],
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
    /// the play-out's own stream for sampling the policy's moves (`Out::pol_u`): seeded from the future's key, so a job seed fixes the play-outs
    rng: u64,
    /// the root's potion ids per slot and the slots whose root potion this play-out has used (or lost) so far (tracked with a worth table only)
    pot0: [u16; POT],
    used: u8,
}

impl Sim {
    /// Marks the root potions no longer in their slot as used.
    #[inline]
    fn track_potions(&mut self) {
        for k in 0..POT {
            if self.pot0[k] != u16::MAX && (self.used >> k) & 1 == 0 && self.cx.player.potions[k].map(|p| p.id) != Some(self.pot0[k]) {
                self.used |= 1 << k;
            }
        }
    }
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
    /// the belt when the job started
    pot_start: [u16; POT],
    opts: [i32; MAX_M],
    ok: [bool; MAX_M],
    /// probabilities of the options of the current decision and the estimates of the finished search
    probs: [f32; MAX_M],
    qs: [f32; MAX_M],
    /// option j still plays its shared prefix on `sims[j * k]` (`sims[(j + 1) * k - 1]` is its scratch copy)
    lead: [bool; MAX_M],
    /// actions the shared prefix of option j has played so far (plus the step that ended it)
    lead_acts: [Vec<u16>; MAX_M],
    /// the line being followed: remaining actions and the estimate of the decision that chose it
    carry: Option<(Vec<u16>, f32)>,
    /// option j is the carried line (estimate in `qs`)
    known: [bool; MAX_M],
    /// determinization seeds of the futures of the current decision
    ks: Vec<u64>,
    /// option j plays on `sims[gbase[j]..gbase[j] + gcnt[j]]`, futures `gf0[j]..gf0[j] + gcnt[j]` (top-M: `j * k`, `k`, 0; Gumbel: per phase)
    gbase: [usize; MAX_M],
    gcnt: [usize; MAX_M],
    gf0: [usize; MAX_M],
    /// the option of every slot
    slot_j: Vec<u8>,
    /// `strat` rotations: top-M rotates future f by f / k; Gumbel by `spread(f, strat_n) / strat_n` (`strat_n` a power of two)
    strat_n: usize,
    /// Gumbel: candidates of the current decision, their halving state, log prior, prior rank among the legal actions
    n_cand: usize,
    hv: Halving,
    logp: [f32; MAX_M],
    prank: [u16; MAX_M],
    /// Gumbel: the value row asked for the root state (the completed Q of the actions not sampled), and its answer
    root_val: Option<u32>,
    vroot: f32,
    todo: Vec<(usize, SimSt)>,
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
    /// per policy row: a uniform in (0, 1) the caller samples a play-out's move with (inverse CDF of the policy); 0.5 for a decision of the real fight
    pol_u: SendPtr<f32>,
    val_obs: SendPtr<f32>,
    /// per value row: bit 1 = a card selection is pending
    val_kind: SendPtr<u8>,
    pol_cap: usize,
    val_cap: usize,
    n_pol: AtomicUsize,
    n_val: AtomicUsize,
    /// Shared layout ([`SearchEngine::advance_shared`]): `val_obs` is the policy observation buffer and value row `r` is written at row
    /// `shared - 1 - r` (from the end backwards); `used` counts policy plus value rows, which may not pass `shared`. 0: separate buffers.
    shared: usize,
    used: AtomicUsize,
}

impl Out {
    /// The observation row value row `r` is written to.
    fn val_obs_row(&self, r: usize) -> usize {
        if self.shared > 0 {
            self.shared - 1 - r
        } else {
            r
        }
    }

    /// Shared layout: one more row of the common capacity (the policy rows grow from the front, the value rows from the back: they meet only if
    /// the total passes it).
    fn take_shared(&self) {
        if self.shared > 0 {
            let u = self.used.fetch_add(1, Ordering::Relaxed);
            assert!(u < self.shared, "shared request buffer too small");
        }
    }
}

/// The caller's answers to the previous call's requests.
struct Inputs<'a> {
    /// `[rows, 2M + 1]`: the M best action indices, their probabilities, the action to play in a play-out.
    pol: &'a [f32],
    /// `[rows, val_w]`
    val: &'a [f32],
    /// Gumbel mode: `[root rows, ACTION_SPACE]` logits (or log-probabilities) of the real fights' decisions, in the order of their policy rows
    /// (`root_rows`: those rows, sorted)
    root: &'a [f32],
    root_rows: &'a [u32],
}

impl Inputs<'_> {
    /// The root logits answering policy row `row`.
    fn root_logits(&self, row: usize) -> Option<&[f32]> {
        let i = self.root_rows.binary_search(&(row as u32)).ok()?;
        self.root.get(i * ACTION_SPACE..(i + 1) * ACTION_SPACE)
    }
}

struct Shared<'a> {
    cfg: SearchCfg,
    scen: &'a [(Scenario, ScenarioExtras)],
    /// per scenario index: what its endings are worth
    worth: &'a [Worth],
    /// per scenario index: a combat to start the job from (a mid-fight state), instead of resetting the scenario
    starts: &'a [Option<Combat>],
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
    } else if crate::looped(cx) {
        // the loop guard tripped: a fight the real game never finishes (a soft-lock) is a loss, not a neutral abort (`crate::looped`)
        Some((OUTCOME_LOSS, if cfg.use_util { cfg.util[0] } else { cfg.loss }))
    } else if cx.overflow != 0 {
        Some((OUTCOME_OVERFLOW, 0.0))
    } else if cx.stage == Stage::Over {
        let me = cx.cr(0);
        match cx.outcome {
            Outcome::Victory => {
                let frac = me.hp as f32 / me.max_hp.max(1) as f32;
                if cfg.use_util {
                    let i = ((frac * 100.0).round().max(0.0) as usize).min(100);
                    Some((OUTCOME_WIN, cfg.util[1 + i]))
                } else {
                    Some((OUTCOME_WIN, cfg.win + cfg.hp_bonus * frac))
                }
            }
            _ => Some((OUTCOME_LOSS, if cfg.use_util { cfg.util[0] } else { cfg.loss })),
        }
    } else if cfg.turn_cap > 0 && cx.player.turn_number > cfg.turn_cap as i32 {
        Some((OUTCOME_LOSS, if cfg.use_util { cfg.util[0] } else { cfg.loss }))
    } else if steps >= max_steps {
        Some((OUTCOME_TRUNCATED, 0.0))
    } else {
        None
    }
}

/// The move that needs no decision: the only legal action, or confirming a selection that is full (picking another card at `max` only
/// swaps the latest pick, which the policy could have chosen directly; a sampled policy otherwise wanders between picks for dozens of steps).
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

fn pol_row(out: &Out, sim: bool, cx: &Combat, u: f32) -> usize {
    let kind = sim as u8 | (cx.decision.is_some() as u8) << 1;
    out.take_shared();
    let r = out.n_pol.fetch_add(1, Ordering::Relaxed);
    assert!(r < out.pol_cap, "policy request buffer too small");
    // SAFETY: row `r` is owned by the caller (unique counter value) and below the capacity.
    unsafe {
        *out.pol_kind.0.add(r) = kind;
        *out.pol_u.0.add(r) = u;
    }
    r
}

/// A uniform in (0, 1) from the stream `s`.
fn unit(s: &mut u64) -> f32 {
    ((splitmix(s) >> 40) as f32 + 0.5) / (1u64 << 24) as f32
}

/// Salt of a play-out's sampling stream (`Sim::rng`), seeded from its future's determinization key.
const ROLL_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;

fn val_row(out: &Out, cx: &Combat) -> usize {
    out.take_shared();
    let r = out.n_val.fetch_add(1, Ordering::Relaxed);
    assert!(r < out.val_cap, "value request buffer too small");
    // SAFETY: as in `pol_row`.
    unsafe { *out.val_kind.0.add(r) = (cx.decision.is_some() as u8) << 1 };
    r
}

/// What a step can consume of the information a player does not have: the RNG streams (call counters) and the order of the draw pile.
/// Also changed by drawing, shuffling, random choices and the enemy turn.
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

/// Does the state show something that depends on the (hidden) order of the draw pile? A pending selection among cards of the draw pile does.
fn shows_draw_pile(cx: &Combat) -> bool {
    matches!(&cx.decision, Some(d) if matches!(d.source, sts2sim::state::DecisionSource::Pile(sts2sim::types::PileType::Draw)))
}

/// Plays `act` on a play-out copy and then every forced move, until it needs the policy / the value network or ends.
///
/// With `scratch` (the shared prefix of an option) every step is first tried on a copy: if it touches hidden information the function returns
/// `Some(act)` with `scratch` holding the state before that step and `sim` spoilt, so the caller can branch there; otherwise it behaves as without.
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
            // cannot happen for an action the policy took from the legal set; count it and score the copy as lost
            st.illegal += 1;
            sim.est += if w.table { w.u[0] } else { cfg.loss };
            sim.st = SimSt::Done;
            return None;
        }
        if w.table {
            sim.track_potions();
        }
        if let Some((oc, r)) = terminal(&sim.cx, 0, u32::MAX, cfg) {
            sim.est += end_score(&sim.cx, oc, r, w, sim.used);
            sim.st = SimSt::Done;
            st.end_term += 1;
            st.end_loop += crate::looped(&sim.cx) as u64;
            return None;
        }
        // with a worth table a capped play-out is valued by the network (0 is not a neutral score in those units)
        let capped = sim.steps >= cfg.roll_cap;
        let at_leaf = (sim.cx.player.turn_number - sim.start_turn) as i64 >= cfg.leaf_turns as i64;
        if at_leaf || (capped && w.table) {
            let t0 = tsc();
            let row = val_row(out, &sim.cx);
            let buf = ActionBuf::new();
            write_row(&mut sim.cx, &buf, None, out.val_obs, None, out.val_obs_row(row));
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
            write_row(&mut sim.cx, &buf, Some(playable), out.pol_obs, Some(out.pol_mask), row);
            st.cy_obs += tsc() - t0;
            st.policy_rows += 1;
            sim.st = SimSt::Pol(row as u32);
            return None;
        }
    }
}

/// Determinizes `cx` as future `f` of the decision (`ks`: the decision's seeds); with `strat`, rotation `rot` of `n`. With `SearchCfg::clairvoyant`
/// (diagnostic only) it leaves the copy of the true state as it is, so the play-out sees the true future.
#[inline]
fn det_future(cx: &mut Combat, cfg: &SearchCfg, ks: &[u64], f: usize, rot: usize, n: usize) {
    if cfg.clairvoyant {
        return;
    }
    if cfg.strat {
        cx.determinize_strat(ks[0], ks[f], rot, n);
    } else {
        cx.determinize(ks[f]);
    }
}

impl Block {
    fn new(sc: &Scenario, ex: &ScenarioExtras, cfg: &SearchCfg) -> Result<Block, EnvError> {
        let main = Combat::try_new_with(sc, ex)?;
        let n_sims = cfg.slots();
        let sims = (0..n_sims).map(|_| Sim { cx: main.clone(), st: SimSt::Idle, start_turn: 0, est: 0.0, steps: 0, rng: 0, pot0: [u16::MAX; POT], used: 0 }).collect();
        let mut gbase = [0; MAX_M];
        let mut gcnt = [0; MAX_M];
        let mut slot_j = vec![0u8; n_sims];
        if cfg.root == RootMode::TopM {
            for j in 0..cfg.m {
                gbase[j] = j * cfg.k;
                gcnt[j] = cfg.k;
            }
            for (i, s) in slot_j.iter_mut().enumerate() {
                *s = (i / cfg.k) as u8;
            }
        }
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
            ks: Vec::new(),
            gbase,
            gcnt,
            gf0: [0; MAX_M],
            slot_j,
            strat_n: cfg.k,
            n_cand: 0,
            hv: Halving::default(),
            logp: [0.0; MAX_M],
            prank: [0; MAX_M],
            root_val: None,
            vroot: 0.0,
            todo: Vec::new(),
            log: Vec::new(),
            stats: SearchStats::default(),
        })
    }

    /// The `strat` rotation of future `f` of the current decision: `(rotation, of)`.
    #[inline]
    fn rot(&self, f: usize, cfg: &SearchCfg) -> (usize, usize) {
        match cfg.root {
            RootMode::TopM => (f, cfg.k),
            RootMode::Gumbel => (spread(f, self.strat_n), self.strat_n),
        }
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
        if let Some(Some(c)) = sh.starts.get(si as usize) {
            self.main = c.clone();
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

    /// Plays `act` in the real fight. Returns true if the job ended (and the next one could not be started: the block is idle).
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
                // (a fight the loop guard already ended, e.g. a start state or combat-start hooks that ran away: a loss, `crate::looped`)
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
        if cfg.root == RootMode::Gumbel {
            return self.on_root_gumbel(inp, row, sh, out);
        }
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
        self.ks.clear();
        for _ in 0..k {
            let x = splitmix(&mut self.rng);
            self.ks.push(x);
        }
        let turn = self.main.player.turn_number;
        let lead = cfg.lead && k >= 2;
        let w = &sh.worth[self.scen as usize];
        let pot0 = pot_ids(&self.main);
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
                for kk in 0..k {
                    self.sims[j * k + kk].st = SimSt::Idle;
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
            self.start_option(j, first, lead, cfg, w, out, pot0, turn);
        }
        self.st = RootSt::Searching;
        self.try_finish_search(sh, out);
    }

    /// Starts option `j` on its slots and futures (`gbase`, `gcnt`, `gf0`): every future a determinized copy of the real fight that plays `first`; with
    /// `lead` only the first future exists until the option needs hidden information (the other slots wait, see `lead_run`).
    #[allow(clippy::too_many_arguments)]
    fn start_option(&mut self, j: usize, first: Action, lead: bool, cfg: &SearchCfg, w: &Worth, out: &Out, pot0: [u16; POT], turn: i32) {
        let (base, c, f0) = (self.gbase[j], self.gcnt[j], self.gf0[j]);
        let n_now = if lead { 1 } else { c };
        for kk in 0..c {
            let (rot, rn) = self.rot(f0 + kk, cfg);
            let sim = &mut self.sims[base + kk];
            if kk >= n_now {
                sim.st = SimSt::Idle;
                continue;
            }
            let t0 = tsc();
            sim.cx.clone_from(&self.main);
            det_future(&mut sim.cx, cfg, &self.ks, f0 + kk, rot, rn);
            self.stats.cy_fork += tsc() - t0;
            sim.start_turn = turn;
            sim.est = 0.0;
            sim.steps = 0;
            sim.rng = self.ks[f0 + kk] ^ ROLL_SALT;
            sim.pot0 = pot0;
            sim.used = 0;
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

    /// Gumbel mode: the root's logits arrived. Samples the candidates (Gumbel-top-k over every legal action but potion discards) and starts the first
    /// phase of sequential halving, or plays at once when fewer than two candidates exist or the prior's top action reaches `conf`.
    fn on_root_gumbel(&mut self, inp: &Inputs, row: usize, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        self.stats.root_decisions += 1;
        let mut buf = ActionBuf::new();
        let mut playable = 0u16;
        self.main.legal_actions_ex(&mut buf, &mut playable);
        let lg = inp.root_logits(row);
        // the prior over the legal actions the network allows (it masks the others with -1e9) as log-probabilities
        let allowed = |x: f32| x.is_finite() && x > -1e8;
        let raw: Vec<(usize, f32, bool)> = buf.iter().map(|a| (a.index(), lg.map_or(f32::NAN, |l| l[a.index()]), matches!(a, Action::DiscardPotion { .. }))).collect();
        let mx = raw.iter().map(|r| r.1).filter(|&x| allowed(x)).fold(f32::NEG_INFINITY, f32::max);
        let lse = if mx.is_finite() { mx + raw.iter().map(|r| r.1).filter(|&x| allowed(x)).map(|x| (x - mx).exp()).sum::<f32>().ln() } else { 0.0 };
        let el: Vec<(usize, f32)> = raw.iter().filter(|r| allowed(r.1) && !r.2).map(|r| (r.0, r.1 - lse)).collect();
        let top = el.iter().copied().fold(None, |b: Option<(usize, f32)>, x| match b {
            Some(y) if y.1 >= x.1 => Some(y),
            _ => Some(x),
        });
        if el.len() < 2 || top.is_some_and(|t| t.1.exp() >= cfg.conf) {
            let a_idx = top.map_or(buf[0].index(), |t| t.0);
            let rec = sh.record.then(|| {
                let mut r = MoveRec::forced(a_idx as u16);
                if let Some((i, l)) = top {
                    r.opts[0] = i as u16;
                    r.p[0] = l.exp();
                    r.legal[0] = true;
                }
                r
            });
            let finished = match Action::from_index(a_idx) {
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
        // Gumbel-top-k: g + log prior, one Gumbel per action from the job's stream (in legal-action order, so a job seed fixes the sample)
        let gl_all: Vec<f32> = el.iter().map(|&(_, l)| gumbel(unit(&mut self.rng)) + l).collect();
        let pick = top_k(&gl_all, cfg.gm.min(MAX_M));
        self.n_cand = pick.len();
        let mut gl = Vec::with_capacity(pick.len());
        for (j, &e) in pick.iter().enumerate() {
            let (i, l) = el[e];
            self.opts[j] = i as i32;
            self.logp[j] = l;
            self.probs[j] = l.exp();
            self.ok[j] = true;
            self.prank[j] = el.iter().filter(|x| x.1 > l).count() as u16;
            gl.push(gl_all[e]);
        }
        for j in pick.len()..MAX_M {
            self.ok[j] = false;
            self.gcnt[j] = 0;
        }
        self.hv = Halving::new(gl, cfg.gn);
        let nf = self.hv.total_futures();
        self.strat_n = nf.next_power_of_two();
        self.ks.clear();
        for _ in 0..nf {
            let x = splitmix(&mut self.rng);
            self.ks.push(x);
        }
        // the root state's value: the completed Q of the actions not sampled (it only enters pi': every candidate is tried in the first phase)
        let r = val_row(out, &self.main);
        let nobuf = ActionBuf::new();
        write_row(&mut self.main, &nobuf, None, out.val_obs, None, out.val_obs_row(r));
        self.stats.value_rows += 1;
        self.root_val = Some(r as u32);
        self.stats.searched += 1;
        self.stats.g_cand += self.n_cand as u64;
        self.st = RootSt::Searching;
        self.launch_phase(sh, out);
        self.try_finish_search(sh, out);
    }

    /// Gumbel mode: starts the current phase of sequential halving, every surviving candidate on the phase's fresh futures (the same ones for all).
    fn launch_phase(&mut self, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        let w = &sh.worth[self.scen as usize];
        let (v, f0) = self.hv.phase_futures();
        let lead = cfg.lead && v >= 2;
        let pot0 = pot_ids(&self.main);
        let turn = self.main.player.turn_number;
        let mut base = 0;
        for j in 0..self.n_cand {
            self.lead[j] = false;
            self.lead_acts[j].clear();
            if !self.hv.alive[j] {
                self.gcnt[j] = 0;
                continue;
            }
            debug_assert!(base + v <= self.sims.len(), "halving phase larger than the slot pool");
            self.gbase[j] = base;
            self.gcnt[j] = v;
            self.gf0[j] = f0;
            for s in &mut self.slot_j[base..base + v] {
                *s = j as u8;
            }
            base += v;
            let first = Action::from_index(self.opts[j] as usize).expect("a candidate is a legal action");
            self.start_option(j, first, lead, cfg, w, out, pot0, turn);
        }
    }

    /// Gumbel mode: every play-out of the phase is done. Adds the phase's futures to the candidates' completed Q, keeps the better half and starts the
    /// next phase, or after the last phase plays the best survivor by `g + logits + sigma(q)`.
    fn finish_phase(&mut self, sh: &Shared, out: &Out) {
        if self.root_val.is_some() {
            return; // the root's value has not arrived yet
        }
        let cfg = &sh.cfg;
        let sig = Sigma::for_job(cfg, &sh.worth[self.scen as usize]);
        for j in 0..self.n_cand {
            let c = self.gcnt[j];
            if c == 0 {
                continue;
            }
            let b = self.gbase[j];
            let s = self.sims[b..b + c].iter().map(|s| s.est).sum::<f32>();
            self.hv.add(j, s, c as u32);
            self.gcnt[j] = 0;
        }
        for s in self.sims.iter_mut() {
            s.st = SimSt::Idle;
        }
        if self.hv.next_phase(&sig) {
            self.launch_phase(sh, out);
            return self.try_finish_search(sh, out);
        }
        let best = self.hv.best(&sig);
        self.stats.g_rank[match self.prank[best] {
            0 => 0,
            1..=4 => 1,
            5..=7 => 2,
            _ => 3,
        }] += 1;
        let rec = self.gumbel_rec(best, &sig, sh);
        let a = Action::from_index(self.opts[best] as usize).expect("a candidate is a legal action");
        if self.main_act(a, sh, rec) {
            self.st = RootSt::Idle;
        } else {
            self.root_next(sh, out);
        }
    }

    /// The record of a Gumbel decision: candidates, their futures and completed Q, and the improved policy (see [`MoveRec`]).
    fn gumbel_rec(&self, best: usize, sig: &Sigma, sh: &Shared) -> Option<MoveRec> {
        if !sh.record {
            return None;
        }
        let hv = &self.hv;
        let mut r = MoveRec::forced(self.opts[best] as u16);
        r.searched = true;
        r.gumbel = true;
        // v_mix (the paper's eq. 33): the root value mixed with the prior-weighted mean of the tried candidates' Q, weighted by the futures played
        let (mut nt, mut sp, mut spq) = (0f64, 0f64, 0f64);
        for j in 0..self.n_cand {
            if hv.n[j] > 0 {
                nt += hv.n[j] as f64;
                sp += self.probs[j] as f64;
                spq += self.probs[j] as f64 * hv.q(j) as f64;
            }
        }
        let v = if sp > 0.0 { ((self.vroot as f64 + nt / sp * spq) / (1.0 + nt)) as f32 } else { self.vroot };
        let mx = hv.max_n();
        let sv = sig.of(v, mx);
        // pi'(a) = p(a) exp(adv(a)) / Z, Z = sum over every legal action = 1 + sum over the candidates of p (exp(adv) - 1)
        let mut z = 1.0f64;
        for j in 0..self.n_cand {
            r.opts[j] = self.opts[j] as u16;
            r.p[j] = self.probs[j];
            r.legal[j] = true;
            r.q[j] = hv.q(j);
            r.n[j] = hv.n[j].min(u16::MAX as u32) as u16;
            r.adv[j] = if hv.n[j] > 0 { sig.of(hv.q(j), mx) - sv } else { 0.0 };
            z += self.probs[j] as f64 * ((r.adv[j] as f64).exp() - 1.0);
        }
        for j in 0..self.n_cand {
            r.pi[j] = (self.probs[j] as f64 * (r.adv[j] as f64).exp() / z) as f32;
        }
        r.v = v;
        Some(r)
    }

    /// Runs the shared prefix of option `j` (see `SearchCfg::lead`) from `act` until it needs the policy / the value network, ends, or reaches a step that
    /// touches hidden information, where the `k` futures branch.
    fn lead_run(&mut self, j: usize, act: Action, cfg: &SearchCfg, w: &Worth, out: &Out) {
        let (base, k, f0) = (self.gbase[j], self.gcnt[j], self.gf0[j]);
        let branch = {
            let (head, tail) = self.sims.split_at_mut(base + k - 1);
            let scratch = &mut tail[0];
            let lead = &mut head[base];
            sim_run(lead, act, cfg, w, out, &mut self.stats, Some(&mut scratch.cx), Some(&mut self.lead_acts[j]))
        };
        match branch {
            Some(a) => {
                // `sims[base + k - 1]` holds the state before the step `a`: every future starts from it with its own determinization
                self.stats.lead_branch += 1;
                self.lead_acts[j].push(a.index() as u16);
                self.stats.lead_prefix_steps += self.sims[base].steps as u64;
                self.stats.lead_first_unclean += (self.sims[base].steps == 0) as u64;
                let (est, steps, start_turn, pot0, used) = {
                    let l = &self.sims[base];
                    (l.est, l.steps, l.start_turn, l.pot0, l.used)
                };
                for kk in 0..k - 1 {
                    let (head, tail) = self.sims.split_at_mut(base + k - 1);
                    head[base + kk].cx.clone_from(&tail[0].cx);
                }
                for kk in 0..k {
                    let (rot, rn) = self.rot(f0 + kk, cfg);
                    let sim = &mut self.sims[base + kk];
                    let t0 = tsc();
                    det_future(&mut sim.cx, cfg, &self.ks, f0 + kk, rot, rn);
                    self.stats.cy_fork += tsc() - t0;
                    self.stats.forks += 1;
                    sim.est = est;
                    sim.steps = steps;
                    sim.start_turn = start_turn;
                    sim.pot0 = pot0;
                    sim.used = used;
                    sim.rng = self.ks[f0 + kk] ^ ROLL_SALT ^ (steps as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    sim_run(sim, a, cfg, w, out, &mut self.stats, None, None);
                }
                self.lead[j] = false;
            }
            None => {
                if self.sims[base].st == SimSt::Done {
                    self.lead_finish(j);
                }
            }
        }
    }

    /// The shared prefix of option `j` ended without needing hidden information: every future has the same result.
    fn lead_finish(&mut self, j: usize) {
        let (base, k) = (self.gbase[j], self.gcnt[j]);
        let est = self.sims[base].est;
        for kk in 1..k {
            let sim = &mut self.sims[base + kk];
            sim.est = est;
            sim.st = SimSt::Done;
        }
        self.lead[j] = false;
        self.stats.lead_clean += 1;
    }

    /// If every play-out of the current decision is done: plays the best option for real and moves on.
    fn try_finish_search(&mut self, sh: &Shared, out: &Out) {
        let cfg = &sh.cfg;
        if self.sims.iter().any(|s| matches!(s.st, SimSt::Pol(_) | SimSt::Val(_))) {
            return;
        }
        if cfg.root == RootMode::Gumbel {
            return self.finish_phase(sh, out);
        }
        let (m, k) = (cfg.m, cfg.k);
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
        if best != 0 && best_q - q0 <= cfg.margin {
            best = 0;
        }
        for s in self.sims[..m * k].iter_mut() {
            s.st = SimSt::Idle;
        }
        let rec = self.move_rec(self.opts[best], true, sh);
        if cfg.carry && !self.known[best] {
            self.carry = if self.lead_acts[best].first() == Some(&(self.opts[best] as u16)) { Some((std::mem::take(&mut self.lead_acts[best]), self.qs[best])) } else { None };
        }
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

    /// After a panic inside this block: the current fight is recorded as aborted (`OUTCOME_OVERFLOW`) and the block starts the next job.
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
        self.root_val = None;
        if self.job != NONE {
            self.record(sh, OUTCOME_OVERFLOW);
            self.job = NONE;
        }
        self.st = RootSt::Idle;
        // the combat the panic left behind is not trusted: `start_job` rebuilds it from the scenario
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
                let w = &sh.worth[self.scen as usize];
                let vw = cfg.val_w;
                if let Some(r) = self.root_val.take() {
                    // Gumbel: the root state's value, asked when the search started
                    let r = r as usize * vw;
                    self.vroot = leaf_value(&inp.val[r..r + vw], &self.main, &cfg, w, 0);
                }
                // only the sims that were waiting when this call began have an answer (a branching option starts others during the loop)
                let mut todo = std::mem::take(&mut self.todo);
                todo.clear();
                todo.extend((0..self.sims.len()).filter(|&i| matches!(self.sims[i].st, SimSt::Pol(_) | SimSt::Val(_))).map(|i| (i, self.sims[i].st)));
                for &(idx, waiting) in todo.iter() {
                    match waiting {
                        SimSt::Pol(row) => {
                            let r = row as usize * stride;
                            let j = self.slot_j[idx] as usize;
                            let is_lead = self.lead[j] && idx == self.gbase[j];
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
                            sim.est += leaf_value(&inp.val[r..r + vw], &sim.cx, &cfg, w, sim.used);
                            sim.st = SimSt::Done;
                            let j = self.slot_j[idx] as usize;
                            if self.lead[j] && idx == self.gbase[j] {
                                self.lead_finish(j);
                            }
                        }
                        _ => {}
                    }
                }
                self.todo = todo;
                self.try_finish_search(sh, out);
            }
        }
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
}

impl SearchEngine {
    /// `scen`: the distinct scenarios; `jobs`: one `(scenario index, seed)` per fight to play; `n_roots`: fights played at the same time.
    pub fn new(scen: Vec<(Scenario, ScenarioExtras)>, jobs: Vec<(u32, u64)>, n_roots: usize, cfg: SearchCfg, threads: usize, record: bool) -> Result<SearchEngine, EnvError> {
        Self::new_with_starts(scen, Vec::new(), jobs, n_roots, cfg, threads, record)
    }

    /// Like `new`; `starts[i]` (when present) is a combat that jobs of scenario `i` start from instead of the scenario's beginning (a fight in progress:
    /// the search then decides from there; hidden information is resampled for every future as always).
    pub fn new_with_starts(scen: Vec<(Scenario, ScenarioExtras)>, starts: Vec<Option<Combat>>, jobs: Vec<(u32, u64)>, n_roots: usize, cfg: SearchCfg, threads: usize, record: bool) -> Result<SearchEngine, EnvError> {
        if scen.is_empty() || cfg.m == 0 || cfg.m > MAX_M || cfg.k == 0 || !(cfg.val_w == 1 || cfg.val_w == HEAD_NC || cfg.val_w == HEAD_NC + POT) {
            return Err(EnvError::Buffer("bad search configuration"));
        }
        if cfg.use_util && cfg.val_w != 1 {
            return Err(EnvError::Buffer("the HP-worth curve (util) needs scalar value rows"));
        }
        if cfg.root == RootMode::Gumbel && (cfg.gm < 2 || cfg.gm > MAX_M || cfg.gn == 0 || !(cfg.c_visit >= 0.0) || !(cfg.c_scale >= 0.0)) {
            return Err(EnvError::Buffer("bad Gumbel configuration (2 <= gm <= MAX_M, gn >= 1, c_visit and c_scale >= 0)"));
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
        Ok(SearchEngine { cfg, scen, worth, starts, jobs, blocks: blocks?, results, logs, record, next_job: 0, pool, started: false })
    }

    /// What each scenario's endings are worth (one [`Worth`] per scenario; default linear). A table needs value rows from the outcome head (`val_w` > 1).
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

    pub fn n_roots(&self) -> usize {
        self.blocks.len()
    }

    pub fn n_jobs(&self) -> usize {
        self.jobs.len()
    }

    /// Largest number of policy / value rows one call can request (size the buffers with it).
    pub fn max_rows(&self) -> (usize, usize) {
        let s = self.cfg.slots();
        // Gumbel: a root also asks for its own value when a search starts (one more value row)
        let extra = (self.cfg.root == RootMode::Gumbel) as usize;
        (self.blocks.len() * (s + 1), self.blocks.len() * (s + extra))
    }

    pub fn root_mode(&self) -> RootMode {
        self.cfg.root
    }

    pub fn val_w(&self) -> usize {
        self.cfg.val_w
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
            t.g_cand += s.g_cand;
            for i in 0..4 {
                t.g_rank[i] += s.g_rank[i];
            }
        }
        t
    }

    /// True when every job is finished.
    pub fn finished(&self) -> bool {
        self.started && self.blocks.iter().all(|b| b.st == RootSt::Idle)
    }

    /// One cycle. The first call (`pol` / `val` = `None`) starts the roots; every later call passes the answers to the rows the previous call
    /// returned: `pol` = `[rows, 2M + 1]` f32 (the `M` best action indices, their probabilities, the action a play-out plays, sampled with the row's
    /// `pol_u` uniform so that the job seeds fix every play-out), `val` = `[rows]`.
    /// Writes the next requests into the buffers and returns `(policy rows, value rows)`; `(0, 0)` with [`SearchEngine::finished`] ends the run.
    pub fn advance(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, pol_obs: &mut [f32], pol_mask: &mut [u8], pol_kind: &mut [u8], pol_u: &mut [f32], val_obs: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        self.advance_root(pol, val, None, pol_obs, pol_mask, pol_kind, pol_u, val_obs, val_kind)
    }

    /// [`SearchEngine::advance`] plus, in Gumbel mode, `root` = `[root rows, ACTION_SPACE]`: the logits (or log-probabilities) over the whole action space
    /// of every policy row of the previous call that is a decision of a real fight (`pol_kind & 1 == 0`), in row order. Illegal entries are ignored
    /// (and entries at or below -1e8, the network's mask), the rest normalized over the legal actions.
    #[allow(clippy::too_many_arguments)]
    pub fn advance_root(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, root: Option<&[f32]>, pol_obs: &mut [f32], pol_mask: &mut [u8], pol_kind: &mut [u8], pol_u: &mut [f32], val_obs: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        let (pc, vc) = self.max_rows();
        if pol_obs.len() < pc * OBS_SIZE || pol_mask.len() < pc * ACTION_SPACE || pol_kind.len() < pc || pol_u.len() < pc || val_obs.len() < vc * OBS_SIZE || val_kind.len() < vc {
            return Err(EnvError::Buffer("request buffers too small, see SearchEngine::max_rows"));
        }
        let out = Out {
            pol_obs: SendPtr(pol_obs.as_mut_ptr()),
            pol_mask: SendPtr(pol_mask.as_mut_ptr()),
            pol_kind: SendPtr(pol_kind.as_mut_ptr()),
            pol_u: SendPtr(pol_u.as_mut_ptr()),
            val_obs: SendPtr(val_obs.as_mut_ptr()),
            val_kind: SendPtr(val_kind.as_mut_ptr()),
            pol_cap: pc,
            val_cap: vc,
            n_pol: AtomicUsize::new(0),
            n_val: AtomicUsize::new(0),
            shared: 0,
            used: AtomicUsize::new(0),
        };
        self.advance_out(pol, val, root, out)
    }

    /// Rows of the one observation buffer [`SearchEngine::advance_shared`] uses: policy plus value rows of one call never pass it (per block,
    /// at most one row per play-out plus one root row). About half of `max_rows`' two buffers together.
    pub fn shared_rows(&self) -> usize {
        self.blocks.len() * (self.cfg.slots() + 1)
    }

    /// [`SearchEngine::advance`] with one observation buffer for both kinds of rows (`shared_rows` rows): policy row `r` at row `r` (as before),
    /// value row `r` at row `shared_rows - 1 - r` (from the end backwards). `mask`, `pol_kind`, `pol_u` and `val_kind` are indexed by the row
    /// number as before and hold `shared_rows` rows each. Same requests, answers and results as `advance`; half the observation memory.
    pub fn advance_shared(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, obs: &mut [f32], mask: &mut [u8], pol_kind: &mut [u8], pol_u: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        self.advance_shared_root(pol, val, None, obs, mask, pol_kind, pol_u, val_kind)
    }

    /// [`SearchEngine::advance_shared`] with the root logits of Gumbel mode (see [`SearchEngine::advance_root`]).
    #[allow(clippy::too_many_arguments)]
    pub fn advance_shared_root(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, root: Option<&[f32]>, obs: &mut [f32], mask: &mut [u8], pol_kind: &mut [u8], pol_u: &mut [f32], val_kind: &mut [u8]) -> Result<(usize, usize), EnvError> {
        let cap = self.shared_rows();
        if obs.len() < cap * OBS_SIZE || mask.len() < cap * ACTION_SPACE || pol_kind.len() < cap || pol_u.len() < cap || val_kind.len() < cap {
            return Err(EnvError::Buffer("request buffers too small, see SearchEngine::shared_rows"));
        }
        let p = obs.as_mut_ptr();
        let out = Out {
            pol_obs: SendPtr(p),
            pol_mask: SendPtr(mask.as_mut_ptr()),
            pol_kind: SendPtr(pol_kind.as_mut_ptr()),
            pol_u: SendPtr(pol_u.as_mut_ptr()),
            val_obs: SendPtr(p),
            val_kind: SendPtr(val_kind.as_mut_ptr()),
            pol_cap: cap,
            val_cap: cap,
            n_pol: AtomicUsize::new(0),
            n_val: AtomicUsize::new(0),
            shared: cap,
            used: AtomicUsize::new(0),
        };
        self.advance_out(pol, val, root, out)
    }

    fn advance_out(&mut self, pol: Option<&[f32]>, val: Option<&[f32]>, root: Option<&[f32]>, out: Out) -> Result<(usize, usize), EnvError> {
        if let Some(v) = val {
            if v.len() % self.cfg.val_w != 0 {
                return Err(EnvError::Buffer("value answers are not a whole number of rows of val_w floats"));
            }
        }
        let first = !self.started;
        if !first && (pol.is_none() || val.is_none()) {
            return Err(EnvError::Buffer("answers missing"));
        }
        // Gumbel: the roots waiting for their policy are the real-fight rows of the previous call, in row order
        let mut root_rows: Vec<u32> = Vec::new();
        if !first && self.cfg.root == RootMode::Gumbel {
            root_rows.extend(self.blocks.iter().filter_map(|b| if let RootSt::Pol(r) = b.st { Some(r) } else { None }));
            root_rows.sort_unstable();
            if root.map_or(0, |r| r.len()) != root_rows.len() * ACTION_SPACE {
                return Err(EnvError::Buffer("Gumbel mode: root logits must be [real-fight policy rows of the previous call, ACTION_SPACE]"));
            }
        }
        self.started = true;
        let sh = Shared { cfg: self.cfg, scen: &self.scen, worth: &self.worth, starts: &self.starts, jobs: &self.jobs, next_job: AtomicUsize::new(self.next_job), results: SendPtr(self.results.as_mut_ptr()), logs: SendPtr(self.logs.as_mut_ptr()), record: self.record };
        let inp = if first { None } else { Some(Inputs { pol: pol.unwrap(), val: val.unwrap(), root: root.unwrap_or(&[]), root_rows: &root_rows }) };
        let blocks = &mut self.blocks;
        self.pool.install(|| {
            blocks.par_iter_mut().for_each(|b| {
                // a simulator bug in one fight must not take the whole batch down: abort that fight (reported on stderr) and go on with the next job
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.advance(inp.as_ref(), &sh, &out))).is_err() {
                    b.recover(&sh, &out);
                }
            });
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
