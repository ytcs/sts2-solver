//! Engine-core mechanisms (death sequence, summons, stun, auto-play, replays, enchantments, extra turns, history).
//! Spec references in the test names' comments.
use sts2sim::dec::Dec;
use sts2sim::engine::{HKind, RunResult};
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(relics: Vec<RelicInit>, potions: Vec<u16>, hp: i32, seed: u64) -> Scenario {
    let mut deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    deck[9] = DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 };
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: hp,
        hp,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics,
        potions,
        rng: RngSet::from_run_seed(seed),
    }
}

fn base() -> Combat {
    Combat::new(&scenario(vec![], vec![], 80, 42))
}

/// Replaces the hand with the given (id, upgrade) cards (old hand to discard).
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

// ---- death sequence (spec 02 §5.3) ----------------------------------------------------------------------------

#[test]
fn fairy_in_a_bottle_beats_lizard_tail_and_heals_30_percent() {
    let mut cx = Combat::new(&scenario(vec![RelicInit { id: ids::relic::LIZARD_TAIL, counter: 0 }], vec![ids::potion::FAIRY_IN_A_BOTTLE], 70, 1));
    cx.kill(&[PLAYER]);
    // ShouldDie (pass 1, Fairy) is consulted before ShouldDieLate (pass 2, Lizard Tail): Fairy fires, Lizard is untouched
    assert_eq!(cx.cr(PLAYER).hp, 21); // max(70 * 0.3, 1) = 21
    assert!(cx.player.potions[0].is_none());
    assert_eq!(cx.player.relics[0].counter, 0);
    assert!(!cx.pending_loss);
    // second death: Lizard Tail (50% of 70 = 35)
    cx.kill(&[PLAYER]);
    assert_eq!(cx.cr(PLAYER).hp, 35);
    assert_eq!(cx.player.relics[0].counter, 1);
    // third death is real
    cx.kill(&[PLAYER]);
    assert_eq!(cx.cr(PLAYER).hp, 0);
    assert!(cx.pending_loss);
    assert!(!cx.player_hooks_active);
}

#[test]
fn forced_kill_bypasses_preventers() {
    let mut cx = Combat::new(&scenario(vec![], vec![ids::potion::FAIRY_IN_A_BOTTLE], 70, 1));
    cx.kill_ex(&[PLAYER], true);
    assert_eq!(cx.cr(PLAYER).hp, 0);
    assert!(cx.player.potions[0].is_some());
}

#[test]
fn dead_player_no_longer_listens() {
    // after DeactivateHooks the player's relics / cards are no longer listeners (snapshot is empty for their hooks)
    let mut cx = Combat::new(&scenario(vec![RelicInit { id: ids::relic::LIZARD_TAIL, counter: 0 }], vec![], 70, 1));
    cx.kill(&[PLAYER]);
    cx.kill(&[PLAYER]);
    assert!(!cx.player_hooks_active);
    let snap = cx.snapshot(sts2sim::hooks::Mask::bit(sts2sim::hooks::hookbit::should_die_late));
    assert!(snap.is_empty());
    // healing revives hooks
    cx.heal(PLAYER, Dec::int(5));
    assert!(cx.player_hooks_active);
}

#[test]
fn minions_die_with_the_last_primary_enemy() {
    let mut cx = base();
    let primary = cx.enemies[0];
    let minion = cx.summon_enemy(ids::monster::NIBBIT, NO, [1, 0]).unwrap();
    cx.apply_power(ids::power::MINION_POWER, minion, Dec::ONE, minion, NO);
    assert!(!cx.is_primary_enemy(minion));
    assert!(cx.is_primary_enemy(primary));
    cx.kill(&[primary]);
    assert!(cx.cr(minion).is_dead(), "secondary enemies are killed with the last primary");
    assert!(cx.enemies.is_empty());
}

#[test]
fn killing_a_minion_does_not_end_combat() {
    let mut cx = base();
    let minion = cx.summon_enemy(ids::monster::NIBBIT, NO, [1, 0]).unwrap();
    cx.apply_power(ids::power::MINION_POWER, minion, Dec::ONE, minion, NO);
    cx.kill(&[minion]);
    assert!(!cx.is_ending());
    assert!(cx.cr(cx.enemies[0]).is_alive());
}

// ---- summon (spec 01 §13.5) ---------------------------------------------------------------------------------------

#[test]
fn summon_during_player_turn_rolls_and_acts_next_enemy_turn() {
    let mut cx = base();
    let before = cx.rng.niche.counter;
    let s = cx.summon_enemy(ids::monster::NIBBIT, NO, [1, 0]).unwrap();
    assert_eq!(cx.rng.niche.counter, before + 1, "one niche draw for the HP");
    assert!(cx.cr(s).monster.spawned_this_turn);
    assert_ne!(cx.cr(s).monster.next_move, NO, "rolled immediately (player turn)");
    let hp = cx.cr(PLAYER).hp;
    assert!(cx.step(Action::EndTurn));
    // SpawnedThisTurn was cleared at the player->enemy switch, so it acted
    assert!(cx.cr(PLAYER).hp < hp || cx.cr(s).monster.performed_first);
    assert!(cx.cr(s).monster.performed_first);
}

#[test]
fn summon_during_enemy_turn_is_not_rolled_and_does_not_act() {
    let mut cx = base();
    cx.side = Side::Enemy;
    let s = cx.summon_enemy(ids::monster::NIBBIT, NO, [1, 0]).unwrap();
    assert_eq!(cx.cr(s).monster.next_move, NO);
    assert!(cx.cr(s).monster.spawned_this_turn);
}

// ---- stun (spec 04 §1.8) -------------------------------------------------------------------------------------------

#[test]
fn stun_replaces_the_pending_move_and_follows_up_with_the_interrupted_move() {
    let mut cx = base();
    let e = cx.enemies[0];
    let pending = cx.cr(e).monster.next_move;
    let logged = cx.last_logged_move(e);
    assert_eq!(pending, logged);
    cx.stun(e, None, None);
    assert!(cx.is_stunned(e));
    // a second stun while an un-performed STUNNED is pending is ignored
    cx.stun(e, None, Some(3));
    assert_eq!(cx.cr(e).monster.stun_follow_up, logged);
    let hp = cx.cr(PLAYER).hp;
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(PLAYER).hp, hp, "the stunned enemy did nothing");
    // next player turn: STUNNED may now be left; the follow-up (the interrupted move) is re-logged and pending again
    assert!(!cx.is_stunned(e));
    assert_eq!(cx.cr(e).monster.next_move, logged);
}

// ---- play history (spec 01 §15) -------------------------------------------------------------------------------------

#[test]
fn history_counts_plays_this_turn_and_resets_per_turn() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::DEFEND_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.plays_this_turn(|_| true), 2);
    assert_eq!(cx.hist_total(HKind::CardPlayStarted), 2);
    assert_eq!(cx.hist_total(HKind::EnergySpent), 2);
    assert!(cx.step(Action::EndTurn));
    let _ = e;
    assert_eq!(cx.plays_this_turn(|_| true), 0, "a new turn is a distinct 'this turn'");
    // ... but the entries of the previous player turn count as "last turn" (side is not checked)
    assert!(cx.hist_any_last_player_turn(HKind::CardPlayStarted, |_| true));
}

// ---- auto-play / Havoc / Sly (spec 03 §5.2-5.4) -------------------------------------------------------------------

#[test]
fn havoc_plays_and_exhausts_the_top_draw_card() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::HAVOC, 0)]);
    let top = cx.player.draw.first().unwrap();
    let top_id = cx.cards[top as usize].id;
    let hp = cx.cr(e).hp;
    let energy = cx.player.energy;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    // the top card was auto-played (Strike: 6 damage to the random hittable enemy; Defend: block) without paying energy
    assert_eq!(cx.player.energy, energy - 1, "only Havoc's own cost was paid");
    assert_eq!(cx.card_pile_type(top), PileType::Exhaust, "forceExhaust");
    if top_id == ids::card::STRIKE_IRONCLAD {
        assert_eq!(cx.cr(e).hp, hp - 6);
    } else {
        assert_eq!(cx.cr(PLAYER).block, 5);
    }
}

#[test]
fn sly_cards_auto_play_when_discarded_by_a_card_effect_not_by_the_flush() {
    let mut cx = base();
    let e = cx.enemies[0];
    let cards = set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    cx.cards[cards[0] as usize].flags |= cflag::SINGLE_TURN_SLY;
    let hp = cx.cr(e).hp;
    assert_eq!(cx.discard_cards(&[cards[0], cards[1]], 0), RunResult::Finished);
    assert_eq!(cx.cr(e).hp, hp - 6, "the Sly Strike played itself (random target, free)");
    assert_eq!(cx.player.energy, 3);
    assert_eq!(cx.card_pile_type(cards[1]), PileType::Discard);
}

#[test]
fn unplayable_auto_play_just_moves_the_card() {
    let mut cx = base();
    let c = cx.new_card(ids::card::ASCENDERS_BANE, 0).unwrap();
    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    assert_eq!(cx.auto_play(c, NO, AutoPlayType::Default, false), RunResult::Finished);
    // Ethereal curse without Exhaust keyword?  Ascender's Bane is Ethereal + Unplayable: goes to the discard pile
    assert_eq!(cx.card_pile_type(c), PileType::Discard);
}

// ---- replays / result location / X (spec 03 §5.1, §2.5) ----------------------------------------------------------

#[test]
fn duplicator_plays_the_next_card_twice_then_expires() {
    let mut cx = Combat::new(&scenario(vec![], vec![ids::potion::DUPLICATOR], 80, 3));
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 12);
    assert!(!cx.has_power(PLAYER, ids::power::DUPLICATION_POWER), "decremented to 0 and removed");
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 18);
}

#[test]
fn chemical_x_adds_two_to_whirlwind() {
    let mut cx = Combat::new(&scenario(vec![RelicInit { id: ids::relic::CHEMICAL_X, counter: 0 }], vec![], 80, 3));
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::WHIRLWIND, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.player.energy, 0, "X took all 3 energy");
    assert_eq!(cx.cr(e).hp, hp - 5 * (3 + 2));
}

#[test]
fn rebound_returns_the_next_played_card_to_the_draw_top() {
    let mut cx = base();
    let e = cx.enemies[0];
    let hand = set_hand(&mut cx, &[(ids::card::REBOUND, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert!(cx.has_power(PLAYER, ids::power::REBOUND_POWER));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.player.draw.first(), Some(hand[1]));
    assert!(!cx.has_power(PLAYER, ids::power::REBOUND_POWER));
    // Rebound itself went to the discard pile (the power did not exist when its result location was computed)
    assert_eq!(cx.card_pile_type(hand[0]), PileType::Discard);
}

#[test]
fn no_draw_blocks_card_effect_draws_but_not_the_hand_draw() {
    let mut cx = base();
    cx.apply_power(ids::power::NO_DRAW_POWER, PLAYER, Dec::ONE, PLAYER, NO);
    let h = cx.player.hand.len();
    assert_eq!(cx.draw_cards(2, false), 0);
    assert_eq!(cx.player.hand.len(), h);
    assert_eq!(cx.draw_cards(2, true), 2);
}

#[test]
fn transform_keeps_the_pile_index() {
    let mut cx = base();
    let hand = set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    let out = cx.transform_cards(&[hand[1]], &[Some((ids::card::MINION_STRIKE, 0))]);
    assert_eq!(cx.player.hand.get(1), Some(out[0]));
    assert_eq!(cx.cards[out[0] as usize].id, ids::card::MINION_STRIKE);
    assert!(cx.cards[hand[1] as usize].flags & cflag::REMOVED != 0);
}

// ---- enchantments (spec 03 §12) -----------------------------------------------------------------------------------

fn enchanted(id: u16, ench: u16, amount: i16) -> Combat {
    let mut sc = scenario(vec![], vec![], 80, 5);
    sc.deck[0] = DeckCard { id, upgrade: 0 };
    let ex = ScenarioExtras { deck: vec![DeckExtra { enchant: ench as u8 + 1, enchant_amount: amount, props: [0; 2] }] };
    Combat::new_with(&sc, &ex)
}

#[test]
fn sharp_adds_damage_before_the_power_passes() {
    let mut cx = enchanted(ids::card::STRIKE_IRONCLAD, ids::enchantment::SHARP, 4);
    let e = cx.enemies[0];
    let c = cx.cards.iter().position(|k| k.enchant != 0).unwrap() as u8;
    // put the enchanted card in hand
    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(1), PLAYER, NO);
    let hp = cx.cr(e).hp;
    let pos = cx.player.hand.iter().position(|&x| x == c).unwrap() as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
    // (6 + 4) * 1.5 = 15
    assert_eq!(cx.cr(e).hp, hp - 15);
}

#[test]
fn glam_replays_once_then_is_disabled() {
    let mut cx = enchanted(ids::card::STRIKE_IRONCLAD, ids::enchantment::GLAM, 1);
    let e = cx.enemies[0];
    let c = cx.cards.iter().position(|k| k.enchant != 0).unwrap() as u8;
    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    let pos = cx.player.hand.iter().position(|&x| x == c).unwrap() as u8;
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 12);
    assert_eq!(cx.cards[c as usize].enchant_status, 1, "Disabled after the first completed play");
    // played again later: no replay
    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    let pos = cx.player.hand.iter().position(|&x| x == c).unwrap() as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 18);
}

#[test]
fn vigorous_only_boosts_the_first_iteration() {
    let mut cx = enchanted(ids::card::STRIKE_IRONCLAD, ids::enchantment::VIGOROUS, 5);
    cx.apply_power(ids::power::DUPLICATION_POWER, PLAYER, Dec::ONE, PLAYER, NO);
    let e = cx.enemies[0];
    let c = cx.cards.iter().position(|k| k.enchant != 0).unwrap() as u8;
    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    let pos = cx.player.hand.iter().position(|&x| x == c).unwrap() as u8;
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (6 + 5) - 6);
}

#[test]
fn steady_and_royally_approved_add_local_keywords_before_upgrading() {
    let cx = enchanted(ids::card::STRIKE_IRONCLAD, ids::enchantment::ROYALLY_APPROVED, 1);
    let c = cx.cards.iter().position(|k| k.enchant != 0).unwrap() as u8;
    let k = cx.card_keywords(c);
    assert!(k & kw::INNATE != 0 && k & kw::RETAIN != 0);
}

// ---- extra turn (spec 01 §9.2) ------------------------------------------------------------------------------------

#[test]
fn ambergris_gives_an_extra_turn_without_an_enemy_turn() {
    let mut cx = Combat::new(&scenario(vec![], vec![ids::potion::AMBERGRIS], 40, 8));
    let e = cx.enemies[0];
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).hp, 40);
    let (round, turn) = (cx.round, cx.player.turn_number);
    let enemy_move_before = cx.cr(e).monster.next_move;
    let hp = cx.cr(PLAYER).hp;
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.side, Side::Player);
    assert_eq!(cx.round, round, "round does not advance on an extra turn");
    assert_eq!(cx.player.turn_number, turn + 1);
    assert_eq!(cx.cr(PLAYER).hp, hp, "enemies did not act");
    assert!(!cx.cr(e).monster.performed_first);
    assert_eq!(cx.cr(e).monster.next_move, enemy_move_before, "no new intents on an extra turn");
    assert!(!cx.has_power(PLAYER, ids::power::AMBERGRIS_POWER));
    // the next end turn is a normal one
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.round, round + 1);
}
