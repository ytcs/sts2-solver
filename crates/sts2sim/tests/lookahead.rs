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
    assert_eq!((argmax(&rows[3]), rows[3].prob[2]), (2, 1.0)); // and SLICE
    // expected damage of SLICE at A10 = 7 (no modifiers), HISS = 0; BUTT = 13 + the Strength HISS gave on the way (projected)
    let s = nibbit_hiss_strength(&cx, e);
    assert_eq!(rows[0].exp_damage, 7.0);
    assert_eq!(rows[1].exp_damage, 0.0);
    assert_eq!(rows[2].exp_damage, 13.0 + s as f32);
    assert_eq!(rows[3].exp_damage, 7.0 + s as f32);
}

/// The Strength a Nibbit gains from HISS, measured by playing the fight (the player passes) until it has hissed.
fn nibbit_hiss_strength(cx: &Combat, e: Cid) -> i32 {
    let mut cx = cx.clone();
    for _ in 0..3 {
        cx.step(Action::EndTurn);
    }
    let s = cx.cr(e).power(ids::power::STRENGTH_POWER).map_or(0, |p| p.amount);
    assert!(s > 0, "HISS gave no Strength");
    s
}

/// Self-state along the projection: the Lagavulin Matriarch sleeps (Asleep 3, counting down every enemy turn) and then attacks; the
/// look-ahead sees it wake up, and the Slumbering Beetle likewise, with no player input.
#[test]
fn sleepers_wake_up_on_schedule() {
    for (enc, mid) in [(ids::encounter::LAGAVULIN_MATRIARCH_BOSS, ids::monster::LAGAVULIN_MATRIARCH), (ids::encounter::SLUMBERING_BEETLE_NORMAL, ids::monster::SLUMBERING_BEETLE)] {
        let mut cx = Combat::new(&scenario(enc, 3));
        let e = *cx.enemies.iter().find(|&&e| cx.cr(e).monster.id == mid).unwrap();
        let rows = cx.lookahead(e);
        // what the monster really does over the next turns when the player passes
        let mut real = vec![];
        for _ in 0..LOOK_H {
            cx.step(Action::EndTurn);
            real.push(cx.cr(e).monster.next_move);
        }
        for (h, &nm) in real.iter().enumerate() {
            assert_eq!(rows[h].prob[nm as usize], 1.0, "{}: turn +{} is node {nm}, look-ahead {:?}", ids::monster::NAMES[mid as usize], h + 1, rows[h].prob);
        }
        assert!(real.windows(2).any(|w| w[0] != w[1]), "{}: the fight never left the sleep move", ids::monster::NAMES[mid as usize]);
    }
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

/// Calibration of the look-ahead against the simulator's own outcomes. Over many fights per encounter (different RNG seeds), every
/// prediction made at the start of a player turn for turn +h is compared with the move the monster then actually has: per monster,
/// horizon and move node, the observed count must match the predicted count (sum of predicted probabilities) within the binomial
/// spread, and moves predicted impossible must not happen. `passive`: the player only ends turns (with a huge HP pool), which is
/// exactly what the projection assumes, so the odds must be right; otherwise random play, where the player changes what conditional
/// branches see (HP thresholds, kills, wake-ups), and only the overall calibration is bounded.
struct Calib {
    /// (monster id, horizon, node slot) -> (predicted count, sum of p(1-p), observed count)
    cells: std::collections::BTreeMap<(u16, usize, usize), (f64, f64, u32)>,
    /// predicted-probability decile -> (sum of p, observed count, samples)
    bins: [(f64, f64, u32); 10],
    /// realized moves that had been given probability 0: (monster id, horizon) -> count
    zero: std::collections::BTreeMap<(u16, usize), u32>,
    samples: u32,
}

fn calibrate(passive: bool, seeds: u64, turns: i32) -> Calib {
    let mut cal = Calib { cells: Default::default(), bins: [(0.0, 0.0, 0); 10], zero: Default::default(), samples: 0 };
    for enc in 0..ids::encounter::COUNT as u16 {
        if !content::encounter_implemented(enc) {
            continue;
        }
        for seed in 0..seeds {
            let mut sc = scenario(enc, 1000 + seed * 7919 + enc as u64);
            if passive {
                (sc.hp, sc.max_hp) = (1_000_000, 1_000_000);
            }
            let mut cx = Combat::new(&sc);
            let mut rng = Rng::new(seed * 31 + enc as u64);
            let mut buf = ActionBuf::new();
            // (turn made, creature, monster id, rows)
            let mut pred: Vec<(i32, Cid, u16, [sts2sim::engine::LookRow; LOOK_H])> = vec![];
            let mut last_turn = 0;
            for _ in 0..2000 {
                if cx.stage == Stage::Over || cx.missing.is_some() || cx.player.turn_number > turns {
                    break;
                }
                if cx.stage == Stage::AwaitAction && cx.player.phase == Phase::Play && cx.player.turn_number != last_turn {
                    last_turn = cx.player.turn_number;
                    for &(t0, e, mid, ref rows) in pred.iter() {
                        let h = (last_turn - t0 - 1) as usize;
                        let cr = cx.cr(e);
                        if h >= LOOK_H || !cr.in_combat || !cr.is_alive() || cr.monster.id != mid {
                            continue;
                        }
                        let nm = cr.monster.next_move;
                        let slot = if nm == STUN_NODE_FOR_TEST || nm as usize >= LOOK_NODES - 1 { LOOK_NODES - 1 } else { nm as usize };
                        // a stun the player caused cannot be foreseen (random play only: a passive player stuns nothing)
                        if !passive && nm == STUN_NODE_FOR_TEST {
                            continue;
                        }
                        let row = &rows[h];
                        let mass: f32 = row.prob.iter().sum();
                        if mass < 0.999 {
                            continue; // the projection lost this monster (it was projected to die / the fight to end): no claim
                        }
                        cal.samples += 1;
                        if row.prob[slot] <= 0.0 {
                            *cal.zero.entry((mid, h)).or_default() += 1;
                        }
                        for (k, &p) in row.prob.iter().enumerate() {
                            let p = p as f64;
                            if p > 0.0 || k == slot {
                                let c = cal.cells.entry((mid, h, k)).or_default();
                                c.0 += p;
                                c.1 += p * (1.0 - p);
                                c.2 += (k == slot) as u32;
                            }
                            if p > 0.0 {
                                let b = &mut cal.bins[((p * 10.0) as usize).min(9)];
                                b.0 += p;
                                b.1 += (k == slot) as u32 as f64;
                                b.2 += 1;
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
                let a = if passive && buf.contains(Action::EndTurn) { Action::EndTurn } else { buf[rng.next_int(buf.len() as i32) as usize] };
                cx.step(a);
            }
        }
    }
    cal
}

/// The cells whose observed count is furthest from the predicted one: (z, monster, horizon, slot, predicted, observed).
fn worst_cells(cal: &Calib, min_expected: f64) -> Vec<(f64, String, usize, usize, f64, u32)> {
    let mut v: Vec<_> = cal
        .cells
        .iter()
        .filter(|(_, c)| c.0 >= min_expected || c.2 as f64 >= min_expected)
        .map(|(&(m, h, k), &(e, var, o))| ((o as f64 - e) / (var + 1.0).sqrt(), ids::monster::NAMES[m as usize].to_string(), h + 1, k, e, o))
        .collect();
    v.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap());
    v
}

fn report(name: &str, cal: &Calib) -> Vec<(f64, String, usize, usize, f64, u32)> {
    let zeros: u32 = cal.zero.values().sum();
    eprintln!("{name}: {} predictions, {zeros} realized moves predicted impossible {:?}", cal.samples, cal.zero);
    let worst = worst_cells(cal, 10.0);
    eprintln!("worst cells (z, monster, turn, node, predicted, observed): {:?}", &worst[..worst.len().min(10)]);
    for (i, b) in cal.bins.iter().enumerate() {
        if b.2 > 0 {
            eprintln!("  p in [{:.1}, {:.1}): mean predicted {:.3}, observed {:.3} ({} samples)", i as f64 / 10.0, (i + 1) as f64 / 10.0, b.0 / b.2 as f64, b.1 / b.2 as f64, b.2);
        }
    }
    worst
}

#[test]
fn lookahead_odds_match_passive_play() {
    let cal = calibrate(true, 24, 14);
    let worst = report("passive play", &cal);
    let zeros: u32 = cal.zero.values().sum();
    assert!(cal.samples > 20_000, "only {} predictions", cal.samples);
    assert!(zeros as f64 <= 0.001 * cal.samples as f64, "{zeros} realized moves had probability 0: {:?}", cal.zero);
    let bad: Vec<_> = worst.iter().filter(|w| w.0.abs() > 5.0).collect();
    assert!(bad.is_empty(), "observed move counts outside 5 sigma of the predicted odds: {bad:?}");
    for b in cal.bins.iter().filter(|b| b.2 >= 200) {
        assert!((b.0 - b.1).abs() / b.2 as f64 <= 0.02, "decile calibration off: {b:?}");
    }
}

#[test]
fn lookahead_is_calibrated_under_random_play() {
    let cal = calibrate(false, 6, 14);
    report("random play", &cal);
    let zeros: u32 = cal.zero.values().sum();
    assert!(cal.samples > 5_000, "only {} predictions", cal.samples);
    // the player can force branches the status-quo projection does not take (damage thresholds, kills, wake-ups, stuns): bounded,
    // not exact
    assert!(zeros as f64 <= 0.03 * cal.samples as f64, "{zeros} of {} realized moves had probability 0: {:?}", cal.samples, cal.zero);
    for b in cal.bins.iter().filter(|b| b.2 >= 200) {
        assert!((b.0 - b.1).abs() / b.2 as f64 <= 0.08, "decile calibration off: {b:?}");
    }
}

/// The same measurements for the look-ahead from before S1 (`LOOK_LEGACY`; it covers 3 turns, so later turns are not scored).
/// Report only: `cargo test --release -p sts2sim --test lookahead -- --ignored --nocapture`.
#[test]
#[ignore]
fn legacy_lookahead_calibration_report() {
    sts2sim::engine::LOOK_LEGACY.store(true, std::sync::atomic::Ordering::Relaxed);
    report("legacy, passive play", &calibrate(true, 24, 14));
    report("legacy, random play", &calibrate(false, 6, 14));
    sts2sim::engine::LOOK_LEGACY.store(false, std::sync::atomic::Ordering::Relaxed);
}

const STUN_NODE_FOR_TEST: u8 = 0xFE;

/// The look-ahead cache never changes a row: cached == fresh for every enemy of every encounter over random play (states revisited
/// in different orders, enemies of one combat looked at one after the other, as `observe` does).
#[test]
fn cached_lookahead_equals_fresh_every_encounter() {
    let (mut checked, mut diffs) = (0u64, vec![]);
    for enc in 0..ids::encounter::COUNT as u16 {
        if !content::encounter_implemented(enc) {
            continue;
        }
        for seed in 0..3u64 {
            let mut cx = Combat::new(&scenario(enc, 50 + seed));
            let mut rng = Rng::new(seed ^ enc as u64);
            let mut buf = ActionBuf::new();
            for _ in 0..60 {
                if cx.stage == Stage::Over || cx.missing.is_some() {
                    break;
                }
                for &e in cx.enemies.clone().iter() {
                    let (a, b) = (cx.lookahead(e), cx.lookahead_fresh(e));
                    checked += 1;
                    if (0..LOOK_H).any(|h| a[h].prob != b[h].prob || a[h].exp_damage != b[h].exp_damage) {
                        diffs.push(ids::encounter::NAMES[enc as usize]);
                    }
                }
                cx.legal_actions(&mut buf);
                cx.step(buf[rng.next_int(buf.len() as i32) as usize]);
            }
        }
    }
    assert!(checked > 5_000, "only {checked} look-aheads checked");
    assert!(diffs.is_empty(), "{} of {checked} cached look-aheads differ: {:?}", diffs.len(), &diffs[..diffs.len().min(10)]);
}
