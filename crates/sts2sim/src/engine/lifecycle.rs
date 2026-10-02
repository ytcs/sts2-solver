//! Creature lifecycle helpers needed by Overgrowth content (spec 04 §1.7-1.10): mid-combat spawns
//! (`CreatureCmd.Add`), forced moves / stun (`MonsterModel.SetMoveImmediate`, `CreatureCmd.Stun`), status-card
//! generation (`CardPileCmd.AddToCombatAndPreview`) and card afflictions (`CardCmd.Afflict`).
//!
//! (Summon / stun / afflict / forced-move machinery lives in `monster.rs` / `enchant.rs` from the engine-core pass.)

use crate::state::*;
use crate::types::*;

impl Combat {
    /// `MonsterModel.SetMoveImmediate(state, forceTransition)` for a state whose successor is `FOLLOW_STORED` (the
    /// dynamically created REVIVE_MOVE): the follow-up node is stored first, only if the transition happens.
    pub fn set_move_immediate_follow(&mut self, c: Cid, node: u8, follow_up: u8, force: bool) {
        let nm = self.cr(c).monster.next_move;
        if nm == NO || self.can_transition_away(c, nm) || force {
            self.creatures[c as usize].monster.stun_follow_up = follow_up;
            self.set_move_immediate(c, node, force);
        }
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
