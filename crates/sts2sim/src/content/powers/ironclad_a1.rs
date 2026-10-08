use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(AggressionPower {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        let n = cx.power_amount(me.owner, me.id);
        let mut attacks: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.discard.iter() {
            if cx.card_def(c).ctype == CardType::Attack {
                attacks.push(c);
            }
        }
        cx.rng.combat_card_selection.shuffle(attacks.as_mut_slice());
        for &c in attacks.iter().take(n.max(0) as usize) {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            if cx.is_upgradable(c) {
                cx.upgrade_in_combat(c);
            }
        }
    }
});

listener!(NoDrawPower {
    fn should_draw(&self, _cx: &Combat, _me: Me, from_hand_draw: bool) -> bool {
        from_hand_draw
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(BarricadePower {
    fn should_clear_block(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        me.owner != creature
    }
});

listener!(ColossusPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() || q.dealer == NO {
            return Dec::ONE;
        }
        if !cx.has_power(q.dealer, ids::power::VULNERABLE_POWER) {
            return Dec::ONE;
        }
        Dec::frac(5, 1)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

listener!(CorruptionPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, _me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if cx.card_def(card).ctype != CardType::Skill {
            return None;
        }
        Some(Dec::ZERO)
    }
    fn modify_card_play_result_location(&self, cx: &Combat, me: Me, card: CardIdx, _is_auto: bool, _energy_value: i32, loc: CardLocation) -> CardLocation {
        let _ = me;
        if cx.card_def(card).ctype != CardType::Skill {
            return loc;
        }
        CardLocation::new(PileType::Exhaust, loc.pos)
    }
});

listener!(CrimsonMantlePower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let dmg = cx.power_mut(me.owner, me.idx).map_or(0, |p| p.aux);
        cx.damage(&[me.owner], Dec::int(dmg as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), me.owner, NO);
        let amt = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(amt as i64), ValueProp::UNPOWERED, NO);
    }
});

listener!(CrueltyPower {});

listener!(DarkEmbracePower {
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, _card: CardIdx, by_ethereal: bool) {
        if by_ethereal {
            if let Some(p) = cx.power_mut(me.owner, me.idx) {
                p.aux += 1;
            }
        } else {
            let n = cx.power_amount(me.owner, me.id);
            cx.draw_cards(n, false);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let n = cx.power_amount(me.owner, me.id);
            let k = cx.power_mut(me.owner, me.idx).map_or(0, |p| p.aux);
            cx.draw_cards(n * k, false);
            if let Some(p) = cx.power_mut(me.owner, me.idx) {
                p.aux = 0;
            }
        }
    }
});

listener!(DemonFormPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let n = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(n as i64), me.owner, NO);
        }
    }
});

listener!(FeelNoPainPower {
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, _card: CardIdx, _by_ethereal: bool) {
        let n = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
    }
});

listener!(FlameBarrierPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _unblocked: i32, props: ValueProp, dealer: Cid) {
        if target == me.owner && dealer != NO && props.is_powered() {
            let n = cx.power_amount(me.owner, me.id);
            cx.damage(&[dealer], Dec::int(n as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(HellraiserPower {
    fn after_card_drawn_early(&self, cx: &mut Combat, _me: Me, card: CardIdx, _from_hand_draw: bool) {
        if cx.card_def(card).tags & tag::STRIKE == 0 {
            return;
        }
        let _ = cx.auto_play(card, NO, AutoPlayType::Default, false);
    }
});
