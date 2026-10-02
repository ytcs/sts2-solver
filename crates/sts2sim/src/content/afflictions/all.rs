//! The 7 afflictions (`Models/Afflictions/*.cs`). Almost all of their logic lives in the powers that apply them
//! (ChainsOfBinding, Tangled, Galvanic, Hex, Ringing, Smoggy, Tainted).

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
// Hexed: if the owner has no Hex power when the card (re-)enters combat, the affliction clears itself.
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
