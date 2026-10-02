//! Monster spawning, move state machine and move execution (spec 04 §1).

use crate::content;
use crate::defs::*;
use crate::hooks::Kind;
use crate::state::*;
use crate::types::*;

/// Pseudo node index of the generic `STUNNED` move created by `CreatureCmd.Stun` (a fresh `MoveState` per stun whose
/// follow-up is `MonsterState::stun_follow_up`). Defs may therefore use at most 63 nodes. See `interrupts.rs`.
pub const STUN_NODE: u8 = 63;

impl Combat {
    /// `CombatState.CreateCreature` + `AddCreature` for an enemy: one `niche` draw for HP, unique among enemies already
    /// added (spec 04 §1.13); then `SetUpForCombat` (state machine built, `SpawnedThisTurn = true`).
    pub fn add_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        let cid = self.create_enemy(monster_id, slot)?;
        self.attach_enemy(cid);
        Some(cid)
    }

    /// `CombatState.CreateCreature` (enemy): allocates the creature and draws its HP (one `niche` draw, unique among the
    /// enemies already attached); the creature is NOT yet in the enemy list (`attach_enemy` is `AddCreature`).
    pub fn create_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        let cid = (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).in_combat)?;
        if !content::monster_implemented(monster_id) {
            self.flag_missing(Kind::Monster, monster_id);
            return None;
        }
        let def = content::monster_def(monster_id);
        let (lo, hi) = (def.hp)(self.ascension);
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
        let ms = MonsterState { id: monster_id, cur_state: def.initial, spawned_this_turn: true, ..Default::default() };
        let mut cr = Creature::default();
        cr.active = true;
        cr.in_combat = true; // reserves the creature slot; hooks only reach it once it is in `enemies`
        cr.side = Side::Enemy;
        cr.hp = hp;
        cr.max_hp = hp;
        cr.slot = slot;
        cr.monster = ms;
        self.creatures[cid as usize] = cr;
        Some(cid)
    }

    /// `CombatState.AddCreature` + `CombatManager.AddCreature` (`SetUpForCombat`, slot sort).
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

    #[inline]
    fn node(&self, c: Cid, n: u8) -> &'static MonsterNode {
        &content::monster_def(self.cr(c).monster.id).nodes[n as usize]
    }

    pub(crate) fn can_transition_away(&self, c: Cid, n: u8) -> bool {
        if n == STUN_NODE {
            // STUNNED has MustPerformOnceBeforeTransitioning.
            return self.cr(c).monster.performed_once >> n & 1 != 0;
        }
        match self.node(c, n) {
            MonsterNode::Move { must_perform_once, .. } => !*must_perform_once || self.cr(c).monster.performed_once >> n & 1 != 0,
            _ => true,
        }
    }

    /// Weight of branch `b` of a random node (spec 04 §1.3).
    fn branch_weight(&self, c: Cid, b: &Branch) -> f32 {
        let ms = &self.cr(c).monster;
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
            return self.cr(c).monster.stun_follow_up;
        }
        match &def.nodes[cur as usize] {
            MonsterNode::Move { follow_up, .. } => {
                if *follow_up == NO { def.initial } else { *follow_up }
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

    /// `MonsterModel.RollMove` → `MonsterMoveStateMachine.RollMove` (spec 04 §1.2).
    pub fn roll_move(&mut self, c: Cid) {
        let def = content::monster_def(self.cr(c).monster.id);
        let cur = self.cr(c).monster.cur_state;
        let is_move = |n: u8| n == STUN_NODE || matches!(def.nodes[n as usize], MonsterNode::Move { .. });
        let performed_first = self.cr(c).monster.performed_first;
        if !self.can_transition_away(c, cur) || (!performed_first && is_move(cur)) {
            // no transition, no RNG
            self.creatures[c as usize].monster.next_move = cur;
            return;
        }
        let mut cur = cur;
        let mut first_logged = NO;
        loop {
            let nxt = self.next_state(c, cur);
            // OnExitState clears _performedAtLeastOnce of the node being left
            self.creatures[c as usize].monster.performed_once &= !(1u64 << cur);
            cur = nxt;
            self.creatures[c as usize].monster.cur_state = cur;
            self.creatures[c as usize].monster.stunned = cur == STUN_NODE;
            if first_logged == NO && is_move(cur) {
                first_logged = cur;
            }
            if is_move(cur) {
                break;
            }
        }
        self.log_move(c, first_logged);
        self.creatures[c as usize].monster.next_move = cur;
    }

    /// `MonsterModel.PerformMove` (spec 04 §1.9).
    pub fn perform_move(&mut self, c: Cid) {
        let def = content::monster_def(self.cr(c).monster.id);
        let nm = self.cr(c).monster.next_move;
        assert!(nm != NO, "monster performing UNSET_MOVE");
        self.creatures[c as usize].monster.is_performing = true;
        if nm == STUN_NODE {
            // the generic STUNNED move performs nothing (the optional stunMove side effects are cosmetic here)
            self.creatures[c as usize].monster.performed_once |= 1u64 << nm;
        } else if let MonsterNode::Move { perform, .. } = &def.nodes[nm as usize] {
            self.creatures[c as usize].monster.performed_once |= 1u64 << nm;
            perform(self, c);
        }
        {
            let ms = &mut self.creatures[c as usize].monster;
            ms.performed.copy_within(1..4, 0);
            ms.performed[3] = nm;
            ms.performed_first = true;
        }
        self.creatures[c as usize].monster.is_performing = false;
        if self.cr(c).is_dead() && self.cr(c).in_combat && self.should_creature_be_removed_after_death(c) {
            self.detach_creature(c);
        }
    }

}
