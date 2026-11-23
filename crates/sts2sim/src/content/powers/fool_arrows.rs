// Powers applied by The Fool's arrow cards.
use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.my_amount(me)
}

fn percent_bonus(cx: &Combat, me: &Me) -> Dec {
    Dec::ONE + Dec::frac(amount(cx, me) as i64, 2)
}

fn powered_arrow(props: ValueProp) -> bool {
    props.is_powered() && props.has(ValueProp::FROM_ARROW)
}

listener!(ExplosiveArrowsPower {
    fn after_arrow_impaled(&self, cx: &mut Combat, me: Me, _target: Cid, _power_uid: u16, _arrow: u8, _source: CardIdx) {
        // `power.Arrows.First().owner.Creature == Owner`: the arrows are the player's.
        if me.owner != PLAYER {
            return;
        }
        let targets = cx.hittable_enemies();
        let n = amount(cx, &me);
        cx.damage(targets.as_slice(), Dec::int(n as i64), ValueProp::UNPOWERED, me.owner, NO);
    }
});

listener!(SerratedArrowsPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.dealer == NO {
            return Dec::ONE;
        }
        let d = cx.cr(q.dealer);
        if q.dealer != me.owner && !(d.is_pet && d.owner == me.owner) {
            return Dec::ONE;
        }
        if q.target == NO || !cx.is_impaled(q.target) {
            return Dec::ONE;
        }
        percent_bonus(cx, &me)
    }
});

// Multiplayer-only effect: block to the other living players.
listener!(CoveringFirePower {});

fn claim_of(card: CardIdx) -> i32 {
    if card == NO { 0x8000 } else { card as i32 + 1 }
}

// aux = claimed attack (card + 1, 0x8000 sourceless), as AimPower's `commandToModify`.
listener!(QuietusPower {
    fn after_arrow_shot(&self, cx: &mut Combat, me: Me, arrow: u8, source: CardIdx) {
        if arrow == NO || source == NO || me.owner != PLAYER {
            return;
        }
        cx.set_arrow_destroy_after_impaled(arrow, true);
    }
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !powered_arrow(attack.props) || cx.my_aux(me) != 0 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, claim_of(attack.card));
    }
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !powered_arrow(q.props) {
            return Dec::ONE;
        }
        let x = cx.my_aux(me);
        if x != 0 && q.card != NO && claim_of(q.card) != x {
            return Dec::ONE;
        }
        Dec::int(amount(cx, &me) as i64)
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        let x = cx.my_aux(me);
        if x != 0 && claim_of(attack.card) == x {
            cx.remove_power(me.owner, me.idx);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(ArcheryPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let n = amount(cx, &me);
            cx.apply_power(ids::power::AIM_POWER, me.owner, Dec::int(n as i64), me.owner, NO);
        }
    }
});

// aux = `triggeredThisTurn`.
listener!(YahPower {
    fn after_arrow_dislodged(&self, cx: &mut Combat, me: Me, _target: Cid, _power_uid: u16, _arrow: u8, _source: CardIdx) {
        if me.owner != PLAYER || cx.my_aux(me) != 0 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, 1);
        let n = amount(cx, &me);
        cx.gain_energy(n);
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.set_power_aux(me.owner, me.idx, 0);
        }
    }
});

listener!(DislikeYouPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !powered_arrow(q.props) {
            return Dec::ONE;
        }
        percent_bonus(cx, &me)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(HoningPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !powered_arrow(q.props) {
            return Dec::ONE;
        }
        percent_bonus(cx, &me)
    }
});
