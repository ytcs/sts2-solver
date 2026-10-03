//! Powers applied by the `EventCardPool` cards (`cards/event_pool.rs`).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- FeedingFrenzyPower (card Feeding Frenzy): `TemporaryStrengthPower`, +Strength until the end of this turn -------------------------
listener!(FeedingFrenzyPower {
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

// ---- HelloWorldPower: before each hand draw, add AmountOnTurnStart random Common cards from the pool to the hand ----------------------
listener!(HelloWorldPower {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let n = cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].amount_on_turn_start);
        if n < 1 {
            return;
        }
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, n as usize, |d| d.rarity == CardRarity::Common);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// ---- ToricToughnessPower (Instanced; Amount = turns left, `aux` = Block per trigger in 1/10000): after the owner's Block is
// cleared at the start of its turn, gain that Block (Unpowered) and count down ---------------------------------------------------------------
listener!(ToricToughnessPower {
    fn after_block_cleared(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature != me.owner {
            return;
        }
        let block = cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].aux);
        cx.gain_block(me.owner, Dec::frac(block as i64, 4), ValueProp::UNPOWERED, NO);
        cx.decrement_power(me.owner, me.idx);
    }
});

// ---- CuriousPower (Mad Science rider): Power cards cost Amount less (never below 0; free cards stay free) --------------------------
listener!(CuriousPower {
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        if cx.card_def(card).ctype != CardType::Power || cost <= Dec::ZERO {
            return None;
        }
        Some((cost - Dec::int(cx.power_amount(me.owner, me.id) as i64)).max(Dec::ZERO))
    }
});

// ---- ImprovementPower (Mad Science rider): at combat end upgrade Amount random upgradable cards of the run deck ---------------------
// (`Rng.CombatCardSelection.NextItem` over the deck in order, without replacement; levels live in `Combat::deck_upgrade`.)
listener!(ImprovementPower {
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let amount = cx.power_amount(me.owner, me.id);
        let mut list: crate::util::ArrayVec<u8, 80> = crate::util::ArrayVec::new();
        for i in 0..cx.deck_len {
            if cx.deck_upgrade[i as usize] < crate::content::card_def(cx.cards[i as usize].id).max_upgrade {
                list.push(i);
            }
        }
        for _ in 0..amount {
            if list.is_empty() {
                break;
            }
            let k = cx.rng.combat_card_selection.next_int_range(0, list.len() as i32) as usize;
            let i = list.remove(k);
            cx.deck_upgrade[i as usize] += 1;
        }
    }
});
