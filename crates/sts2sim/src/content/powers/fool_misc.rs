use crate::dec::Dec;
use crate::engine::RunResult;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.power_aux(me.owner, me.idx)
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount)
}

fn add_to_hand(cx: &mut Combat, id: u16, upgrade: u8) {
    if let Some(c) = cx.new_card(id, 0) {
        if upgrade != 0 {
            cx.upgrade_in_combat(c);
        }
        cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
    }
}

listener!(AllIsWellPower {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx, added_by_player: bool) {
        if !added_by_player || me.owner != PLAYER || cx.cards[card as usize].id != ids::card::SADNESS {
            return;
        }
        let a = amount(cx, &me);
        cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
    }
});

listener!(CompostPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, _was_removal_prevented: bool) {
        if cx.cr(creature).side == cx.cr(me.owner).side {
            return;
        }
        if !cx.cr(PLAYER).is_dead() {
            let a = amount(cx, &me);
            cx.gain_energy(a);
        }
    }
});

listener!(EcstasyPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let mut list: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in cx.player.hand.iter() {
            if cx.cards[c as usize].id == ids::card::SADNESS {
                list.push(c);
            }
        }
        if list.is_empty() {
            return;
        }
        cx.stable_shuffle_selection(list.as_mut_slice());
        let n = (amount(cx, &me).max(0) as usize).min(list.len());
        let repl = [Some((ids::card::JOY, 0u8)); MAX_HAND];
        cx.transform_cards(&list.as_slice()[..n], &repl[..n]);
    }
});

// aux = `DynamicVars.Cards` (Sadness per hand draw: one per Ennui play).
listener!(EnnuiPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        for _ in 0..aux(cx, &me) {
            add_to_hand(cx, ids::card::SADNESS, 0);
        }
    }
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if me.owner != PLAYER {
            return count;
        }
        count + Dec::int(amount(cx, &me) as i64)
    }
});

listener!(HiddenRootsPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        let a = amount(cx, &me);
        cx.apply_power(ids::power::STRETCH_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
    }
});

// aux = `DynamicVars["Upgraded"]` (upgraded Mythic Form plays).
listener!(MythicFormPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let upgraded = aux(cx, &me);
        let mut i = 0;
        while i < amount(cx, &me) {
            let id = cx.random_tarot();
            add_to_hand(cx, id, (i < upgraded) as u8);
            i += 1;
        }
    }
});

// aux = the Show Off card + 1 (`ShowOffInstance`, 0 = none).
listener!(ShowOffPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, _was_removal_prevented: bool) {
        if cx.power_idx(me.owner, me.idx).is_none() || cx.cr(creature).side == cx.cr(me.owner).side || !cx.all_powers_trigger_fatal(creature) {
            return;
        }
        let card = aux(cx, &me);
        if card > 0 {
            let a = amount(cx, &me);
            let k = &mut cx.cards[(card - 1) as usize];
            k.counter[0] = k.counter[0].saturating_add(a.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        }
        cx.remove_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// aux = autoplays already done in this post-play phase: an autoplay that stops for a choice re-enters the hook
// (`run_post_play_hooks` resumes at this listener) once the play finished.
listener!(SimpletonPower {
    fn after_auto_post_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let mut i = aux(cx, &me);
        while i < amount(cx, &me) {
            i += 1;
            let mut best = i32::MAX;
            for &c in cx.player.hand.iter() {
                if cx.card_keywords(c) & kw::UNPLAYABLE != 0 {
                    continue;
                }
                let k = if cx.card_def(c).x_cost { 999_999_999 } else { cx.card_cost_resolved(c) };
                best = best.min(k);
            }
            if best == i32::MAX {
                continue;
            }
            let mut group: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
            for &c in cx.player.hand.iter() {
                if cx.card_keywords(c) & kw::UNPLAYABLE != 0 {
                    continue;
                }
                let k = if cx.card_def(c).x_cost { 999_999_999 } else { cx.card_cost_resolved(c) };
                if k == best {
                    group.push(c);
                }
            }
            let Some(c) = cx.select_item(group.as_slice()) else { continue };
            if cx.auto_play(c, NO, AutoPlayType::Default, false) == RunResult::Suspended {
                cx.set_power_aux(me.owner, me.idx, i);
                return;
            }
        }
        cx.set_power_aux(me.owner, me.idx, 0);
    }
});
