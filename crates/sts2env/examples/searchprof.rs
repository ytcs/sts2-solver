//! CPU cost of the search engine (`sts2env::search`) without a network: the collection's search shape (top-M root 5 x 32, play-outs of 2 player turns,
//! `lead`, `strat`, `carry`, outcome-head value rows, `record`) on real scenarios, with a stand-in network whose answers depend only on the request (a hash
//! of the observation picks the play-out move, ranks the options and sets the value), so the run is reproducible whatever the thread count.
//!
//!   cargo run --release -p sts2env --example searchprof -- data/bench/mix.json [fights 128] [threads 1] [roots 64] [m 5] [k 32] [obs version 1]
//!
//! Prints the time inside `advance` (the engine), the engine's cycle counters, and a checksum over the results and every recorded move: an optimisation
//! of the engine or the simulator that keeps the search identical keeps the checksum. With `--features sts2sim/obs_prof` it also splits `observe_ex`.
use std::time::Instant;
use sts2env::search::*;
use sts2sim::engine::ACTION_SPACE;

fn hash(o: &[f32]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for (i, x) in o.iter().enumerate() {
        if x.to_bits() != 0 {
            h = (h ^ x.to_bits() as u64 ^ (i as u64) << 32).wrapping_mul(0x100000001b3);
        }
    }
    h ^ (h >> 29)
}

/// A policy row: the legal actions ranked by a hash of (observation, action), probabilities from that rank, a play-out move picked by the row's uniform.
fn answer(obs: &[f32], mask: &[u8], u: f32, m: usize, out: &mut [f32]) {
    let h = hash(obs);
    let mut legal: Vec<(u64, usize)> = (0..ACTION_SPACE).filter(|&a| mask[a] > 0).map(|a| ((h ^ a as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 16, a)).collect();
    legal.sort_unstable();
    let n = legal.len();
    let w: Vec<f32> = (0..n).map(|r| 1.0 / (r as f32 + 1.0)).collect();
    let tot: f32 = w.iter().sum();
    for j in 0..m {
        out[j] = legal.get(j).map_or(0, |x| x.1) as f32;
        out[m + j] = if j < n { w[j] / tot } else { 0.0 };
    }
    let mut c = 0.0;
    let mut pick = legal[n - 1].1;
    for (r, x) in legal.iter().enumerate() {
        c += w[r] / tot;
        if u <= c {
            pick = x.1;
            break;
        }
    }
    out[2 * m] = pick as f32;
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("scenario json (a list of scenarios or of {scenario: ...})");
    let arg = |i: usize, d: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (n_fights, threads, roots, m, k, ver) = (arg(2, 128), arg(3, 1), arg(4, 64), arg(5, 5), arg(6, 32), arg(7, 1) as u8);
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut scen = vec![];
    for sj in v.as_array().unwrap() {
        let sj = sj.get("scenario").filter(|_| sj.get("deck").is_none()).unwrap_or(sj);
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_ok() {
            scen.push((sc, ex));
        }
        if scen.len() == n_fights {
            break;
        }
    }
    let jobs: Vec<(u32, u64)> = (0..scen.len()).map(|i| (i as u32, 101 * 1_000_003 + i as u64)).collect();
    let cfg = SearchCfg { m, k, roll_cap: 120, leaf_turns: 2, turn_cap: 30, val_w: HEAD_NC, ..SearchCfg::default() };
    let mut eng = SearchEngine::new(scen, jobs.clone(), roots, cfg, threads, true).unwrap();
    eng.set_obs_version(ver).expect("observation version 1 or 2");
    #[allow(non_snake_case)]
    let OBS_SIZE = eng.obs_size();
    let cap = eng.shared_rows();
    let (mut obs, mut mask, mut pk, mut pu, mut vk) = (vec![0f32; cap * OBS_SIZE], vec![0u8; cap * ACTION_SPACE], vec![0u8; cap], vec![0f32; cap], vec![0u8; cap]);
    let stride = 2 * m + 1;
    let (mut pol, mut val) = (vec![0f32; cap * stride], vec![0f32; cap * HEAD_NC]);
    let t_all = Instant::now();
    let mut t_eng = 0.0;
    let t = Instant::now();
    let (mut np, mut nv) = eng.advance_shared(None, None, &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
    t_eng += t.elapsed().as_secs_f64();
    let mut cycles = 0;
    while np + nv > 0 {
        for r in 0..np {
            answer(&obs[r * OBS_SIZE..(r + 1) * OBS_SIZE], &mask[r * ACTION_SPACE..(r + 1) * ACTION_SPACE], pu[r], m, &mut pol[r * stride..(r + 1) * stride]);
        }
        for r in 0..nv {
            let row = cap - 1 - r;
            let h = hash(&obs[row * OBS_SIZE..(row + 1) * OBS_SIZE]);
            // a distribution over the outcome classes: loss with probability from the hash, the rest spread over a few win bins
            let pl = (h % 1000) as f32 / 1000.0;
            let p = &mut val[r * HEAD_NC..(r + 1) * HEAD_NC];
            p.fill(0.0);
            p[0] = pl;
            for b in 0..4 {
                p[1 + ((h >> (10 + 8 * b)) % (HEAD_NC as u64 - 1)) as usize] += (1.0 - pl) / 4.0;
            }
        }
        let t = Instant::now();
        (np, nv) = eng.advance_shared(Some(&pol[..np * stride]), Some(&val[..nv * HEAD_NC]), &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
        t_eng += t.elapsed().as_secs_f64();
        cycles += 1;
    }
    let wall = t_all.elapsed().as_secs_f64();
    let mut h = 0xcbf29ce484222325u64;
    let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x100000001b3);
    let mut wins = 0;
    for (j, r) in eng.results().iter().enumerate() {
        wins += (r.outcome == 1) as u32;
        mix(r.outcome as u64 ^ (r.len as u64) << 8 ^ (r.hp_end_abs as u64) << 32);
        for mv in eng.moves(j) {
            mix(mv.action as u64 | (mv.searched as u64) << 16);
            for x in 0..m {
                mix(mv.opts[x] as u64 ^ (mv.q[x].to_bits() as u64) << 16 ^ (mv.p[x].to_bits() as u64) << 40);
            }
        }
    }
    let st = eng.stats();
    let tot = (st.cy_step + st.cy_obs + st.cy_fork + st.cy_legal + st.cy_main).max(1) as f64;
    println!("{} fights, {threads} threads, {roots} roots, {m}x{k}, obs v{ver}: engine {t_eng:.2}s of {wall:.2}s ({cycles} cycles); wins {wins}; checksum {h:016x}", jobs.len());
    println!(
        "  cycles: step {:.1}% (end turn {:.1}%), obs {:.1}%, fork {:.1}%, legal {:.1}%, main {:.1}%",
        100.0 * st.cy_step as f64 / tot,
        100.0 * st.cy_endturn as f64 / tot,
        100.0 * st.cy_obs as f64 / tot,
        100.0 * st.cy_fork as f64 / tot,
        100.0 * st.cy_legal as f64 / tot,
        100.0 * st.cy_main as f64 / tot
    );
    println!(
        "  sim steps {}, policy rows {}, value rows {}, forks {}, lead branch {} clean {}; per policy row {:.2} us of engine",
        st.sim_steps,
        st.policy_rows,
        st.value_rows,
        st.forks,
        st.lead_branch,
        st.lead_clean,
        t_eng * threads as f64 / st.policy_rows.max(1) as f64 * 1e6
    );
    #[cfg(feature = "obs_prof")]
    unsafe {
        let p = sts2sim::observe::OBS_PROF;
        let tot: u64 = p.iter().sum();
        let n = (st.policy_rows + st.value_rows) as f64;
        for (name, k) in [("hand cards", 1), ("piles", 2), ("enemies (intents)", 3), ("lookahead", 4), ("player+relics", 5), ("  card damage", 6), ("  card block", 7), ("  card cost", 8), ("  card keywords", 9), ("  look key", 10), ("  look paths/cache", 11)] {
            println!("  {name:<20} {:8.0} cycles/row  {:5.1}% of the profiled sections", p[k] as f64 / n, 100.0 * p[k] as f64 / tot as f64);
        }
        println!("  look-ahead cache: {} lookups, {} misses", p[12], p[13]);
    }
}
