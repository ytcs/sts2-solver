//! Powers owned or applied by the Act 1a "Overgrowth" monsters (spec 04 §3.1; C# `Models/Powers/*`).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `participants.Contains(Owner)` for a side-turn notification.
#[inline]
fn owner_side(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

// ---- SlipperyPower ----------------------------------------------------------------------------------------------
listener!(SlipperyPower {
    fn modify_hp_lost_after_osty(&self, _cx: &Combat, me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if target != me.owner || amount < Dec::ONE {
            return amount;
        }
        Dec::ONE
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target == me.owner && unblocked >= 1 {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// ---- ConstrictPower ---------------------------------------------------------------------------------------------
listener!(ConstrictPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            let amount = cx.power_amount(me.owner, me.id);
            cx.damage(&[me.owner], Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            if !was_removal_prevented && cx.cr(me.owner).powers[i].applier == creature {
                cx.remove_power(me.owner, me.idx);
            }
        }
    }
});

// ---- TangledPower: Attack cards become Entangled (+Amount energy) until the end of the owner's turn ----------------
listener!(TangledPower {
    fn after_applied(&self, cx: &mut Combat, _me: Me) {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_def(c).ctype == CardType::Attack {
                cx.afflict_card(c, ids::affliction::ENTANGLED, 1);
            }
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        if cx.card_affliction(card).is_none() && cx.card_def(card).ctype == CardType::Attack {
            cx.afflict_card(card, ids::affliction::ENTANGLED, 1);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, _old_owner: Cid) {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c) == Some(ids::affliction::ENTANGLED) {
                cx.clear_affliction(c);
            }
        }
    }
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        if cx.card_affliction(card) != Some(ids::affliction::ENTANGLED) {
            return None;
        }
        Some(cost + Dec::int(cx.power_amount(me.owner, me.id) as i64))
    }
});

// ---- RingingPower: Ringing cards cannot be played after the first card of the turn ----------------------------------
listener!(RingingPower {
    fn after_applied(&self, cx: &mut Combat, _me: Me) {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c).is_none() {
                cx.afflict_card(c, ids::affliction::RINGING, 1);
            }
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        if cx.card_affliction(card).is_none() {
            cx.afflict_card(card, ids::affliction::RINGING, 1);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, _old_owner: Cid) {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c) == Some(ids::affliction::RINGING) {
                cx.clear_affliction(c);
            }
        }
    }
    fn should_play(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        if cx.card_affliction(card) != Some(ids::affliction::RINGING) {
            return true;
        }
        // CardPlaysStarted.Any(HappenedThisTurn && by the owner)
        cx.hist.cards_played_this_turn == 0
    }
});

// ---- SlowPower (Bygone Effigy): aux = SlowAmount ----------------------------------------------------------------
listener!(SlowPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux += 1;
        }
    }
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let aux = cx.cr(me.owner).power(me.id).map_or(0, |p| p.aux);
        // 1 + 0.1 * SlowAmount
        Dec::ONE + Dec::frac(aux as i64, 1)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux = 0;
            }
        }
    }
});

// ---- TerritorialPower (Byrdonis) ---------------------------------------------------------------------------------
listener!(TerritorialPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            let amount = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(amount as i64), me.owner, NO);
        }
    }
});

// ---- InfestedPower (Phrog Parasite): on death 4 stunned Wrigglers; keeps combat open ----------------------------------
listener!(InfestedPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if creature != me.owner || was_removal_prevented {
            return;
        }
        for i in 0..4u8 {
            // vars[0] = StartStunned
            cx.summon_enemy(ids::monster::WRIGGLER, crate::content::monsters::phrog::wriggler_slot(i), [1, 0]);
        }
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
});

// ---- PlowPower (Ceremonial Beast): at HP <= Amount the Beast loses its strength and is stunned into phase 2 ---------
listener!(PlowPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target != me.owner || unblocked <= 0 || cx.cr(target).hp > cx.power_amount(me.owner, me.id) {
            return;
        }
        // TemporaryStrength instances, then Strength itself.
        let mut ids_to_remove: crate::util::ArrayVec<u16, MAX_POWERS> = crate::util::ArrayVec::new();
        for p in cx.cr(me.owner).powers.iter() {
            if is_temporary_strength(p.id) {
                ids_to_remove.push(p.uid);
            }
        }
        for &uid in ids_to_remove.iter() {
            cx.remove_power(me.owner, uid);
        }
        if let Some(p) = cx.cr(me.owner).power(ids::power::STRENGTH_POWER) {
            let uid = p.uid;
            cx.remove_power(me.owner, uid);
        }
        // CeremonialBeast.SetStunned + Stun(StunnedMove, "BEAST_CRY_MOVE")
        crate::content::monsters::ceremonial_beast::stun_into_phase_two(cx, me.owner);
        cx.remove_power(me.owner, me.idx);
    }
});

/// `TemporaryStrengthPower` subclasses (`GetPowerInstances<TemporaryStrengthPower>`).
pub fn is_temporary_strength(id: u16) -> bool {
    matches!(
        id,
        ids::power::COORDINATE_POWER
            | ids::power::CRUSH_UNDER_POWER
            | ids::power::DARK_SHACKLES_POWER
            | ids::power::DYING_STAR_POWER
            | ids::power::ENFEEBLING_TOUCH_POWER
            | ids::power::FEEDING_FRENZY_POWER
            | ids::power::FLEX_POTION_POWER
            | ids::power::MANGLE_POWER
            | ids::power::MONARCHS_GAZE_STRENGTH_DOWN_POWER
            | ids::power::PIERCING_WAIL_POWER
            | ids::power::REPTILE_TRINKET_POWER
            | ids::power::SHACKLING_POTION_POWER
            | ids::power::SETUP_STRIKE_POWER
    )
}

// ---- IllusionPower (Eye With Teeth, Parafright): dies -> REVIVE_MOVE -> back at full HP ----------------------------------
// Power.aux = `isReviving`.
listener!(IllusionPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        if !cx.has_power(me.owner, ids::power::MINION_POWER) {
            cx.apply_power(ids::power::MINION_POWER, me.owner, Dec::ONE, NO, NO);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if creature != me.owner || was_removal_prevented {
            return;
        }
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = 1;
        }
        // MoveState REVIVE_MOVE{FollowUpStateId = last logged move, MustPerformOnce}; SetMoveImmediate WITHOUT force.
        let mid = cx.cr(me.owner).monster.id;
        if let Some(node) = crate::content::node_by_name(mid, "REVIVE_MOVE") {
            let next = cx.last_logged_move(me.owner);
            cx.set_move_immediate_follow(me.owner, node, next, false);
        }
    }
    fn should_allow_hitting(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        if creature != me.owner {
            return true;
        }
        cx.cr(me.owner).power(me.id).map_or(true, |p| p.aux == 0)
    }
    fn should_creature_be_removed_from_combat_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn should_power_be_removed_on_death(&self, _cx: &Combat, _me: Me, _power_owner: Cid, power_id: u16) -> bool {
        // debuffs (except temporary powers) are removed; everything else stays
        crate::content::power_def(power_id).ptype == PowerType::Debuff && !is_temporary_strength(power_id)
    }
});

/// `IllusionPower.ReviveMove`: `isReviving = false`, heal to full.
pub fn illusion_revive(cx: &mut Combat, me: Cid) {
    if let Some(p) = cx.cr(me).power(ids::power::ILLUSION_POWER) {
        let uid = p.uid;
        if let Some(i) = cx.power_idx(me, uid) {
            cx.cr_mut(me).powers[i].aux = 0;
        }
    }
    let missing = cx.cr(me).max_hp - cx.cr(me).hp;
    cx.heal(me, Dec::int(missing as i64));
}
