//! Hardening: fixed capacities never panic / corrupt in release and never drop data silently (`Combat::overflow`), the
//! in-place `reset` is indistinguishable from `new`, and the combat state stays inside its memory budget.
use std::mem::size_of;
use sts2sim::dec::Dec;
use sts2sim::engine::{ActionBuf, HKind};
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::scenario::ScenarioError;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn deck(n: usize) -> Vec<DeckCard> {
    (0..n)
        .map(|i| match i % 3 {
            0 => DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 },
            1 => DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 },
            _ => DeckCard { id: ids::card::BASH, upgrade: 0 },
        })
        .collect()
}

fn scenario(encounter: u16, n_deck: usize, seed: u64) -> Scenario {
    Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck: deck(n_deck),
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

#[test]
fn fresh_combat_has_no_overflow() {
    let cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 10, 1));
    assert_eq!(cx.overflow, 0);
}

#[test]
fn card_arena_overflow_is_flagged_not_fatal() {
    let mut cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 10, 1));
    // keep generating status cards far beyond the arena: no panic, a visible flag, the sim keeps working
    for _ in 0..(MAX_CARDS + 40) {
        cx.add_status_cards(ids::card::WOUND, PileType::Discard, 1, CardPilePosition::Bottom);
    }
    assert!(cx.overflow & ov::CARDS != 0, "overflow = {:#x}", cx.overflow);
    assert_eq!(cx.n_cards as usize, MAX_CARDS);
    // the combat can still be stepped (and the flag stays)
    let mut buf = ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert!(cx.step(buf[buf.len() - 1]));
    assert!(cx.overflow & ov::CARDS != 0);
}

#[test]
fn power_list_overflow_is_flagged() {
    let mut cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 10, 1));
    let e = cx.enemies[0];
    // apply distinct implemented powers until the creature's list is full and keep going: nothing may panic, the surplus is
    // dropped and the combat is flagged
    for id in 0..ids::power::NAMES.len() as u16 {
        if sts2sim::content::power_implemented(id) {
            cx.apply_power(id, e, Dec::int(1), PLAYER, NO);
        }
    }
    cx.sync_overflow();
    assert!(cx.cr(e).powers.len() <= MAX_POWERS);
    assert!(cx.overflow & ov::CONTAINER != 0, "no overflow flag (powers = {})", cx.cr(e).powers.len());
}

#[test]
fn array_vec_overflow_sets_thread_local_flag() {
    sts2sim::util::take_overflow();
    let mut v: sts2sim::util::ArrayVec<u8, 3> = sts2sim::util::ArrayVec::new();
    for i in 0..5 {
        v.push(i);
    }
    assert_eq!(v.len(), 3);
    assert_eq!(sts2sim::util::take_overflow() & sts2sim::util::OV_CONTAINER, sts2sim::util::OV_CONTAINER);
    assert_eq!(sts2sim::util::take_overflow(), 0);
    v.insert(0, 9);
    assert_eq!(v.len(), 3);
    assert_ne!(sts2sim::util::take_overflow(), 0);
}

#[test]
fn history_ring_overwriting_a_live_entry_is_flagged() {
    let mut cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 10, 1));
    sts2sim::util::take_overflow();
    // far more per-turn entries than the ring holds: the oldest ones of the current turn get lost
    for _ in 0..(sts2sim::engine::HIST_CAP + 10) {
        cx.hist_push(HKind::BlockGained, PLAYER, NO, 0, NO, 1, 0, 0, 0);
    }
    cx.sync_overflow();
    assert!(cx.overflow & ov::HISTORY != 0);
    // counter-only kinds never pressure the ring
    let mut cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 10, 1));
    sts2sim::util::take_overflow();
    for _ in 0..(sts2sim::engine::HIST_CAP * 3) {
        cx.hist_push(HKind::MonsterPerformedMove, 1, NO, 0, NO, 0, 0, 0, 0);
    }
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0);
    assert_eq!(cx.hist_total(HKind::MonsterPerformedMove), sts2sim::engine::HIST_CAP * 3);
}

#[test]
fn invalid_scenarios_are_errors_not_panics() {
    assert!(matches!(Combat::try_new(&scenario(ids::encounter::NIBBITS_WEAK, MAX_DECK + 1, 1)), Err(ScenarioError::DeckTooLarge)));
    let mut s = scenario(ids::encounter::NIBBITS_WEAK, 10, 1);
    s.relics = (0..MAX_RELICS + 1).map(|_| RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }).collect();
    assert!(matches!(Combat::try_new(&s), Err(ScenarioError::TooManyRelics)));
    let mut s = scenario(ids::encounter::NIBBITS_WEAK, 10, 1);
    s.potions = vec![ids::potion::BLOOD_POTION; MAX_POTIONS + 1];
    s.potion_slots = 9;
    assert!(Combat::try_new(&s).is_err());
    // the largest accepted deck works and leaves room in the arena
    let cx = Combat::try_new(&scenario(ids::encounter::NIBBITS_WEAK, MAX_DECK, 1)).unwrap();
    assert_eq!(cx.overflow, 0);
}

/// FNV-style fold of everything an agent / the diff can see plus engine counters.
fn fingerprint(cx: &Combat, obs: &mut [f32]) -> u64 {
    cx.observe(obs);
    let mut h = 0xcbf29ce484222325u64;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    for v in obs.iter() {
        mix(v.to_bits() as u64);
    }
    for c in cx.rng.shuffle.state() {
        mix(c);
    }
    mix(cx.rng.niche.state()[0]);
    mix(cx.rng.monster_ai.state()[0]);
    mix(cx.n_cards as u64);
    mix(cx.hist_log.n as u64);
    mix(cx.next_power_uid as u64);
    mix(cx.play_serial as u64);
    mix(cx.cr(0).hp as u64);
    h
}

fn play_random(cx: &mut Combat, seed: u64, steps: usize) -> Vec<u64> {
    let mut obs = vec![0f32; OBS_SIZE];
    let mut r = sts2sim::rng::Rng::new(seed);
    let mut out = vec![fingerprint(cx, &mut obs)];
    let mut buf = ActionBuf::new();
    for _ in 0..steps {
        if cx.stage == Stage::Over {
            break;
        }
        cx.legal_actions(&mut buf);
        let a = buf[r.next_int(buf.len() as i32) as usize];
        assert!(cx.step(a));
        out.push(fingerprint(cx, &mut obs));
    }
    out
}

#[test]
fn reset_is_bit_identical_to_new() {
    // a reused combat that already lived through fights of very different shape (deck size, enemies, relics, potions)
    let shapes = [
        scenario(ids::encounter::NIBBITS_WEAK, 10, 5),
        scenario(ids::encounter::NIBBITS_WEAK, 60, 6),
        {
            let mut s = scenario(ids::encounter::NIBBITS_WEAK, 25, 7);
            s.relics = vec![];
            s.potion_slots = 3;
            s
        },
    ];
    let mut reused = Combat::new(&shapes[1]);
    for round in 0..12u64 {
        let sc = {
            let mut s = shapes[(round % 3) as usize].clone();
            s.run_seed = 1000 + round;
            s.rng = RngSet::from_run_seed(1000 + round);
            s
        };
        // dirty the reused combat with some play, then reset it
        play_random(&mut reused, 77 + round, 40);
        reused.reset(&sc).unwrap();
        let mut fresh = Combat::new(&sc);
        let a = play_random(&mut fresh, round, 300);
        let b = play_random(&mut reused, round, 300);
        assert_eq!(a, b, "round {round}: reset diverged from new");
        assert_eq!(fresh.overflow, reused.overflow);
        assert_eq!(fresh.outcome, reused.outcome);
    }
}

#[test]
fn reset_validated_with_run_seed_matches_scenario_seeding() {
    let sc = scenario(ids::encounter::NIBBITS_WEAK, 12, 31337);
    let mut a = Combat::new(&sc);
    let mut b = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 40, 1));
    // the env derives (run_seed, rng) per episode and passes them separately; the fast stream seeding equals the named one
    b.reset_validated(&sc, &ScenarioExtras::default(), 31337, RngSet::from_run_seed_fast(31337)).unwrap();
    assert_eq!(play_random(&mut a, 3, 200), play_random(&mut b, 3, 200));
}

/// Memory budget of the simulation state (see docs/design.md "Memory"): a failing assert means a field grew; shrink something
/// or raise the budget deliberately.
#[test]
fn state_size_budget() {
    let sz = size_of::<Combat>();
    println!("size_of::<Combat>() = {sz}");
    assert!(sz <= COMBAT_BUDGET, "Combat is {sz} bytes, budget {COMBAT_BUDGET}");
}

const COMBAT_BUDGET: usize = 17_600;

/// Creature slots: the player, Osty and the biggest encounter's enemies must fit with room for summons.
#[test]
fn every_encounter_fits_the_creature_slots() {
    let mut max = 0;
    let mut worst = "";
    for id in 0..ids::encounter::COUNT as u16 {
        if !sts2sim::content::encounter_implemented(id) {
            continue;
        }
        for seed in 0..8u64 {
            let mut r = sts2sim::rng::Rng::new(seed);
            let n = sts2sim::content::encounter_spawns(id, &mut r, 10).unwrap().len();
            if n > max {
                max = n;
                worst = ids::encounter::NAMES[id as usize];
            }
        }
    }
    println!("largest encounter: {worst} with {max} enemies (MAX_CREATURES = {MAX_CREATURES})");
    assert!(max + 2 <= MAX_CREATURES, "{worst}: {max} enemies leave no room for the player + a pet");
}
