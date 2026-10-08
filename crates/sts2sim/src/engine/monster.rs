//! Monster spawning, move state machine, move execution and interrupts (spec 04 §1).
//!
//! Interrupts (spec 04 §1.8): `stun` replaces the pending move with a synthetic `STUNNED` state (node `STUN_NODE`);
//! `set_move_immediate` forces a move node (revive / dead / enrage states of Illusion, Reattach, Adaptable ...).

use crate::content;
use crate::defs::*;
use crate::hooks::{hookbit, Kind, Me};
use crate::state::*;
use crate::types::*;

/// Synthetic node index of the `STUNNED` state built by `CreatureCmd.Stun` (it is not part of the monster's def).
pub const STUN_NODE: u8 = 0xFE;

/// Intents of the `STUNNED` move.
pub static STUN_INTENTS: [Intent; 1] = [Intent::Stun];

impl Combat {
    /// A free creature slot. Never-used slots come first so a dead creature's slot is not recycled while powers it
    /// applied may still refer to it; only then are detached slots reused.
    fn alloc_slot(&self) -> Option<Cid> {
        (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).active).or_else(|| (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).in_combat))
    }

    /// `CombatState.CreateCreature` + `AddCreature` for an enemy: one `niche` draw for HP, unique among enemies already
    /// added (spec 04 §1.13); then `SetUpForCombat` (state machine built, `SpawnedThisTurn = true`).
    pub fn add_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        self.add_enemy_v(monster_id, slot, [0, 0])
    }

    /// `add_enemy` with the monster's private integers (`vars`) already set when its HP range is computed (Axebot's
    /// `MinInitialHp` depends on its respawn count, which is a monster field in the C# model).
    pub fn add_enemy_v(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let cid = self.create_enemy_v(monster_id, slot, vars)?;
        self.attach_enemy(cid);
        Some(cid)
    }

    /// `CombatState.CreateCreature` (enemy): allocates the creature and draws its HP (one `niche` draw, unique among the
    /// enemies already attached) but does NOT add it to the enemy list yet (`attach_enemy` = `AddCreature`). Needed by
    /// SurprisePower: the Fat Gremlin is created (HP draw #1, invisible to Sneaky's draw), then Sneaky is added, then Fat.
    pub fn create_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        self.create_enemy_v(monster_id, slot, [0, 0])
    }

    /// `create_enemy` with the monster's private integers set before the HP range is computed.
    pub fn create_enemy_v(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        if !content::monster_implemented(monster_id) {
            self.flag_missing(Kind::Monster, monster_id);
            return None;
        }
        let Some(cid) = self.alloc_slot() else {
            // No free creature slot: the summon cannot happen, which the real game would not do.
            self.overflow |= ov::CREATURES;
            return None;
        };
        let def = content::monster_def(monster_id);
        let (mut lo, mut hi) = (def.hp)(self.ascension);
        let bonus = content::monster_hp_bonus(monster_id, vars);
        lo += bonus;
        hi += bonus;
        // set = {lo..=hi} minus the MaxHp of every enemy already in the list
        let mut cands: crate::util::ArrayVec<i32, 64> = crate::util::ArrayVec::new();
        for hp in lo..=hi {
            if !self.enemies.iter().any(|&e| self.cr(e).max_hp == hp) {
                cands.push(hp);
            }
        }
        let hp = if cands.is_empty() {
            self.rng.niche.next_int_range(lo, hi + 1)
        } else {
            cands[self.rng.niche.next_int_range(0, cands.len() as i32) as usize]
        };
        self.listen |= content::monster_mask(monster_id);
        let mut ms = MonsterState { id: monster_id, cur_state: def.initial, spawned_this_turn: true, ..Default::default() };
        ms.vars[0] = vars[0];
        ms.vars[1] = vars[1];
        let mut cr = Creature::default();
        cr.active = true;
        cr.in_combat = true;
        cr.side = Side::Enemy;
        cr.set_hp(hp);
        cr.max_hp = hp;
        cr.slot = slot;
        cr.monster = ms;
        self.creatures[cid as usize] = cr;
        Some(cid)
    }

    /// `CombatState.AddCreature` + `CombatManager.AddCreature` for a created enemy (`SetUpForCombat`, slot sort).
    pub fn attach_enemy(&mut self, cid: Cid) {
        let def = content::monster_def(self.cr(cid).monster.id);
        let slot = self.cr(cid).slot;
        self.enemies.push(cid);
        // SetUpForCombat: if the initial node is a Move it is logged immediately.
        if matches!(def.nodes[def.initial as usize], MonsterNode::Move { .. }) {
            self.log_move(cid, def.initial);
        }
        if slot != NO {
            self.sort_enemies_by_slot();
        }
    }

    /// `CreatureCmd.Add(monster, state, Enemy, slot)` — mid-combat summon (spec 01 §13.5):
    /// `CreateCreature` (niche HP draw) -> `AddCreature` -> `SetUpForCombat` (`SpawnedThisTurn = true`, slot sort) ->
    /// `AfterAddedToRoom` -> `RollMove` only if it is the player's turn -> `Hook.AfterCreatureAddedToCombat` (unguarded).
    /// A monster spawned during the enemy turn does not act that turn (`SpawnedThisTurn`); one spawned during the
    /// player turn rolls its first move immediately and acts in the following enemy turn.
    /// `vars` are the monster's private integers, set before its spawn hook / first roll run.
    pub fn summon_enemy(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let c = self.create_enemy_v(monster_id, slot, vars)?;
        self.attach_enemy(c);
        self.after_enemy_added(c);
        Some(c)
    }

    /// `CreatureCmd.Add(creature)` after `AddCreature`: `AfterAddedToRoom`, `RollMove` on the player's turn,
    /// `Hook.AfterCreatureAddedToCombat`.
    pub fn after_enemy_added(&mut self, c: Cid) {
        let def = content::monster_def(self.cr(c).monster.id);
        if let Some(f) = def.on_spawn {
            f(self, c);
        }
        if self.side == Side::Player {
            self.roll_move(c);
        }
        self.dispatch_u(hookbit::after_creature_added_to_combat, |cx, me, l| l.after_creature_added_to_combat(cx, me, c));
    }

    /// `EncounterModel.GetNextSlot`: the first slot index in `0..n_slots` not occupied by a current enemy (`NO` if all
    /// are taken).
    pub fn next_free_slot(&self, n_slots: u8) -> u8 {
        (0..n_slots).find(|&s| !self.enemies.iter().any(|&e| self.cr(e).slot == s)).unwrap_or(NO)
    }

    /// `Encounter.Slots.LastOrDefault(s => Enemies.All(c => c.SlotName != s))`: the LAST slot index in `0..n_slots` not
    /// occupied by a current enemy (`NO` if all are taken). Ovicopter eggs, TwoTailedRat backup.
    pub fn last_free_slot(&self, n_slots: u8) -> u8 {
        (0..n_slots).rev().find(|&s| !self.enemies.iter().any(|&e| self.cr(e).slot == s)).unwrap_or(NO)
    }

    /// `CombatState.SortEnemiesBySlotName` — stable (insertion sort, <= 16 elements); unknown slot sorts first.
    pub fn sort_enemies_by_slot(&mut self) {
        let n = self.enemies.len();
        for i in 1..n {
            let x = self.enemies[i];
            let kx = self.slot_key(x);
            let mut j = i;
            while j > 0 && self.slot_key(self.enemies[j - 1]) > kx {
                self.enemies[j] = self.enemies[j - 1];
                j -= 1;
            }
            self.enemies[j] = x;
        }
    }
    fn slot_key(&self, c: Cid) -> i32 {
        let s = self.cr(c).slot;
        if s == NO { -1 } else { s as i32 }
    }

    fn log_move(&mut self, c: Cid, node: u8) {
        let ms = &mut self.creatures[c as usize].monster;
        ms.log[(ms.log_len & 7) as usize] = node;
        ms.log_len += 1;
        ms.ever_logged |= 1u64 << node;
    }

    /// The node of the most recently LOGGED move (`StateLog.Last()`): the last move that was rolled (not necessarily
    /// performed). `NO` if nothing has been logged.
    pub fn last_logged_move(&self, c: Cid) -> u8 {
        let ms = &self.cr(c).monster;
        if ms.log_len == 0 { NO } else { ms.log[((ms.log_len - 1) & 7) as usize] }
    }

    #[inline]
    fn node(&self, c: Cid, n: u8) -> &'static MonsterNode {
        &content::monster_def(self.cr(c).monster.id).nodes[n as usize]
    }

    /// `state.IsMove` (the synthetic STUNNED state is a move).
    fn node_is_move(&self, c: Cid, n: u8) -> bool {
        n == STUN_NODE || matches!(self.node(c, n), MonsterNode::Move { .. })
    }

    /// `CanTransitionAway` of node `n` (`MustPerformOnceBeforeTransitioning` => needs `_performedAtLeastOnce`).
    pub fn can_transition_away(&self, c: Cid, n: u8) -> bool {
        if n == STUN_NODE {
            return self.cr(c).monster.stun_performed;
        }
        match self.node(c, n) {
            MonsterNode::Move { must_perform_once, .. } => !*must_perform_once || self.cr(c).monster.performed_once >> n & 1 != 0,
            _ => true,
        }
    }

    /// Id and intents of the pending move (`NextMove`), including the synthetic STUNNED move.
    pub fn move_view(&self, c: Cid) -> Option<(&'static str, &'static [Intent])> {
        let nm = self.cr(c).monster.next_move;
        if nm == NO {
            return None;
        }
        if nm == STUN_NODE {
            return Some(("STUNNED", &STUN_INTENTS));
        }
        match self.node(c, nm) {
            MonsterNode::Move { id, intents, .. } => Some((id, intents)),
            _ => None,
        }
    }

    /// `Creature.IsStunned` (`NextMove.Id == "STUNNED"`).
    pub fn is_stunned(&self, c: Cid) -> bool {
        self.cr(c).monster.next_move == STUN_NODE
    }

    /// Weight of branch `b` of a random node (spec 04 §1.3).
    fn branch_weight(&self, c: Cid, b: &Branch) -> f32 {
        self.branch_weight_ms(c, &self.cr(c).monster, b)
    }

    /// `branch_weight` against an explicit (possibly hypothetical) monster state: only the move log / once-flags are read
    /// from it; weight lambdas still read the live combat.
    fn branch_weight_ms(&self, c: Cid, ms: &MonsterState, b: &Branch) -> f32 {
        let n = ms.log_len as usize;
        // repeat rule multiplier
        let mult: f32 = match b.repeat {
            Repeat::CanRepeatForever => 1.0,
            Repeat::UseOnlyOnce => {
                if ms.ever_logged >> b.target & 1 != 0 { 0.0 } else { 1.0 }
            }
            Repeat::CanRepeatXTimes(k) => {
                let k = k as usize;
                if n < k {
                    1.0
                } else {
                    // 1 iff any of the last k entries is a different state
                    let mut any_diff = false;
                    for i in 0..k {
                        if ms.log[(n - 1 - i) & 7] != b.target {
                            any_diff = true;
                            break;
                        }
                    }
                    if any_diff { 1.0 } else { 0.0 }
                }
            }
        };
        if b.cooldown > 0 {
            let mut hit = false;
            for i in 0..(b.cooldown as usize).min(n) {
                if ms.log[(n - 1 - i) & 7] == b.target {
                    hit = true;
                    break;
                }
            }
            if hit {
                return 0.0;
            }
        }
        let w = match b.weight_fn {
            Some(f) => f(self, c),
            None => b.weight,
        };
        mult * w
    }

    /// `GetNextState` of the current node: returns the next node index.
    fn next_state(&mut self, c: Cid, cur: u8) -> u8 {
        let def = content::monster_def(self.cr(c).monster.id);
        if cur == STUN_NODE {
            // FollowUpStateId (a move id or a branch id); empty => the machine's initial state.
            let f = self.cr(c).monster.stun_follow_up;
            return if f == NO { def.initial } else { f };
        }
        match &def.nodes[cur as usize] {
            MonsterNode::Move { follow_up, .. } => {
                if *follow_up == NO {
                    def.initial
                } else if *follow_up == crate::defs::FOLLOW_STORED {
                    self.cr(c).monster.stun_follow_up
                } else {
                    *follow_up
                }
            }
            MonsterNode::Random { branches, .. } => {
                let mut ws = [0f32; 12];
                let mut sum = 0f64;
                for (i, b) in branches.iter().enumerate() {
                    ws[i] = self.branch_weight(c, b);
                    sum += ws[i] as f64;
                }
                let max = sum as f32;
                let mut r = self.rng.monster_ai.next_float_max(max);
                for (i, b) in branches.iter().enumerate() {
                    // weight lambdas are re-evaluated during the walk (same values: no mutation in between)
                    r -= ws[i];
                    if r <= 0.0 {
                        return b.target;
                    }
                }
                // f32 rounding: the draw is below the (double-accumulated) total, but subtracting the weights one by one in f32 can leave a tiny positive
                // remainder. The game throws here (`RandomBranchState.GetNextState`, a crash that rare); a long search job must not abort, so the
                // remainder goes to the last branch with positive weight (where it lies).
                let last = (0..branches.len()).rev().find(|&i| ws[i] > 0.0).unwrap_or(branches.len() - 1);
                branches[last].target
            }
            MonsterNode::Cond { arms, .. } => {
                for (target, pred) in arms.iter() {
                    if pred(self, c) {
                        return *target;
                    }
                }
                panic!("ConditionalBranchState: no arm true");
            }
        }
    }

    /// `OnExitState` of node `n`: clears its `_performedAtLeastOnce`.
    fn on_exit_state(&mut self, c: Cid, n: u8) {
        let ms = &mut self.creatures[c as usize].monster;
        if n == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << n);
        }
    }

    /// `Creature.PrepareForNextTurn(rollNewMove: true)`: rolls a move unless the state machine was reset (a monster
    /// removed from combat has none).
    pub fn prepare_for_next_turn(&mut self, c: Cid) {
        if self.cr(c).in_combat {
            self.roll_move(c);
        }
    }

    /// `MonsterModel.RollMove` → `MonsterMoveStateMachine.RollMove` (spec 04 §1.2).
    pub fn roll_move(&mut self, c: Cid) {
        let cur = self.cr(c).monster.cur_state;
        let performed_first = self.cr(c).monster.performed_first;
        if !self.can_transition_away(c, cur) || (!performed_first && self.node_is_move(c, cur)) {
            // no transition, no RNG
            self.creatures[c as usize].monster.next_move = cur;
            return;
        }
        let mut cur = cur;
        let mut first_logged = NO;
        loop {
            if !self.tick() {
                return; // runaway-work safeguard (`engine/budget.rs`): a state machine cycling without reaching a move
            }
            let nxt = self.next_state(c, cur);
            self.on_exit_state(c, cur);
            cur = nxt;
            self.creatures[c as usize].monster.cur_state = cur;
            if first_logged == NO && self.node_is_move(c, cur) {
                first_logged = cur;
            }
            if self.node_is_move(c, cur) {
                break;
            }
        }
        self.log_move(c, first_logged);
        self.creatures[c as usize].monster.next_move = cur;
    }

    /// `MonsterModel.SetMoveImmediate(state, forceTransition)` (spec 04 §1.8): only if the pending move can be
    /// transitioned away from (or `force`): `NextMove = state; machine.ForceCurrentState(state)` — the old state's
    /// `OnExitState` runs; the forced state is NOT logged.
    pub fn set_move_immediate(&mut self, c: Cid, node: u8, force: bool) {
        let nm = self.cr(c).monster.next_move;
        let can = nm == NO || self.can_transition_away(c, nm);
        if can || force {
            let cur = self.cr(c).monster.cur_state;
            self.on_exit_state(c, cur);
            let ms = &mut self.creatures[c as usize].monster;
            ms.next_move = node;
            ms.cur_state = node;
        }
    }

    /// `CreatureCmd.Stun(creature, stunMove, nextMoveId)` / `Creature.StunInternal`: ignored for a dead or detached
    /// creature. `next_move = None` defaults to the last LOGGED move (the pending move being interrupted). The
    /// `STUNNED` move must be performed once before the machine can transition away from it; it then follows
    /// `next_move` (a move node or a branch node, walked with its RNG draw).
    pub fn stun(&mut self, c: Cid, stun_move: Option<MoveFn>, next_move: Option<u8>) {
        if self.cr(c).is_player || !self.cr(c).in_combat || self.cr(c).is_dead() {
            return;
        }
        let follow = match next_move {
            Some(n) => n,
            None => {
                let l = self.last_logged_move(c);
                assert!(l != NO, "StateLog is empty");
                l
            }
        };
        let nm = self.cr(c).monster.next_move;
        // SetMoveImmediate(state) without force: a creature already holding an un-performed MustPerform state ignores
        // a second stun (the new state is built and discarded).
        if nm == NO || self.can_transition_away(c, nm) {
            let ms = &mut self.creatures[c as usize].monster;
            ms.stun_follow_up = follow;
            ms.stun_move = stun_move;
            ms.stun_performed = false;
            self.set_move_immediate(c, STUN_NODE, false);
        }
    }

    /// `MonsterModel.PerformMove` (spec 04 §1.9).
    /// Returns `Some(node)` if the move suspended on a decision (the caller keeps `node` for `finish_move`).
    pub fn perform_move(&mut self, c: Cid) -> Option<u8> {
        let def = content::monster_def(self.cr(c).monster.id);
        let nm = self.cr(c).monster.next_move;
        assert!(nm != NO, "monster performing UNSET_MOVE");
        self.creatures[c as usize].monster.is_performing = true;
        if nm == STUN_NODE {
            let f = self.cr(c).monster.stun_move;
            self.creatures[c as usize].monster.stun_performed = true;
            if let Some(f) = f {
                f(self, c);
            }
        } else if let MonsterNode::Move { perform, .. } = &def.nodes[nm as usize] {
            self.creatures[c as usize].monster.performed_once |= 1u64 << nm;
            perform(self, c);
        }
        if self.stage == Stage::AwaitChoice {
            // The move raised a decision (Knowledge Demon): the bookkeeping below runs in `finish_move` once it resumed.
            return Some(nm);
        }
        self.finish_move(c, nm);
        None
    }

    /// The tail of `PerformMove`: `OnMovePerformed`, history, `IsPerformingMove = false` and the removal of a creature
    /// that died during its own move.
    pub(crate) fn finish_move(&mut self, c: Cid, nm: u8) {
        {
            let ms = &mut self.creatures[c as usize].monster;
            ms.performed.copy_within(1..4, 0);
            ms.performed[3] = nm;
            ms.performed_first = true;
        }
        self.hist_push(crate::engine::HKind::MonsterPerformedMove, c, NO, nm as u16, NO, 0, 0, 0, 0);
        self.creatures[c as usize].monster.is_performing = false;
        if self.cr(c).is_dead() && self.cr(c).in_combat && self.should_creature_be_removed_after_death(c) {
            self.detach_creature(c);
        }
    }

    /// The listener `Me` of a monster creature.
    pub fn monster_me(&self, c: Cid) -> Me {
        Me { kind: Kind::Monster, owner: c, idx: 0, id: self.cr(c).monster.id, amount: 0 }
    }
}


// ---- Expert pattern knowledge: what an experienced player knows about the upcoming turns -------------------------------
//
// The game only shows the CURRENT intent, but experienced players know each monster's pattern by heart: the cycle of a
// deterministic monster, the possible moves (with their odds) at random branches, and what the monster does to itself on the
// way (falls asleep / wakes up, counts down to a summon, buffs itself). `lookahead` reproduces exactly that knowledge by playing
// the next turns forward on a projected COPY of the combat (`look_turn`): the player passes, the engine runs each enemy turn
// (moves performed, powers ticking, summons), and at every roll the monster's state machine branches into each possible move
// with the game's odds, conditions and weights reading the projected combat. The copy's RNG streams are replaced by a fixed seed:
// the real random state is never read (a random effect inside a projected move gets an arbitrary fixed outcome).
//
// Projections are per monster. The monster looked at branches; so do its peers when its odds read them (`LOOK_JOINT`: Two-Tailed
// Rats read each other's pending summon and share a call count, so all rats branch jointly). Every other enemy takes its most
// likely move at each roll, which keeps their deterministic patterns (summons, deaths) acting on the projected combat without
// multiplying the paths.

/// Future turns covered (turn +1 .. +LOOK_H after the one whose intent is shown).
pub const LOOK_H: usize = 4;
/// Move-node slots per horizon (nodes >= LOOK_NODES-1, and the synthetic STUNNED node, share the last slot).
pub const LOOK_NODES: usize = 16;
/// Projected combats kept per horizon after merging identical ones; beyond it the least likely are dropped (a row then sums
/// to less than 1).
const LOOK_PATHS: usize = 8;
/// Monsters whose roll reads the pending moves / counters of the other enemies with the same id: they branch jointly.
const LOOK_JOINT: &[u16] = &[crate::ids::monster::TWO_TAILED_RAT];
/// Run seed of the projected combat's RNG streams.
const LOOK_SEED: u64 = 0x10_0CA4_EAD;

/// Use the look-ahead from before S1 (each monster's machine walked alone over 3 turns, conditions reading the current combat).
/// Off by default; kept to reproduce observations of networks trained before (A/B tests, the compatibility check of `rl/model.py`).
pub static LOOK_LEGACY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Per-horizon knowledge: probability of each move node being the monster's move, and the expected total attack damage
/// (per-hit damage as the intent would show it in the projected combat x hits, probability-weighted).
#[derive(Clone, Copy)]
pub struct LookRow {
    pub prob: [f32; LOOK_NODES],
    pub exp_damage: f32,
}

const EMPTY_ROW: LookRow = LookRow { prob: [0.0; LOOK_NODES], exp_damage: 0.0 };

/// Outcomes of one roll: the monster's machine state after it, and the probability.
type Outcomes = crate::util::ArrayVec<(MonsterState, f32), 48>;
/// Per horizon: (move node, probability, probability x attack damage), one entry per node.
type LookList = crate::util::ArrayVec<(u8, f32, f32), 20>;

/// Cycle counters of the look-ahead's projection phases (feature `obs_prof`, `observe::OBS_PROF[14..]`; not thread safe, diagnostics only).
macro_rules! lprof {
    ($k:expr, $body:expr) => {{
        #[cfg(feature = "obs_prof")]
        let t = unsafe { core::arch::x86_64::_rdtsc() };
        let r = $body;
        #[cfg(feature = "obs_prof")]
        unsafe {
            crate::observe::OBS_PROF[$k] += core::arch::x86_64::_rdtsc() - t;
        }
        r
    }};
}
macro_rules! lcount {
    ($k:expr, $n:expr) => {{
        #[cfg(feature = "obs_prof")]
        unsafe {
            crate::observe::OBS_PROF[$k] += $n as u64;
        }
    }};
}

impl Combat {
    /// `RollMove` continued on a hypothetical state: leave `left`, enter `to`, and keep walking branch states until a move.
    fn look_enter(&self, c: Cid, mut ms: MonsterState, left: u8, to: u8, first: u8, p: f32, out: &mut Outcomes) {
        if left == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << left);
        }
        ms.cur_state = to;
        let is_move = self.node_is_move(c, to);
        let first = if first == NO && is_move { to } else { first };
        if is_move {
            ms.log[(ms.log_len & 7) as usize] = first;
            ms.log_len += 1;
            if first < 64 {
                ms.ever_logged |= 1u64 << first;
            }
            ms.next_move = to;
            if out.len() < 48 {
                out.push((ms, p));
            }
            return;
        }
        let def = content::monster_def(ms.id);
        match &def.nodes[to as usize] {
            MonsterNode::Cond { arms, .. } => {
                for (target, pred) in arms.iter() {
                    if pred(self, c) {
                        self.look_enter(c, ms, to, *target, first, p, out);
                        return;
                    }
                }
            }
            MonsterNode::Random { branches, .. } => {
                let mut ws = [0f32; 12];
                let mut sum = 0f64;
                for (i, b) in branches.iter().enumerate().take(12) {
                    ws[i] = self.branch_weight_ms(c, &ms, b);
                    sum += ws[i] as f64;
                }
                if sum <= 0.0 {
                    // all weights 0: the real walk takes the first branch (`0 - 0 <= 0`)
                    if let Some(b) = branches.first() {
                        self.look_enter(c, ms, to, b.target, first, p, out);
                    }
                    return;
                }
                for (i, b) in branches.iter().enumerate().take(12) {
                    if ws[i] > 0.0 {
                        self.look_enter(c, ms, to, b.target, first, p * (ws[i] as f64 / sum) as f32, out);
                    }
                }
            }
            MonsterNode::Move { .. } => {}
        }
    }

    /// Every outcome of the roll monster `c` makes now (`roll_move` with each random branch taken), conditions and weights
    /// reading this combat.
    fn look_outcomes(&self, c: Cid, out: &mut Outcomes) {
        let ms = self.cr(c).monster;
        let cur = ms.cur_state;
        if !self.can_transition_away(c, cur) || (!ms.performed_first && self.node_is_move(c, cur)) {
            out.push((MonsterState { next_move: cur, ..ms }, 1.0));
            return;
        }
        let def = content::monster_def(ms.id);
        let nxt = if cur == STUN_NODE {
            if ms.stun_follow_up == NO { def.initial } else { ms.stun_follow_up }
        } else {
            match &def.nodes[cur as usize] {
                MonsterNode::Move { follow_up, .. } if *follow_up == NO => def.initial,
                MonsterNode::Move { follow_up, .. } if *follow_up == crate::defs::FOLLOW_STORED => ms.stun_follow_up,
                MonsterNode::Move { follow_up, .. } => *follow_up,
                // a monster that has not rolled yet (summoned during the enemy turn) starts its walk at its initial branch
                _ => cur,
            }
        };
        self.look_enter(c, ms, cur, nxt, NO, 1.0, out);
    }

    /// Roll `i` of a projected turn: the `i`-th enemy of the list (as in `start_player_turn`) rolls on this combat. A forking
    /// monster (`fork`) sends one combat per possible move to `out`, any other takes its most likely move. `random` collects the
    /// monsters whose roll had more than one possible outcome.
    fn look_roll_one(mut self: Box<Self>, i: usize, fork: &impl Fn(&Combat, Cid) -> bool, p: f32, out: &mut Vec<(Box<Combat>, f32)>, random: &mut u16) {
        let Some(e) = self.enemies.get(i) else { return out.push((self, p)) };
        let mut outs = Outcomes::new();
        if self.cr(e).in_combat {
            self.look_outcomes(e, &mut outs);
        }
        if outs.len() > 1 {
            *random |= 1 << e;
        }
        if outs.len() <= 1 || !fork(&self, e) {
            // the first most likely outcome
            if let Some(best) = outs.iter().fold(None, |b: Option<(MonsterState, f32)>, &o| if b.is_some_and(|b| b.1 >= o.1) { b } else { Some(o) }) {
                self.creatures[e as usize].monster = best.0;
            }
            return out.push((self, p));
        }
        let n = outs.len();
        lcount!(20, n - 1);
        for &(m, q) in outs.iter().take(n - 1) {
            let mut cx = look_box(&self);
            cx.creatures[e as usize].monster = m;
            out.push((cx, p * q));
        }
        self.creatures[e as usize].monster = outs[n - 1].0;
        out.push((self, p * outs[n - 1].1));
    }

    /// The projected moves of the enemies `who`: per enemy and future turn, each possible move node with its probability and
    /// expected damage. `fork` says which monsters branch at their rolls (the others take their most likely move). Also returns
    /// the set of monsters (bit = creature id) that had a choice at some roll.
    fn look_project(&self, who: &[Cid], fork: impl Fn(&Combat, Cid) -> bool) -> (crate::util::ArrayVec<[LookList; LOOK_H], MAX_CREATURES>, u16) {
        let mut random = 0u16;
        let mut lists = crate::util::ArrayVec::<[LookList; LOOK_H], MAX_CREATURES>::new();
        let mut ids = [0u16; MAX_CREATURES];
        for (k, &c) in who.iter().enumerate() {
            lists.push(core::array::from_fn(|_| LookList::new()));
            ids[k] = self.cr(c).monster.id;
        }
        lcount!(19, 1);
        crate::util::quiet(|| {
            let mut base = lprof!(14, look_box(self));
            base.rng = *look_rng();
            base.auto_select = true;
            base.replay = None;
            // the player passes and is inert: no powers, relics, cards or block act in the projection, and it cannot die
            base.player_hooks_active = false;
            let pl = &mut base.creatures[PLAYER as usize];
            pl.set_hp(1 << 24);
            pl.max_hp = 1 << 24;
            pl.set_block(0);
            pl.powers.clear();
            // the enemies' HP and block start out pristine: a read of a starting value is recorded (`look_dep`), so a projection that reads
            // none is cached for every HP and block (`lookahead_with`)
            for cr in base.creatures.iter_mut().filter(|cr| cr.active && !cr.is_player) {
                cr.pristine = PRISTINE_HP | PRISTINE_BLOCK;
            }
            // path buffers, reused across turns and rolls (and across projections: the thread's `LOOK_VECS`)
            let [mut paths, mut next, mut rolled] = LOOK_VECS.with(|v| core::mem::take(&mut *v.borrow_mut()));
            paths.push((base, 1.0));
            for h in 0..LOOK_H {
                lcount!(21, paths.len());
                lprof!(15, for (mut cx, p) in paths.drain(..) {
                    if cx.look_turn() {
                        next.push((cx, p));
                    } else {
                        look_free(cx);
                    }
                });
                // the enemies roll one after the other (a roll can read the moves rolled before it); the paths are merged and
                // capped once all have rolled, and in between when they outgrow the cap
                let n_roll = next.iter().map(|(cx, _)| cx.enemies.len()).max().unwrap_or(0);
                for i in 0..n_roll {
                    lprof!(16, for (cx, p) in next.drain(..) {
                        cx.look_roll_one(i, &fork, p, &mut rolled, &mut random);
                    });
                    core::mem::swap(&mut next, &mut rolled);
                    if next.len() > LOOK_PATHS || (i + 1 == n_roll && next.len() > 1) {
                        lprof!(17, look_merge(&mut next));
                    }
                }
                lprof!(18, for (cx, p) in next.iter_mut() {
                    // the damage the intent would show: the projected monster against the player's current modifiers
                    let mut inert = crate::util::ArrayVec::new();
                    inert.copy_from(&cx.creatures[PLAYER as usize].powers);
                    cx.creatures[PLAYER as usize].powers.copy_from(&self.cr(PLAYER).powers);
                    cx.player_hooks_active = self.player_hooks_active;
                    for (k, &f) in who.iter().enumerate() {
                        let cr = cx.cr(f);
                        let node = cr.monster.next_move;
                        if !cr.is_alive() || !cr.in_combat || cr.monster.id != ids[k] || node == NO {
                            continue;
                        }
                        let pd = *p * cx.node_attack_damage(f, node);
                        let list = &mut lists[k][h];
                        match list.as_mut_slice().iter_mut().find(|e| e.0 == node) {
                            Some(e) => {
                                e.1 += *p;
                                e.2 += pd;
                            }
                            None => list.push((node, *p, pd)),
                        }
                    }
                    cx.creatures[PLAYER as usize].powers.copy_from(&inert);
                    cx.player_hooks_active = false;
                });
                core::mem::swap(&mut paths, &mut next);
                if paths.is_empty() {
                    break;
                }
            }
            for (cx, _) in paths.drain(..) {
                look_free(cx);
            }
            for (cx, _) in next.drain(..).chain(rolled.drain(..)) {
                look_free(cx);
            }
            LOOK_VECS.with(|v| *v.borrow_mut() = [paths, next, rolled]);
        });
        (lists, random)
    }

    /// `look_project` of one monster: it branches (with its joint peers, `LOOK_JOINT`), the other enemies take their most likely moves.
    fn look_project_one(&self, f: Cid) -> [LookList; LOOK_H] {
        self.look_project(&[f], look_fork(self, f)).0[0]
    }

    /// What a human reads off the move pattern: per future turn (+1 .. +LOOK_H after the shown intent), each possible move with its
    /// probability and a short intent text ("30 + status", "7x2", "buff"), damage with the current modifiers, followed by the projected
    /// total when the projection changes it ("12 (projected 15)": Strength gained on the way, Weak expired). Same projection as
    /// `lookahead`; consumes no RNG.
    pub fn intent_plan(&self, c: Cid) -> Vec<Vec<(String, f32, String)>> {
        let cr = self.cr(c);
        if !cr.is_alive() || !cr.in_combat || cr.monster.next_move == NO || cr.is_player {
            return Vec::new();
        }
        let def = content::monster_def(cr.monster.id);
        self.look_project_one(c)
            .iter()
            .map(|list| {
                let mut acc: Vec<(u8, f32, f32)> = list.iter().copied().collect();
                acc.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
                acc.into_iter()
                    .map(|(node, p, pd)| {
                        if node == STUN_NODE {
                            return ("STUNNED".to_string(), p, "stunned".to_string());
                        }
                        let MonsterNode::Move { id, intents, .. } = &def.nodes[node as usize] else { return ("?".to_string(), p, String::new()) };
                        let parts: Vec<String> = intents
                            .iter()
                            .map(|it| match it {
                                Intent::Attack { damage, hits } => {
                                    let (d, h) = (self.intent_damage(c, damage(self, c)), hits(self, c));
                                    if h > 1 { format!("{d}x{h}") } else { format!("{d}") }
                                }
                                Intent::DeathBlowAttack { damage } => format!("{} (dies)", self.intent_damage(c, damage(self, c))),
                                Intent::StatusCard => "status".into(),
                                Intent::CardDebuff => "card debuff".into(),
                                Intent::Buff => "buff".into(),
                                Intent::Debuff => "debuff".into(),
                                Intent::DebuffStrong => "strong debuff".into(),
                                Intent::Defend => "block".into(),
                                Intent::Escape => "escape".into(),
                                Intent::Heal => "heal".into(),
                                Intent::Hidden => "hidden".into(),
                                Intent::Summon => "summon".into(),
                                Intent::Sleep => "sleep".into(),
                                Intent::Stun => "stun".into(),
                                Intent::DeathBlow => "death blow".into(),
                            })
                            .collect();
                        let mut text = parts.join(" + ");
                        let projected = if p > 0.0 { pd / p } else { 0.0 };
                        if (projected - self.node_attack_damage(c, node)).abs() >= 0.5 {
                            text = format!("{text} (projected {projected:.0})");
                        }
                        let fx = self.move_effects(c, node);
                        if !fx.is_empty() {
                            text = format!("{text} [{fx}]");
                        }
                        (id.trim_end_matches("_MOVE").to_string(), p, text)
                    })
                    .collect()
            })
            .collect()
    }

    /// What one move does besides its damage, found by performing it on a copy of the combat (the player's block set huge so the copy cannot die or
    /// lose HP): debuffs / buffs on me, cards it adds to my piles, its own powers and block, summons. E.g. "me: Vulnerable +3; discard: +3 Wound; self: Strength +2".
    /// Amounts are those of the move performed now (a scaling move shows today's numbers). Consumes no RNG of the real combat.
    pub fn move_effects(&self, c: Cid, node: u8) -> String {
        use std::collections::BTreeMap;
        let mut cx = self.clone();
        cx.budget_reset();
        cx.rng = *look_rng();
        cx.creatures[PLAYER as usize].set_block(1 << 20);
        cx.creatures[c as usize].monster.next_move = node;
        let powers = |cx: &Combat, who: Cid| -> BTreeMap<u16, i32> {
            let mut m = BTreeMap::new();
            for p in cx.cr(who).powers.iter() {
                *m.entry(p.id).or_insert(0) += p.amount;
            }
            m
        };
        let piles = |cx: &Combat| -> [BTreeMap<u16, i32>; 4] {
            let mut out: [BTreeMap<u16, i32>; 4] = Default::default();
            for (k, pile) in [&cx.player.hand, &cx.player.draw, &cx.player.discard, &cx.player.exhaust].iter().enumerate() {
                for &ci in pile.iter() {
                    *out[k].entry(cx.cards[ci as usize].id).or_insert(0) += 1;
                }
            }
            out
        };
        let (me0, self0, piles0, blk0) = (powers(&cx, PLAYER), powers(&cx, c), piles(&cx), cx.cr(c).block());
        let alive0 = cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).count();
        let _ = cx.perform_move(c);
        let (me1, self1, piles1, blk1) = (powers(&cx, PLAYER), powers(&cx, c), piles(&cx), cx.cr(c).block());
        let alive1 = cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).count();
        let pdiff = |a: &BTreeMap<u16, i32>, b: &BTreeMap<u16, i32>| -> Vec<String> {
            let keys: std::collections::BTreeSet<u16> = a.keys().chain(b.keys()).copied().collect();
            keys.into_iter()
                .filter_map(|k| {
                    let d = b.get(&k).copied().unwrap_or(0) - a.get(&k).copied().unwrap_or(0);
                    (d != 0).then(|| format!("{} {:+}", crate::ids::power::NAMES[k as usize].trim_end_matches("_POWER"), d))
                })
                .collect()
        };
        let mut out = Vec::new();
        let me = pdiff(&me0, &me1);
        if !me.is_empty() {
            out.push(format!("me: {}", me.join(", ")));
        }
        for (k, name) in ["hand", "draw", "discard", "exhaust"].iter().enumerate() {
            let added: Vec<String> = piles1[k]
                .iter()
                .filter_map(|(&id, &n)| {
                    let d = n - piles0[k].get(&id).copied().unwrap_or(0);
                    (d > 0).then(|| format!("+{d} {}", crate::ids::card::NAMES[id as usize]))
                })
                .collect();
            if !added.is_empty() {
                out.push(format!("{name}: {}", added.join(", ")));
            }
        }
        let mut me_self = pdiff(&self0, &self1);
        if blk1 > blk0 {
            me_self.push(format!("block +{}", blk1 - blk0));
        }
        if !me_self.is_empty() {
            out.push(format!("self: {}", me_self.join(", ")));
        }
        if alive1 > alive0 {
            out.push(format!("summons {}", alive1 - alive0));
        }
        out.join("; ")
    }

    /// Expected total attack damage of one move node (what its attack intents will show / do against the player).
    fn node_attack_damage(&self, c: Cid, node: u8) -> f32 {
        if node == STUN_NODE {
            return 0.0;
        }
        let def = content::monster_def(self.cr(c).monster.id);
        let MonsterNode::Move { intents, .. } = &def.nodes[node as usize] else { return 0.0 };
        let mut total = 0i64;
        for it in intents.iter() {
            match it {
                Intent::Attack { damage, hits } => total += self.intent_damage(c, damage(self, c)) as i64 * hits(self, c) as i64,
                Intent::DeathBlowAttack { damage } => total += self.intent_damage(c, damage(self, c)) as i64,
                _ => {}
            }
        }
        total as f32
    }

    /// The cache key of `look_project(c)` from a digest of this combat (`LookDigests`): the whole projected world, where the turn stands, and `c`.
    fn look_key_of(&self, c: Cid, digest: u64) -> u64 {
        let mut h = digest;
        let v = c as u64
            | (self.round as u32 as u64) << 8
            | (self.player.turn_number as u32 as u64) << 24
            | (self.side as u64) << 40
            | (self.stage as u64) << 44
            | (self.enemy_cont.is_some() as u64) << 50
            | (LOOK_LEGACY.load(std::sync::atomic::Ordering::Relaxed) as u64) << 51;
        h = (h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
        h
    }

    /// The monster's move distribution for the next `LOOK_H` turns after the current intent (see the section comment).
    pub fn lookahead(&self, c: Cid) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, true, &mut LookDigests::default())
    }

    /// `lookahead` of several enemies of this (unchanged) combat: `d` keeps the digests the keys are made of, computed once for all of them.
    pub fn lookahead_shared(&self, c: Cid, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, true, d)
    }

    /// `lookahead` without the cache (tests compare the two).
    pub fn lookahead_fresh(&self, c: Cid) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, false, &mut LookDigests::default())
    }

    fn lookahead_with(&self, c: Cid, cached: bool, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        let cr = self.cr(c);
        let legacy = LOOK_LEGACY.load(std::sync::atomic::Ordering::Relaxed);
        if !cr.is_alive() || !cr.in_combat || cr.monster.next_move == NO || cr.is_player || (self.stage == Stage::Over && !legacy) {
            return [EMPTY_ROW; LOOK_H];
        }
        // Two keys: the exact one, and (not for the legacy look-ahead, which reads the live combat) the relaxed one of a projection that read no
        // enemy's starting HP or block beyond alive / dead (`Creature::pristine`). A projection runs the same steps for every HP and block it does not
        // read, so its rows hold for all of them: in a 5x32 search 60% of the states the exact key misses differ from a cached one only there.
        #[cfg(feature = "obs_prof")]
        let t0 = unsafe { core::arch::x86_64::_rdtsc() };
        let rkey = if cached && !legacy { self.look_key_of(c, d.relaxed(self)) } else { 0 };
        #[cfg(feature = "obs_prof")]
        unsafe { crate::observe::OBS_PROF[10] += core::arch::x86_64::_rdtsc() - t0; }
        #[cfg(feature = "obs_prof")]
        let t1 = unsafe { core::arch::x86_64::_rdtsc() };
        let rows = if !cached { self.look_rows(c, false, d) } else { LOOK_CACHE.with(|t| {
            let mut t = t.borrow_mut();
            #[cfg(feature = "obs_prof")]
            unsafe { crate::observe::OBS_PROF[12] += 1; }
            if !legacy {
                if let Some(r) = t.get(rkey) {
                    if LOOK_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
                        look_verify(self, c, &r);
                    }
                    return r;
                }
            }
            let key = self.look_key_of(c, d.key(self));
            if let Some(r) = t.get(key) {
                if LOOK_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
                    look_verify(self, c, &r);
                }
                return r;
            }
            #[cfg(feature = "obs_prof")]
            unsafe { crate::observe::OBS_PROF[13] += 1; }
            let outer = LOOK_DEP.with(|d| d.replace(false));
            let r = self.look_rows(c, true, d);
            let dep = LOOK_DEP.with(|d| d.replace(outer || d.get()));
            t.put(key, r);
            if !dep && !legacy {
                t.put(rkey, r);
            }
            r
        }) };
        #[cfg(feature = "obs_prof")]
        unsafe { crate::observe::OBS_PROF[11] += core::arch::x86_64::_rdtsc() - t1; }
        rows
    }

    /// The enemies a projection reports on: those alive with a pending move.
    fn look_who(&self) -> crate::util::ArrayVec<Cid, MAX_CREATURES> {
        let mut v = crate::util::ArrayVec::new();
        for &e in self.enemies.iter() {
            let cr = self.cr(e);
            if cr.is_alive() && cr.in_combat && cr.monster.next_move != NO {
                v.push(e);
            }
        }
        v
    }

    /// Rows of every enemy (by creature id) from a projection of `who`, and the set of monsters that had a choice at some roll.
    fn look_project_rows(&self, who: &[Cid], fork: impl Fn(&Combat, Cid) -> bool) -> ([[LookRow; LOOK_H]; MAX_CREATURES], u16) {
        let (lists, random) = self.look_project(who, fork);
        let mut rows = [[EMPTY_ROW; LOOK_H]; MAX_CREATURES];
        for (&e, l) in who.iter().zip(lists.iter()) {
            rows[e as usize] = look_rows_of(l);
        }
        (rows, random)
    }

    /// Rows of monster `c`. A monster that never has a choice has the same rows in every projection in which it does not branch, so
    /// one projection in which nobody branches (the "mode" projection) serves all of them; a monster with a choice gets its own.
    /// `cached`: the mode projection of this combat is memoized (`LOOK_MODE`).
    fn look_rows(&self, c: Cid, cached: bool, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        if LOOK_LEGACY.load(std::sync::atomic::Ordering::Relaxed) {
            return self.legacy_rows(c);
        }
        let key = if cached { self.look_key_of(NO, d.key(self)) } else { 0 };
        let memo = if cached { LOOK_MODE.with(|m| m.borrow().filter(|m| m.0 == key)) } else { None };
        if let Some((_, rows, random, dep)) = memo {
            if dep {
                look_dep(); // the memoized projection read a starting HP or block
            }
            return if random >> c & 1 == 0 { rows[c as usize] } else { look_rows_of(&self.look_project_one(c)) };
        }
        // c's own projection, reported for every enemy: if c had no choice, it is the mode projection
        let who = self.look_who();
        let outer = LOOK_DEP.with(|d| d.replace(false));
        let (rows, random) = self.look_project_rows(who.as_slice(), look_fork(self, c));
        let dep = LOOK_DEP.with(|d| d.replace(outer || d.get()));
        if random >> c & 1 == 0 && cached {
            LOOK_MODE.with(|m| *m.borrow_mut() = Some((key, rows, random, dep)));
        }
        rows[c as usize]
    }
}

/// Which monsters branch in the projection of monster `f`: itself and its joint peers.
fn look_fork(cx: &Combat, f: Cid) -> impl Fn(&Combat, Cid) -> bool {
    let fid = cx.cr(f).monster.id;
    let joint = LOOK_JOINT.contains(&fid);
    move |cx: &Combat, e: Cid| e == f || (joint && cx.cr(e).monster.id == fid)
}

/// Observation rows of projected move lists.
fn look_rows_of(lists: &[LookList; LOOK_H]) -> [LookRow; LOOK_H] {
    let mut rows = [EMPTY_ROW; LOOK_H];
    for (row, list) in rows.iter_mut().zip(lists.iter()) {
        for &(node, p, pd) in list.iter() {
            row.prob[look_slot(node)] += p;
            row.exp_damage += pd;
        }
    }
    rows
}

/// The two digests of a combat the look-ahead cache keys are made of (`look_key_of`), each computed when first needed: the enemies of one
/// observation look ahead on the same combat and share them (`Combat::lookahead_shared`). Only valid for one unchanged combat.
#[derive(Default)]
pub struct LookDigests {
    relaxed: Option<u64>,
    key: Option<u64>,
}

impl LookDigests {
    #[inline]
    fn relaxed(&mut self, cx: &Combat) -> u64 {
        *self.relaxed.get_or_insert_with(|| look_digest_of(cx, Digest::Relaxed))
    }
    #[inline]
    fn key(&mut self, cx: &Combat) -> u64 {
        *self.key.get_or_insert_with(|| look_digest_of(cx, Digest::Key))
    }
}

/// The hooks through which the player's powers and relics reach a look-ahead's rows. The projection itself runs with the player's powers cleared
/// and its hooks inactive (`look_project`: relics, potions and cards do not listen), so only the intent damage reads them
/// (`node_attack_damage` -> `intent_damage` -> `modify_damage_value`: the listeners of these three hooks; the monsters' damage and hit-count
/// functions read the ascension and the monster's own state only). The cache keys hold the player's powers and relics that have one of these
/// hooks, the player powers such a hook reads by id (`LOOK_READ_POWERS`) and the relics the engine reads by id (`LOOK_READ_RELICS`); no other
/// power or relic of the player can change the rows.
const LOOK_PLAYER_HOOKS: crate::hooks::Mask = crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_additive)
    .or(crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_multiplicative))
    .or(crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_cap));
/// Player powers a damage hook reads by id: Debilitate (Vulnerable's multiplier on its own owner).
const LOOK_READ_POWERS: [u16; 1] = [crate::ids::power::DEBILITATE_POWER];
/// Relics read with `has_relic` (Paper Krane and Paper Phrog by Weak / Vulnerable, Whispering Earring at turn 1), whatever their hooks.
const LOOK_READ_RELICS: [u16; 3] = [crate::ids::relic::PAPER_KRANE, crate::ids::relic::PAPER_PHROG, crate::ids::relic::WHISPERING_EARRING];

#[derive(Clone, Copy, PartialEq)]
enum Digest {
    /// the cache key: everything
    Key,
    /// merging projected combats in `look_project`: no player, no relics, only the part of a monster's move log its rolls can read
    /// (`look_memory`), no power uids, no performed-move history
    Canon,
    /// the relaxed cache key: `Key` with the enemies' HP reduced to alive / dead and without their block
    Relaxed,
}

/// Digest of what makes two combats different for the look-ahead: every creature (machine state, powers, HP, block, presence;
/// the inert projected player only by its powers) and the player's relics; less for `Digest::Canon` / `Relaxed`. (It reads HP and block
/// directly: a digest is not a read the projection's result depends on, see `look_merge`.)
fn look_digest_of(cx: &Combat, mode: Digest) -> u64 {
    #[inline(always)]
    fn mix(h: &mut u64, v: u64) {
        *h = (*h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
    }
    let canon = mode == Digest::Canon;
    let mut h = if mode == Digest::Relaxed { 0x84222325cbf29ce4u64 } else { 0xcbf29ce484222325u64 };
    for (i, cr) in cx.creatures.iter().enumerate() {
        if !cr.active || (canon && cr.is_player) {
            continue;
        }
        if !cr.is_player {
            if mode == Digest::Relaxed {
                mix(&mut h, i as u64 | (cr.in_combat as u64) << 8 | (cr.slot as u64) << 16 | ((cr.hp > 0) as u64) << 32);
                mix(&mut h, cr.max_hp as u32 as u64);
            } else {
                mix(&mut h, i as u64 | (cr.in_combat as u64) << 8 | (cr.slot as u64) << 16 | (cr.hp as u32 as u64) << 32);
                mix(&mut h, cr.max_hp as u32 as u64 | (cr.block as u32 as u64) << 32);
            }
        }
        for p in cr.powers.as_slice() {
            if cr.is_player && !content::power_mask(p.id).intersects(LOOK_PLAYER_HOOKS) && !LOOK_READ_POWERS.contains(&p.id) {
                continue; // a player power the look-ahead cannot read (`LOOK_PLAYER_HOOKS`)
            }
            mix(&mut h, (p.id as u64) << 48 | (if canon { 0 } else { p.uid as u64 }) << 32 | p.amount as u32 as u64);
            mix(&mut h, p.aux as u32 as u64 | (p.applier as u64) << 32 | (p.skip_next_tick as u64) << 40);
        }
        if cr.is_player {
            continue;
        }
        let m = &cr.monster;
        mix(&mut h, m.id as u64 | (m.cur_state as u64) << 16 | (m.next_move as u64) << 24 | (m.performed_first as u64) << 32 | (m.spawned_this_turn as u64) << 33
            | (m.is_performing as u64) << 34 | (m.stunned as u64) << 35 | (m.stun_performed as u64) << 36 | (m.stun_move.is_some() as u64) << 37 | (m.stun_follow_up as u64) << 40);
        if canon {
            let (w, once) = look_memory(m.id);
            let n = (m.log_len as usize).min(w);
            let mut last = 0u64;
            for k in 0..n {
                last = last << 8 | m.log[(m.log_len as usize - 1 - k) & 7] as u64;
            }
            mix(&mut h, last << 8 | n as u64);
            mix(&mut h, m.ever_logged & once);
        } else {
            mix(&mut h, u64::from_le_bytes(m.log));
            mix(&mut h, m.log_len as u64 | (u32::from_le_bytes(m.performed) as u64) << 16);
            mix(&mut h, m.ever_logged);
        }
        mix(&mut h, m.performed_once);
        for v in m.vars {
            mix(&mut h, v as u32 as u64);
        }
    }
    for &e in cx.enemies.iter() {
        mix(&mut h, e as u64);
    }
    if !canon {
        for r in cx.player.relics.iter() {
            if !content::relic_mask(r.id).intersects(LOOK_PLAYER_HOOKS) && !LOOK_READ_RELICS.contains(&r.id) {
                continue; // a relic the look-ahead cannot read (`LOOK_PLAYER_HOOKS`)
            }
            mix(&mut h, r.id as u64 | (r.counter as u32 as u64) << 16 | (r.flags as u64) << 48);
            mix(&mut h, r.aux as u32 as u64);
        }
    }
    h
}

/// Merges projected combats with equal canonical digests (probabilities add up, the first is kept) and keeps the `LOOK_PATHS`
/// most likely (a stable sort: ties keep their order).
fn look_merge(v: &mut Vec<(Box<Combat>, f32)>) {
    // The digests compare the paths' HP and block. Where a value is pristine on every path it is the starting value everywhere, and where it was
    // written on every path it no longer is: either way which paths merge does not depend on the starting values. A value pristine on some paths
    // only does (a written value may equal the starting one), so the projection then counts as having read it.
    let (mut any, mut all) = ([0u8; MAX_CREATURES], [u8::MAX; MAX_CREATURES]);
    for (cx, _) in v.iter() {
        for (k, cr) in cx.creatures.iter().enumerate() {
            if cr.active && !cr.is_player {
                any[k] |= cr.pristine;
                all[k] &= cr.pristine;
            }
        }
    }
    if (0..MAX_CREATURES).any(|k| any[k] & !all[k] != 0) {
        look_dep();
    }
    let mut digests = crate::util::ArrayVec::<u64, 512>::new();
    let mut k = 0;
    for i in 0..v.len() {
        let d = look_digest_of(&v[i].0, Digest::Canon);
        match digests.iter().position(|&x| x == d) {
            Some(j) if j < k => v[j].1 += v[i].1,
            _ => {
                digests.push(d);
                v.swap(k, i);
                k += 1;
            }
        }
    }
    for (cx, _) in v.drain(k..) {
        look_free(cx);
    }
    if v.len() > LOOK_PATHS {
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        for (cx, _) in v.drain(LOOK_PATHS..) {
            look_free(cx);
        }
    }
}

thread_local! {
    /// The path buffers of `look_project` (three allocations per projection otherwise).
    static LOOK_VECS: std::cell::RefCell<[Vec<(Box<Combat>, f32)>; 3]> = const { std::cell::RefCell::new([Vec::new(), Vec::new(), Vec::new()]) };
    /// Recycled boxes for projected combats (an 18.8 KB allocation per copy otherwise).
    static LOOK_POOL: std::cell::RefCell<Vec<Box<Combat>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A boxed copy of `src` (from the pool when possible).
fn look_box(src: &Combat) -> Box<Combat> {
    match LOOK_POOL.with(|p| p.borrow_mut().pop()) {
        Some(mut b) => {
            (*b).clone_from(src);
            b
        }
        None => Box::new(src.clone()),
    }
}

fn look_free(b: Box<Combat>) {
    LOOK_POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() < 2 * LOOK_PATHS {
            p.push(b);
        }
    });
}

/// What a monster's rolls read of its move log: the last `w` entries (repeat limits, cooldowns, `last_logged_move`; at least 1)
/// and the use-once targets in `ever_logged`.
fn look_memory(id: u16) -> (usize, u64) {
    static T: std::sync::OnceLock<Vec<(usize, u64)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        (0..crate::ids::monster::COUNT as u16)
            .map(|id| {
                let (mut w, mut once) = (1usize, 0u64);
                if content::monster_implemented(id) {
                    for n in content::monster_def(id).nodes.iter() {
                        if let MonsterNode::Random { branches, .. } = n {
                            for b in branches.iter() {
                                match b.repeat {
                                    Repeat::UseOnlyOnce => once |= 1u64 << (b.target & 63),
                                    Repeat::CanRepeatXTimes(k) => w = w.max(k as usize),
                                    Repeat::CanRepeatForever => {}
                                }
                                w = w.max(b.cooldown as usize);
                            }
                        }
                    }
                }
                (w.min(8), once)
            })
            .collect()
    })[id as usize]
}

/// The observation slot of a move node.
fn look_slot(node: u8) -> usize {
    if node == STUN_NODE || node as usize >= LOOK_NODES - 1 { LOOK_NODES - 1 } else { node as usize }
}

fn look_rng() -> &'static RngSet {
    static R: std::sync::OnceLock<RngSet> = std::sync::OnceLock::new();
    R.get_or_init(|| RngSet::from_run_seed(LOOK_SEED))
}

/// Entries of a look-ahead cache (`LOOK_WAYS`-way set associative, the least recently used entry of a set is replaced; ~290 B each): the
/// thread's own (`LOOK_CACHE`), or one installed for a while with [`with_look_cache`] (the search gives each root its own: a root's
/// play-outs repeat each other's states, and the thread pool hands a root to any thread). The direct-mapped 1,024-entry thread cache of
/// before missed 60% of the lookups of a 5x32 search where a cache per root misses 36% (34% of the lookups are keys never seen before).
pub const LOOK_CACHE_ENTRIES: usize = 1024;
const LOOK_WAYS: usize = 8;

/// `look_rows` by `look_key`: per set the keys and their last use (0 = empty), then the rows.
pub struct LookCache {
    keys: Vec<[u64; LOOK_WAYS]>,
    used: Vec<[u32; LOOK_WAYS]>,
    rows: Vec<[LookRow; LOOK_H]>,
    clock: u32,
}

impl LookCache {
    /// `entries`: a power of two, at least `LOOK_WAYS`.
    pub fn new(entries: usize) -> LookCache {
        assert!(entries.is_power_of_two() && entries >= LOOK_WAYS, "look-ahead cache entries must be a power of two >= {LOOK_WAYS}");
        let sets = entries / LOOK_WAYS;
        LookCache { keys: vec![[0; LOOK_WAYS]; sets], used: vec![[0; LOOK_WAYS]; sets], rows: vec![[EMPTY_ROW; LOOK_H]; entries], clock: 0 }
    }

    #[inline]
    fn tick(&mut self) -> u32 {
        if self.clock == u32::MAX {
            // after 4 billion uses: start over (every entry empty)
            self.used.iter_mut().for_each(|u| *u = [0; LOOK_WAYS]);
            self.clock = 0;
        }
        self.clock += 1;
        self.clock
    }

    #[inline]
    fn get(&mut self, key: u64) -> Option<[LookRow; LOOK_H]> {
        let set = (key as usize) & (self.keys.len() - 1);
        let w = (0..LOOK_WAYS).find(|&w| self.keys[set][w] == key && self.used[set][w] != 0)?;
        self.used[set][w] = self.tick();
        Some(self.rows[set * LOOK_WAYS + w])
    }

    fn put(&mut self, key: u64, r: [LookRow; LOOK_H]) {
        let set = (key as usize) & (self.keys.len() - 1);
        let w = (0..LOOK_WAYS).min_by_key(|&w| self.used[set][w]).unwrap_or(0);
        self.keys[set][w] = key;
        self.used[set][w] = self.tick();
        self.rows[set * LOOK_WAYS + w] = r;
    }
}

/// Records that the running projection read an enemy's starting HP or block (`Creature::hp` / `block` of a pristine value): its rows are only
/// cached under the exact key.
#[cold]
#[inline(never)]
pub fn look_dep() {
    LOOK_DEP.with(|d| d.set(true));
}

/// Check every hit of the look-ahead cache (relaxed and exact key) against a fresh projection (panics on a difference; `LOOK_VERIFIED` counts the
/// checks): tests and diagnostics only.
pub static LOOK_VERIFY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static LOOK_VERIFIED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cold]
fn look_verify(cx: &Combat, c: Cid, r: &[LookRow; LOOK_H]) {
    let outer = LOOK_DEP.with(|d| d.get());
    let f = cx.look_rows(c, false, &mut LookDigests::default());
    LOOK_DEP.with(|d| d.set(outer));
    let same = r.iter().zip(f.iter()).all(|(a, b)| a.exp_damage.to_bits() == b.exp_damage.to_bits() && a.prob.iter().zip(b.prob.iter()).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(same, "look-ahead: a cache hit differs from the fresh rows (monster {}, creature {c})", crate::ids::monster::NAMES[cx.cr(c).monster.id as usize]);
    LOOK_VERIFIED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Runs `f` with `cache` as this thread's look-ahead cache (the thread's own comes back afterwards, also when `f` unwinds). The cache only
/// saves work: what `lookahead` returns does not depend on which cache is installed.
pub fn with_look_cache<R>(cache: &mut Box<LookCache>, f: impl FnOnce() -> R) -> R {
    struct Restore<'a>(&'a mut Box<LookCache>);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            LOOK_CACHE.with(|t| std::mem::swap(&mut *t.borrow_mut(), self.0));
        }
    }
    LOOK_CACHE.with(|t| std::mem::swap(&mut *t.borrow_mut(), cache));
    let _restore = Restore(cache);
    f()
}

thread_local! {
    /// `look_rows` by `look_key` (a search simulates many copies of the same enemies), allocated on a thread's first look-ahead.
    static LOOK_CACHE: std::cell::RefCell<Box<LookCache>> = std::cell::RefCell::new(Box::new(LookCache::new(LOOK_CACHE_ENTRIES)));
    /// The last `look_mode` result, by `look_key(NO)` (the enemies of one combat are looked at one after the other).
    /// (and whether that projection read a starting HP or block)
    static LOOK_MODE: std::cell::RefCell<Option<(u64, [[LookRow; LOOK_H]; MAX_CREATURES], u16, bool)>> = const { std::cell::RefCell::new(None) };
    /// The projection running on this thread read an enemy's starting HP or block (`Creature::pristine`).
    static LOOK_DEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// ---- the look-ahead from before S1 (`LOOK_LEGACY`) ------------------------------------------------------------------------------

const LEGACY_H: usize = 3;
type LegacyPaths = Outcomes;

fn same_machine_state(a: &MonsterState, b: &MonsterState) -> bool {
    a.cur_state == b.cur_state
        && a.log == b.log
        && a.log_len == b.log_len
        && a.ever_logged == b.ever_logged
        && a.performed_once == b.performed_once
        && a.stun_performed == b.stun_performed
        && a.stun_follow_up == b.stun_follow_up
}

impl Combat {
    /// One turn forward on the monster's machine alone: the pending move is performed, then the next player turn rolls.
    fn legacy_roll(&self, c: Cid, ms: &MonsterState, p: f32, out: &mut LegacyPaths) {
        let mut ms = *ms;
        ms.performed_first = true;
        let cur = ms.cur_state;
        let def = content::monster_def(ms.id);
        let nxt = if cur == STUN_NODE {
            ms.stun_performed = true;
            if ms.stun_follow_up == NO { def.initial } else { ms.stun_follow_up }
        } else {
            ms.performed_once |= 1u64 << cur;
            match &def.nodes[cur as usize] {
                MonsterNode::Move { follow_up, .. } => {
                    if *follow_up == NO {
                        def.initial
                    } else if *follow_up == crate::defs::FOLLOW_STORED {
                        ms.stun_follow_up
                    } else {
                        *follow_up
                    }
                }
                _ => return,
            }
        };
        self.look_enter(c, ms, cur, nxt, NO, p, out);
    }

    /// Rows of the legacy look-ahead: the machine walked on a copy of the monster state, conditions and weights reading the
    /// current combat, damage with the current modifiers; turns past the third are empty.
    fn legacy_rows(&self, c: Cid) -> [LookRow; LOOK_H] {
        let mut rows = [EMPTY_ROW; LOOK_H];
        let mut cur: LegacyPaths = crate::util::ArrayVec::new();
        cur.push((self.cr(c).monster, 1.0));
        let mut node_dmg = [-1f32; 256];
        for row in rows.iter_mut().take(LEGACY_H) {
            let mut next: LegacyPaths = crate::util::ArrayVec::new();
            for (ms, p) in cur.iter() {
                self.legacy_roll(c, ms, *p, &mut next);
            }
            let mut merged: LegacyPaths = crate::util::ArrayVec::new();
            for (ms, p) in next.iter() {
                if let Some(e) = merged.as_mut_slice().iter_mut().find(|(m, _)| same_machine_state(m, ms)) {
                    e.1 += *p;
                } else {
                    merged.push((*ms, *p));
                }
            }
            for (ms, p) in merged.iter() {
                let node = ms.cur_state;
                row.prob[look_slot(node)] += p;
                if node_dmg[node as usize] < 0.0 {
                    node_dmg[node as usize] = self.node_attack_damage(c, node);
                }
                row.exp_damage += p * node_dmg[node as usize];
            }
            cur = merged;
            if cur.is_empty() {
                break;
            }
        }
        rows
    }
}


// ---- provable bounds on enemy damage (used by `bounds`) -----------------------------------------------------------------------
impl Combat {
    /// `look_enter` for a bound: every conditional arm and every random branch is possible (a superset of what can happen), states go
    /// into an unbounded list.
    fn bound_enter(&self, c: Cid, mut ms: MonsterState, left: u8, to: u8, first: u8, out: &mut Vec<MonsterState>, depth: u32) {
        if depth > 64 {
            return;
        }
        if left == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << left);
        }
        ms.cur_state = to;
        if self.node_is_move(c, to) {
            let first = if first == NO { to } else { first };
            ms.log[(ms.log_len & 7) as usize] = first;
            ms.log_len += 1;
            if first < 64 {
                ms.ever_logged |= 1u64 << first;
            }
            ms.next_move = to;
            out.push(ms);
            return;
        }
        let def = content::monster_def(ms.id);
        match &def.nodes[to as usize] {
            MonsterNode::Cond { arms, .. } => {
                for (target, _) in arms.iter() {
                    self.bound_enter(c, ms, to, *target, first, out, depth + 1);
                }
            }
            MonsterNode::Random { branches, .. } => {
                for b in branches.iter() {
                    self.bound_enter(c, ms, to, b.target, first, out, depth + 1);
                }
            }
            MonsterNode::Move { .. } => {}
        }
    }

    /// The states the monster can be in one turn later (the pending move is performed, the next one is rolled).
    pub fn bound_next_states(&self, c: Cid, ms: &MonsterState, out: &mut Vec<MonsterState>) {
        let mut ms = *ms;
        ms.performed_first = true;
        let cur = ms.cur_state;
        let def = content::monster_def(ms.id);
        let nxt = if cur == STUN_NODE {
            ms.stun_performed = true;
            if ms.stun_follow_up == NO { def.initial } else { ms.stun_follow_up }
        } else {
            ms.performed_once |= 1u64 << cur;
            match &def.nodes[cur as usize] {
                MonsterNode::Move { follow_up, .. } => {
                    if *follow_up == NO {
                        def.initial
                    } else if *follow_up == crate::defs::FOLLOW_STORED {
                        ms.stun_follow_up
                    } else {
                        *follow_up
                    }
                }
                _ => return,
            }
        };
        self.bound_enter(c, ms, cur, nxt, NO, out, 0);
    }

    /// States are identified by the move node (and the stun bookkeeping): the bound takes every branch, so history never restricts it.
    pub fn bound_same_state(a: &MonsterState, b: &MonsterState) -> bool {
        a.cur_state == b.cur_state && a.stun_performed == b.stun_performed && a.stun_follow_up == b.stun_follow_up
    }

    /// Total attack damage of a move node against the player (as `node_attack_damage`), or with the player's attack-weakening
    /// applied to every hit (`weak`: `floor(3/4 x)` of the per-hit damage, which is <= what the game computes).
    pub fn bound_node_damage(&self, c: Cid, node: u8, weak: bool) -> i64 {
        if node == STUN_NODE {
            return 0;
        }
        let def = content::monster_def(self.cr(c).monster.id);
        let MonsterNode::Move { intents, .. } = &def.nodes[node as usize] else { return 0 };
        let hit = |d: i32| -> i64 { if weak { (d as i64 * 3) / 4 } else { d as i64 } };
        let mut total = 0i64;
        for it in intents.iter() {
            match it {
                Intent::Attack { damage, hits } => total += hit(self.intent_damage(c, damage(self, c))) * hits(self, c) as i64,
                Intent::DeathBlowAttack { damage } => total += hit(self.intent_damage(c, damage(self, c))),
                _ => {}
            }
        }
        total
    }
}
