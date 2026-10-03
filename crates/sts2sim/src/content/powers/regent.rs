//! Regent powers (stars / Forge / Sovereign Blade) and the shared "next turn" powers Regent cards apply.
//! Bodies follow the decompiled `Models/Powers/*.cs`.

use crate::dec::Dec;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- helpers --------------------------------------------------------------------------------------------------------

/// `PowerModel.Aux` of the instance `me`.
fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].aux)
}
fn set_aux(cx: &mut Combat, me: &Me, v: i32) {
    if let Some(i) = cx.power_idx(me.owner, me.idx) {
        cx.cr_mut(me.owner).powers[i].aux = v;
    }
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount)
}

/// `TemporaryStrengthPower.BeforeApplied`: the Strength change arrives with the power.
fn temp_strength_before_applied(cx: &mut Combat, sign: i64, target: Cid, amt: Dec, applier: Cid, card: CardIdx) {
    cx.apply_power(ids::power::STRENGTH_POWER, target, Dec::int(sign) * amt, applier, card);
}
/// `TemporaryStrengthPower.AfterPowerAmountChanged`: stacking adds the same sign * delta of Strength.
fn temp_strength_after_changed(cx: &mut Combat, me: Me, sign: i64, ch: &PowerChange) {
    if ch.target == me.owner && ch.uid == me.idx && ch.amount != amount(cx, &me) {
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(sign * ch.amount as i64), ch.applier, NO);
    }
}
/// `TemporaryStrengthPower.AfterSideTurnEnd`: remove the power and undo the Strength.
fn temp_strength_side_end(cx: &mut Combat, me: Me, sign: i64, side: Side) {
    if cx.cr(me.owner).side == side {
        let a = amount(cx, &me);
        cx.remove_power(me.owner, me.idx);
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(-sign * a as i64), me.owner, NO);
    }
}

// ---- temporary Strength family ------------------------------------------------------------------------------------

// CrushUnderPower (`IsPositive => false`): enemies lose Strength until the end of their turn.
listener!(CrushUnderPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_strength_before_applied(cx, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_strength_after_changed(cx, me, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_strength_side_end(cx, me, -1, side);
    }
});

listener!(DyingStarPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_strength_before_applied(cx, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_strength_after_changed(cx, me, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_strength_side_end(cx, me, -1, side);
    }
});

listener!(MonarchsGazeStrengthDownPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_strength_before_applied(cx, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        temp_strength_after_changed(cx, me, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_strength_side_end(cx, me, -1, side);
    }
});

// ---- stars ---------------------------------------------------------------------------------------------------------

// StarNextTurnPower: gain Amount stars after the next energy reset.
listener!(StarNextTurnPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let a = amount(cx, &me);
        cx.gain_stars(a);
        cx.remove_power(me.owner, me.idx);
    }
});

// GenesisPower: +Amount stars every turn after the energy reset.
listener!(GenesisPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let a = amount(cx, &me);
        cx.gain_stars(a);
    }
});

// ChildOfTheStarsPower: spending stars gives Amount * stars spent unpowered block.
listener!(ChildOfTheStarsPower {
    fn after_stars_spent(&self, cx: &mut Combat, me: Me, spent: i32) {
        if spent > 0 {
            let a = amount(cx, &me);
            cx.gain_block(PLAYER, Dec::int(a as i64 * spent as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// TheSealedThronePower: playing a card gives Amount stars (BeforeCardPlayed).
listener!(TheSealedThronePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let a = amount(cx, &me);
        cx.gain_stars(a);
    }
});

// BlackHolePower: damage to all enemies whenever stars are spent on a card (after its last play) or gained.
fn black_hole_hit(cx: &mut Combat, me: Me) {
    let a = amount(cx, &me);
    let targets = cx.hittable_enemies();
    cx.damage(targets.as_slice(), Dec::int(a as i64), ValueProp::UNPOWERED, PLAYER, NO);
}
listener!(BlackHolePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if play.stars_spent > 0 && play.play_index + 1 >= play.play_count {
            black_hole_hit(cx, me);
        }
    }
    fn after_stars_gained(&self, cx: &mut Combat, me: Me, gained: i32) {
        if gained > 0 {
            black_hole_hit(cx, me);
        }
    }
});

// ---- Forge ---------------------------------------------------------------------------------------------------------

// FurnacePower: Forge Amount at the start of the owner's turn.
listener!(FurnacePower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let a = amount(cx, &me);
            cx.forge(a);
        }
    }
});

// HammerTimePower: forges for the other players (multiplayer only): nothing in single player.
listener!(HammerTimePower {});
// ParryPower / SeekingEdgePower do nothing themselves: Sovereign Blade reads them.
listener!(ParryPower {});
listener!(SeekingEdgePower {});

// ConquerorPower: Sovereign Blade deals double damage to the owner; decrements at the end of its turn.
listener!(ConquerorPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.card == NO || cx.cards[q.card as usize].id != ids::card::SOVEREIGN_BLADE || !q.props.is_powered() || q.target != me.owner {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// SwordSagePower: every Sovereign Blade replays Amount more times.
fn add_blade_replays(cx: &mut Combat, c: CardIdx, n: i32) {
    if cx.cards[c as usize].id == ids::card::SOVEREIGN_BLADE {
        let r = &mut cx.cards[c as usize].base_replay;
        *r = (*r as i32 + n).clamp(0, 255) as u8;
    }
}
listener!(SwordSagePower {
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.power_id != me.id || ch.target != me.owner {
            return;
        }
        let all = cx.player_combat_cards();
        for &c in all.iter() {
            add_blade_replays(cx, c, ch.amount);
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return; // `card.IsClone`: a clone / dupe already carries the original's replays
        }
        let a = amount(cx, &me);
        add_blade_replays(cx, card, a);
    }
    fn after_removed(&self, cx: &mut Combat, me: Me, _old_owner: Cid) {
        let all = cx.player_combat_cards();
        for &c in all.iter() {
            add_blade_replays(cx, c, -me.amount);
        }
    }
});

// ---- generated-card powers -----------------------------------------------------------------------------------------

// ArsenalPower: +Amount Strength whenever the owner generates a card.
listener!(ArsenalPower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, _card: CardIdx, added_by_player: bool) {
        if added_by_player {
            let a = amount(cx, &me);
            cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(a as i64), PLAYER, NO);
        }
    }
});

// PillarOfCreationPower: Amount unpowered block whenever the owner generates a card.
listener!(PillarOfCreationPower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, _card: CardIdx, added_by_player: bool) {
        if added_by_player {
            let a = amount(cx, &me);
            cx.gain_block(PLAYER, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// SpectrumShiftPower: Amount random colorless cards at the start of each turn's draw.
listener!(SpectrumShiftPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let a = amount(cx, &me);
        let pool = &crate::content::gen_pools::COLORLESS;
        let cards = cx.get_distinct_for_combat(pool, a.max(0) as usize, |_| true);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// ---- draw / energy / misc ------------------------------------------------------------------------------------------

// ForegoneConclusionPower: before the next draw, pick Amount cards from the draw pile into the hand (then removed).
listener!(ForegoneConclusionPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        cx.hook_shuffle = true;
        cx.shuffle_if_necessary();
        cx.hook_shuffle = false;
        if cx.stage == Stage::AwaitChoice {
            // An `AfterShuffle` listener (Stratagem) asked for a decision: the pick from the draw pile follows it (`resume_hook` phase 2).
            cx.hook_after = Some((me, 2));
            return;
        }
        let a = amount(cx, &me).clamp(0, 255) as u8;
        match cx.ask_pile(ids::card::FOREGONE_CONCLUSION, PileType::Draw, a, a, |_, _| true) {
            Ask::Resolved(cards) => {
                for &c in cards.iter() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                cx.remove_power(me.owner, me.idx);
            }
            Ask::Pending => {
                cx.hook_ctx = Some((me, 1));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, me: Me, phase: u8) {
        if phase == 2 {
            // The shuffle's `AfterShuffle` decision is done: now the pick itself.
            let a = amount(cx, &me).clamp(0, 255) as u8;
            match cx.ask_pile(ids::card::FOREGONE_CONCLUSION, PileType::Draw, a, a, |_, _| true) {
                Ask::Resolved(cards) => {
                    for &c in cards.iter() {
                        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                    cx.remove_power(me.owner, me.idx);
                }
                Ask::Pending => {
                    cx.hook_ctx = Some((me, 1));
                    cx.stage = Stage::AwaitChoice;
                }
            }
            return;
        }
        let picked = cx.choice.cards;
        for &c in picked.iter() {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        cx.remove_power(me.owner, me.idx);
    }
});

// TyrannyPower: +Amount cards drawn each turn, then exhaust Amount cards from the hand.
listener!(TyrannyPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        count + Dec::int(cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount) as i64)
    }
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let a = amount(cx, &me).clamp(0, 255) as u8;
        match cx.ask_hand(ids::card::TYRANNY, a, a, |_, _| true) {
            Ask::Resolved(cards) => {
                for &c in cards.iter() {
                    cx.exhaust_card(c, false);
                }
            }
            Ask::Pending => {
                cx.hook_ctx = Some((me, 1));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let picked = cx.choice.cards;
        for &c in picked.iter() {
            cx.exhaust_card(c, false);
        }
    }
});

// MonarchsGazePower: the owner's powered attacks make the target lose Strength until its turn ends.
listener!(MonarchsGazePower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, _unblocked: i32, props: ValueProp) {
        if dealer == me.owner && props.is_powered() {
            let a = amount(cx, &me);
            cx.apply_power(ids::power::MONARCHS_GAZE_STRENGTH_DOWN_POWER, target, Dec::int(a as i64), me.owner, NO);
        }
    }
});

// ReflectPower: blocked powered-attack damage is dealt back to the attacker; decrements at the owner's turn start.
listener!(ReflectPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _unblocked: i32, props: ValueProp, dealer: Cid) {
        let blocked = cx.dmg_result.blocked;
        if target == me.owner && blocked > 0 && props.is_powered() && dealer != NO {
            cx.damage(&[dealer], Dec::int(blocked as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// RoyaltiesPower: only adds a gold reward after combat (run-level, not modelled).
listener!(RoyaltiesPower {});

// OrbitPower (instanced): every 4 energy spent on cards gives Amount energy. `aux` = energy spent so far.
listener!(OrbitPower {
    fn after_energy_spent(&self, cx: &mut Combat, me: Me, _card: CardIdx, spent: i32) {
        if spent > 0 {
            let before = aux(cx, &me);
            let after = before + spent;
            set_aux(cx, &me, after);
            let triggers = after / 4 - before / 4;
            if triggers > 0 {
                let a = amount(cx, &me);
                cx.gain_energy(a * triggers);
            }
        }
    }
});

// PaleBlueDotPower: after the 5th card played in a turn, draw Amount extra cards next turn. `aux` = activated this turn.
listener!(PaleBlueDotPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if aux(cx, &me) == 0 && cx.hist.cards_finished_this_turn >= 5 {
            set_aux(cx, &me, 1);
            let a = amount(cx, &me);
            cx.apply_power(ids::power::DRAW_CARDS_NEXT_TURN_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            set_aux(cx, &me, 0);
        }
    }
});

// MonologuePower (instanced; the card sets its Strength var to 1): every card played while it exists gives +1 Strength
// after the play; everything is undone at the end of the turn. `aux` = StrengthApplied | (plays in flight << 16).
const MONOLOGUE_STRENGTH: i32 = 1;
listener!(MonologuePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let a = aux(cx, &me);
        set_aux(cx, &me, a + (1 << 16));
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let a = aux(cx, &me);
        if a >> 16 > 0 {
            set_aux(cx, &me, a - (1 << 16) + MONOLOGUE_STRENGTH);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(MONOLOGUE_STRENGTH as i64), me.owner, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let applied = aux(cx, &me) & 0xFFFF;
            cx.remove_power(me.owner, me.idx);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(-(applied as i64)), me.owner, NO);
        }
    }
});

