//! Powers that Overgrowth needs but that other content branches (Underdocks, potions) also define: ArtifactPower,
//! ShrinkPower, MinionPower. Semantics are identical in every branch; on merge keep ONE definition of each and delete
//! this file (it contains nothing else).

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

// ---- ShrinkPower (player debuff; Amount < 0 = infinite) ---------------------------------------------------------
listener!(ShrinkPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(7, 1) // (100 - 30) / 100
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        let amount = cx.power_amount(me.owner, me.id);
        if amount >= 0 && cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            if cx.cr(me.owner).powers[i].applier == creature {
                cx.remove_power(me.owner, me.idx);
            }
        }
    }
});

// ---- MinionPower ------------------------------------------------------------------------------------------------
listener!(MinionPower {
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});

