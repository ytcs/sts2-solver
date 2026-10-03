//! Defect powers.

use crate::dec::Dec;
use crate::engine::{HKind, VALID_ORBS};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- Focus family ----------------------------------------------------------------------------------------------------

// FocusPower.ModifyOrbValue: `max(value + Amount, 0)` for the owner's orbs.
listener!(FocusPower {
    fn modify_orb_value(&self, cx: &Combat, me: Me, _orb: &Orb, value: Dec) -> Dec {
        if me.owner != PLAYER {
            return value;
        }
        (value + Dec::int(cx.power_amount(me.owner, me.id) as i64)).max(Dec::ZERO)
    }
});

// `TemporaryFocusPower` (sign +1 for Focused Strike / Hotfix / Synchronize, -1 for Hyperbeam):
//  * BeforeApplied: apply `sign * amount` Focus.
//  * AfterPowerAmountChanged (this power, and the change is not the initial application): apply `sign * delta` Focus.
//  * AfterSideTurnEnd: remove itself, then apply `-sign * Amount` Focus.
fn temp_before_applied(cx: &mut Combat, sign: i32, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
    cx.apply_power(ids::power::FOCUS_POWER, target, Dec::int(sign as i64) * amount, applier, card);
}

fn temp_after_changed(cx: &mut Combat, me: Me, sign: i32, ch: &PowerChange) {
    // `power == this` (identified by target + uid) and not the initial application (`amount == Amount`).
    if ch.uid != me.idx || ch.target != me.owner || ch.amount == cx.power_amount(me.owner, me.id) {
        return;
    }
    cx.apply_power(ids::power::FOCUS_POWER, me.owner, Dec::int((sign * ch.amount) as i64), ch.applier, ch.card);
}

fn temp_side_turn_end(cx: &mut Combat, me: Me, sign: i32, side: Side) {
    if side != Side::Player {
        return;
    }
    let amount = cx.power_amount(me.owner, me.id);
    cx.remove_power(me.owner, me.idx);
    cx.apply_power(ids::power::FOCUS_POWER, me.owner, Dec::int((-sign * amount) as i64), me.owner, NO);
}

listener!(FocusedStrikePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_after_changed(cx, me, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_side_turn_end(cx, me, 1, side);
    }
});

listener!(HotfixPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_after_changed(cx, me, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_side_turn_end(cx, me, 1, side);
    }
});

listener!(SynchronizePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_after_changed(cx, me, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_side_turn_end(cx, me, 1, side);
    }
});

listener!(HyperbeamFocusDownPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_after_changed(cx, me, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_side_turn_end(cx, me, -1, side);
    }
});

// BiasedCognitionPower: at the start of the owner's turn, lose `Amount` Focus.
listener!(BiasedCognitionPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let a = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::FOCUS_POWER, me.owner, Dec::int(-(a as i64)), me.owner, NO);
        }
    }
});

// ---- orb powers -------------------------------------------------------------------------------------------------------

// Lightning Rod: after the energy reset, channel a Lightning, then decrement.
listener!(LightningRodPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        cx.channel_orb(ids::orb::LIGHTNING_ORB);
        cx.decrement_power(me.owner, me.idx);
    }
});

// Loop: after the player's turn start, trigger the FRONT orb's passive `Amount` times (no trigger-count hooks).
listener!(LoopPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if cx.orb_count() == 0 {
            return;
        }
        let n = cx.power_amount(me.owner, me.id);
        for _ in 0..n {
            if let Some(front) = cx.player.orbs.first() {
                cx.orb_passive(front, NO, false);
            }
        }
    }
});

// Coolant: at the start of the turn gain `distinct orb types x Amount` block (Unpowered).
listener!(CoolantPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let n = cx.distinct_orb_types() * cx.power_amount(me.owner, me.id);
            cx.gain_block(PLAYER, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// Consuming Shadow: at the end of the turn evoke the LAST orb `Amount` times.
listener!(ConsumingShadowPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.orb_count() != 0 {
            let n = cx.power_amount(me.owner, me.id);
            for _ in 0..n {
                cx.evoke_last(true);
            }
        }
    }
});

// Hailstorm: before the turn ends, if enough Frost orbs (the `FrostOrbs` var = 1) are channeled, damage all enemies.
listener!(HailstormPower {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player {
            return;
        }
        let frost = cx.player.orbs.iter().filter(|o| o.kind == ids::orb::FROST_ORB).count() as i32;
        if frost >= 1 {
            let targets = cx.hittable_enemies();
            let a = cx.power_amount(me.owner, me.id);
            cx.damage(targets.as_slice(), Dec::int(a as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
});

// Hibernate (multiplayer card): decrements at the start of the owner's turn; Frost orbs read it for other players.
listener!(HibernatePower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        cx.decrement_power(me.owner, me.idx);
    }
});

// Spinner: after the energy reset, channel `Amount` Glass.
listener!(SpinnerPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let n = cx.power_amount(me.owner, me.id);
        for _ in 0..n {
            cx.channel_orb(ids::orb::GLASS_ORB);
        }
    }
});

// Thunder: whenever one of the owner's Lightning orbs is evoked, deal `Amount` to the (living) targets it hit.
listener!(ThunderPower {
    fn after_orb_evoked(&self, cx: &mut Combat, me: Me, orb: &Orb, targets: &[Cid]) {
        if orb.kind != ids::orb::LIGHTNING_ORB {
            return;
        }
        let mut living: crate::util::ArrayVec<Cid, MAX_CREATURES> = crate::util::ArrayVec::new();
        for &t in targets {
            if cx.cr(t).is_alive() {
                living.push(t);
            }
        }
        let a = cx.power_amount(me.owner, me.id);
        cx.damage(living.as_slice(), Dec::int(a as i64), ValueProp::UNPOWERED, me.owner, NO);
    }
});

// Trash to Treasure / Smokestack: react to Status cards the owner's own effects generate (`creator == owner`).
listener!(TrashToTreasurePower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx, added_by_player: bool) {
        if cx.card_def(card).ctype == CardType::Status && added_by_player && me.owner == PLAYER {
            let n = cx.power_amount(me.owner, me.id);
            for _ in 0..n {
                let i = cx.rng.combat_orbs.next_int_range(0, VALID_ORBS.len() as i32) as usize;
                cx.channel_orb(VALID_ORBS[i]);
            }
        }
    }
});

listener!(SmokestackPower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx, added_by_player: bool) {
        if cx.card_def(card).ctype == CardType::Status && added_by_player && me.owner == PLAYER {
            let targets = cx.hittable_enemies();
            let a = cx.power_amount(me.owner, me.id);
            cx.damage(targets.as_slice(), Dec::int(a as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
});

// ---- card-play powers --------------------------------------------------------------------------------------------------

/// Storm / Subroutine remember `(power card being played, Amount at that moment)` between `BeforeCardPlayed` and
/// `AfterCardPlayed` (the game keeps a `Dictionary<CardModel, int>`): `hist.play_amounts` (plays nest).
fn remember_power_card_play(cx: &mut Combat, me: Me, play: &CardPlay) {
    if cx.card_def(play.card).ctype != CardType::Power {
        return;
    }
    let amount = cx.power_amount(me.owner, me.id);
    cx.hist.remember_play(me.idx, play.card, amount);
}

/// Pops the remembered amount for `play.card` (0 if it was not remembered).
fn take_power_card_play(cx: &mut Combat, me: Me, play: &CardPlay) -> i32 {
    cx.hist.take_play(me.idx, play.card).unwrap_or(0).max(0)
}

// Storm: whenever the owner plays a Power card, channel (Amount at the time it started playing) Lightning.
listener!(StormPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        remember_power_card_play(cx, me, play);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let n = take_power_card_play(cx, me, play);
        for _ in 0..n {
            cx.channel_orb(ids::orb::LIGHTNING_ORB);
        }
    }
});

// Subroutine: whenever the owner plays a Power card, gain (Amount at the time it started playing) energy.
listener!(SubroutinePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        remember_power_card_play(cx, me, play);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let n = take_power_card_play(cx, me, play);
        for _ in 0..n {
            cx.gain_energy(1);
        }
    }
});

// Synthesis: the next Power card played this turn costs 0 (in hand or being played), then the power decrements.
fn power_card_in_play_zone(cx: &Combat, card: CardIdx) -> bool {
    cx.card_def(card).ctype == CardType::Power && matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play)
}

listener!(FreePowerPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, _me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if power_card_in_play_zone(cx, card) {
            Some(Dec::ZERO)
        } else {
            None
        }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if power_card_in_play_zone(cx, play.card) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// Machine Learning: draw `Amount` extra cards at the start of each turn.
listener!(MachineLearningPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount + Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// Iteration: the first Status card drawn each turn draws `Amount` more cards.
listener!(IterationPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if cx.card_def(card).ctype == CardType::Status
            && cx.hist_count_this_turn(HKind::CardDrawn, |e| crate::content::card_def(e.id).ctype == CardType::Status) <= 1 {
            let n = cx.power_amount(me.owner, me.id);
            cx.draw_cards(n, false);
        }
    }
});

// Creative AI: before the hand is drawn, generate `Amount` random Power cards from the Defect pool into hand.
listener!(CreativeAiPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let n = cx.power_amount(me.owner, me.id);
        for _ in 0..n {
            let pool = cx.character_pool();
            let cards = cx.get_distinct_for_combat(pool, 1, |d| d.ctype == CardType::Power);
            if let Some(&c) = cards.first().as_ref() {
                cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
        }
    }
});

// One For All (multiplayer card): +Amount damage on the owner's 0-energy-spent, non-X attacks.
listener!(OneForAllPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.card == NO || me.owner != PLAYER {
            return Dec::ZERO;
        }
        let d = cx.card_def(q.card);
        if d.x_cost {
            return Dec::ZERO;
        }
        // cardPlay == null path: current cost; cardPlay path: energy actually spent (identical while the card is in play).
        let spent = match cx.play_stack.iter().rev().find(|ctx| ctx.play.card == q.card) {
            Some(ctx) => ctx.play.energy_spent,
            None => cx.card_cost(q.card, true),
        };
        if spent != 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// Echo Form: the first `Amount` cards the owner plays each turn are played twice.
listener!(EchoFormPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, _card: CardIdx, _target: Cid, count: i32) -> i32 {
        let first_series = cx.hist_count_this_turn(HKind::CardPlayStarted, |e| e.flags & 2 != 0) as i32; // IsFirstInSeries
        if first_series >= cx.power_amount(me.owner, me.id) {
            count
        } else {
            count + 1
        }
    }
});

// Signal Boost: the owner's next Power card is played twice (the power then decrements).
listener!(SignalBoostPower {
    fn modify_card_play_count(&self, cx: &Combat, _me: Me, card: CardIdx, _target: Cid, count: i32) -> i32 {
        if cx.card_def(card).ctype != CardType::Power {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
});

// Feral: the first `Amount` Attacks played for 0 energy each turn return to the top of the hand instead of the discard
// pile. `Power::aux` = zero-cost attacks played so far this turn (`Data.zeroCostAttacksPlayed`).
listener!(FeralPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        let n = cx.hist_count_this_turn(HKind::CardPlayStarted, |e| e.aux == 0 && cx.card_def(e.card).ctype == CardType::Attack) as i32;
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = n;
        }
    }
    fn modify_card_play_result_location(&self, cx: &Combat, me: Me, card: CardIdx, _is_auto: bool, energy_value: i32, loc: CardLocation) -> CardLocation {
        if cx.card_def(card).ctype != CardType::Attack || energy_value > 0 || cx.cards[card as usize].flags & cflag::IS_DUPE != 0 {
            return loc;
        }
        let used = cx.cr(me.owner).power(me.id).map_or(0, |p| p.aux);
        if used >= cx.power_amount(me.owner, me.id) {
            return loc;
        }
        CardLocation::new(PileType::Hand, CardPilePosition::Top)
    }
    fn after_modifying_card_play_result_location(&self, cx: &mut Combat, me: Me, _card: CardIdx, _loc: CardLocation) {
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux += 1;
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux = 0;
            }
        }
    }
});

// Multiplayer-only (Imitation Learning is an ally-targeted card): never applied in single player.
listener!(ImitationLearningPower {});
