//! Expert pattern knowledge: `Combat::lookahead` must reproduce what an experienced player knows (the pattern), without
//! consuming RNG or revealing the realized outcome of random branches.
use sts2sim::content;
use sts2sim::engine::{ActionBuf, LOOK_H, LOOK_NODES};
use sts2sim::ids;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(encounter: u16, seed: u64) -> Scenario {
    let mut deck: Vec<DeckCard> = vec![];
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    deck.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter,
        max_hp: 400,
        hp: 400,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

#[test]
fn deterministic_cycle_is_known_exactly() {
    // Nibbit alone: BUTT -> SLICE -> HISS -> BUTT (nodes 1, 2, 3).
    let cx = Combat::new(&scenario(ids::encounter::NIBBITS_WEAK, 5));
    let e = cx.enemies[0];
    assert_eq!(cx.cr(e).monster.next_move, 1);
    let rows = cx.lookahead(e);
    let argmax = |r: &sts2sim::engine::LookRow| r.prob.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
    assert_eq!((argmax(&rows[0]), rows[0].prob[2]), (2, 1.0)); // next turn: SLICE for sure
    assert_eq!((argmax(&rows[1]), rows[1].prob[3]), (3, 1.0)); // then HISS
    assert_eq!((argmax(&rows[2]), rows[2].prob[1]), (1, 1.0)); // then BUTT again
    // expected damage of SLICE at A10 = 7 (no modifiers), HISS = 0, BUTT = 13
    assert_eq!(rows[0].exp_damage, 7.0);
    assert_eq!(rows[1].exp_damage, 0.0);
    assert_eq!(rows[2].exp_damage, 13.0);
}

#[test]
fn lookahead_consumes_no_rng_and_does_not_change_state() {
    for enc in 0..ids::encounter::COUNT as u16 {
        if !content::encounter_implemented(enc) {
            continue;
        }
        let cx = Combat::new(&scenario(enc, 11));
        let before = (cx.rng.monster_ai.counter, cx.rng.monster_ai.state());
        for &e in cx.enemies.iter() {
            let ms = cx.cr(e).monster;
            let _ = cx.lookahead(e);
            assert_eq!(cx.cr(e).monster.log, ms.log);
            assert_eq!(cx.cr(e).monster.log_len, ms.log_len);
            assert_eq!(cx.cr(e).monster.next_move, ms.next_move);
        }
        assert_eq!(before, (cx.rng.monster_ai.counter, cx.rng.monster_ai.state()), "{}", ids::encounter::NAMES[enc as usize]);
    }
}

#[test]
fn every_encounter_lookahead_is_sane_while_playing() {
    let mut checked = 0;
    for enc in 0..ids::encounter::COUNT as u16 {
        if !content::encounter_implemented(enc) {
            continue;
        }
        let name = ids::encounter::NAMES[enc as usize];
        let mut cx = Combat::new(&scenario(enc, 21));
        let mut rng = Rng::new(enc as u64 + 1);
        let mut buf = ActionBuf::new();
        for step in 0..60 {
            if cx.stage == Stage::Over || cx.missing.is_some() {
                break;
            }
            for &e in cx.enemies.iter() {
                if !cx.cr(e).is_alive() {
                    continue;
                }
                let rows = cx.lookahead(e);
                for (h, r) in rows.iter().enumerate() {
                    let sum: f32 = r.prob.iter().sum();
                    assert!(sum <= 1.0001, "{name} step {step} h{h}: probabilities sum to {sum}");
                    assert!(r.prob.iter().all(|&p| (0.0..=1.0001).contains(&p)), "{name}");
                    assert!(r.exp_damage >= 0.0 && r.exp_damage.is_finite(), "{name}");
                }
                // the first future turn should almost always be known (a conditional with no true arm would give 0)
                if step == 0 {
                    let s0: f32 = rows[0].prob.iter().sum();
                    assert!(s0 > 0.5, "{name}: first future turn has probability mass {s0}");
                }
                checked += 1;
            }
            cx.legal_actions(&mut buf);
            let a = buf[rng.next_int(buf.len() as i32) as usize];
            cx.step(a);
        }
    }
    assert!(checked > 200, "checked only {checked} lookaheads");
}

#[test]
fn all_move_graphs_fit_the_lookahead_slots() {
    for id in 0..ids::monster::COUNT as u16 {
        if content::monster_implemented(id) {
            let n = content::monster_def(id).nodes.len();
            assert!(n < LOOK_NODES, "{} has {n} nodes (>= {LOOK_NODES}); widen LOOK_NODES", ids::monster::NAMES[id as usize]);
        }
    }
    let _ = LOOK_H;
}

/// Soundness: the move an enemy actually makes at turn t+h had positive probability in the lookahead made at turn t
/// (stuns, forced moves, summons and state-dependent conditions can legitimately break the prediction, so a small miss
/// rate is allowed; deterministic monsters must be exact).
#[test]
fn realized_moves_were_predicted() {
    let (mut total, mut missed) = (0u32, 0u32);
    let mut worst: Vec<(String, u32, u32)> = vec![];
    for enc in 0..ids::encounter::COUNT as u16 {
        if !content::encounter_implemented(enc) {
            continue;
        }
        let name = ids::encounter::NAMES[enc as usize];
        let (mut t, mut m) = (0u32, 0u32);
        for seed in 0..4u64 {
            let mut cx = Combat::new(&scenario(enc, 100 + seed));
            let mut rng = Rng::new(seed * 7 + enc as u64);
            let mut buf = ActionBuf::new();
            // history of (turn, creature, node-probabilities at horizon h)
            let mut pred: Vec<(i32, Cid, u16, [sts2sim::engine::LookRow; LOOK_H])> = vec![];
            let mut last_turn = 0;
            for _ in 0..400 {
                if cx.stage == Stage::Over || cx.missing.is_some() {
                    break;
                }
                if cx.stage == Stage::AwaitAction && cx.player.phase == Phase::Play && cx.player.turn_number != last_turn {
                    last_turn = cx.player.turn_number;
                    // check earlier predictions against the move now pending
                    for &(t0, e, mid, ref rows) in pred.iter() {
                        let h = (last_turn - t0 - 1) as usize;
                        if h < LOOK_H && cx.cr(e).in_combat && cx.cr(e).is_alive() && cx.cr(e).monster.id == mid {
                            let nm = cx.cr(e).monster.next_move;
                            if nm == STUN_NODE_FOR_TEST || cx.cr(e).monster.stunned {
                                continue;
                            }
                            let slot = if (nm as usize) >= LOOK_NODES - 1 { LOOK_NODES - 1 } else { nm as usize };
                            t += 1;
                            if rows[h].prob[slot] <= 0.0 {
                                m += 1;
                            }
                        }
                    }
                    pred.retain(|(t0, ..)| last_turn - *t0 < LOOK_H as i32);
                    for &e in cx.enemies.iter() {
                        if cx.cr(e).is_alive() {
                            pred.push((last_turn, e, cx.cr(e).monster.id, cx.lookahead(e)));
                        }
                    }
                }
                cx.legal_actions(&mut buf);
                let a = buf[rng.next_int(buf.len() as i32) as usize];
                cx.step(a);
            }
        }
        total += t;
        missed += m;
        if m > 0 {
            worst.push((name.to_string(), m, t));
        }
    }
    worst.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("lookahead soundness: {missed} unpredicted of {total}; encounters with misses: {:?}", &worst[..worst.len().min(12)]);
    assert!(total > 1000, "only {total} comparisons");
    assert!(missed as f64 / total as f64 <= 0.03, "{missed}/{total} realized moves were not predicted: {worst:?}");
}

const STUN_NODE_FOR_TEST: u8 = 0xFE;
