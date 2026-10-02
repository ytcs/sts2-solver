//! Shared powers (needed by Overgrowth monsters, potions, Necrobinder, ...): ArtifactPower, MinionPower. Exactly one
//! `listener!` per class: later branches that define the same class must drop their copy.

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- ArtifactPower ----------------------------------------------------------------------------------------------
listener!(ArtifactPower {
    fn try_modify_power_amount_received(&self, _cx: &Combat, me: Me, power_id: u16, target: Cid, amount: Dec, _applier: Cid) -> Option<Dec> {
        if target != me.owner {
            return None;
        }
        // canonicalPower.GetTypeForAmount(amount) != Debuff -> unchanged; hidden powers are not blocked.
        if Combat::power_type_for_amount(power_id, amount.trunc()) != PowerType::Debuff {
            return None;
        }
        if !crate::content::power_def(power_id).visible {
            return None;
        }
        Some(Dec::ZERO)
    }
    fn after_modifying_power_amount_received(&self, cx: &mut Combat, me: Me, _power_id: u16) {
        cx.decrement_power(me.owner, me.idx);
    }
});

// ---- MinionPower ------------------------------------------------------------------------------------------------
listener!(MinionPower {
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
    // Killing a minion does not count as a "fatal" kill (Feed / Hand of Greed / The Hunt rewards).
    fn should_owner_death_trigger_fatal(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});

