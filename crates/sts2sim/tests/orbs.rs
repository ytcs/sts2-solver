//! Orb subsystem rules (spec 05 §5) that the oracle sweeps cannot easily pin down in isolation.
use sts2sim::dec::Dec;
use sts2sim::engine::VALID_ORBS;
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(seed: u64, relics: bool) -> Scenario {
    let mut deck = vec![];
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::STRIKE_DEFECT, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_DEFECT, upgrade: 0 });
    }
    deck.push(DeckCard { id: ids::card::ZAP, upgrade: 0 });
    deck.push(DeckCard { id: ids::card::DUALCAST, upgrade: 0 });
    Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 2,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 75,
        hp: 75,
        max_energy: 3,
        orb_slots: 3,
        potion_slots: 2,
        deck,
        relics: if relics { vec![RelicInit { id: ids::relic::CRACKED_CORE, counter: 0, ..Default::default() }] } else { vec![] },
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

fn enemy_hp(cx: &Combat) -> i32 {
    cx.cr(cx.enemies[0]).hp
}

fn kinds(cx: &Combat) -> Vec<u16> {
    cx.player.orbs.iter().map(|o| o.kind).collect()
}

#[test]
fn cracked_core_channels_one_lightning_on_turn_one_only() {
    let mut cx = Combat::new(&scenario(1, true));
    assert_eq!(kinds(&cx), vec![ids::orb::LIGHTNING_ORB]);
    assert_eq!(cx.player.orb_slots, 3);
    // turn 2: no new orb (the first Lightning's passive hit the enemy at the end of turn 1)
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.player.turn_number, 2);
    assert_eq!(kinds(&cx), vec![ids::orb::LIGHTNING_ORB]);
}

#[test]
fn channel_into_a_full_queue_evokes_the_front_orb() {
    let mut cx = Combat::new(&scenario(2, false));
    for k in [ids::orb::FROST_ORB, ids::orb::DARK_ORB, ids::orb::LIGHTNING_ORB] {
        cx.channel_orb(k);
    }
    assert_eq!(kinds(&cx), vec![ids::orb::FROST_ORB, ids::orb::DARK_ORB, ids::orb::LIGHTNING_ORB]);
    let hp = enemy_hp(&cx);
    let block = cx.cr(PLAYER).block;
    cx.channel_orb(ids::orb::PLASMA_ORB); // evokes Frost (5 block), then enqueues Plasma at the back
    assert_eq!(kinds(&cx), vec![ids::orb::DARK_ORB, ids::orb::LIGHTNING_ORB, ids::orb::PLASMA_ORB]);
    assert_eq!(cx.cr(PLAYER).block, block + 5);
    assert_eq!(enemy_hp(&cx), hp);
}

#[test]
fn removing_slots_drops_orbs_from_the_back_silently() {
    let mut cx = Combat::new(&scenario(3, false));
    for k in [ids::orb::LIGHTNING_ORB, ids::orb::FROST_ORB, ids::orb::DARK_ORB] {
        cx.channel_orb(k);
    }
    let (hp, block, energy) = (enemy_hp(&cx), cx.cr(PLAYER).block, cx.player.energy);
    cx.remove_orb_slots(1);
    assert_eq!(cx.player.orb_slots, 2);
    assert_eq!(kinds(&cx), vec![ids::orb::LIGHTNING_ORB, ids::orb::FROST_ORB]);
    assert_eq!((enemy_hp(&cx), cx.cr(PLAYER).block, cx.player.energy), (hp, block, energy), "dropped orbs are not evoked");
    cx.remove_orb_slots(5);
    assert_eq!(cx.player.orb_slots, 0);
    assert!(cx.player.orbs.is_empty());
    // A Defect with no slots loses the channeled orb (no lazy slot for characters with BaseOrbSlotCount > 0).
    cx.channel_orb(ids::orb::LIGHTNING_ORB);
    assert!(cx.player.orbs.is_empty());
    assert_eq!(cx.player.orb_slots, 0);
}

#[test]
fn add_slots_caps_at_ten() {
    let mut cx = Combat::new(&scenario(4, false));
    cx.add_orb_slots(20);
    assert_eq!(cx.player.orb_slots, 10);
}

#[test]
fn dark_orb_accumulates_with_focus_at_passive_time_only() {
    let mut cx = Combat::new(&scenario(5, false));
    cx.channel_orb(ids::orb::DARK_ORB);
    cx.apply_power(ids::power::FOCUS_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    let o = cx.player.orbs[0];
    assert_eq!(cx.orb_passive_val(&o).trunc(), 8); // 6 + Focus
    assert_eq!(cx.orb_evoke_val(&o).trunc(), 6); // accumulated value, NOT focus-modified
    cx.orb_passive(o, NO, true);
    let o = cx.player.orbs[0];
    assert_eq!(cx.orb_evoke_val(&o).trunc(), 14); // 6 + 8
    // Focus loss afterwards does not touch what was accumulated
    cx.apply_power(ids::power::FOCUS_POWER, PLAYER, Dec::int(-5), PLAYER, NO);
    let o = cx.player.orbs[0];
    assert_eq!(cx.orb_evoke_val(&o).trunc(), 14);
    assert_eq!(cx.orb_passive_val(&o).trunc(), 3); // 6 - 3
}

#[test]
fn orb_values_floor_at_zero_with_negative_focus() {
    let mut cx = Combat::new(&scenario(6, false));
    cx.channel_orb(ids::orb::LIGHTNING_ORB);
    cx.channel_orb(ids::orb::FROST_ORB);
    cx.apply_power(ids::power::FOCUS_POWER, PLAYER, Dec::int(-10), PLAYER, NO);
    for o in cx.player.orbs.iter() {
        assert_eq!(cx.orb_passive_val(o).trunc(), 0);
        assert_eq!(cx.orb_evoke_val(o).trunc(), 0);
    }
}

#[test]
fn glass_orb_decays_by_one_per_passive() {
    let mut cx = Combat::new(&scenario(7, false));
    cx.channel_orb(ids::orb::GLASS_ORB);
    let hp0 = enemy_hp(&cx);
    let o = cx.player.orbs[0];
    assert_eq!((cx.orb_passive_val(&o).trunc(), cx.orb_evoke_val(&o).trunc()), (4, 8));
    cx.orb_passive(o, NO, true);
    assert_eq!(enemy_hp(&cx), hp0 - 4);
    let o = cx.player.orbs[0];
    assert_eq!((cx.orb_passive_val(&o).trunc(), cx.orb_evoke_val(&o).trunc()), (3, 6));
}

#[test]
fn random_orb_uses_the_game_order_not_the_id_order() {
    assert_eq!(VALID_ORBS, [ids::orb::LIGHTNING_ORB, ids::orb::FROST_ORB, ids::orb::DARK_ORB, ids::orb::PLASMA_ORB, ids::orb::GLASS_ORB]);
}

#[test]
fn lightning_draws_one_target_even_with_a_single_enemy() {
    let mut cx = Combat::new(&scenario(8, false));
    let before = cx.rng.combat_targets.counter;
    cx.channel_orb(ids::orb::LIGHTNING_ORB);
    let o = cx.player.orbs[0];
    cx.orb_passive(o, NO, true);
    assert_eq!(cx.rng.combat_targets.counter, before + 1);
    // with an explicit target (Tesla Coil) nothing is drawn
    let t = cx.enemies[0];
    cx.orb_passive(o, t, false);
    assert_eq!(cx.rng.combat_targets.counter, before + 1);
}

#[test]
fn observation_exposes_orbs_and_slots() {
    let cx = Combat::new(&scenario(9, true));
    let mut v = vec![0f32; OBS_SIZE];
    cx.observe(&mut v);
    let after = sts2sim::observe::LOOK_F + sts2sim::observe::ENEMY_MOVES_F; // the sections that follow the orbs
    let tail = &v[OBS_SIZE - after - (10 * 3 + 1)..OBS_SIZE - after];
    assert_eq!(&tail[0..3], &[ids::orb::LIGHTNING_ORB as f32 + 1.0, 3.0, 8.0]); // kind+1, passive, evoke
    assert_eq!(&tail[3..6], &[0.0, 0.0, 0.0]); // empty slot
    assert_eq!(tail[30], 1.0); // Lightning orbs channeled so far (Cracked Core's)
}

#[test]
fn player_death_clears_the_queue() {
    let mut cx = Combat::new(&scenario(10, true));
    assert_eq!(cx.player.orbs.len(), 1);
    cx.kill(&[PLAYER]);
    assert!(cx.player.orbs.is_empty());
    assert_eq!(cx.player.orb_slots, 0);
}
