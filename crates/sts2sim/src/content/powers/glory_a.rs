//! Powers owned or applied by the Act 3 "Glory" weak / normal monsters (spec 04 §3.4; C# `Models/Powers/*`).
//! Plating / Artifact / Minion / Ritual / Strength / Weak / Frail / Vulnerable live in their shared files.

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

// ---- StockPower (Axebot): on death spawns a replacement Axebot (StockAmount - 1, +10 max HP per respawn) ---------------
// The replacement's private `vars[0]` is `StockAmount + 1` (0 = the original, un-overridden Axebot).
listener!(StockPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
        if amount > 0 {
            let slot = cx.cr(me.owner).slot;
            // `CreatureCmd.Add(axebot, CombatState, Owner.Side, Owner.SlotName)`; axebot.StockAmount = Amount - 1.
            cx.summon_enemy(ids::monster::AXEBOT, slot, [amount, 0]);
        }
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
});

// ---- RampartPower (Living Shield): at the start of the player's turn every Turret Operator gains Amount block --------
listener!(RampartPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player || cx.extra_turn {
            return;
        }
        let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
        let list: crate::util::ArrayVec<Cid, MAX_CREATURES> = {
            let mut l = crate::util::ArrayVec::new();
            for &e in cx.enemies.iter() {
                if cx.cr(e).monster.id == ids::monster::TURRET_OPERATOR {
                    l.push(e);
                }
            }
            l
        };
        for &e in list.iter() {
            cx.gain_block(e, Dec::int(amount as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// ---- PaperCutsPower (Scroll of Biting): unblocked powered-attack damage to the player also removes Amount max HP -----
listener!(PaperCutsPower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, unblocked: i32, props: ValueProp) {
        if dealer == me.owner && cx.cr(target).is_player && props.is_powered() && unblocked > 0 {
            let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
            cx.lose_max_hp(target, Dec::int(amount as i64), false);
        }
    }
});

// ---- GalvanicPower (Globe Head): the player's Power cards are Galvanized (damage the player Amount when played) ---------
listener!(GalvanicPower {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_def(c).ctype == CardType::Power {
                cx.afflict_card(c, ids::affliction::GALVANIZED, amount);
            }
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if cx.card_affliction(card).is_none() && cx.card_def(card).ctype == CardType::Power {
            let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
            cx.afflict_card(card, ids::affliction::GALVANIZED, amount);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_affliction(play.card) == Some(ids::affliction::GALVANIZED) {
            let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
            cx.damage(&[PLAYER], Dec::int(amount as i64), ValueProp::UNPOWERED.or(ValueProp::MOVE), NO, NO);
        }
    }
});

// ---- SoarPower (Owl Magistrate): powered attacks against the owner deal 50% (DamageDecrease / 100) ---------------------
listener!(SoarPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(5, 1)
    }
});

// ---- PossessStrengthPower (The Lost): Strength it takes from the player (applier == owner) comes back when it dies -------
// aux = the (negative) total stolen from the player (`stolenStrength[player]`).
listener!(PossessStrengthPower {
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.applier != me.owner || !cx.cr(ch.target).is_player || ch.power_id != ids::power::STRENGTH_POWER || ch.amount >= 0 {
            return;
        }
        if let Some(p) = cx.power_mut(me.owner, me.idx) {
            p.aux += ch.amount;
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        let stolen = cx.cr(me.owner).power(me.id).map_or(0, |p| p.aux);
        if stolen != 0 {
            cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(-(stolen as i64)), NO, NO);
        }
    }
});

// ---- PossessSpeedPower (The Forgotten): same for Dexterity ----------------------------------------------------------------
listener!(PossessSpeedPower {
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.applier != me.owner || !cx.cr(ch.target).is_player || ch.power_id != ids::power::DEXTERITY_POWER || ch.amount >= 0 {
            return;
        }
        if let Some(p) = cx.power_mut(me.owner, me.idx) {
            p.aux += ch.amount;
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        let stolen = cx.cr(me.owner).power(me.id).map_or(0, |p| p.aux);
        if stolen != 0 {
            cx.apply_power(ids::power::DEXTERITY_POWER, PLAYER, Dec::int(-(stolen as i64)), NO, NO);
        }
    }
});

// ---- HighVoltagePower (Zapbot): at the end of its side's turn the owner gains Amount Strength -------------------------------
listener!(HighVoltagePower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            let amount = cx.cr(me.owner).power(me.id).map_or(me.amount, |p| p.amount);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(amount as i64), me.owner, NO);
        }
    }
});
