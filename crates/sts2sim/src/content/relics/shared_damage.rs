use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn powered_card_attack(cx: &Combat, q: &DmgQ) -> bool {
    let _ = cx;
    q.props.is_powered() && q.card != NO
}

listener!(StrikeDummy {
    fn modify_damage_additive(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if !powered_card_attack(cx, q) || cx.card_def(q.card).tags & tag::STRIKE == 0 {
            return Dec::ZERO;
        }
        Dec::int(g::strike_dummy::EXTRA_DAMAGE as i64)
    }
});

listener!(FakeStrikeDummy {
    fn modify_damage_additive(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if !powered_card_attack(cx, q) || cx.card_def(q.card).tags & tag::STRIKE == 0 {
            return Dec::ZERO;
        }
        Dec::int(g::fake_strike_dummy::EXTRA_DAMAGE as i64)
    }
});

listener!(MiniatureCannon {
    fn modify_damage_additive(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if !powered_card_attack(cx, q) || cx.cards[q.card as usize].upgrade == 0 {
            return Dec::ZERO;
        }
        Dec::int(g::miniature_cannon::EXTRA_DAMAGE as i64)
    }
});

listener!(MysticLighter {
    fn modify_damage_additive(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if !powered_card_attack(cx, q) || cx.cards[q.card as usize].enchant == 0 {
            return Dec::ZERO;
        }
        Dec::int(g::mystic_lighter::DAMAGE as i64)
    }
});

listener!(UndyingSigil {
    fn modify_damage_multiplicative(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if q.dealer == NO || !q.props.is_powered() || q.target != PLAYER || q.dealer == PLAYER {
            return Dec::ONE;
        }
        if cx.cr(q.dealer).hp() > cx.power_amount(q.dealer, ids::power::DOOM_POWER) {
            return Dec::ONE;
        }
        g::undying_sigil::DAMAGE_DECREASE
    }
});

listener!(VitruvianMinion {
    fn modify_damage_multiplicative(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if q.card == NO || cx.card_def(q.card).tags & tag::MINION == 0 {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn modify_block_multiplicative(&self, cx: &Combat, _me: Me, q: &BlockQ) -> Dec {
        if q.card == NO || cx.card_def(q.card).tags & tag::MINION == 0 {
            return Dec::ONE;
        }
        Dec::int(2)
    }
});

listener!(SneckoSkull {
    fn modify_power_amount_given_additive(&self, _cx: &Combat, _me: Me, power_id: u16, giver: Cid, _amount: Dec, _target: Cid, _card: CardIdx) -> Dec {
        if power_id != ids::power::POISON_POWER || giver != PLAYER {
            return Dec::ZERO;
        }
        Dec::int(g::snecko_skull::POISON_POWER as i64)
    }
});

listener!(Vambrace {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.aux = 0;
        r.set_flag(0, false);
    }
    fn modify_block_multiplicative(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if !q.props.has(ValueProp::MOVE) || q.card == NO {
            return Dec::ONE;
        }
        let r = cx.rel(me);
        if r.aux != 0 && r.aux != q.card as i32 + 1 {
            return Dec::ONE;
        }
        if r.flag(0) {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn after_modifying_block_amount(&self, cx: &mut Combat, me: Me, amount: Dec, card: CardIdx) {
        if amount <= Dec::ZERO || card == NO {
            return;
        }
        cx.rel_mut(me).aux = card as i32 + 1;
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let r = cx.rel_mut(me);
        if r.aux != play.card as i32 + 1 || r.flag(0) {
            return;
        }
        r.set_flag(0, true);
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.aux = 0;
        r.set_flag(0, false);
    }
});

fn temporary_kind(id: u16) -> Option<u8> {
    use ids::power as p;
    Some(match id {
        p::DYING_STAR_POWER | p::ENFEEBLING_TOUCH_POWER | p::FEEDING_FRENZY_POWER | p::FLEX_POTION_POWER | p::DARK_SHACKLES_POWER
        | p::MONARCHS_GAZE_STRENGTH_DOWN_POWER | p::PIERCING_WAIL_POWER | p::REPTILE_TRINKET_POWER | p::SHACKLING_POTION_POWER
        | p::MANGLE_POWER | p::COORDINATE_POWER | p::CRUSH_UNDER_POWER | p::SETUP_STRIKE_POWER => 0,
        p::ANTICIPATE_POWER | p::HELICAL_DART_POWER | p::SPEED_POTION_POWER | p::FADE_POWER => 1,
        p::FOCUSED_STRIKE_POWER | p::HYPERBEAM_FOCUS_DOWN_POWER | p::SYNCHRONIZE_POWER | p::HOTFIX_POWER => 2,
        _ => return None,
    })
}
fn internal_kind(id: u16) -> Option<u8> {
    match id {
        ids::power::STRENGTH_POWER => Some(0),
        ids::power::DEXTERITY_POWER => Some(1),
        ids::power::FOCUS_POWER => Some(2),
        _ => None,
    }
}

listener!(UnsettlingLamp {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.aux = 0;
        r.counter = 0;
        r.flags = 0;
    }
    fn before_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, amount: Dec, target: Cid, applier: Cid) {
        let r = cx.rel(me);
        let card = cx.cur_power_card;
        if r.aux != 0 || r.flag(0) || card == NO || applier != PLAYER || cx.cr(target).side == Side::Player {
            return;
        }
        if !crate::content::power_def(power_id).visible || Combat::power_type_for_amount(power_id, amount.trunc()) != PowerType::Debuff {
            return;
        }
        if cx.has_power(target, ids::power::ARTIFACT_POWER) {
            return;
        }
        let r = cx.rel_mut(me);
        r.aux = card as i32 + 1;
        if let Some(k) = temporary_kind(power_id) {
            r.counter |= 1 << k;
        }
    }
    fn modify_power_amount_given_multiplicative(&self, cx: &Combat, me: Me, power_id: u16, _giver: Cid, amount: Dec, _target: Cid, card: CardIdx) -> Dec {
        let r = cx.rel(me);
        if r.aux == 0 || card == NO || card as i32 + 1 != r.aux || r.flag(0) {
            return Dec::ONE;
        }
        if let Some(k) = internal_kind(power_id) {
            if r.counter & (1 << k) != 0 {
                return Dec::ONE;
            }
        }
        if Combat::power_type_for_amount(power_id, amount.trunc()) != PowerType::Debuff {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let r = cx.rel_mut(me);
        if r.aux == play.card as i32 + 1 && !r.flag(0) {
            r.set_flag(0, true);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.aux = 0;
        r.counter = 0;
        r.flags = 0;
    }
});
