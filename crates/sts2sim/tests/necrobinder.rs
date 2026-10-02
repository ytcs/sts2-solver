//! Osty / Doom / Necrobinder card rules (hand-constructed situations; the differential sweeps cover the rest).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn base() -> Combat {
    let mut deck: Vec<DeckCard> = (0..4).map(|_| DeckCard { id: ids::card::STRIKE_NECROBINDER, upgrade: 0 }).collect();
    deck.extend((0..4).map(|_| DeckCard { id: ids::card::DEFEND_NECROBINDER, upgrade: 0 }));
    deck.push(DeckCard { id: ids::card::BODYGUARD, upgrade: 0 });
    deck.push(DeckCard { id: ids::card::UNLEASH, upgrade: 0 });
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 3,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 66,
        hp: 66,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![RelicInit { id: ids::relic::BOUND_PHYLACTERY, counter: 0 }],
        potions: vec![],
        rng: RngSet::from_run_seed(7),
    })
}

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
fn bound_phylactery_summons_osty_with_1_hp() {
    let cx = base();
    let o = cx.osty().expect("Osty summoned at combat start");
    assert_eq!((cx.cr(o).hp, cx.cr(o).max_hp), (1, 1));
    assert!(cx.cr(o).is_pet && cx.cr(o).side == Side::Player);
    assert!(cx.has_power(o, ids::power::DIE_FOR_YOU_POWER));
}

#[test]
fn summon_grows_a_living_osty_and_revives_a_dead_one_with_new_max_hp() {
    let mut cx = base();
    let o = cx.osty().unwrap();
    cx.summon(5);
    assert_eq!((cx.cr(o).hp, cx.cr(o).max_hp), (6, 6)); // max HP and current HP both grow
    cx.cr_mut(o).hp = 2;
    cx.summon(3);
    assert_eq!((cx.cr(o).hp, cx.cr(o).max_hp), (5, 9));
    cx.kill(&[o]);
    assert!(cx.cr(o).is_dead() && cx.osty() == Some(o), "dead Osty stays in the combat");
    assert!(cx.has_power(o, ids::power::DIE_FOR_YOU_POWER), "DieForYou survives death");
    cx.summon(4);
    assert_eq!((cx.cr(o).hp, cx.cr(o).max_hp), (4, 4)); // revive: max HP is reset to the summon amount
}

#[test]
fn osty_absorbs_powered_attacks_blocks_use_owner_block_and_overkill_spills() {
    let mut cx = base();
    let o = cx.osty().unwrap();
    cx.summon(5); // Osty 6/6
    cx.cr_mut(PLAYER).block = 4;
    let hp = cx.cr(PLAYER).hp;
    // 10 powered damage: 4 blocked by the PLAYER's block, 6 to Osty (exactly lethal, no overkill).
    cx.damage(&[PLAYER], Dec::int(10), ValueProp::MOVE, NO, NO);
    assert_eq!(cx.cr(PLAYER).block, 0);
    assert_eq!(cx.cr(PLAYER).hp, hp);
    assert!(cx.cr(o).is_dead());
    // Osty dead: the hit goes to the player.
    cx.damage(&[PLAYER], Dec::int(3), ValueProp::MOVE, NO, NO);
    assert_eq!(cx.cr(PLAYER).hp, hp - 3);
    // Revive with 2 HP: 7 powered damage -> 2 to Osty, overkill 5 to the player.
    cx.summon(2);
    cx.damage(&[PLAYER], Dec::int(7), ValueProp::MOVE, NO, NO);
    assert!(cx.cr(o).is_dead());
    assert_eq!(cx.cr(PLAYER).hp, hp - 3 - 5);
}

#[test]
fn unpowered_or_unblockable_damage_ignores_osty() {
    let mut cx = base();
    let o = cx.osty().unwrap();
    cx.summon(9);
    let hp = cx.cr(PLAYER).hp;
    cx.damage(&[PLAYER], Dec::int(4), ValueProp::MOVE.or(ValueProp::UNPOWERED), NO, NO);
    assert_eq!(cx.cr(PLAYER).hp, hp - 4);
    assert_eq!(cx.cr(o).hp, 10);
}

#[test]
fn osty_attack_ignores_player_strength_and_weak_but_not_vulnerable() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.summon(5); // Osty 6/6 -> Unleash = 6 + 6
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(3), PLAYER, NO);
    cx.apply_power(ids::power::WEAK_POWER, PLAYER, Dec::int(2), NO, NO);
    cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(2), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::UNLEASH, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 18); // 12 * 1.5, no Strength / Weak
}

#[test]
fn osty_attack_card_does_nothing_when_osty_is_dead_but_is_still_paid() {
    let mut cx = base();
    let e = cx.enemies[0];
    let o = cx.osty().unwrap();
    cx.kill(&[o]);
    set_hand(&mut cx, &[(ids::card::POKE, 0), (ids::card::HIGH_FIVE, 0)]);
    let hp = cx.cr(e).hp;
    assert!(!cx.can_play(cx.player.hand[1]), "High Five is unplayable without Osty");
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp);
}

#[test]
fn doom_kills_at_end_of_the_enemy_turn_when_hp_is_at_most_doom() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.cr_mut(e).hp = 7;
    cx.apply_power(ids::power::DOOM_POWER, e, Dec::int(7), PLAYER, NO);
    assert!(cx.cr(e).is_alive());
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}

#[test]
fn doom_below_threshold_does_not_kill_and_does_not_decay() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.apply_power(ids::power::DOOM_POWER, e, Dec::int(5), PLAYER, NO);
    assert!(cx.step(Action::EndTurn));
    assert!(cx.cr(e).is_alive());
    assert_eq!(cx.power_amount(e, ids::power::DOOM_POWER), 5);
}

#[test]
fn blight_strike_applies_doom_equal_to_damage_dealt() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::BLIGHT_STRIKE, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.power_amount(e, ids::power::DOOM_POWER), 8);
}

#[test]
fn bone_shards_kills_osty_and_necro_mastery_hurts_enemies_for_its_lost_hp() {
    let mut cx = base();
    let e = cx.enemies[0];
    let o = cx.osty().unwrap();
    cx.summon(5); // Osty 6/6
    cx.apply_power(ids::power::NECRO_MASTERY_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::BONE_SHARDS, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    // 9 attack damage, then Osty (6 HP) is killed: 6 x 2 unblockable damage.
    assert_eq!(cx.cr(e).hp, hp - 9 - 12);
    assert!(cx.cr(o).is_dead());
    assert_eq!(cx.cr(PLAYER).block, 9);
}

#[test]
fn observation_exposes_osty_at_the_end_of_the_vector() {
    let mut cx = base();
    cx.summon(5); // Osty 6/6
    set_hand(&mut cx, &[(ids::card::POKE, 0)]);
    let mut v = vec![0f32; observe::OBS_SIZE];
    cx.observe(&mut v);
    let osty = &v[observe::OBS_SIZE - observe::OSTY_F..];
    assert_eq!(&osty[..4], &[1.0, 1.0, 6.0, 6.0]); // present, alive, hp, max hp
    let preview = osty[4 + 2 * observe::OBS_POWERS]; // first hand slot: Poke = 6 damage from Osty
    assert_eq!(preview, 6.0);
}
