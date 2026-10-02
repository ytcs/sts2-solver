//! Enchantments and afflictions on card instances (spec 03 §12).

use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// The `Me` of a card's enchantment (`card.enchant != 0`; ids are stored `+1`, `0` = none).
    #[inline]
    pub fn enchantment_me(&self, card: CardIdx) -> Me {
        let c = &self.cards[card as usize];
        Me { kind: Kind::Enchantment, owner: PLAYER, idx: card as u16, id: (c.enchant - 1) as u16, amount: c.enchant_amount as i32 }
    }

    /// The `Me` of a card's affliction.
    #[inline]
    pub fn affliction_me(&self, card: CardIdx) -> Me {
        let c = &self.cards[card as usize];
        Me { kind: Kind::Affliction, owner: PLAYER, idx: card as u16, id: (c.affliction - 1) as u16, amount: c.affliction_amount as i32 }
    }
}
