use crate::state::*;
use crate::types::*;

impl Combat {
    pub fn set_move_immediate_follow(&mut self, c: Cid, node: u8, follow_up: u8, force: bool) {
        let nm = self.cr(c).monster.next_move;
        if nm == NO || self.can_transition_away(c, nm) || force {
            self.creatures[c as usize].monster.stun_follow_up = follow_up;
            self.set_move_immediate(c, node, force);
        }
    }

    pub fn add_status_cards(&mut self, card_id: u16, pile: PileType, count: i32, pos: CardPilePosition) {
        let by_player = self.side == Side::Player;
        self.add_status_cards_as(card_id, pile, count, pos, by_player);
    }

    pub fn add_status_cards_as(&mut self, card_id: u16, pile: PileType, count: i32, pos: CardPilePosition, _by_player: bool) {
        if self.cr(PLAYER).is_dead() {
            return;
        }
        for _ in 0..count {
            if let Some(c) = self.new_card(card_id, 0) {
                self.add_generated_card_by(c, pile, pos, false);
            }
        }
    }

    #[inline]
    pub fn card_affliction(&self, c: CardIdx) -> Option<u16> {
        let a = self.cards[c as usize].affliction;
        if a == 0 { None } else { Some(a as u16 - 1) }
    }

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
