//! Ironclad powers (batch a1): the powers applied by the cards at positions [0,45) of the Ironclad pool.

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Counter: at the start of the player's turn, move `Amount` random Attacks from the discard pile to the hand (upgraded).
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
        // UnstableShuffle(Rng.CombatCardSelection).Take(Amount)
        cx.rng.combat_card_selection.shuffle(attacks.as_mut_slice());
        for &c in attacks.iter().take(n.max(0) as usize) {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            if cx.is_upgradable(c) {
                cx.upgrade_in_combat(c);
            }
        }
    }
});

// Debuff: no draws except the start-of-turn hand draw; removed at the end of the owner's turn.
listener!(NoDrawPower {
    fn should_draw(&self, _cx: &Combat, _me: Me, from_hand_draw: bool) -> bool {
        // (single player: `player != Owner.Player` is never true)
        from_hand_draw
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Block is not cleared at the start of the owner's turn.
listener!(BarricadePower {
    fn should_clear_block(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        me.owner != creature
    }
});

// Damage dealt to the owner by a Vulnerable attacker is halved; counts down at the end of the enemy turn.
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

// Skills cost 0 and exhaust (hooks: free-cost pass and result-location modifier).
listener!(CorruptionPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, _me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if cx.card_def(card).ctype != CardType::Skill {
            return None;
        }
        Some(Dec::ZERO)
    }
    fn modify_card_play_result_location(&self, cx: &Combat, _me: Me, card: CardIdx, _is_auto: bool, pile: PileType) -> PileType {
        if cx.card_def(card).ctype != CardType::Skill {
            return pile;
        }
        PileType::Exhaust
    }
});

// `aux` = SelfDamage (starts at 0, +1 each time the card is played). Start of turn: lose that much HP, gain `Amount` block.
listener!(CrimsonMantlePower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        let dmg = cx.power_mut(me.owner, me.idx).map_or(0, |p| p.aux);
        cx.damage(&[me.owner], Dec::int(dmg as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), me.owner, NO);
        let amt = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(amt as i64), ValueProp::UNPOWERED, NO);
    }
});

// Vulnerable multiplier bonus lives in `VulnerablePower` (it asks the dealer's Cruelty power).
listener!(CrueltyPower {});

// `aux` = Ethereal cards exhausted at end of turn (drawn after the flush).
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

// Start of the owner's turn: gain `Amount` Strength.
listener!(DemonFormPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let n = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(n as i64), me.owner, NO);
        }
    }
});

// Whenever a card is exhausted, gain `Amount` block.
listener!(FeelNoPainPower {
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, _card: CardIdx, _by_ethereal: bool) {
        let n = cx.power_amount(me.owner, me.id);
        cx.gain_block(me.owner, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
    }
});

// Powered attacks that damage the owner deal `Amount` damage back; removed at the end of the next enemy turn.
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
