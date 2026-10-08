use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

#[inline]
fn on_side(cx: &Combat, me: &Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

listener!(NoBlockPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn modify_block_multiplicative(&self, _cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if q.target != me.owner || q.props.has(ValueProp::UNPOWERED) || q.card == NO {
            return Dec::ONE;
        }
        Dec::ZERO
    }
});

listener!(FastenPower {
    fn modify_block_additive(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if me.owner != q.target || !q.props.is_powered() {
            return Dec::ZERO;
        }
        if q.card != NO && cx.card_def(q.card).tags & tag::DEFEND == 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

listener!(PrepTimePower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            let a = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::VIGOR_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
        }
    }
});

listener!(AutomationPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, _from_hand_draw: bool) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let (amount, n) = {
            let p = &mut cx.cr_mut(me.owner).powers[i];
            p.aux += 1;
            (p.amount, p.aux)
        };
        if n >= 10 {
            cx.gain_energy(amount);
            cx.cr_mut(me.owner).powers[i].aux = 0;
        }
    }
});

listener!(PanachePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let aux = cx.cr(me.owner).powers[i].aux;
        let mut n = aux & 0xFF;
        if aux & 0x100 != 0 {
            n += 1;
            if n >= 5 {
                let amount = cx.cr(me.owner).powers[i].amount;
                let enemies = cx.hittable_enemies();
                cx.damage(enemies.as_slice(), Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
                n = 0;
            }
        }
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = 0x100 | n;
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux &= !0xFF;
            }
        }
    }
});

const ROLLING_BOULDER_INCREMENT: i32 = 5;

listener!(RollingBoulderPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let Some(amount) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map(|p| p.amount) else { return };
        let enemies = cx.hittable_enemies();
        cx.damage(enemies.as_slice(), Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
        cx.set_power_amount(me.owner, me.idx, amount + ROLLING_BOULDER_INCREMENT);
    }
});

listener!(TheBombPower {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !on_side(cx, &me, side) {
            return;
        }
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.amount > 1 {
            cx.decrement_power(me.owner, me.idx);
            return;
        }
        let enemies = cx.hittable_enemies();
        cx.damage(enemies.as_slice(), Dec::int(p.aux as i64), ValueProp::UNPOWERED, me.owner, NO);
        cx.remove_power(me.owner, me.idx);
    }
});

listener!(TheGambitPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, _dealer: Cid) {
        if target == me.owner && props.is_powered() && unblocked > 0 {
            cx.remove_power(me.owner, me.idx);
            cx.kill(&[me.owner]);
        }
    }
});

listener!(DarkShacklesPower {
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
listener!(CoordinatePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.temp_before_applied(ids::power::STRENGTH_POWER, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        cx.temp_after_amount_changed(me, ids::power::STRENGTH_POWER, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        cx.temp_after_side_turn_end(me, ids::power::STRENGTH_POWER, 1, side);
    }
});

listener!(BeaconOfHopePower {});

listener!(KnockdownPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let applier = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(NO, |p| p.applier);
        if q.dealer == applier {
            return Dec::ONE;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(TagTeamPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, target: Cid, count: i32) -> i32 {
        if cx.card_def(card).ctype != CardType::Attack {
            return count;
        }
        let applier = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(NO, |p| p.applier);
        if applier == PLAYER {
            return count;
        }
        let tt = cx.card_target_type(card);
        if tt == TargetType::AnyEnemy && target != me.owner {
            return count;
        }
        if !matches!(tt, TargetType::AnyEnemy | TargetType::AllEnemies) {
            return count;
        }
        count + cx.power_amount(me.owner, me.id)
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.remove_power(me.owner, me.idx);
    }
});

listener!(CalamityPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype != CardType::Attack {
            return;
        }
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            let a = cx.cr(me.owner).powers[i].amount;
            cx.play_amount_add(me.idx, play.card, a);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(amount) = cx.play_amount_take(me.idx, play.card) else { return };
        let amount = amount.max(0) as usize;
        let pool = cx.character_pool();
        let cards = cx.get_for_combat_where(pool, amount, |d| d.ctype == CardType::Attack);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

listener!(NostalgiaPower {
    fn modify_card_play_result_location(&self, cx: &Combat, me: Me, card: CardIdx, _is_auto: bool, _energy_value: i32, loc: CardLocation) -> CardLocation {
        if me.owner != PLAYER || !matches!(cx.card_def(card).ctype, CardType::Attack | CardType::Skill) || loc.pile != PileType::Discard {
            return loc;
        }
        let started = cx.plays_this_turn(|e| matches!(cx.card_def(e.card).ctype, CardType::Attack | CardType::Skill)) as i32;
        if started >= cx.power_amount(me.owner, me.id) {
            return loc;
        }
        CardLocation::new(PileType::Draw, CardPilePosition::Top)
    }
});

listener!(MayhemPower {
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if me.owner == PLAYER {
            let a = cx.power_amount(me.owner, me.id);
            cx.auto_play_from_draw_pile(a, CardPilePosition::Top, false);
        }
    }
});

fn entropy_transform(cx: &mut Combat, cards: &[CardIdx]) {
    for &c in cards {
        cx.transform_cards(&[c], &[None]);
    }
}
listener!(EntropyPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let n = cx.power_amount(me.owner, me.id).clamp(0, 16) as u8;
        match cx.ask_hand(ids::card::ENTROPY, n, n, |_, _| true) {
            crate::engine::Ask::Resolved(cards) => entropy_transform(cx, cards.as_slice()),
            crate::engine::Ask::Pending => {
                cx.hook_ctx = Some((me, 1));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        entropy_transform(cx, cards.as_slice());
    }
});

listener!(StratagemPower {
    fn after_shuffle(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let n = cx.power_amount(me.owner, me.id).clamp(0, 16) as u8;
        match cx.ask_pile(ids::card::STRATAGEM, PileType::Draw, n, n, |_, _| true) {
            crate::engine::Ask::Resolved(cards) => {
                for &c in cards.iter() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
            }
            crate::engine::Ask::Pending => {
                if cx.draw_decision_resumable() {
                    cx.hook_ctx = Some((me, 1));
                    cx.stage = Stage::AwaitChoice;
                } else {
                    match cx.replay_prompt() {
                        crate::engine::ReplayAnswer::Cards(cards) => {
                            for &c in cards.iter() {
                                cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                            }
                        }
                        crate::engine::ReplayAnswer::Captured => {}
                        crate::engine::ReplayAnswer::Unavailable => {
                            cx.decision = None;
                            cx.flag_missing(Kind::Power, me.id);
                        }
                    }
                }
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        for &c in cards.iter() {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});
