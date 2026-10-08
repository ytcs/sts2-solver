use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::{listener, relic_props};

const UNPOWERED: ValueProp = ValueProp::UNPOWERED;

fn self_power(cx: &mut Combat, id: u16, n: i32) {
    cx.apply_power(id, PLAYER, Dec::int(n as i64), PLAYER, NO);
}
fn relic_block(cx: &mut Combat, n: i32) {
    cx.gain_block(PLAYER, Dec::int(n as i64), UNPOWERED, NO);
}
fn ctype(cx: &Combat, c: CardIdx) -> CardType {
    cx.card_def(c).ctype
}

listener!(Kunai {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Attack {
            cx.rel_mut(me).counter += 1;
            if cx.rel(me).counter % g::kunai::CARDS == 0 {
                self_power(cx, ids::power::DEXTERITY_POWER, g::kunai::DEXTERITY_POWER);
            }
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter % g::kunai::CARDS)
    }
});

listener!(Shuriken {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Attack {
            cx.rel_mut(me).counter += 1;
            if cx.rel(me).counter % g::shuriken::CARDS == 0 {
                self_power(cx, ids::power::STRENGTH_POWER, g::shuriken::STRENGTH_POWER);
            }
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter % g::shuriken::CARDS)
    }
});

listener!(OrnamentalFan {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Attack {
            cx.rel_mut(me).counter += 1;
            if cx.rel(me).counter % g::ornamental_fan::CARDS == 0 {
                relic_block(cx, g::ornamental_fan::BLOCK);
            }
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter % g::ornamental_fan::CARDS)
    }
});

listener!(Kusarigama {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if !cx.in_progress || ctype(cx, play.card) != CardType::Attack {
            return;
        }
        cx.rel_mut(me).counter += 1;
        if cx.rel(me).counter % g::kusarigama::CARDS == 0 {
            cx.damage_random_hittable_enemy(g::kusarigama::DAMAGE, UNPOWERED);
        }
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter % g::kusarigama::CARDS)
    }
});

listener!(LetterOpener {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() != 1 {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Skill {
            cx.rel_mut(me).counter += 1;
            if cx.rel(me).counter % g::letter_opener::CARDS == 0 {
                cx.damage_hittable_enemies(g::letter_opener::DAMAGE, UNPOWERED);
            }
        }
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter % g::letter_opener::CARDS)
    }
});

listener!(Nunchaku {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if ctype(cx, play.card) == CardType::Attack {
            cx.rel_mut(me).counter += 1;
            if cx.in_progress && cx.rel(me).counter % g::nunchaku::CARDS == 0 {
                cx.gain_energy(g::nunchaku::ENERGY);
            }
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("AttacksPlayed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::nunchaku::CARDS)
    }
});

listener!(IronClub {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        cx.rel_mut(me).counter += 1;
        if cx.in_progress && cx.rel(me).counter % g::iron_club::CARDS == 0 {
            cx.draw_cards(1, false);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CardsPlayed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::iron_club::CARDS)
    }
});

listener!(TuningFork {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if ctype(cx, play.card) == CardType::Skill {
            cx.rel_mut(me).counter += 1;
            if cx.rel(me).counter >= g::tuning_fork::CARDS {
                relic_block(cx, g::tuning_fork::BLOCK);
                cx.rel_mut(me).counter -= g::tuning_fork::CARDS;
            }
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("SkillsPlayed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

fn joss_paper_draw(cx: &mut Combat, me: Me) {
    let n = g::joss_paper::EXHAUST_AMOUNT;
    if cx.rel(me).counter < n {
        return;
    }
    let draws = cx.rel(me).counter / n;
    cx.draw_cards(draws, false);
    cx.rel_mut(me).counter %= n;
}
listener!(JossPaper {
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, _card: CardIdx, by_ethereal: bool) {
        if by_ethereal {
            cx.rel_mut(me).aux += 1;
            return;
        }
        cx.rel_mut(me).counter += 1;
        joss_paper_draw(cx, me);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let e = cx.rel(me).aux;
            let r = cx.rel_mut(me);
            r.counter += e;
            r.aux = 0;
            joss_paper_draw(cx, me);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).aux = 0;
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CardsExhausted", Slot::Counter).skip_default()]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(HappyFlower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let t = g::happy_flower::TURNS;
            let r = cx.rel_mut(me);
            r.counter = (r.counter + 1) % t;
            if r.counter == 0 {
                cx.gain_energy(g::happy_flower::ENERGY);
            }
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TurnsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(FakeHappyFlower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let t = g::fake_happy_flower::TURNS;
            let r = cx.rel_mut(me);
            r.counter = (r.counter + 1) % t;
            if r.counter == 0 {
                cx.gain_energy(g::fake_happy_flower::ENERGY);
            }
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TurnsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(Pendulum {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let t = g::pendulum::TURNS;
        let r = cx.rel_mut(me);
        r.counter = (r.counter + 1) % t;
    }
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if cx.rel(me).counter != 0 {
            return count;
        }
        count + Dec::int(g::pendulum::CARDS as i64)
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TurnsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(PollinousCore {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter += 1;
    }
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if cx.rel(me).counter < g::pollinous_core::TURNS {
            return count;
        }
        count + Dec::int(g::pollinous_core::CARDS as i64)
    }
    fn after_modifying_hand_draw(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TurnsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(Pocketwatch {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.in_progress {
            cx.rel_mut(me).counter += 1;
        }
    }
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if cx.turn_number() == 1 || cx.rel(me).aux > g::pocketwatch::CARD_THRESHOLD {
            return count;
        }
        count + Dec::int(g::pocketwatch::CARDS as i64)
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let r = cx.rel_mut(me);
            r.aux = r.counter;
            r.counter = 0;
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.counter = 0;
        r.aux = 0;
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter)
    }
});

listener!(VelvetChoker {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::velvet_choker::ENERGY as i64)
    }
    fn should_play(&self, cx: &Combat, me: Me, _card: CardIdx) -> bool {
        cx.rel(me).counter < g::velvet_choker::CARDS
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        cx.rel_mut(me).counter += 1;
    }
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        cx.in_progress.then(|| r.counter)
    }
});

listener!(ArtOfWar {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Attack && !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(1, true);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let r = cx.rel_mut(me);
            let this = r.flag(1);
            r.set_flag(0, this);
            r.set_flag(1, false);
        }
    }
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() > 1 {
            if !cx.rel(me).flag(0) {
                cx.gain_energy(g::art_of_war::ENERGY);
            }
            let r = cx.rel_mut(me);
            r.set_flag(0, false);
            r.set_flag(1, false);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.set_flag(0, false);
        r.set_flag(1, false);
    }
});

listener!(Permafrost {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Power && !cx.rel(me).flag(0) {
            relic_block(cx, g::permafrost::BLOCK);
            cx.rel_mut(me).set_flag(0, true);
        }
    }
});

listener!(GamePiece {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Power {
            cx.draw_cards(g::game_piece::CARDS, false);
        }
    }
});

listener!(LostWisp {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if cx.in_progress && ctype(cx, play.card) == CardType::Power {
            cx.damage_hittable_enemies(g::lost_wisp::DAMAGE, UNPOWERED);
        }
    }
});

listener!(DaughterOfTheWind {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if ctype(cx, play.card) == CardType::Attack {
            relic_block(cx, g::daughter_of_the_wind::BLOCK);
        }
    }
});

listener!(IntimidatingHelmet {
    fn before_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if play.energy_value >= g::intimidating_helmet::ENERGY {
            relic_block(cx, g::intimidating_helmet::BLOCK);
        }
    }
});

listener!(IvoryTile {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if play.energy_value >= g::ivory_tile::ENERGY_THRESHOLD {
            cx.gain_energy(g::ivory_tile::ENERGY);
        }
    }
});

listener!(RazorTooth {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if matches!(ctype(cx, play.card), CardType::Attack | CardType::Skill) && cx.is_upgradable(play.card) {
            cx.upgrade_in_combat(play.card);
        }
    }
});

listener!(RainbowRing {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).flags = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if !cx.in_progress || cx.rel(me).flag(3) {
            return;
        }
        let bit = match ctype(cx, play.card) {
            CardType::Attack => 0,
            CardType::Skill => 1,
            CardType::Power => 2,
            _ => 7,
        };
        if bit < 3 {
            cx.rel_mut(me).set_flag(bit, true);
        }
        let r = cx.rel(me);
        if r.flag(0) && r.flag(1) && r.flag(2) {
            self_power(cx, ids::power::STRENGTH_POWER, g::rainbow_ring::STRENGTH_POWER);
            self_power(cx, ids::power::DEXTERITY_POWER, g::rainbow_ring::DEXTERITY_POWER);
            cx.rel_mut(me).set_flag(3, true);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).flags = 0;
    }
});

listener!(PenNib {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if !q.props.is_powered() || q.card == NO || !cx.is_owner_or_osty(q.dealer) {
            return Dec::ONE;
        }
        let r = cx.rel(me);
        if r.aux == 0 {
            if cx.card_pile_type(q.card) != PileType::Play && r.counter == 9 {
                return Dec::int(2);
            }
            return Dec::ONE;
        }
        if q.card as i32 + 1 == r.aux {
            return Dec::int(2);
        }
        Dec::ONE
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if ctype(cx, play.card) != CardType::Attack {
            return;
        }
        let r = cx.rel_mut(me);
        r.counter = (r.counter + 1) % 10;
        if r.counter == 0 {
            r.aux = play.card as i32 + 1;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let r = cx.rel_mut(me);
        if r.aux != 0 && r.aux == play.card as i32 + 1 {
            r.aux = 0;
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("AttacksPlayed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(ChemicalX {
    fn modify_x_value(&self, _cx: &Combat, _me: Me, _card: CardIdx, value: i32) -> i32 {
        value + g::chemical_x::INCREASE
    }
});

listener!(ThrowingAxe {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn modify_card_play_count(&self, cx: &Combat, me: Me, _card: CardIdx, _target: Cid, count: i32) -> i32 {
        if cx.rel(me).flag(0) {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.rel_mut(me).set_flag(0, true);
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(MusicBox {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let r = cx.rel(me);
        if r.aux != 0 || r.flag(0) || ctype(cx, play.card) != CardType::Attack {
            return;
        }
        cx.rel_mut(me).aux = play.card as i32 + 1;
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.rel(me).aux != play.card as i32 + 1 {
            return;
        }
        if let Some(c) = cx.clone_card(play.card) {
            cx.apply_keyword(c, kw::ETHEREAL);
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        let r = cx.rel_mut(me);
        r.set_flag(0, true);
        r.aux = 0;
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let r = cx.rel_mut(me);
            r.set_flag(0, false);
            r.aux = 0;
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.set_flag(0, false);
        r.aux = 0;
    }
});

listener!(BurningSticks {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, card: CardIdx, _by_ethereal: bool) {
        if !cx.rel(me).flag(0) && ctype(cx, card) == CardType::Skill {
            if let Some(c) = cx.clone_card(card) {
                cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(MummifiedHand {
    fn after_card_played(&self, cx: &mut Combat, _me: Me, play: &CardPlay) {
        if !cx.in_progress || ctype(cx, play.card) != CardType::Power {
            return;
        }
        let hand = cx.player.hand;
        let mut list: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            let base = cx.cards[c as usize].cost_base;
            let star = cx.card_def(c).star_cost;
            if base > 0 || star > 0 {
                list.push(c);
            }
        }
        let pick = |cx: &mut Combat, src: &[CardIdx], only_costing: bool| -> Option<CardIdx> {
            let mut f: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
            for &c in src {
                if !only_costing || cx.costs_energy_or_stars(c) {
                    f.push(c);
                }
            }
            cx.select_item(f.as_slice())
        };
        let mut card = pick(cx, list.as_slice(), true);
        if card.is_none() {
            card = pick(cx, hand.as_slice(), true);
        }
        if card.is_none() {
            card = pick(cx, list.as_slice(), false);
        }
        if card.is_none() {
            card = pick(cx, hand.as_slice(), false);
        }
        if let Some(c) = card {
            cx.set_to_free_this_turn(c);
        }
    }
});

listener!(Bookmark {
    fn after_flush(&self, cx: &mut Combat, _me: Me) {
        let hand = cx.player.hand;
        let mut list: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            if !cx.card_def(c).x_cost && cx.card_cost(c, false) > 0 {
                list.push(c);
            }
        }
        if let Some(c) = cx.select_item(list.as_slice()) {
            cx.add_cost_until_played(c, -1, false);
        }
    }
});

listener!(CentennialPuzzle {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if cx.in_progress && target == PLAYER && unblocked > 0 && !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, true);
            for _ in 0..g::centennial_puzzle::CARDS {
                cx.draw_cards(1, false);
            }
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(GremlinHorn {
    fn after_death(&self, cx: &mut Combat, _me: Me, creature: Cid, _was_removal_prevented: bool) {
        if cx.cr(creature).side != Side::Player {
            cx.gain_energy(g::gremlin_horn::ENERGY);
            cx.draw_cards(g::gremlin_horn::CARDS, false);
        }
    }
});

listener!(UnceasingTop {
    fn after_hand_emptied(&self, cx: &mut Combat, _me: Me) {
        if matches!(cx.player.phase, Phase::AutoPrePlay | Phase::Play | Phase::AutoPostPlay) {
            cx.draw_cards(1, false);
        }
    }
});

listener!(TheAbacus {
    fn after_shuffle(&self, cx: &mut Combat, _me: Me) {
        relic_block(cx, g::the_abacus::BLOCK);
    }
});

listener!(Tingsha {
    fn after_card_discarded(&self, cx: &mut Combat, _me: Me, _card: CardIdx) {
        if cx.side == Side::Player {
            cx.damage_random_hittable_enemy(g::tingsha::DAMAGE, UNPOWERED);
        }
    }
});

listener!(ToughBandages {
    fn after_card_discarded(&self, cx: &mut Combat, _me: Me, _card: CardIdx) {
        if cx.side == Side::Player {
            relic_block(cx, g::tough_bandages::BLOCK);
        }
    }
});

listener!(Regalite {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, _card: CardIdx, added_by_player: bool) {
        if added_by_player && !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, true);
            relic_block(cx, g::regalite::BLOCK);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).set_flag(0, false);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(CloakClasp {
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player {
            let n = cx.player.hand.len() as i32;
            if n != 0 {
                relic_block(cx, n * g::cloak_clasp::BLOCK);
            }
        }
    }
});

listener!(ScreamingFlagon {
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.hand.is_empty() {
            cx.damage_hittable_enemies(g::screaming_flagon::DAMAGE, UNPOWERED);
        }
    }
});

listener!(RippleBasin {
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.hist.attacks_played_this_turn == 0 {
            relic_block(cx, g::ripple_basin::BLOCK);
        }
    }
});

listener!(StoneCalendar {
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() == g::stone_calendar::DAMAGE_TURN {
            cx.damage_hittable_enemies(g::stone_calendar::DAMAGE, UNPOWERED);
        }
    }
    fn meta_display(&self, cx: &Combat, _r: &Relic) -> Option<i32> {
        if !cx.in_progress || cx.turn_number() >= g::stone_calendar::DAMAGE_TURN {
            return None;
        }
        Some(cx.turn_number())
    }
});

fn belt_buckle_apply(cx: &mut Combat, me: Me) {
    if !cx.rel(me).flag(0) {
        cx.rel_mut(me).set_flag(0, true);
        cx.apply_power(ids::power::DEXTERITY_POWER, PLAYER, Dec::int(g::belt_buckle::DEXTERITY_POWER as i64), NO, NO);
    }
}
fn belt_buckle_remove(cx: &mut Combat, me: Me) {
    if cx.rel(me).flag(0) {
        cx.rel_mut(me).set_flag(0, false);
        cx.apply_power(ids::power::DEXTERITY_POWER, PLAYER, Dec::int(-(g::belt_buckle::DEXTERITY_POWER as i64)), NO, NO);
    }
}
listener!(BeltBuckle {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
        if !cx.has_potions() {
            belt_buckle_apply(cx, me);
        }
    }
    fn after_potion_used(&self, cx: &mut Combat, me: Me, _potion: u16, _target: Cid) {
        if cx.in_progress && !cx.has_potions() {
            belt_buckle_apply(cx, me);
        }
    }
    fn after_potion_procured(&self, cx: &mut Combat, me: Me, _potion: u16) {
        if cx.in_progress && cx.has_potions() {
            belt_buckle_remove(cx, me);
        }
    }
    fn after_potion_discarded(&self, cx: &mut Combat, me: Me, _potion: u16) {
        if cx.in_progress && !cx.has_potions() {
            belt_buckle_apply(cx, me);
        }
    }
    fn after_combat_victory(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(ReptileTrinket {
    fn after_potion_used(&self, cx: &mut Combat, _me: Me, _potion: u16, _target: Cid) {
        if cx.in_progress {
            self_power(cx, ids::power::REPTILE_TRINKET_POWER, g::reptile_trinket::STRENGTH_POWER);
        }
    }
});

listener!(HandDrill {
    fn after_block_broken(&self, cx: &mut Combat, _me: Me, target: Cid, breaker: Cid) {
        if cx.is_owner_or_osty(breaker) && !cx.cr(target).is_player {
            cx.apply_power(ids::power::VULNERABLE_POWER, target, Dec::int(g::hand_drill::VULNERABLE_POWER as i64), PLAYER, NO);
        }
    }
});

listener!(LizardTail {
    fn should_die_late(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != PLAYER || cx.rel(me).flag(0)
    }
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        cx.rel_mut(me).set_flag(0, true);
        let amount = (Dec::int(cx.cr(creature).max_hp as i64) * Dec::frac(g::lizard_tail::HEAL as i64, 2)).max(Dec::ONE);
        cx.heal(creature, amount);
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("WasUsed", 0)]
    }
});
