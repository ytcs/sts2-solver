//! Powers applied by the Ironclad cards Infernal Blade .. Whirlwind (second batch).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- Inferno: lose `SelfDamage` HP at turn start; whenever the owner loses HP on its own turn, damage all enemies ---------

// Per-instance state: `aux` = the `SelfDamage` var (incremented once per Inferno card played, see the card).
listener!(InfernoPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let dmg = cx.cr(me.owner).power(me.id).map_or(0, |p| p.aux);
        cx.damage(&[me.owner], Dec::int(dmg as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), me.owner, NO);
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target != me.owner || unblocked <= 0 || cx.side != cx.cr(me.owner).side {
            return;
        }
        let amount = cx.power_amount(me.owner, me.id);
        let targets = cx.hittable_enemies();
        cx.damage(targets.as_slice(), Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
    }
});

// ---- Juggernaut: damage a random enemy whenever the owner gains block -------------------------------------------------------
listener!(JuggernautPower {
    fn after_block_gained(&self, cx: &mut Combat, me: Me, creature: Cid, amount: Dec) {
        if amount <= Dec::ZERO || creature != me.owner {
            return;
        }
        let hittable = cx.hittable_enemies();
        if hittable.is_empty() {
            return;
        }
        let i = cx.rng.combat_targets.next_int_range(0, hittable.len() as i32) as usize;
        let target = hittable[i];
        let n = cx.power_amount(me.owner, me.id);
        cx.damage(&[target], Dec::int(n as i64), ValueProp::UNPOWERED, me.owner, NO);
    }
});

// ---- Juggling: the 3rd attack played each turn is cloned into the hand ------------------------------------------------------
// `aux` = attacks played this turn (the C# `Data.attacksPlayedThisTurn`).
listener!(JugglingPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        let n = cx.hist.attacks_played_this_turn as i32;
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = n;
        }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype != CardType::Attack {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        cx.cr_mut(me.owner).powers[i].aux += 1;
        if cx.cr(me.owner).powers[i].aux == 3 {
            let amount = cx.cr(me.owner).powers[i].amount;
            for _ in 0..amount {
                if let Some(c) = cx.clone_card(play.card) {
                    cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
            }
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != cx.cr(me.owner).side {
            return;
        }
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = 0;
        }
    }
});

// ---- TemporaryStrengthPower subclasses: Mangle (debuff, -Strength) / SetupStrike (+Strength) --------------------------------
// Shared logic (`sign` = +1 for a positive power, -1 for `IsPositive == false`).

fn temp_strength_before_applied(cx: &mut Combat, target: Cid, amount: Dec, applier: Cid, card: CardIdx, sign: i64) {
    cx.apply_power(ids::power::STRENGTH_POWER, target, Dec::int(sign) * amount, applier, card);
}

fn temp_strength_amount_changed(cx: &mut Combat, me: Me, power_id: u16, target: Cid, delta: i32, applier: Cid, card: CardIdx, sign: i64) {
    // `power == this && amount != Amount`
    if power_id != me.id || target != me.owner || delta == cx.power_amount(me.owner, me.id) {
        return;
    }
    cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(sign * delta as i64), applier, card);
}

fn temp_strength_turn_end(cx: &mut Combat, me: Me, side: Side, sign: i64) {
    if side != cx.cr(me.owner).side {
        return;
    }
    let amount = cx.power_amount(me.owner, me.id);
    cx.remove_power(me.owner, me.idx);
    cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(-sign * amount as i64), me.owner, NO);
}

listener!(ManglePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_strength_before_applied(cx, target, amount, applier, card, -1);
    }
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, target: Cid, delta: i32, applier: Cid, card: CardIdx) {
        temp_strength_amount_changed(cx, me, power_id, target, delta, applier, card, -1);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_strength_turn_end(cx, me, side, -1);
    }
});

listener!(SetupStrikePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_strength_before_applied(cx, target, amount, applier, card, 1);
    }
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, target: Cid, delta: i32, applier: Cid, card: CardIdx) {
        temp_strength_amount_changed(cx, me, power_id, target, delta, applier, card, 1);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_strength_turn_end(cx, me, side, 1);
    }
});

// ---- One-Two Punch: the next `Amount` attacks are played twice -------------------------------------------------------------
listener!(OneTwoPunchPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, _target: Cid, count: i32) -> i32 {
        if me.owner != PLAYER || cx.card_def(card).ctype != CardType::Attack {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == cx.cr(me.owner).side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// ---- Pyre: +Amount max energy ---------------------------------------------------------------------------------------------
listener!(PyrePower {
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        if me.owner != PLAYER {
            return amount;
        }
        amount + Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// ---- Rage: block per attack played this turn ------------------------------------------------------------------------------
listener!(RagePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner == PLAYER && cx.card_def(play.card).ctype == CardType::Attack {
            let n = cx.power_amount(me.owner, me.id);
            cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == cx.cr(me.owner).side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// ---- Rupture: Strength when the owner loses HP on its own turn --------------------------------------------------------------
// C# keeps `Dictionary<CardModel,int> playedCards` (cards currently being played -> Strength to grant when they finish).
// `aux` packs up to two entries (nested auto-plays), 16 bits each: `(card + 1) << 8 | accumulated amount`.
fn rupture_slot(aux: i32, i: u32) -> (i32, i32) {
    let v = (aux >> (16 * i)) & 0xFFFF;
    (v >> 8, v & 0xFF)
}
fn rupture_set(aux: i32, i: u32, card1: i32, acc: i32) -> i32 {
    (aux & !(0xFFFF << (16 * i))) | (((card1 << 8) | acc.min(255)) << (16 * i))
}
fn rupture_find(aux: i32, card: CardIdx) -> Option<u32> {
    (0..2).find(|&i| rupture_slot(aux, i).0 == card as i32 + 1)
}

listener!(RupturePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.side != cx.cr(me.owner).side {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let aux = cx.cr(me.owner).powers[i].aux;
        if let Some(slot) = (0..2).find(|&s| rupture_slot(aux, s).0 == 0) {
            cx.cr_mut(me.owner).powers[i].aux = rupture_set(aux, slot, play.card as i32 + 1, 0);
        }
    }
    fn after_damage_received_src(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid, card: CardIdx) {
        if target != me.owner || unblocked <= 0 || cx.side != cx.cr(me.owner).side {
            return;
        }
        let amount = cx.power_amount(me.owner, me.id);
        let tracked = cx.power_idx(me.owner, me.idx).and_then(|i| {
            let aux = cx.cr(me.owner).powers[i].aux;
            if card == NO { None } else { rupture_find(aux, card).map(|s| (i, aux, s)) }
        });
        match tracked {
            Some((i, aux, s)) => {
                let (c1, acc) = rupture_slot(aux, s);
                cx.cr_mut(me.owner).powers[i].aux = rupture_set(aux, s, c1, acc + amount);
            }
            None => {
                cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(amount as i64), me.owner, NO);
            }
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let aux = cx.cr(me.owner).powers[i].aux;
        if let Some(s) = rupture_find(aux, play.card) {
            let (_, acc) = rupture_slot(aux, s);
            cx.cr_mut(me.owner).powers[i].aux = rupture_set(aux, s, 0, 0);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(acc as i64), me.owner, NO);
        }
    }
});

// ---- Stampede: at end of turn auto-play `Amount` random attacks from hand -----------------------------------------------------
listener!(StampedePower {
    fn after_auto_post_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        let mut i = 0;
        while i < cx.power_amount(me.owner, me.id) {
            let mut items: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
            for &c in cx.player.hand.iter() {
                if cx.card_def(c).ctype == CardType::Attack && cx.card_keywords(c) & kw::UNPLAYABLE == 0 {
                    items.push(c);
                }
            }
            if !items.is_empty() {
                let k = cx.rng.shuffle.next_int_range(0, items.len() as i32) as usize;
                cx.auto_play(items[k], NO);
            }
            i += 1;
        }
    }
});

// ---- Plating: block at the end of the owner's turn, then loses 1 at the start of the next one --------------------------------
listener!(PlatingPower {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        // Enemies that start the combat with Plating also start it with block.
        if side != Side::Player || me.owner == PLAYER || cx.round > 1 {
            return;
        }
        let n = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
    }
    fn before_side_turn_end_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == cx.cr(me.owner).side {
            let n = cx.power_amount(me.owner, me.id);
            cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != cx.cr(me.owner).side {
            return;
        }
        let owner_is_player = me.owner == PLAYER;
        if (owner_is_player && cx.player.turn_number == 1) || (!owner_is_player && cx.round == 1) {
            return;
        }
        // Enemy: ModifyAmount(-Decrement) with Decrement = player count (1); player: Decrement(this) — both are -1.
        cx.decrement_power(me.owner, me.idx);
    }
});

// ---- Tank (multiplayer-only card): the owner takes 1.5x attack damage -----------------------------------------------------------
listener!(TankPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(15, 1)
    }
});

// ---- Unmovable: the first `Amount` card-block gains each turn are doubled -------------------------------------------------------
listener!(UnmovablePower {
    fn modify_block_multiplicative(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if !cx.cr(q.target).is_player || !q.props.has(ValueProp::MOVE) {
            return Dec::ONE;
        }
        if q.card != NO && me.owner != PLAYER {
            return Dec::ONE;
        }
        // Earlier BlockGained entries of this turn that came from a card play other than the current one.
        let mut n = cx.hist.card_blocks_this_turn as i32;
        if q.card != NO && cx.play_ctx.is_some() {
            n -= cx.hist.card_blocks_this_play as i32;
        }
        if n >= cx.power_amount(me.owner, me.id) { Dec::ONE } else { Dec::int(2) }
    }
});

// ---- Free Attack (Unrelenting): the next attack costs 0 ----------------------------------------------------------------------------
fn free_attack_applies(cx: &Combat, me: Me, card: CardIdx) -> bool {
    me.owner == PLAYER && cx.card_def(card).ctype == CardType::Attack && matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play)
}
listener!(FreeAttackPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if free_attack_applies(cx, me, card) { Some(Dec::ZERO) } else { None }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if free_attack_applies(cx, me, play.card) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// ---- Vicious: draw when the owner applies Vulnerable -------------------------------------------------------------------------------
listener!(ViciousPower {
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, _target: Cid, delta: i32, applier: Cid, _card: CardIdx) {
        if delta <= 0 || applier != me.owner || power_id != ids::power::VULNERABLE_POWER {
            return;
        }
        let n = cx.power_amount(me.owner, me.id);
        cx.draw_cards(n, false);
    }
});
