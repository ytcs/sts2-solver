//! Relics that modify damage / block / power amounts, plus the Silent / Necrobinder / Regent pool relics that are pure
//! modifiers.

use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `props.IsPoweredAttack()` with a card source whose owner / dealer is the relic owner.
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

// Attacks (not the owner's) from a creature that is about to die to Doom are halved.
listener!(UndyingSigil {
    fn modify_damage_multiplicative(&self, cx: &Combat, _me: Me, q: &DmgQ) -> Dec {
        if q.dealer == NO || !q.props.is_powered() || q.target != PLAYER || q.dealer == PLAYER {
            return Dec::ONE;
        }
        if cx.cr(q.dealer).hp > cx.power_amount(q.dealer, ids::power::DOOM_POWER) {
            return Dec::ONE;
        }
        g::undying_sigil::DAMAGE_DECREASE
    }
});

// Minion cards deal / gain double.
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

// Poison applied by the owner gets +`Poison`.
listener!(SneckoSkull {
    fn modify_power_amount_given_additive(&self, _cx: &Combat, _me: Me, power_id: u16, giver: Cid, _amount: Dec, _target: Cid, _card: CardIdx) -> Dec {
        if power_id != ids::power::POISON_POWER || giver != PLAYER {
            return Dec::ZERO;
        }
        Dec::int(g::snecko_skull::POISON_POWER as i64)
    }
});

// flag 0 = `BlockGainedThisCombat`, aux = `TriggeringCard` + 1 (0 = none); neither saved. The first card block of a combat is doubled.
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
