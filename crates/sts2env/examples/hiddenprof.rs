//! Which actions touch hidden information (RNG streams, draw pile order)? Steps every legal action of many mid-fight states on a copy.
//!   cargo run --release -p sts2env --example hiddenprof -- scenarios.json
use std::collections::HashMap;
use sts2sim::engine::ActionBuf;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::{Action, Combat};

fn sig(cx: &Combat) -> (i64, u64) {
    let r = &cx.rng;
    let c = r.shuffle.counter as i64 + r.combat_card_generation.counter as i64 + r.combat_potion_generation.counter as i64 + r.combat_card_selection.counter as i64
        + r.combat_energy_costs.counter as i64 + r.combat_targets.counter as i64 + r.monster_ai.counter as i64 + r.niche.counter as i64 + r.combat_orbs.counter as i64;
    let mut h = 0xcbf29ce484222325u64;
    for &x in cx.player.draw.as_slice() {
        h = (h ^ x as u64).wrapping_mul(0x100000001b3);
    }
    (c, h)
}

fn main() {
    let path = std::env::args().nth(1).expect("scenario json (list)");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut rng = Rng::new(5);
    let names = sts2sim::ids::card::NAMES;
    let mut by: HashMap<String, (u64, u64)> = HashMap::new();
    for (i, sj) in v.as_array().unwrap().iter().enumerate().take(400) {
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_err() {
            continue;
        }
        let mut cx = Combat::try_new_with(&sc, &ex).unwrap();
        cx.reset_validated(&sc, &ex, i as u64 + 1, RngSet::from_run_seed_fast(i as u64 + 1)).unwrap();
        for step in 0..100 {
            let mut buf = ActionBuf::new();
            cx.legal_actions(&mut buf);
            if buf.is_empty() {
                break;
            }
            if step % 4 == 1 {
                for &a in buf.iter() {
                    let mut c2 = cx.clone();
                    let s0 = sig(&c2);
                    if !c2.step(a) {
                        continue;
                    }
                    let key = match a {
                        Action::EndTurn => "EndTurn".to_string(),
                        Action::PlayCard { hand_pos, .. } => format!("play {}", names[cx.cards[cx.player.hand.get(hand_pos as usize).unwrap() as usize].id as usize]),
                        Action::UsePotion { .. } => "potion".into(),
                        Action::Pick { .. } => "pick".into(),
                        Action::Confirm => "confirm".into(),
                        _ => "other".into(),
                    };
                    let e = by.entry(key).or_default();
                    e.0 += 1;
                    e.1 += (sig(&c2) != s0) as u64;
                }
            }
            let a = buf[rng.next_int(buf.len() as i32) as usize];
            if !cx.step(a) {
                break;
            }
        }
    }
    let mut rows: Vec<_> = by.into_iter().collect();
    rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    let (tn, tc): (u64, u64) = rows.iter().fold((0, 0), |a, (_, (n, c))| (a.0 + n, a.1 + c));
    println!("all actions: {tn}, touching hidden info: {tc} ({:.1}%)", 100.0 * tc as f64 / tn as f64);
    for (k, (n, c)) in rows.iter().take(40) {
        println!("{k:36} {n:7} {:5.1}% touch hidden info", 100.0 * *c as f64 / *n as f64);
    }
}
