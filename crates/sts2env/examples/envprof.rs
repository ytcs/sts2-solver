//! Throughput of the PPO env (`BatchEnv::step`: one simulator step + observation + mask per env) on a scenario pool, with a stand-in policy whose
//! choice depends only on the observation row (a hash picks a legal action), so a run is reproducible whatever the thread count.
//!
//!   cargo run --release -p sts2env --example envprof -- target/train/train.json [envs 1024] [steps 200] [obs version 1] [scenarios 4000]
//!
//! Prints the time inside `step` (steps/s) and a checksum over every observation, mask, reward and outcome: an optimisation of the env, the
//! simulator or the observation that keeps the env identical keeps the checksum. Threads: `RAYON_NUM_THREADS` (default: all cores).
//! With `--features sts2sim/obs_prof` (and `RAYON_NUM_THREADS=1`) it also splits the observation and counts look-ahead cache lookups / misses.
use std::time::Instant;
use sts2env::*;
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // STS2_LOOK_VERIFY=1: every look-ahead cache hit is checked against a fresh projection (slow; panics on a difference)
    let verify = std::env::var("STS2_LOOK_VERIFY").is_ok_and(|v| v == "1");
    sts2sim::engine::LOOK_VERIFY.store(verify, std::sync::atomic::Ordering::Relaxed);
    let path = args.get(1).expect("scenario json (a list of scenarios)");
    let arg = |i: usize, d: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (n, steps, ver, n_scen) = (arg(2, 1024), arg(3, 200), arg(4, 1) as u8, arg(5, 4000));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut scen = vec![];
    for sj in v.as_array().unwrap() {
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_ok() {
            scen.push((sc, ex));
        }
        if scen.len() == n_scen {
            break;
        }
    }
    let cfg = RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.5, step: 0.0, turn_cap: 30 };
    let mut env = BatchEnv::try_new(n, Box::new(PoolScenario::with_extras(scen)), cfg, 600, 1000).unwrap();
    env.set_obs_version(ver).expect("observation version 1 or 2");
    let osz = env.obs_size();
    let (mut obs, mut mask) = (vec![0f32; n * osz], vec![0u8; n * ACTION_SPACE]);
    let (mut rew, mut done, mut oc, mut ill) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    env.observe_all(&mut obs, &mut mask).unwrap();
    let mut acts = vec![0i32; n];
    let mut h = 0xcbf29ce484222325u64;
    let mut mix = |h: &mut u64, x: u64| *h = (*h ^ x).wrapping_mul(0x100000001b3);
    let (mut t_step, mut eps, mut wins) = (0.0, 0u64, 0u64);
    for _ in 0..steps {
        for i in 0..n {
            let hr = hash(&obs[i * osz..(i + 1) * osz]);
            let m = &mask[i * ACTION_SPACE..(i + 1) * ACTION_SPACE];
            mix(&mut h, hr);
            let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| m[a] > 0).collect();
            mix(&mut h, legal.len() as u64 ^ (legal.iter().fold(0u64, |s, &a| s.wrapping_mul(31).wrapping_add(a as u64)) << 8));
            // no legal action (a finished or broken fight): the env counts the step as illegal and ends the episode
            acts[i] = if legal.is_empty() { 0 } else { legal[((hr >> 17) % legal.len() as u64) as usize] as i32 };
        }
        let t = Instant::now();
        env.step(&acts, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut rew, done: &mut done, outcome: &mut oc, illegal: &mut ill }).unwrap();
        t_step += t.elapsed().as_secs_f64();
        for i in 0..n {
            mix(&mut h, rew[i].to_bits() as u64 ^ (done[i] as u64) << 32 ^ ((oc[i] as u8) as u64) << 40 ^ (ill[i] as u64) << 48);
            eps += done[i] as u64;
            wins += (oc[i] == OUTCOME_WIN) as u64;
        }
    }
    let total = (n * steps) as f64;
    println!(
        "{n} envs x {steps} steps, obs v{ver}, {} threads: step {t_step:.2}s = {:.0} steps/s ({:.2} us per env step x threads); episodes {eps}, wins {wins}; checksum {h:016x}",
        rayon::current_num_threads(),
        total / t_step,
        t_step * rayon::current_num_threads() as f64 / total * 1e6
    );
    if verify {
        println!("  verified {} look-ahead cache hits against fresh projections", sts2sim::engine::LOOK_VERIFIED.load(std::sync::atomic::Ordering::Relaxed));
    }
    #[cfg(feature = "obs_prof")]
    unsafe {
        let p = sts2sim::observe::OBS_PROF;
        let tot: u64 = p[1..=5].iter().sum();
        for (name, k) in [("hand cards", 1), ("piles", 2), ("enemies (intents)", 3), ("lookahead", 4), ("player+relics", 5), ("  look key", 10), ("  look paths/cache", 11)] {
            println!("  {name:<20} {:8.0} cycles/row  {:5.1}% of the profiled sections", p[k] as f64 / total, 100.0 * p[k] as f64 / tot as f64);
        }
        println!("  look-ahead cache: {} lookups, {} misses", p[12], p[13]);
        let np = p[19].max(1) as f64;
        println!("  per projection ({} projections, {:.2} turned paths, {:.2} forked boxes each):", p[19], p[21] as f64 / np, p[20] as f64 / np);
        for (name, k) in [("base copy", 14), ("look_turn", 15), ("  player end + enemy start", 22), ("  enemy moves", 23), ("  enemy turn end", 24),
                          ("  next player turn start", 25), ("rolls (+fork copies)", 16), ("merge", 17), ("intent damage", 18)] {
            println!("    {name:<22} {:8.0} cycles", p[k] as f64 / np);
        }
    }
}
