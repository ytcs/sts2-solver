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
        cr.hp = hp;
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
                panic!("RandomBranchState: no branch selected");
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
// deterministic monster, and the possible moves (with their odds) at random branches. `lookahead` reproduces exactly that
// knowledge by walking the monster's own state machine forward on a COPY of its state — no RNG is consumed, so the realized
// random outcomes stay hidden. Random nodes branch by their current weights (repeat rules / cooldowns evaluated on the
// hypothetical move log); conditional nodes and weight lambdas read the combat as it is now.

/// Future turns covered (turn +1 .. +LOOK_H after the one whose intent is shown).
pub const LOOK_H: usize = 3;
/// Move-node slots per horizon (nodes >= LOOK_NODES-1, and the synthetic STUNNED node, share the last slot).
pub const LOOK_NODES: usize = 16;

/// Per-horizon knowledge: probability of each move node being the monster's move, and the expected total attack damage
/// (per-hit damage with the player's and monster's current modifiers x hits, probability-weighted).
#[derive(Clone, Copy)]
pub struct LookRow {
    pub prob: [f32; LOOK_NODES],
    pub exp_damage: f32,
}

type Paths = crate::util::ArrayVec<(MonsterState, f32), 48>;

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
    /// `RollMove` continued on a hypothetical state: leave `left`, enter `to`, and keep walking branch states until a move.
    fn look_enter(&self, c: Cid, mut ms: MonsterState, left: u8, to: u8, first: u8, p: f32, out: &mut Paths) {
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

    /// One turn forward: the pending move is performed in the enemy turn, then the next player turn rolls.
    fn look_roll(&self, c: Cid, ms: &MonsterState, p: f32, out: &mut Paths) {
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

    /// The monster's move distribution for the next `LOOK_H` turns after the current intent (see the section comment).
    pub fn lookahead(&self, c: Cid) -> [LookRow; LOOK_H] {
        let mut rows = [LookRow { prob: [0.0; LOOK_NODES], exp_damage: 0.0 }; LOOK_H];
        let cr = self.cr(c);
        if !cr.is_alive() || !cr.in_combat || cr.monster.next_move == NO || cr.is_player {
            return rows;
        }
        let mut cur: Paths = crate::util::ArrayVec::new();
        cur.push((cr.monster, 1.0));
        for row in rows.iter_mut() {
            let mut next: Paths = crate::util::ArrayVec::new();
            for (ms, p) in cur.iter() {
                self.look_roll(c, ms, *p, &mut next);
            }
            // merge identical machine states reached through different branches
            let mut merged: Paths = crate::util::ArrayVec::new();
            for (ms, p) in next.iter() {
                if let Some(e) = merged.as_mut_slice().iter_mut().find(|(m, _)| same_machine_state(m, ms)) {
                    e.1 += *p;
                } else {
                    merged.push((*ms, *p));
                }
            }
            for (ms, p) in merged.iter() {
                let slot = if ms.cur_state == STUN_NODE || ms.cur_state as usize >= LOOK_NODES - 1 { LOOK_NODES - 1 } else { ms.cur_state as usize };
                row.prob[slot] += *p;
                row.exp_damage += *p * self.node_attack_damage(c, ms.cur_state);
            }
            cur = merged;
            if cur.is_empty() {
                break;
            }
        }
        rows
    }
}
