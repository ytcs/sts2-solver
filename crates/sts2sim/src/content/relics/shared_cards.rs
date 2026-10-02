//! Relics that generate / move / modify cards at combat start and during turns.

use crate::content::gen_pools;
use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;
use crate::{listener, relic_props};

fn self_power(cx: &mut Combat, id: u16, n: i32) {
    cx.apply_power(id, PLAYER, Dec::int(n as i64), PLAYER, NO);
}

/// `CardPileCmd.AddGeneratedCardsToCombat(cards, pile, owner[, position])` for freshly created cards, in order.
fn add_generated(cx: &mut Combat, cards: &[CardIdx], pile: PileType, pos: CardPilePosition) {
    for &c in cards {
        cx.add_generated_card(c, pile, pos);
    }
}

/// `n` fresh copies of card `id` (`CombatState.CreateCard<T>(owner)`).
fn create_cards(cx: &mut Combat, id: u16, n: i32) -> ArrayVec<CardIdx, 16> {
    let mut v = ArrayVec::new();
    for _ in 0..n {
        if let Some(c) = cx.new_card(id, 0) {
            v.push(c);
        }
    }
    v
}

// ---- random generation ------------------------------------------------------------------------------------------------------

// Ethereal cards of the character's pool: two random ones into the hand on turn 1.
listener!(BigHat {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side != Side::Player || cx.turn_number() > 1 {
            return;
        }
        let pool = cx.character_pool();
        let any = pool.iter().any(|&id| crate::content::card_def(id).keywords & kw::ETHEREAL != 0);
        if any {
            let cards = cx.get_distinct_for_combat(pool, g::big_hat::CARDS as usize, |d| d.keywords & kw::ETHEREAL != 0);
            add_generated(cx, cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// One free random Attack of the character's pool into the hand every turn.
listener!(Crossbow {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side != Side::Player {
            return;
        }
        let pool = cx.character_pool();
        if !pool.iter().any(|&id| crate::content::card_def(id).ctype == CardType::Attack) {
            return;
        }
        let cards = cx.get_distinct_for_combat(pool, 1, |d| d.ctype == CardType::Attack);
        for &c in cards.iter() {
            cx.set_to_free_this_turn(c);
        }
        add_generated(cx, cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
    }
});

listener!(OrangeDough {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            let cards = cx.get_distinct_for_combat(&gen_pools::COLORLESS, g::orange_dough::CARDS as usize, |_| true);
            add_generated(cx, cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// A free random card of the character's pool into the hand (after the turn-1 draw).
listener!(VexingPuzzlebox {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() != 1 {
            return;
        }
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, 1, |_| true);
        if let Some(&c) = cards.first().as_ref() {
            cx.set_to_free_this_turn(c);
            add_generated(cx, &[c], PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// ---- fixed generated cards -------------------------------------------------------------------------------------------------

listener!(FuneraryMask {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() == 1 {
            for _ in 0..g::funerary_mask::CARDS {
                let cards = create_cards(cx, ids::card::SOUL, 1);
                add_generated(cx, cards.as_slice(), PileType::Draw, CardPilePosition::Random);
            }
        }
    }
});

listener!(BlessedAntler {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::blessed_antler::ENERGY as i64)
    }
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() == 1 {
            let cards = create_cards(cx, ids::card::DAZED, g::blessed_antler::CARDS);
            add_generated(cx, cards.as_slice(), PileType::Draw, CardPilePosition::Random);
        }
    }
});

listener!(NinjaScroll {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 && !cx.is_over_or_ending() {
            let cards = create_cards(cx, ids::card::SHIV, g::ninja_scroll::SHIVS);
            add_generated(cx, cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

listener!(RadiantPearl {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() == 1 {
            let cards = create_cards(cx, ids::card::LUMINESCE, g::radiant_pearl::CARDS);
            add_generated(cx, cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// counter = `CombatsLeft` (saved, initialiser 1); ShowCounter is false.
listener!(TeaOfDiscourtesy {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).counter > 0 {
            for _ in 0..g::tea_of_discourtesy::DAZED_COUNT {
                let cards = create_cards(cx, ids::card::DAZED, 1);
                add_generated(cx, cards.as_slice(), PileType::Draw, CardPilePosition::Random);
            }
            cx.rel_mut(me).counter -= 1;
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsLeft", Slot::Counter)]
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (1, 0, 0)
    }
});

// A Soot into the draw pile (random position) after every shuffle.
listener!(BiiigHug {
    fn after_shuffle(&self, cx: &mut Combat, _me: Me) {
        let cards = create_cards(cx, ids::card::SOOT, 1);
        add_generated(cx, cards.as_slice(), PileType::Draw, CardPilePosition::Random);
    }
});

// Shivs played give a (temporary) Dexterity.
listener!(HelicalDart {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if cx.card_def(play.card).tags & tag::SHIV != 0 {
            self_power(cx, ids::power::HELICAL_DART_POWER, g::helical_dart::DEXTERITY_POWER);
        }
    }
});

// ---- moving / upgrading cards of the draw pile ----------------------------------------------------------------------------------

// Turn 1: one random Power card of the draw pile (non-Innate preferred) is made free and moved to the hand.
listener!(JeweledMask {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        let draw = cx.player.draw;
        let mut powers: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in draw.iter() {
            if cx.card_def(c).ctype == CardType::Power {
                powers.push(c);
            }
        }
        if powers.is_empty() {
            return;
        }
        let mut non_innate: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in powers.iter() {
            if cx.card_keywords(c) & kw::INNATE == 0 {
                non_innate.push(c);
            }
        }
        let list = if non_innate.is_empty() { powers } else { non_innate };
        if let Some(c) = cx.select_item(list.as_slice()) {
            cx.set_to_free_this_turn(c);
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// Turn 1: up to `Cards` zero-cost (non-X) cards of the draw pile are moved to the hand (selection via StableShuffle).
listener!(PowerCell {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side != Side::Player || cx.turn_number() > 1 {
            return;
        }
        let draw = cx.player.draw;
        let mut list: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in draw.iter() {
            if !cx.card_def(c).x_cost && cx.card_cost(c, false) == 0 {
                list.push(c);
            }
        }
        cx.stable_shuffle_selection(list.as_mut_slice());
        for &c in list.iter().take(g::power_cell::CARDS as usize) {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// Upgrades up to `Cards` upgradable cards of the draw pile when the combat is entered.
listener!(StoneCracker {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        let draw = cx.player.draw;
        let mut list: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in draw.iter() {
            if cx.is_upgradable(c) {
                list.push(c);
            }
        }
        cx.stable_shuffle_selection(list.as_mut_slice());
        for &c in list.iter().take(g::stone_cracker::CARDS as usize) {
            cx.upgrade_in_combat(c);
        }
    }
});

fn ghost_seed_can_affect(cx: &Combat, c: CardIdx) -> bool {
    let d = cx.card_def(c);
    d.rarity == CardRarity::Basic && d.tags & (tag::STRIKE | tag::DEFEND) != 0 && cx.cards[c as usize].kw_add & kw::ETHEREAL == 0
}
// Basic Strikes / Defends become Ethereal.
listener!(GhostSeed {
    fn after_card_entered_combat(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        if ghost_seed_can_affect(cx, card) {
            cx.apply_keyword(card, kw::ETHEREAL);
        }
    }
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        let all = cx.all_cards();
        for &c in all.iter() {
            if ghost_seed_can_affect(cx, c) {
                cx.apply_keyword(c, kw::ETHEREAL);
            }
        }
    }
});

// Adds a Potion-Shaped Rock after the other combat-start effects.
listener!(PetrifiedToad {
    fn before_combat_start_late(&self, cx: &mut Combat, _me: Me) {
        cx.try_procure_potion(ids::potion::POTION_SHAPED_ROCK);
    }
});

// Fills every open potion slot with a random potion (`PotionFactory.CreateRandomPotionOutOfCombat`, `CombatPotionGeneration`).
listener!(DelicateFrond {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        while cx.has_open_potion_slots() {
            let Some(p) = cx.create_random_potion(false) else { break };
            if !cx.try_procure_potion(p) {
                break;
            }
        }
    }
});
