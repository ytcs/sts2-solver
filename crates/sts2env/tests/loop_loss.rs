//! A fight the loop guard ends (`ov::LOOP`) is a loss, not a neutral abort: the real game soft-locks on it (Pillage + Hellraiser +
//! Velvet Choker, `docs/research/evidence.md` E6). The search scores a play-out into it as a loss (and so avoids it); the same chain
//! without the Choker is a finite combo that still wins.
use sts2env::search::*;
use sts2env::{OUTCOME_LOSS, OUTCOME_WIN};
use sts2sim::dec::Dec;
use sts2sim::engine::{Action, ACTION_SPACE};
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(choker: bool) -> Scenario {
    let mut relics = vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }];
    if choker {
        relics.push(RelicInit { id: ids::relic::VELVET_CHOKER, ..Default::default() });
    }
    Scenario {
        run_seed: 3,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck: (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect(),
        relics,
        potions: vec![],
        rng: RngSet::from_run_seed(3),
    }
}

/// Pillage alone in hand, two Strikes in the discard pile and nothing else to draw, Hellraiser on; with the Choker, Pillage is the 6th play
/// of the turn (`crates/sts2sim/tests/loop_guard.rs`: the Choker loop trips on it, without the Choker the Strikes win the fight).
fn pillage_state(sc: &Scenario, choker: bool) -> Combat {
    let mut cx = Combat::new(sc);
    let all: Vec<CardIdx> = cx.player.hand.iter().chain(cx.player.draw.iter()).chain(cx.player.discard.iter()).copied().collect();
    for (k, &c) in all.iter().enumerate() {
        cx.move_card(c, if k < 2 { PileType::Discard } else { PileType::Exhaust }, CardPilePosition::Bottom);
    }
    let p = cx.new_card(ids::card::PILLAGE, 0).unwrap();
    cx.move_card(p, PileType::Hand, CardPilePosition::Bottom);
    cx.apply_power(ids::power::HELLRAISER_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    if choker {
        let r = cx.player.relics.iter().position(|r| r.id == ids::relic::VELVET_CHOKER).unwrap();
        cx.player.relics[r].counter = 5;
    }
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0);
    cx
}

fn hash(o: &[f32]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for (i, x) in o.iter().enumerate().step_by(7) {
        h = (h ^ x.to_bits() as u64 ^ i as u64).wrapping_mul(0x100000001b3);
    }
    h
}

/// The worth table of the table test: a loss is worth -0.7, a win ending in class b (2-HP bins) 0.3 + b / 100.
fn table() -> Worth {
    let mut u = [0.0f32; HEAD_NC];
    u[0] = -0.7;
    for (b, x) in u.iter_mut().enumerate().skip(1) {
        *x = 0.3 + b as f32 / 100.0;
    }
    Worth { table: true, u, price: [0.0; POT] }
}

/// A stand-in network: the `m` first legal actions with equal probabilities, a play-out move by the observation's hash; values in
/// [-0.25, 0.25] (scalar rows), or uniform outcome-class probabilities with the worth table (`with_table`).
fn search(choker: bool, with_table: bool) -> (JobResult, Vec<MoveRec>, SearchStats, Combat) {
    let sc = scenario(choker);
    let start = pillage_state(&sc, choker);
    let vw = if with_table { HEAD_NC } else { 1 };
    let cfg = SearchCfg { m: 4, k: 4, conf: 1.01, pmin: 0.0, margin: 0.0, roll_cap: 60, leaf_turns: 1, lead: false, carry: false, strat: false, max_steps: 300, win: 1.0, loss: -1.0, hp_bonus: 0.5, util: [0.0; 102], use_util: false, turn_cap: 0, val_w: vw };
    let mut eng = SearchEngine::new_with_starts(vec![(sc, ScenarioExtras::default())], vec![Some(start.clone())], vec![(0, 17)], 1, cfg, 2, true).unwrap();
    if with_table {
        eng.set_worth(vec![table()]).unwrap();
    }
    let (pc, vc) = eng.max_rows();
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; pc * stride], vec![0f32; vc * vw]);
    let (mut np, mut nv) = eng.advance(None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
    let mut cycles = 0;
    while np + nv > 0 {
        for r in 0..np {
            let (obs, mask) = (&po[r * OBS_SIZE..(r + 1) * OBS_SIZE], &pm[r * ACTION_SPACE..(r + 1) * ACTION_SPACE]);
            let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| mask[a] > 0).collect();
            let out = &mut pol[r * stride..(r + 1) * stride];
            for j in 0..cfg.m {
                out[j] = *legal.get(j).unwrap_or(&0) as f32;
                out[cfg.m + j] = if j < legal.len() { 1.0 / cfg.m.min(legal.len()) as f32 } else { 0.0 };
            }
            out[2 * cfg.m] = legal[(hash(obs) % legal.len() as u64) as usize] as f32;
        }
        for r in 0..nv {
            if with_table {
                val[r * vw..(r + 1) * vw].fill(1.0 / HEAD_NC as f32);
            } else {
                val[r] = (hash(&vo[r * OBS_SIZE..(r + 1) * OBS_SIZE]) % 1000) as f32 / 2000.0 - 0.25;
            }
        }
        let (pa, va) = (pol[..np * stride].to_vec(), val[..nv * vw].to_vec());
        (np, nv) = eng.advance(Some(&pa), Some(&va), &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
        cycles += 1;
        assert!(cycles < 100_000, "the engine does not terminate");
    }
    assert!(eng.finished());
    (eng.results()[0], eng.moves(0).to_vec(), eng.stats(), start)
}

/// The first decision's options that play Pillage, with their estimates.
fn pillage_options(first: &MoveRec, start: &Combat) -> Vec<f32> {
    let plays: Vec<u16> = start.enemies.iter().map(|&e| Action::PlayCard { hand_pos: 0, target: e }.index() as u16).collect();
    (0..MAX_M).filter(|&j| first.legal[j] && plays.contains(&first.opts[j])).map(|j| first.q[j]).collect()
}

#[test]
fn a_play_out_into_the_choker_loop_scores_as_a_loss_and_the_search_avoids_it() {
    let (res, moves, st, start) = search(true, false);
    let first = moves[0];
    assert!(first.searched, "the first decision (Pillage or end the turn) is searched");
    let q = pillage_options(&first, &start);
    assert!(!q.is_empty(), "a Pillage option was tried");
    for &x in &q {
        assert_eq!(x, -1.0, "every future of Pillage soft-locks: scored as a loss (was 0, a neutral abort)");
    }
    assert!(st.end_loop > 0 && st.end_loop <= st.end_term, "loop play-outs counted ({} of {} terminal)", st.end_loop, st.end_term);
    let plays: Vec<u16> = start.enemies.iter().map(|&e| Action::PlayCard { hand_pos: 0, target: e }.index() as u16).collect();
    assert!(!plays.contains(&first.action), "the search ends the turn instead of soft-locking the fight");
    assert_eq!(st.fight_loops, 0);
    assert!(res.done && res.outcome != sts2env::OUTCOME_OVERFLOW, "outcome {}", res.outcome);
}

#[test]
fn the_finite_combo_without_the_choker_still_wins() {
    let (res, moves, st, start) = search(false, false);
    let q = pillage_options(&moves[0], &start);
    assert!(!q.is_empty());
    for &x in &q {
        assert_eq!(x, 1.5, "Pillage wins the fight at full HP in every future (1 + 0.5 x HP left)");
    }
    assert_eq!(st.end_loop, 0);
    assert_eq!(res.outcome, OUTCOME_WIN);
}

#[test]
fn with_a_worth_table_the_loop_is_worth_a_loss_and_the_combo_a_win() {
    let w = table();
    let (_, moves, st, start) = search(true, true);
    let q = pillage_options(&moves[0], &start);
    assert!(!q.is_empty() && st.end_loop > 0);
    for &x in &q {
        assert!((x - w.u[0]).abs() < 1e-6, "the loop is worth the loss class ({x} vs {})", w.u[0]);
    }
    let (res, moves, _, start) = search(false, true);
    let q = pillage_options(&moves[0], &start);
    assert!(!q.is_empty());
    for &x in &q {
        assert!((x - w.u[40]).abs() < 1e-6, "a win at 80 HP is worth its end class, 40 ({x} vs {})", w.u[40]);
    }
    assert_eq!(res.outcome, OUTCOME_WIN);
}

#[test]
fn a_real_fight_in_the_loop_is_recorded_as_a_loss() {
    // a job that starts from a fight the guard already ended (the real fight's own step tripped): a loss, not a truncation
    let sc = scenario(true);
    let mut start = pillage_state(&sc, true);
    let e = start.enemies[0];
    assert!(start.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert!(sts2env::looped(&start));
    let cfg = SearchCfg { m: 4, k: 4, conf: 1.01, pmin: 0.0, margin: 0.0, roll_cap: 60, leaf_turns: 1, lead: false, carry: false, strat: false, max_steps: 300, win: 1.0, loss: -1.0, hp_bonus: 0.5, util: [0.0; 102], use_util: false, turn_cap: 0, val_w: 1 };
    let mut eng = SearchEngine::new_with_starts(vec![(sc, ScenarioExtras::default())], vec![Some(start)], vec![(0, 17)], 1, cfg, 1, false).unwrap();
    let (pc, vc) = eng.max_rows();
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let (np, nv) = eng.advance(None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
    assert_eq!(np + nv, 0, "nothing to ask: the fight is over");
    assert!(eng.finished());
    assert_eq!(eng.results()[0].outcome, OUTCOME_LOSS);
    assert_eq!(eng.stats().fight_loops, 1);
}
