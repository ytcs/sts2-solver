use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(Bound {});
listener!(Entangled {});
listener!(Galvanized {
    fn affliction_is_stackable(&self) -> bool {
        true
    }
});
listener!(Hexed {
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx {
            return;
        }
        if !cx.has_power(PLAYER, crate::ids::power::HEX_POWER) {
            cx.clear_affliction(card);
        }
    }
});
listener!(Ringing {});
listener!(Smog {});
listener!(Tainted {
    fn affliction_is_stackable(&self) -> bool {
        true
    }
    fn can_afflict_card_type(&self, t: CardType) -> bool {
        t == CardType::Skill
    }
});
