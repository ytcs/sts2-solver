//! Powers owned or applied by the Act 2 "Hive" weak / normal monsters (spec 04 §3.3; C# `Models/Powers/*`):
//! Imbalanced (BowlbugRock), Burrowed (Tunneler), HardToKill (Exoskeleton), Tender (HunterKiller), CurlUp (LouseProgenitor),
//! Hatch (ToughEgg), Slumber (SlumberingBeetle), Flutter / Swipe / EscapeArtist (ThievingHopper).

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

// ---- ImbalancedPower (BowlbugRock): its own damage being fully blocked knocks it off balance ------------------------
listener!(ImbalancedPower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, _target: Cid, _unblocked: i32, _props: ValueProp) {
        if dealer != me.owner || !cx.dmg_result.fully_blocked {
            return;
        }
        if cx.cr(me.owner).monster.id == ids::monster::BOWLBUG_ROCK {
            // BowlbugRock.IsOffBalance = true (vars[0])
            cx.creatures[me.owner as usize].monster.vars[0] = 1;
        } else {
            cx.stun(me.owner, None, None);
        }
    }
});

// ---- BurrowedPower (Tunneler): block persists; breaking it stuns the Tunneler -----------------------------------------
listener!(BurrowedPower {
    fn should_clear_block(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn after_block_broken(&self, cx: &mut Combat, me: Me, target: Cid, _breaker: Cid) {
        if target != me.owner || cx.cr(target).monster.id != ids::monster::TUNNELER {
            return;
        }
        crate::content::monsters::hive_a::tunneler_get_stunned(cx, target);
        cx.remove_power(me.owner, me.idx);
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, old_owner: Cid) {
        cx.lose_block(old_owner, Dec::int(999_999_999), NO);
    }
});

// ---- HardToKillPower (Exoskeleton): every hit is capped to Amount --------------------------------------------------
listener!(HardToKillPower {
    fn modify_damage_cap(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner {
            return Dec::MAX;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// ---- TenderPower (HunterKiller debuff on the player): every card played costs 1 Strength and 1 Dexterity this turn ------
// aux = CardsPlayedThisTurn.
listener!(TenderPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        cx.cr_mut(me.owner).powers[i].aux += 1;
        let applier = cx.cr(me.owner).powers[i].applier;
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(-1), applier, NO);
        cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(-1), applier, NO);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !owner_side(cx, me, side) {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let n = cx.cr(me.owner).powers[i].aux;
        let applier = cx.cr(me.owner).powers[i].applier;
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(n as i64), applier, NO);
        cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(n as i64), applier, NO);
        cx.set_power_aux(me.owner, me.idx, 0);
    }
});

// ---- CurlUpPower (LouseProgenitor): after a card hurt it, gain Amount block once that card finished ---------------------
// aux = (first powered-attack card that damaged the owner) + 1, 0 = none (`Data.playedCard`).
listener!(CurlUpPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _unblocked: i32, props: ValueProp, _dealer: Cid) {
        if target != me.owner || !props.is_powered() {
            return;
        }
        let card = cx.dmg_card;
        if card == NO {
            return;
        }
        let cur = cx.power_aux(me.owner, me.idx);
        if cur != 0 && card as i32 != cur - 1 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, card as i32 + 1);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let cur = cx.power_aux(me.owner, me.idx);
        if cur == 0 || play.card as i32 != cur - 1 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, 0);
        let amount = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(amount as i64), ValueProp::UNPOWERED, NO);
        if cx.cr(me.owner).monster.id == ids::monster::LOUSE_PROGENITOR {
            cx.creatures[me.owner as usize].monster.vars[0] = 1; // Curled
        }
        cx.remove_power(me.owner, me.idx);
    }
});

// ---- HatchPower (ToughEgg): countdown display ---------------------------------------------------------------------------
listener!(HatchPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// ---- SlumberPower (SlumberingBeetle) ----------------------------------------------------------------------------------
listener!(SlumberPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target != me.owner || unblocked == 0 {
            return;
        }
        cx.decrement_power(me.owner, me.idx);
        // `Amount <= 0` of the (possibly removed) power instance
        if cx.power_amount(me.owner, me.id) <= 0 {
            crate::content::monsters::hive_a::beetle_wake_up_stun(cx, me.owner);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !owner_side(cx, me, side) {
            return;
        }
        cx.decrement_power(me.owner, me.idx);
        if cx.power_amount(me.owner, me.id) <= 0 {
            crate::content::monsters::hive_a::beetle_wake_up(cx, me.owner);
        }
    }
});

// ---- FlutterPower (ThievingHopper): -50% powered-attack damage; Amount unblocked hits stun it --------------------------
listener!(FlutterPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        // DynamicVars["DamageDecrease"] (50) / 100
        Dec::frac(5, 1)
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, _dealer: Cid) {
        if target != me.owner || unblocked == 0 || !props.is_powered() {
            return;
        }
        cx.decrement_power(me.owner, me.idx);
        if cx.power_amount(me.owner, me.id) <= 0 {
            // nextState = StateLog.Last().GetNextState(...): the successor of the pending move (a plain follow-up: no RNG)
            let last = cx.last_logged_move(me.owner);
            let def = crate::content::monster_def(cx.cr(me.owner).monster.id);
            let next = match &def.nodes[last as usize] {
                crate::defs::MonsterNode::Move { follow_up, .. } => *follow_up,
                _ => panic!("StateLog.Last() is a move"),
            };
            cx.stun(me.owner, None, Some(next));
        }
    }
});

// ---- SwipePower (ThievingHopper): one instance per stolen card; the card goes back to the deck run-level (not modelled) -
listener!(SwipePower {});

// ---- EscapeArtistPower (ThievingHopper): countdown display --------------------------------------------------------------
listener!(EscapeArtistPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) && cx.power_amount(me.owner, me.id) > 1 {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});
