//! Monster interrupts and live creature changes needed by the Act 1b (Underdocks) content: stun /
//! `SetMoveImmediate`, live summons (`CreatureCmd.Add`), escape, HP resets (spec 04 §1.7-1.8, spec 02 §5).
//!
//! NOTE: written by the Underdocks port as the *minimum* needed so it does not block on engine-core, which is
//! implementing the general versions in parallel. On merge, keep engine-core's implementations and delete whatever
//! is duplicated here (only `stun`, `set_move_immediate`, `spawn_enemy_live`, `escape`, `set_max_and_current_hp`).

use super::monster::STUN_NODE;
use crate::content;
use crate::dec::Dec;
use crate::defs::*;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// Node index of the state with id `id` in the creature's move graph.
    pub fn node_by_id(&self, c: Cid, id: &str) -> u8 {
        let def = content::monster_def(self.cr(c).monster.id);
        for (i, n) in def.nodes.iter().enumerate() {
            let nid = match n {
                MonsterNode::Move { id, .. } | MonsterNode::Random { id, .. } | MonsterNode::Cond { id, .. } => *id,
            };
            if nid == id {
                return i as u8;
            }
        }
        panic!("monster {} has no state {id}", crate::ids::monster::NAMES[self.cr(c).monster.id as usize]);
    }

    /// `MonsterModel.SetMoveImmediate(state, forceTransition)`: only if the pending move can be left (or forced).
    /// Returns whether the move was replaced. The forced state is not logged.
    pub fn set_move_immediate(&mut self, c: Cid, node: u8, force: bool) -> bool {
        let nm = self.cr(c).monster.next_move;
        if !(force || nm == NO || self.can_transition_away(c, nm)) {
            return false;
        }
        let ms = &mut self.creatures[c as usize].monster;
        // ForceCurrentState: OnExitState of the old state clears its `_performedAtLeastOnce`.
        ms.performed_once &= !(1u64 << ms.cur_state);
        ms.cur_state = node;
        ms.next_move = node;
        ms.stunned = node == STUN_NODE;
        true
    }

    /// `CreatureCmd.Stun(creature, nextMoveId)` / `Creature.StunInternal`: replaces the pending move with a generic
    /// `STUNNED` move (does nothing when performed). `next_move` = id of the move (or branch) performed after the
    /// stun; `None` = the last move that was rolled (`StateLog.Last()`).
    pub fn stun(&mut self, c: Cid, next_move: Option<&str>) {
        if !self.cr(c).in_combat || self.cr(c).is_dead() {
            return;
        }
        let follow = match next_move {
            Some(id) => self.node_by_id(c, id),
            None => {
                let ms = &self.cr(c).monster;
                ms.log[(ms.log_len as usize - 1) & 7]
            }
        };
        let nm = self.cr(c).monster.next_move;
        if nm == NO || self.can_transition_away(c, nm) {
            self.creatures[c as usize].monster.stun_follow_up = follow;
            self.set_move_immediate(c, STUN_NODE, false);
        }
    }

    /// Next free encounter slot: the first (`last == false`, `Encounter.GetNextSlot`) or last (`last == true`,
    /// `Slots.LastOrDefault(free)`) slot index not occupied by an enemy; `NO` if none / the encounter has no slots.
    pub fn free_slot(&self, last: bool) -> u8 {
        let n = content::encounters::underdocks::slot_count(self.encounter);
        let mut found = NO;
        for s in 0..n {
            if !self.enemies.iter().any(|&e| self.cr(e).slot == s) {
                found = s;
                if !last {
                    break;
                }
            }
        }
        found
    }

    /// `AfterCreatureAdded` for a creature that was added mid-combat: `AfterAddedToRoom`, then `RollMove` only if it is
    /// the player's turn (`PrepareForNextTurn(rollNewMove: false)` only refreshes the intent UI).
    pub fn after_enemy_added(&mut self, c: Cid) {
        let def = content::monster_def(self.cr(c).monster.id);
        if let Some(f) = def.on_spawn {
            f(self, c);
        }
        if self.side == Side::Player {
            self.roll_move(c);
        }
    }

    /// `CreatureCmd.Add<T>(combatState, slot)`: create (HP draw), attach, `AfterCreatureAdded`.
    pub fn spawn_enemy_live(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let c = self.create_enemy(monster_id, slot)?;
        self.cr_mut(c).monster.vars[0] = vars[0];
        self.cr_mut(c).monster.vars[1] = vars[1];
        self.attach_enemy(c);
        self.after_enemy_added(c);
        Some(c)
    }

    /// `CreatureCmd.Escape`: strips every power (no callbacks) and leaves the combat without dying.
    pub fn escape(&mut self, c: Cid) {
        if self.cr(c).is_dead() || !self.cr(c).in_combat {
            return;
        }
        self.cr_mut(c).powers.clear();
        self.detach_creature(c);
    }

    /// `CreatureCmd.SetMaxAndCurrentHp`.
    pub fn set_max_and_current_hp(&mut self, c: Cid, amount: i32) {
        // SetMaxHp
        let cr = self.cr_mut(c);
        cr.max_hp = amount.max(0);
        if cr.hp > cr.max_hp {
            cr.hp = cr.max_hp;
        }
        // SetCurrentHp
        let before = self.cr(c).hp;
        self.set_current_hp_internal(c, Dec::int(amount as i64));
        let delta = self.cr(c).hp - before;
        if delta != 0 {
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, delta));
        }
        if self.cr(c).is_dead() {
            self.kill(&[c]);
        }
    }
}
