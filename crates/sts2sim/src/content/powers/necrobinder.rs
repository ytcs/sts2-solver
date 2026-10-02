//! Necrobinder powers (Osty / Doom / Souls / Ethereal).

use crate::dec::Dec;
use crate::engine::{is_temporary_power, DamageResult};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

#[inline]
fn on_side(cx: &Combat, owner: Cid, side: Side) -> bool {
    cx.creatures_on(side).contains(owner)
}

#[inline]
fn amount_of(cx: &Combat, me: Me) -> i32 {
    cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(me.amount, |p| p.amount)
}

// ---- Osty -----------------------------------------------------------------------------------------------------------

// Osty's permanent power: the owner's powered attacks hit Osty first. It survives Osty's death (and revival), and a dead
// Osty (hp 0) is not hittable / cannot receive powers.
listener!(DieForYouPower {
    fn modify_unblocked_damage_target(&self, cx: &Combat, me: Me, target: Cid, _amount: Dec, props: ValueProp, _dealer: Cid) -> Cid {
        if target != cx.cr(me.owner).owner {
            return target;
        }
        if cx.cr(me.owner).is_dead() {
            return target;
        }
        if !props.is_powered() {
            return target;
        }
        me.owner
    }
    fn should_allow_hitting(&self, cx: &Combat, _me: Me, creature: Cid) -> bool {
        cx.cr(creature).is_alive()
    }
    fn should_power_be_removed_after_owner_death(&self) -> bool {
        false
    }
});

// Osty's attacks deal +Amount damage.
listener!(CalcifyPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer == NO || !cx.cr(q.dealer).is_pet || cx.cr(q.dealer).monster.id != ids::monster::OSTY {
            return Dec::ZERO;
        }
        if me.owner != cx.cr(q.dealer).owner || !q.props.is_powered() {
            return Dec::ZERO;
        }
        Dec::int(amount_of(cx, me) as i64)
    }
});

// Whenever Osty loses HP, every hittable enemy takes that much x Amount (unblockable, unpowered).
listener!(NecroMasteryPower {
    fn after_current_hp_changed(&self, cx: &mut Combat, me: Me, creature: Cid, delta: i32) {
        if delta >= 0 || !cx.cr(creature).is_pet || cx.cr(creature).monster.id != ids::monster::OSTY || cx.cr(creature).owner != me.owner {
            return;
        }
        let targets = cx.hittable_enemies();
        let amt = Dec::int((-delta) as i64 * amount_of(cx, me) as i64);
        cx.damage(targets.as_slice(), amt, ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), me.owner, NO);
    }
});

// Next turn: summon (Invoke). `AmountOnTurnStart != 0` guards against powers applied during the turn-start itself.
listener!(SummonNextTurnPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.amount_on_turn_start != 0 {
            cx.summon(p.amount);
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Enemy debuff: when the player's Osty hits this enemy, summon (Amount); expires at the end of the enemy turn.
listener!(SicEmPower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, _unblocked: i32, _props: ValueProp) {
        if dealer == NO || !cx.cr(dealer).is_pet || cx.cr(dealer).monster.id != ids::monster::OSTY || target != me.owner {
            return;
        }
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.applier != NO && cx.cr(dealer).owner == p.applier {
            cx.summon(p.amount);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, me.owner, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// ---- Energy / draw ----------------------------------------------------------------------------------------------------

listener!(EnergyNextTurnPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let n = amount_of(cx, me);
        cx.gain_energy(n);
        cx.remove_power(me.owner, me.idx);
    }
});

// +Amount max energy for the rest of the combat (Friendship).
listener!(FriendshipPower {
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount + Dec::int(amount_of(cx, me) as i64)
    }
});

// +Amount max energy and +Amount cards drawn per turn (Demesne).
listener!(DemesnePower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount + Dec::int(amount_of(cx, me) as i64)
    }
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount + Dec::int(amount_of(cx, me) as i64)
    }
});

// Cards cost +Amount this turn (Borrowed Time).
listener!(BorrowedTimePower {
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, _card: CardIdx, cost: Dec) -> Option<Dec> {
        Some(cost + Dec::int(amount_of(cx, me) as i64))
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, me.owner, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Gain Block when playing a card that costs >= 2 (the power's own EnergyVar(2)).
listener!(DanseMacabrePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let c = play.card;
        let resolved = if cx.card_def(c).x_cost { cx.cards[c as usize].x_value as i32 } else { cx.card_cost(c, true).max(0) };
        if resolved >= 2 {
            let n = amount_of(cx, me);
            cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// Draw Amount cards whenever an Ethereal card is drawn.
listener!(PagestormPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if cx.card_keywords(card) & kw::ETHEREAL != 0 {
            let n = amount_of(cx, me);
            cx.draw_cards(n, false);
        }
    }
});

// Block when playing an Ethereal card.
listener!(SpiritOfAshPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_keywords(play.card) & kw::ETHEREAL != 0 {
            let n = amount_of(cx, me);
            cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// The next Amount Ethereal cards cost 0 (Veilpiercer).
listener!(VeilpiercerPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, _me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if cx.card_keywords(card) & kw::ETHEREAL == 0 {
            return None;
        }
        match cx.card_pile_type(card) {
            PileType::Hand | PileType::Play => Some(Dec::ZERO),
            _ => None,
        }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_keywords(play.card) & kw::ETHEREAL == 0 {
            return;
        }
        if matches!(cx.card_pile_type(play.card), PileType::Hand | PileType::Play) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// Before the hand draw: add Amount random Ethereal cards from the character's pool to the hand (Call of the Void).
listener!(CallOfTheVoidPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let pool = cx.character_pool();
        let n = amount_of(cx, me);
        let mut made: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
        for _ in 0..n {
            let got = cx.get_distinct_for_combat(pool, 1, |_| true);
            if let Some(c) = got.first() {
                cx.apply_keyword(c, kw::ETHEREAL);
                made.push(c);
            }
        }
        for &c in made.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// Before the hand draw: add Amount Sweeping Gaze to the hand (Sentry Mode).
listener!(SentryModePower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let n = amount_of(cx, me);
        for _ in 0..n {
            if let Some(c) = cx.new_card(ids::card::SWEEPING_GAZE, 0) {
                cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
        }
    }
});

// ---- Doom -----------------------------------------------------------------------------------------------------------

fn doom_triggers(cx: &Combat, me: Me, side: Side) -> bool {
    if cx.is_over_or_ending() {
        return false;
    }
    if !on_side(cx, me.owner, side) || cx.cr(me.owner).is_dead() || !cx.is_doomed(me.owner) {
        return false;
    }
    cx.doomed_on_side(side).first() == Some(me.owner)
}

// Doom: kills the creature at the end of its side's turn once its HP <= Amount. Only the first doomed creature of a side
// triggers; it kills every doomed creature of the side at once.
listener!(DoomPower {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player && doom_triggers(cx, me, side) {
            let doomed = cx.doomed_on_side(side);
            cx.doom_kill(doomed.as_slice());
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Enemy && doom_triggers(cx, me, side) {
            let doomed = cx.doomed_on_side(side);
            cx.doom_kill(doomed.as_slice());
        }
    }
});

// Start of your turn: Doom on a random hittable enemy.
listener!(CountdownPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if !on_side(cx, me.owner, side) {
            return;
        }
        if let Some(t) = cx.random_hittable_enemy() {
            let n = amount_of(cx, me);
            cx.apply_power(ids::power::DOOM_POWER, t, Dec::int(n as i64), me.owner, NO);
        }
    }
});

// Start of your turn: you gain Doom (Neurosurge).
listener!(NeurosurgePower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, me.owner, side) {
            let n = amount_of(cx, me);
            cx.apply_power(ids::power::DOOM_POWER, me.owner, Dec::int(n as i64), me.owner, NO);
        }
    }
});

// Whenever you / Osty deal powered damage, apply that much x Amount Doom to the target.
listener!(ReaperFormPower {
    fn after_damage_given_full(&self, cx: &mut Combat, me: Me, dealer: Cid, res: &DamageResult, props: ValueProp, _card: CardIdx) {
        if dealer == NO || !(dealer == me.owner || (cx.cr(dealer).is_pet && cx.cr(dealer).owner == me.owner)) {
            return;
        }
        if !props.is_powered() || res.total() <= 0 {
            return;
        }
        let amt = res.total() as i64 * amount_of(cx, me) as i64;
        cx.apply_power(ids::power::DOOM_POWER, res.receiver, Dec::int(amt), me.owner, NO);
    }
});

// Gain Block whenever you apply Doom.
listener!(ShroudPower {
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, _target: Cid, _delta: i32, applier: Cid, _card: CardIdx) {
        if applier == me.owner && power_id == ids::power::DOOM_POWER {
            let n = amount_of(cx, me);
            cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// Whenever you apply a debuff to an enemy (other than a temporary one) it takes Amount damage.
listener!(SleightOfFleshPower {
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, target: Cid, delta: i32, applier: Cid, _card: CardIdx) {
        if delta == 0 || applier != me.owner || cx.cr(target).side != Side::Enemy || is_temporary_power(power_id) {
            return;
        }
        if Combat::power_type_for_amount(power_id, delta) != PowerType::Debuff {
            return;
        }
        let n = amount_of(cx, me);
        cx.damage(&[target], Dec::int(n as i64), ValueProp::UNPOWERED, me.owner, NO);
    }
});

// Enemy debuff: the player's cards played from now on (this turn) apply Doom(Amount) to this enemy afterwards. Expires at end of turn.
// `aux` = (card index + 1) << 20 | recorded amount of the card play in flight.
listener!(OblivionPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let amt = amount_of(cx, me);
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = (((play.card as i32) + 1) << 20) | amt.min((1 << 20) - 1);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.aux >> 20 == (play.card as i32) + 1 {
            let applier = p.applier;
            let value = p.aux & ((1 << 20) - 1);
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux = 0;
            }
            cx.apply_power(ids::power::DOOM_POWER, me.owner, Dec::int(value as i64), applier, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// ---- Souls -----------------------------------------------------------------------------------------------------------

// Playing a Soul summons Osty (Amount).
listener!(DevourLifePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.is_card(play.card, ids::card::SOUL) {
            let n = amount_of(cx, me);
            cx.summon(n);
        }
    }
});

// Playing a Soul deals Amount unblockable unpowered damage to a random enemy.
listener!(HauntPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if !cx.is_card(play.card, ids::card::SOUL) {
            return;
        }
        let h = cx.hittable_enemies();
        if h.is_empty() {
            return;
        }
        let i = cx.rng.combat_targets.next_int_range(0, h.len() as i32) as usize;
        let n = amount_of(cx, me);
        cx.damage(&[h[i]], Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
    }
});

// ---- Attack modifiers --------------------------------------------------------------------------------------------------

// The Hang card deals x Amount damage to this enemy.
listener!(HangPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !cx.is_card(q.card, ids::card::HANG) {
            return Dec::ONE;
        }
        Dec::int(amount_of(cx, me) as i64)
    }
});

// Your first attack each turn deals +Amount% damage.
listener!(LethalityPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.card == NO || me.owner != PLAYER {
            return Dec::ONE;
        }
        let in_play = cx.card_pile_type(q.card) == PileType::Play;
        if in_play {
            if let Some(ctx) = cx.play_ctx {
                if ctx.play.card == q.card && ctx.play.play_index > 0 {
                    return Dec::ONE;
                }
            }
        }
        let started = cx.hist.attacks_played_this_turn as i32;
        if started > in_play as i32 {
            return Dec::ONE;
        }
        Dec::ONE + Dec::frac(amount_of(cx, me) as i64, 2)
    }
});

// Debuff on the owner: Vulnerable (to the owner) / Weak (from the owner) are doubled; ticks down at the end of its side's turn.
// (The multiplier change itself lives in `VulnerablePower` / `WeakPower`.)
listener!(DebilitatePower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, me.owner, side) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// ---- Temporary Strength (Enfeebling Touch) ------------------------------------------------------------------------------
// `TemporaryStrengthPower` with `IsPositive = false`: applying it applies -Amount Strength; it removes itself (and restores
// the Strength) at the end of its owner's side turn.
listener!(EnfeeblingTouchPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.apply_power(ids::power::STRENGTH_POWER, target, -amount, applier, card);
    }
    fn after_power_amount_changed_ex(&self, cx: &mut Combat, me: Me, power_id: u16, target: Cid, delta: i32, applier: Cid, card: CardIdx) {
        if power_id != ids::power::ENFEEBLING_TOUCH_POWER || target != me.owner {
            return;
        }
        let amt = amount_of(cx, me);
        if delta == amt {
            return;
        }
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(-(delta as i64)), applier, card);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, me.owner, side) {
            let n = amount_of(cx, me);
            cx.remove_power(me.owner, me.idx);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(n as i64), me.owner, NO);
        }
    }
});

// ---- Misc ---------------------------------------------------------------------------------------------------------------

listener!(ForbiddenGrimoirePower {});

// ---- multiplayer-only cards' powers (single-player reductions) ---------------------------------------------------------------

// Every 33 cards drawn: Amount damage (unpowered, dealer = you) to a random enemy. `aux` = cards drawn since the last trigger.
listener!(CacophonyPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, _from_hand_draw: bool) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        cx.cr_mut(me.owner).powers[i].aux += 1;
        if cx.cr(me.owner).powers[i].aux >= 33 {
            cx.cr_mut(me.owner).powers[i].aux = 0;
            let amt = amount_of(cx, me);
            if let Some(t) = cx.random_hittable_enemy() {
                cx.damage(&[t], Dec::int(amt as i64), ValueProp::UNPOWERED, me.owner, NO);
            }
        }
    }
});

// Whenever you create a Soul, add Amount more Souls to the draw pile (not recursively).
listener!(SoulboundPower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if !cx.is_card(card, ids::card::SOUL) {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        if cx.cr(me.owner).powers[i].aux != 0 {
            return;
        }
        cx.cr_mut(me.owner).powers[i].aux = 1;
        let n = amount_of(cx, me);
        cx.add_souls_to_draw_pile(n, false);
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = 0;
        }
    }
});

// Damage dealt by your allies (not Osty) applies Doom x Amount; expires at the end of the enemy turn.
listener!(UnderworldPower {
    fn after_damage_given_full(&self, cx: &mut Combat, me: Me, dealer: Cid, res: &DamageResult, props: ValueProp, _card: CardIdx) {
        if dealer == NO || cx.cr(dealer).side != cx.cr(me.owner).side || dealer == me.owner {
            return;
        }
        if (cx.cr(dealer).is_pet && cx.cr(dealer).owner == me.owner) || !props.is_powered() || res.total() <= 0 {
            return;
        }
        let amt = res.total() as i64 * amount_of(cx, me) as i64;
        cx.apply_power(ids::power::DOOM_POWER, res.receiver, Dec::int(amt), me.owner, NO);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
