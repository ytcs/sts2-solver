//! Creature lifecycle helpers needed by Overgrowth content (spec 04 §1.7-1.10): mid-combat spawns
//! (`CreatureCmd.Add`), forced moves / stun (`MonsterModel.SetMoveImmediate`, `CreatureCmd.Stun`), status-card
//! generation (`CardPileCmd.AddToCombatAndPreview`) and card afflictions (`CardCmd.Afflict`).
//!
//! NOTE (merge): these are the minimal versions written by the Overgrowth port; the engine-core branch implements the
//! general summon/stun/revive machinery. When merging, keep the engine-core versions and adapt callers.

use crate::content;
use crate::defs::*;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// `CreatureCmd.Add<T>(combat, slot)` for an enemy: create (one `niche` draw, unique HP vs current enemies) +
    /// add + `AfterAddedToRoom` + `RollMove` (only when it is the player's side to move). Spawned during the enemy turn,
    /// the creature is not in the turn snapshot and its first roll happens at the next player turn start (spec 04 §1.7).
    pub fn summon_enemy(&mut self, monster: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let c = self.add_enemy(monster, slot)?;
        self.creatures[c as usize].monster.vars[0] = vars[0];
        self.creatures[c as usize].monster.vars[1] = vars[1];
        let def = content::monster_def(monster);
        if let Some(f) = def.on_spawn {
            f(self, c);
        }
        if self.side == Side::Player {
            self.roll_move(c);
        }
        Some(c)
    }

    /// Node of the most recently logged (rolled) move: `StateLog.Last()`.
    pub fn last_logged_move(&self, c: Cid) -> u8 {
        let ms = &self.cr(c).monster;
        if ms.log_len == 0 { NO } else { ms.log[((ms.log_len - 1) & 7) as usize] }
    }

    /// `MonsterModel.SetMoveImmediate(state, forceTransition)`: only if the pending move may be left (or forced).
    /// `follow_up` is stored for nodes whose successor is `FOLLOW_STORED`.
    pub fn set_move_immediate(&mut self, c: Cid, node: u8, follow_up: u8, force: bool) {
        let nm = self.cr(c).monster.next_move;
        let can = nm == NO || self.can_transition_away(c, nm);
        if can || force {
            let ms = &mut self.creatures[c as usize].monster;
            ms.next_move = node;
            ms.stun_follow_up = follow_up;
            // ForceCurrentState -> SetCurrentState: OnExitState of the old state clears its performed flag.
            let old = ms.cur_state;
            ms.performed_once &= !(1u64 << old);
            ms.cur_state = node;
        }
    }

    /// `CreatureCmd.Stun(creature, stunMove, nextMoveId)` -> `Creature.StunInternal`: replaces the pending move with a fresh
    /// `STUNNED` state (`stun_node`, MustPerformOnce) whose successor is `next_node` (`NO` = the last rolled move).
    pub fn stun_creature(&mut self, c: Cid, stun_node: u8, next_node: u8) {
        if !self.cr(c).in_combat || self.cr(c).is_dead() {
            return;
        }
        let next = if next_node == NO { self.last_logged_move(c) } else { next_node };
        self.set_move_immediate(c, stun_node, next, false);
    }

    /// `CardPileCmd.AddToCombatAndPreview<T>(targets, pile, count, creator, position)` for the (single) player.
    pub fn add_status_cards(&mut self, card_id: u16, pile: PileType, count: i32, pos: CardPilePosition) {
        if self.cr(PLAYER).is_dead() {
            return;
        }
        for _ in 0..count {
            if let Some(c) = self.new_card(card_id, 0) {
                self.add_generated_card(c, pile, pos);
            }
        }
    }

    // ---- afflictions (`Card.affliction` = affliction id + 1, 0 = none) ----------------------------------------------

    /// The card's current affliction id (`ids::affliction::*`), if any.
    #[inline]
    pub fn card_affliction(&self, c: CardIdx) -> Option<u16> {
        let a = self.cards[c as usize].affliction;
        if a == 0 { None } else { Some(a as u16 - 1) }
    }

    /// `CardCmd.Afflict<T>(card, amount)` (no `Hook.ShouldAfflict` content, all card types allowed).
    pub fn afflict_card(&mut self, c: CardIdx, affliction: u16, amount: i32) {
        if self.is_over_or_ending() && self.card_pile_type(c) != PileType::None {
            return;
        }
        match self.card_affliction(c) {
            None => {
                self.cards[c as usize].affliction = affliction as u8 + 1;
                self.cards[c as usize].affliction_amount = amount as i16;
            }
            Some(a) if a == affliction => {} // not stackable: `CanAfflict` refuses
            Some(_) => {}
        }
    }

    /// `CardCmd.ClearAffliction`.
    pub fn clear_affliction(&mut self, c: CardIdx) {
        self.cards[c as usize].affliction = 0;
        self.cards[c as usize].affliction_amount = 0;
    }

    /// All cards of the player's combat piles (`PlayerCombatState.AllCards`), in arena order.
    pub fn all_combat_cards(&self) -> crate::util::ArrayVec<CardIdx, MAX_CARDS> {
        let mut o = crate::util::ArrayVec::new();
        for i in 0..self.n_cards as usize {
            let p = self.cards[i].pile;
            if p != 0 && p <= PileType::Play as u8 {
                o.push(i as CardIdx);
            }
        }
        o
    }
}
