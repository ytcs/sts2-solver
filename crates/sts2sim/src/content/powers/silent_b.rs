use crate::dec::Dec;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn owner_on(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

fn is_owner_or_pet(cx: &Combat, me: Me, dealer: Cid) -> bool {
    dealer != NO && (dealer == me.owner || (cx.cr(dealer).is_pet && cx.cr(dealer).owner == me.owner))
}

fn add_keyword(cx: &mut Combat, c: CardIdx, k: u8) {
    let card = &mut cx.cards[c as usize];
    card.kw_add |= k;
    card.kw_remove &= !k;
}

listener!(InfiniteBladesPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let amt = cx.power_amount(me.owner, me.id);
        cx.create_shivs_in_hand(amt);
    }
});

listener!(PhantomBladesPower {
    fn after_card_entered_combat(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        if cx.card_def(card).tags & tag::SHIV != 0 {
            add_keyword(cx, card, kw::RETAIN);
        }
    }
    fn after_applied(&self, cx: &mut Combat, _me: Me) {
        for i in 0..cx.n_cards as usize {
            let p = cx.cards[i].pile;
            if (1..=5).contains(&p) && cx.card_def(i as CardIdx).tags & tag::SHIV != 0 {
                add_keyword(cx, i as CardIdx, kw::RETAIN);
            }
        }
    }
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.card == NO || cx.card_def(q.card).tags & tag::SHIV == 0 || q.dealer != me.owner {
            return Dec::ZERO;
        }
        if cx.hist.shivs_finished_this_turn > 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

listener!(ToolsOfTheTradePower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        count + Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let a = cx.power_amount(me.owner, me.id).clamp(0, 255) as u8;
        match cx.ask_hand(ids::card::TOOLS_OF_THE_TRADE, a, a, |_, _| true) {
            Ask::Resolved(cards) => {
                if !cards.is_empty() {
                    crate::content::cards::silent_b::discard_suspended(cx, cards.as_slice());
                }
            }
            Ask::Pending => {
                cx.hook_ctx = Some((me, 1));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        if !cards.is_empty() {
            crate::content::cards::silent_b::discard_suspended(cx, cards.as_slice());
        }
    }
});

listener!(FreeSkillPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, _me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if cx.card_def(card).ctype != CardType::Skill {
            return None;
        }
        match cx.card_pile_type(card) {
            PileType::Hand | PileType::Play => Some(Dec::ZERO),
            _ => None,
        }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Skill && matches!(cx.card_pile_type(play.card), PileType::Hand | PileType::Play) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

listener!(MasterPlannerPower {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Skill {
            add_keyword(cx, play.card, kw::SLY);
        }
    }
});

listener!(NightmarePower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let stored = cx.cr(me.owner).powers[i].aux;
        let amt = cx.cr(me.owner).powers[i].amount;
        if stored > 0 {
            for _ in 0..amt {
                if let Some(c) = cx.clone_card((stored - 1) as CardIdx) {
                    cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
            }
        }
        cx.remove_power(me.owner, me.idx);
    }
});

listener!(SerpentFormPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let amount = cx.power_amount(me.owner, me.id);
        cx.hist.play_amounts.push(PlayAmount { uid: me.idx, card: play.card, amount });
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(pos) = cx.hist.play_amounts.as_slice().iter().rposition(|e| e.uid == me.idx && e.card == play.card) else { return };
        let amount = cx.hist.play_amounts.remove(pos).amount;
        if amount > 0 {
            let h = cx.hittable_enemies();
            if !h.is_empty() {
                let k = cx.rng.combat_targets.next_int_range(0, h.len() as i32) as usize;
                cx.damage(&[h[k]], Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
            }
        }
    }
});

listener!(StranglePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        if cx.cr(me.owner).powers[i].applier == NO {
            return;
        }
        let amount = cx.cr(me.owner).powers[i].amount;
        cx.hist.play_amounts.push(PlayAmount { uid: me.idx, card: play.card, amount });
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(pos) = cx.hist.play_amounts.as_slice().iter().rposition(|e| e.uid == me.idx && e.card == play.card) else { return };
        let amount = cx.hist.play_amounts.remove(pos).amount;
        cx.damage(&[me.owner], Dec::int(amount as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(SpeedsterPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, from_hand_draw: bool) {
        if !from_hand_draw && cx.side == cx.cr(me.owner).side {
            let amt = cx.power_amount(me.owner, me.id);
            let h = cx.hittable_enemies();
            cx.damage(h.as_slice(), Dec::int(amt as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
});

listener!(NoxiousFumesPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.apply_power_to_hittable_enemies(ids::power::POISON_POWER, Dec::int(amt as i64), me.owner, NO);
        }
    }
});

listener!(PiercingWailPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.temp_before_applied(ids::power::STRENGTH_POWER, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        cx.temp_after_amount_changed(me, ids::power::STRENGTH_POWER, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        cx.temp_after_side_turn_end(me, ids::power::STRENGTH_POWER, -1, side);
    }
});

listener!(TrackingPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.card == NO || !is_owner_or_pet(cx, me, q.dealer) {
            return Dec::ONE;
        }
        if q.target == NO || !cx.has_power(q.target, ids::power::WEAK_POWER) {
            return Dec::ONE;
        }
        Dec::ONE + Dec::frac(cx.power_amount(me.owner, me.id) as i64, 2)
    }
});

listener!(DoubleDamagePower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !is_owner_or_pet(cx, me, q.dealer) || !q.props.is_powered() || q.card == NO {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

listener!(ShadowStepPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::DOUBLE_DAMAGE_POWER, me.owner, Dec::int(amt as i64), me.owner, NO);
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(ShadowmeldPower {
    fn modify_block_multiplicative(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if q.target != me.owner {
            return Dec::ONE;
        }
        Dec::int(1i64 << cx.power_amount(me.owner, me.id).clamp(0, 40))
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(WraithFormPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(-(amt as i64)), me.owner, NO);
        }
    }
});

listener!(WellLaidPlansPower {
    fn should_flush(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});

listener!(TheHuntPower {});

listener!(SneakyPower {});

fn amount_on_turn_start(cx: &Combat, me: Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].amount_on_turn_start)
}

listener!(DrawCardsNextTurnPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if amount_on_turn_start(cx, me) == 0 {
            return count;
        }
        count + Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) && amount_on_turn_start(cx, me) != 0 {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
