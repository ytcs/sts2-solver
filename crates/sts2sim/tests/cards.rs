//! Behaviour of ported Ironclad cards (hand-constructed situations).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::state::DecisionSource;
use sts2sim::*;

fn base() -> Combat {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
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

/// Empties the hand (to discard) and puts the given cards in it, returning their arena indices.
fn set_hand(cx: &mut Combat, cards: &[(u16, u8)]) -> Vec<u8> {
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, sts2sim::types::CardPilePosition::Bottom);
    }
    cards
        .iter()
        .map(|&(id, up)| {
            let c = cx.new_card(id, up).unwrap();
            cx.move_card(c, PileType::Hand, sts2sim::types::CardPilePosition::Bottom);
            c
        })
        .collect()
}

fn ids_of(cx: &Combat, pile: PileType) -> Vec<u16> {
    cx.pile(pile).iter().map(|&c| cx.cards[c as usize].id).collect()
}

#[test]
fn twin_strike_hits_twice_with_strength() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::TWIN_STRIKE, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 2 * (5 + 2));
}

#[test]
fn bloodletting_unpowered_self_damage_and_energy() {
    let mut cx = base();
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(5), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::BLOODLETTING, 0)]);
    cx.cr_mut(PLAYER).block = 10; // unblockable: block untouched
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).hp, 77);
    assert_eq!(cx.cr(PLAYER).block, 10);
    assert_eq!(cx.player.energy, 3 + 2); // 0-cost, +2 energy
}

#[test]
fn shrug_it_off_draws_and_blocks() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::SHRUG_IT_OFF, 0)]);
    let draw0 = cx.player.draw.len();
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 8);
    assert_eq!(cx.player.hand.len(), 1);
    assert_eq!(cx.player.draw.len(), draw0 - 1);
}

#[test]
fn thunderclap_hits_all_and_applies_vulnerable() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::THUNDERCLAP, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(e).hp, hp - 4);
    assert_eq!(cx.power_amount(e, ids::power::VULNERABLE_POWER), 1);
}

#[test]
fn anger_clones_into_discard() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::ANGER, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    let d = ids_of(&cx, PileType::Discard);
    assert_eq!(d.iter().filter(|&&c| c == ids::card::ANGER).count(), 2); // original + clone
}

#[test]
fn armaments_decision_upgrades_chosen_card() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::ARMAMENTS, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    // Two upgradable candidates (Armaments itself is in the Play pile, not the hand) -> a real decision.
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let d = cx.decision.unwrap();
    assert_eq!((d.min, d.max, d.cands.len()), (1, 1, 2));
    // illegal answers are rejected and leave the decision pending
    assert!(!cx.step(Action::Confirm)); // nothing selected
    assert!(!cx.step(Action::Pick { idx: 5 })); // no such candidate
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.step(Action::Pick { idx: 1 })); // exact-1 choice completes on its own (no confirm needed)
    assert_eq!(cx.stage, Stage::AwaitAction);
    let ups: Vec<u8> = cx.player.hand.iter().map(|&c| cx.cards[c as usize].upgrade).collect();
    assert_eq!(ups, vec![0, 1]);
    assert_eq!(cx.cr(PLAYER).block, 5);
    assert_eq!(cx.player.discard.iter().filter(|&&c| cx.cards[c as usize].id == ids::card::ARMAMENTS).count(), 1);
}

#[test]
fn armaments_auto_resolves_with_single_candidate() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::ARMAMENTS, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitAction); // forced choice: no decision raised
    assert_eq!(cx.cards[cx.player.hand[0] as usize].upgrade, 1);
}

#[test]
fn true_grit_upgraded_asks_unupgraded_is_random() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::TRUE_GRIT, 1), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(ids_of(&cx, PileType::Exhaust), vec![ids::card::STRIKE_IRONCLAD]);
    assert_eq!(cx.cr(PLAYER).block, 9); // 7 + 2

    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::TRUE_GRIT, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    let before = cx.rng.combat_card_selection;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.exhaust.len(), 1);
    assert_eq!(cx.rng.combat_card_selection.counter, before.counter + 1); // exactly one draw
}

#[test]
fn sword_boomerang_consumes_one_target_draw_per_hit() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::SWORD_BOOMERANG, 0)]);
    let before = cx.rng.combat_targets.counter;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.rng.combat_targets.counter, before + 3);
}

#[test]
fn power_card_leaves_combat_and_inflame_gives_strength() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::INFLAME, 1)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::STRENGTH_POWER), 3);
    assert!(cx.player.discard.iter().all(|&c| cx.cards[c as usize].id != ids::card::INFLAME));
    assert!(cx.player.exhaust.iter().all(|&c| cx.cards[c as usize].id != ids::card::INFLAME));
}

#[test]
fn hand_full_redirects_generated_cards_to_discard() {
    let mut cx = base();
    let e = cx.enemies[0];
    let ten: Vec<(u16, u8)> = std::iter::once((ids::card::ANGER, 0)).chain((0..9).map(|_| (ids::card::STRIKE_IRONCLAD, 0))).collect();
    set_hand(&mut cx, &ten);
    assert_eq!(cx.player.hand.len(), 10);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.player.hand.len(), 9);
}

#[test]
fn headbutt_selects_from_discard_and_puts_on_top() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::HEADBUTT, 0)]);
    // set_hand discarded the opening hand (5 Strikes) -> real choice among 5 discard candidates
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let d = cx.decision.unwrap();
    assert_eq!(d.source, DecisionSource::Pile(PileType::Discard));
    assert_eq!(d.cands.len(), 5);
    let want = d.cands[3];
    assert!(cx.step(Action::Pick { idx: 3 }));
    assert_eq!(cx.player.draw.first(), Some(want));
    assert_eq!(cx.stage, Stage::AwaitAction);
}

#[test]
fn draw_pile_candidates_are_presented_sorted_not_in_pile_order() {
    let mut cx = base();
    // Build a draw pile in a known hidden order: Bash, Strike, Defend.
    let old = cx.player.draw;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, sts2sim::types::CardPilePosition::Bottom);
    }
    for id in [ids::card::BASH, ids::card::STRIKE_IRONCLAD, ids::card::DEFEND_IRONCLAD] {
        let c = cx.new_card(id, 0).unwrap();
        cx.move_card(c, PileType::Draw, sts2sim::types::CardPilePosition::Bottom);
    }
    match cx.ask_pile(0, PileType::Draw, 1, 1, |_, _| true) {
        sts2sim::engine::Ask::Pending => {}
        _ => panic!(),
    }
    let ids_: Vec<u16> = cx.decision.unwrap().cands.iter().map(|&c| cx.cards[c as usize].id).collect();
    // (Basic rarity for all three) -> ordinal id order: BASH < DEFEND_IRONCLAD < STRIKE_IRONCLAD
    assert_eq!(ids_, vec![ids::card::BASH, ids::card::DEFEND_IRONCLAD, ids::card::STRIKE_IRONCLAD]);
}

#[test]
fn burning_pact_multi_phase_exhaust_then_draw() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::BURNING_PACT, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.step(Action::Pick { idx: 1 })); // Defend
    assert_eq!(ids_of(&cx, PileType::Exhaust), vec![ids::card::DEFEND_IRONCLAD]);
    assert_eq!(cx.player.hand.len(), 1 + 2); // Strike + 2 drawn
}

#[test]
fn optional_multi_select_uses_pick_toggle_and_confirm() {
    // A (0..3) hand selection must be confirmed explicitly; picks toggle; exceeding max replaces the last pick.
    let mut cx = base();
    let hand = set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::BASH, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    let _ = hand;
    cx.stage = Stage::AwaitAction;
    match cx.ask_hand(0, 0, 2, |_, _| true) {
        sts2sim::engine::Ask::Pending => {}
        _ => panic!(),
    }
    cx.stage = Stage::AwaitChoice;
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert!(buf.iter().any(|a| *a == Action::Confirm)); // min 0: confirming nothing is legal
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert!(cx.step(Action::Pick { idx: 1 }));
    assert!(cx.step(Action::Pick { idx: 2 })); // at max: replaces the most recent pick (idx 1)
    assert_eq!(cx.decision.unwrap().selected.as_slice(), &[0, 2]);
    assert!(cx.step(Action::Pick { idx: 0 })); // toggle off
    assert_eq!(cx.decision.unwrap().selected.as_slice(), &[2]);
    assert!(cx.step(Action::Confirm));
    assert!(cx.decision.is_none());
    assert_eq!(cx.choice.cards.len(), 1);
}

#[test]
fn potions_use_and_discard_and_legal_actions() {
    let mut sc = Scenario {
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
        deck: (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect(),
        relics: vec![],
        potions: vec![ids::potion::FIRE_POTION, ids::potion::BLOCK_POTION, ids::potion::SWIFT_POTION],
        rng: RngSet::from_run_seed(9),
    };
    sc.potions.truncate(3);
    let mut cx = Combat::new(&sc);
    let e = cx.enemies[0];
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    // Fire potion targets the (single) enemy; Block/Swift are self-targeted (no target choice).
    assert!(buf.iter().any(|a| *a == Action::UsePotion { slot: 0, target: e }));
    assert!(buf.iter().any(|a| *a == Action::UsePotion { slot: 1, target: NO }));
    assert!(buf.iter().any(|a| *a == Action::DiscardPotion { slot: 2 }));
    assert!(buf.iter().any(|a| *a == Action::EndTurn));
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::UsePotion { slot: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 20); // Unpowered: no Strength etc.
    assert!(cx.player.potions[0].is_none());
    assert!(cx.step(Action::UsePotion { slot: 1, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 12);
    let h = cx.player.hand.len();
    assert!(cx.step(Action::UsePotion { slot: 2, target: NO }));
    assert_eq!(cx.player.hand.len(), h + 3);
    // wrong target for an enemy potion is rejected
    let mut cx2 = Combat::new(&sc);
    assert!(!cx2.step(Action::UsePotion { slot: 0, target: NO }));
}

#[test]
fn action_index_roundtrip_and_mask() {
    use sts2sim::engine::ACTION_SPACE;
    for i in 0..ACTION_SPACE {
        let a = Action::from_index(i).unwrap();
        assert_eq!(a.index(), i);
    }
    assert!(Action::from_index(ACTION_SPACE).is_none());
    let cx = base();
    let mut mask = vec![false; ACTION_SPACE];
    cx.action_mask(&mut mask);
    assert!(mask[Action::EndTurn.index()]);
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert_eq!(mask.iter().filter(|&&m| m).count(), buf.len());
}

#[test]
fn discovery_generates_three_distinct_chooses_one_free() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::DISCOVERY, 0)]);
    let before = cx.rng.combat_card_generation;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let d = cx.decision.unwrap();
    assert_eq!(d.source, DecisionSource::Options);
    assert_eq!(d.cands.len(), 3);
    assert!(d.can_skip && !d.confirm_required && d.min == 0 && d.max == 1);
    let idsv: Vec<u16> = d.cands.iter().map(|&c| cx.cards[c as usize].id).collect();
    assert!(idsv[0] != idsv[1] && idsv[1] != idsv[2] && idsv[0] != idsv[2]);
    // n-1 draws for the full shuffle of the filtered pool (independent of count=3)
    let pool = sts2sim::content::gen_pools::IRONCLAD
        .iter()
        .filter(|&&id| {
            let c = sts2sim::content::card_def(id);
            c.can_be_generated_in_combat && !c.multiplayer_only && !matches!(c.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event)
        })
        .count();
    assert_eq!(cx.rng.combat_card_generation.counter, before.counter + pool as i32 - 1);
    // Reproduce the exact options with an independent Fisher-Yates over the filtered pool.
    let mut list: Vec<u16> = sts2sim::content::gen_pools::IRONCLAD
        .iter()
        .copied()
        .filter(|&id| {
            let c = sts2sim::content::card_def(id);
            c.can_be_generated_in_combat && !c.multiplayer_only && !matches!(c.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event)
        })
        .collect();
    let mut r = before;
    r.shuffle(&mut list);
    assert_eq!(idsv, list[..3].to_vec());
    // pick the second: it lands in hand, free this turn, and the Discovery card is exhausted (un-upgraded)
    assert!(cx.step(Action::Pick { idx: 1 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    let got = *cx.player.hand.as_slice().last().unwrap();
    assert_eq!(cx.cards[got as usize].id, idsv[1]);
    assert_eq!(cx.card_cost(got, true), 0);
    assert_eq!(ids_of(&cx, PileType::Exhaust), vec![ids::card::DISCOVERY]);
}

#[test]
fn discovery_can_be_skipped() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::DISCOVERY, 1)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert!(buf.iter().any(|a| *a == Action::Confirm)); // skip
    assert!(cx.step(Action::Confirm));
    assert_eq!(cx.player.hand.len(), 0);
    assert_eq!(ids_of(&cx, PileType::Discard).iter().filter(|&&c| c == ids::card::DISCOVERY).count(), 1); // upgraded: no Exhaust
}
