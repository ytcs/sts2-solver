//! Rules of the colorless / curse / status / token cards and of the engine extensions they rely on
//! (nested auto-play, hook-raised decisions, transform).
use sts2sim::content::gen_pools;
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn base_with(deck: &[u16]) -> Combat {
    let deck: Vec<DeckCard> = deck.iter().map(|&id| DeckCard { id, upgrade: 0 }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(42),
    })
}

fn base() -> Combat {
    base_with(&[ids::card::ULTIMATE_STRIKE; 12])
}

/// Empties the hand (to discard) and puts the given cards in it, returning their arena indices.
fn set_hand(cx: &mut Combat, cards: &[(u16, u8)]) -> Vec<u8> {
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    cards
        .iter()
        .map(|&(id, up)| {
            let c = cx.new_card(id, up).unwrap();
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            c
        })
        .collect()
}

#[test]
fn every_colorless_curse_status_token_card_is_implemented() {
    let mut missing = vec![];
    for pool in [&gen_pools::COLORLESS[..], &gen_pools::CURSE[..], &gen_pools::STATUS[..], &gen_pools::TOKEN[..]] {
        for &id in pool {
            if !sts2sim::content::card_implemented(id) {
                missing.push(ids::card::NAMES[id as usize]);
            }
        }
    }
    assert!(missing.is_empty(), "unported cards: {missing:?}");
}

#[test]
fn void_costs_energy_when_drawn() {
    let mut cx = base();
    let e = cx.player.energy;
    let v = cx.new_card(ids::card::VOID, 0).unwrap();
    cx.add_generated_card(v, PileType::Draw, CardPilePosition::Top);
    cx.draw_cards(1, false);
    assert_eq!(cx.player.energy, e - 1);
    cx.player.energy = 0;
    let v2 = cx.new_card(ids::card::VOID, 0).unwrap();
    cx.add_generated_card(v2, PileType::Draw, CardPilePosition::Top);
    cx.draw_cards(1, false);
    assert_eq!(cx.player.energy, 0, "energy floors at 0");
}

#[test]
fn burn_and_wither_hurt_at_turn_end_wither_scales() {
    let mut a = base();
    set_hand(&mut a, &[(ids::card::BURN, 0)]);
    let mut b = base();
    set_hand(&mut b, &[]);
    a.step(Action::EndTurn);
    b.step(Action::EndTurn);
    assert_eq!(a.cr(PLAYER).hp, b.cr(PLAYER).hp - 2, "Burn deals 2");
    // Wither: 3 + 3 per FakeUpgrade
    let mut c = base();
    let w = set_hand(&mut c, &[(ids::card::WITHER, 0)])[0];
    sts2sim::content::cards::status::wither_fake_upgrade(&mut c, w);
    c.step(Action::EndTurn);
    let mut d = base();
    set_hand(&mut d, &[(ids::card::WITHER, 0)]);
    d.step(Action::EndTurn);
    assert_eq!(c.cr(PLAYER).hp, d.cr(PLAYER).hp - 3);
}

#[test]
fn regret_deals_damage_equal_to_hand_size() {
    let mut a = base();
    set_hand(&mut a, &[(ids::card::REGRET, 0), (ids::card::ULTIMATE_STRIKE, 0), (ids::card::ULTIMATE_STRIKE, 0)]);
    let mut b = base();
    set_hand(&mut b, &[(ids::card::ULTIMATE_STRIKE, 0), (ids::card::ULTIMATE_STRIKE, 0)]);
    a.step(Action::EndTurn);
    b.step(Action::EndTurn);
    assert_eq!(a.cr(PLAYER).hp, b.cr(PLAYER).hp - 3);
}

#[test]
fn normality_blocks_the_fourth_card() {
    let mut cx = base();
    cx.player.energy = 10;
    set_hand(&mut cx, &[(ids::card::NORMALITY, 0), (ids::card::ULTIMATE_DEFEND, 0), (ids::card::ULTIMATE_DEFEND, 0), (ids::card::ULTIMATE_DEFEND, 0), (ids::card::ULTIMATE_DEFEND, 0)]);
    for _ in 0..3 {
        assert!(cx.can_play(cx.player.hand[1]));
        assert!(cx.step(Action::PlayCard { hand_pos: 1, target: NO }));
    }
    assert!(!cx.can_play(cx.player.hand[1]), "three cards were played this turn");
}

#[test]
fn enthralled_blocks_other_cards_but_not_auto_plays() {
    let mut cx = base();
    cx.player.energy = 10;
    set_hand(&mut cx, &[(ids::card::ENTHRALLED, 0), (ids::card::ULTIMATE_DEFEND, 0)]);
    assert!(!cx.can_play(cx.player.hand[1]));
    assert!(cx.can_play(cx.player.hand[0]));
}

#[test]
fn frantic_escape_cost_grows_without_overflowing_the_modifier_list() {
    let mut cx = base();
    let c = set_hand(&mut cx, &[(ids::card::FRANTIC_ESCAPE, 0)])[0];
    for n in 1..=6 {
        cx.player.energy = 10;
        cx.cards[c as usize].pile = PileType::Hand as u8;
        assert_eq!(cx.card_cost(c, true), n);
        cx.add_energy_cost_this_combat(c, 1);
    }
}

#[test]
fn prolong_and_block_next_turn() {
    let mut cx = base();
    cx.gain_block(PLAYER, Dec::int(7), ValueProp::UNPOWERED, NO);
    set_hand(&mut cx, &[(ids::card::PROLONG, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::BLOCK_NEXT_TURN_POWER), 7);
}

#[test]
fn entropy_raises_a_hook_decision_and_replays_the_step() {
    let mut cx = base_with(&[ids::card::ULTIMATE_DEFEND; 10]);
    cx.apply_power(ids::power::ENTROPY_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    assert!(cx.ext.unwind_enabled == false);
    let e = cx.new_card(ids::card::ENTROPY, 0).unwrap(); // announces hook-decision content
    let _ = e;
    assert!(cx.ext.unwind_enabled);
    assert!(cx.step(Action::EndTurn));
    // The decision is exposed after the rollback: choose 1 of the 5 cards of the new hand.
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let d = cx.decision.expect("decision pending");
    assert_eq!((d.min, d.max, d.cands.len()), (1, 1, 5));
    let target = d.cands[2];
    let before = cx.cards[target as usize].id;
    assert!(cx.step(Action::Pick { idx: 2 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.hand.len(), 5);
    assert_ne!(cx.player.hand.iter().map(|&c| cx.cards[c as usize].id).filter(|&i| i == before).count(), 5, "one card was transformed");
    assert_eq!(cx.player.turn_number, 2);
}

#[test]
fn nested_auto_play_resumes_through_a_decision() {
    // Catastrophe auto-plays the only draw-pile card (a Headbutt), which asks for a discard card.
    let mut cx = base_with(&[ids::card::ULTIMATE_DEFEND; 12]);
    set_hand(&mut cx, &[(ids::card::CATASTROPHE, 0)]);
    let draw = cx.player.draw;
    for &c in draw.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    let hb = cx.new_card(ids::card::HEADBUTT, 0).unwrap();
    cx.add_generated_card(hb, PileType::Draw, CardPilePosition::Top);
    cx.player.energy = 3;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice, "the auto-played Headbutt asks for a discard card");
    assert!(cx.play_ctx.is_some() && !cx.play_stack.is_empty(), "Catastrophe is parked beneath Headbutt");
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.play_ctx.is_none() && cx.play_stack.is_empty());
    // Catastrophe finished its second iteration (empty draw pile: nothing to play) and sits in the discard pile.
    assert!(cx.player.discard.iter().any(|&c| cx.cards[c as usize].id == ids::card::CATASTROPHE));
}
