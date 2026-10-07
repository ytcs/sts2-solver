//! The search engine as a state machine: with a stand-in "network" (a hash of the observation picks among the legal actions) every job finishes,
//! the bookkeeping is consistent, and the run is reproducible whatever the thread count (answers depend on the request, not on the row order).
use sts2env::search::*;
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::RngSet;
use sts2sim::engine::ACTION_SPACE;
use sts2sim::engine::ActionBuf;
use sts2sim::state::Stage;
use sts2sim::types::Outcome;
use sts2sim::{Action, Combat, DeckCard, RelicInit, Scenario, ScenarioExtras};

fn scenario(n_deck: usize, enc: u16) -> Scenario {
    let mut deck = vec![];
    for i in 0..n_deck {
        deck.push(DeckCard { id: if i % 2 == 0 { ids::card::STRIKE_IRONCLAD } else { ids::card::DEFEND_IRONCLAD }, upgrade: 0 });
    }
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: enc,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(0),
    }
}

fn hash(o: &[f32]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for (i, x) in o.iter().enumerate().step_by(7) {
        h = (h ^ x.to_bits() as u64 ^ i as u64).wrapping_mul(0x100000001b3);
    }
    h
}

/// Answers a policy row: the `m` first legal actions with equal probabilities, a play-out move picked by the hash of the observation.
fn answer(obs: &[f32], mask: &[u8], m: usize, out: &mut [f32]) {
    let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| mask[a] > 0).collect();
    assert!(legal.len() >= 2, "a request is only made for a real decision");
    for j in 0..m {
        out[j] = *legal.get(j).unwrap_or(&0) as f32;
        out[m + j] = if j < legal.len() { 1.0 / m.min(legal.len()) as f32 } else { 0.0 };
    }
    out[2 * m] = legal[(hash(obs) % legal.len() as u64) as usize] as f32;
}

fn run(threads: usize, n_roots: usize, jobs: Vec<(u32, u64)>, cfg: SearchCfg) -> (Vec<JobResult>, SearchStats) {
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default()), (scenario(14, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let mut eng = SearchEngine::new(scen, jobs, n_roots, cfg, threads, false).unwrap();
    let (pc, vc) = eng.max_rows();
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; pc * stride], vec![0f32; vc]);
    let (mut np, mut nv) = eng.advance(None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
    let mut cycles = 0;
    while np + nv > 0 {
        for r in 0..np {
            answer(&po[r * OBS_SIZE..(r + 1) * OBS_SIZE], &pm[r * ACTION_SPACE..(r + 1) * ACTION_SPACE], cfg.m, &mut pol[r * stride..(r + 1) * stride]);
        }
        for r in 0..nv {
            val[r] = (hash(&vo[r * OBS_SIZE..(r + 1) * OBS_SIZE]) % 1000) as f32 / 2000.0 - 0.25;
        }
        let pa = pol[..np * stride].to_vec();
        let va = val[..nv].to_vec();
        (np, nv) = eng.advance(Some(&pa), Some(&va), &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
        cycles += 1;
        assert!(cycles < 100_000, "the engine does not terminate");
    }
    assert!(eng.finished());
    (eng.results().to_vec(), eng.stats())
}

fn cfg() -> SearchCfg {
    SearchCfg { m: 3, k: 4, conf: 1.01, pmin: 0.0, margin: 0.0, roll_cap: 60, leaf_turns: 1, lead: false, carry: false, strat: false, max_steps: 300, win: 1.0, loss: -1.0, hp_bonus: 0.5, util: [0.0; 102], use_util: false, turn_cap: 0, val_w: 1, ..SearchCfg::default() }
}

#[test]
fn every_job_finishes_and_the_run_is_reproducible() {
    let jobs: Vec<(u32, u64)> = (0..24).map(|i| ((i % 2) as u32, 1000 + i as u64)).collect();
    let (r1, s1) = run(2, 5, jobs.clone(), cfg());
    assert_eq!(r1.len(), 24);
    for (i, r) in r1.iter().enumerate() {
        assert!(r.done, "job {i} not finished");
        assert_eq!(r.scen as usize, i % 2);
        assert!(matches!(r.outcome, 1 | -1 | 2), "outcome {} of job {i}", r.outcome);
        assert!(r.hp_lost >= -1e-6 && r.hp_lost <= 1.0 + 1e-6, "hp_lost {}", r.hp_lost);
        assert!(r.len > 0);
    }
    assert_eq!(s1.illegal, 0);
    assert!(s1.searched > 0 && s1.policy_rows > 0 && s1.value_rows > 0 && s1.forks >= s1.searched);
    // thread count and the number of fights in flight change the schedule, not the answers
    let (r2, _) = run(1, 24, jobs.clone(), cfg());
    let (r3, _) = run(4, 3, jobs, cfg());
    for i in 0..24 {
        for r in [&r2[i], &r3[i]] {
            assert_eq!((r.outcome, r.len), (r1[i].outcome, r1[i].len), "job {i}");
            assert!((r.hp_lost - r1[i].hp_lost).abs() < 1e-6);
        }
    }
}

#[test]
fn confident_policy_skips_the_search() {
    let jobs: Vec<(u32, u64)> = (0..6).map(|i| (0u32, 7 + i as u64)).collect();
    let mut c = cfg();
    c.conf = 0.0; // the top action always counts as confident
    let (r, s) = run(2, 3, jobs, c);
    assert!(r.iter().all(|x| x.done));
    assert_eq!((s.searched, s.forks, s.value_rows), (0, 0, 0));
}

#[test]
fn shared_prefix_search_finishes_reproducibly_and_does_less_work() {
    let jobs: Vec<(u32, u64)> = (0..24).map(|i| ((i % 2) as u32, 500 + i as u64)).collect();
    let mut c = cfg();
    c.lead = true;
    let (r1, s1) = run(2, 5, jobs.clone(), c);
    assert!(r1.iter().all(|x| x.done && matches!(x.outcome, 1 | -1 | 2)));
    assert_eq!(s1.illegal, 0);
    assert!(s1.lead_branch + s1.lead_clean > 0);
    let (r2, _) = run(1, 24, jobs.clone(), c);
    for i in 0..24 {
        assert_eq!((r1[i].outcome, r1[i].len), (r2[i].outcome, r2[i].len), "job {i}");
    }
    let (_, s0) = run(2, 5, jobs, cfg());
    assert!(s1.sim_steps < s0.sim_steps, "{} vs {}", s1.sim_steps, s0.sim_steps);
}

#[test]
fn carried_lines_finish_and_save_searches() {
    let jobs: Vec<(u32, u64)> = (0..24).map(|i| ((i % 2) as u32, 900 + i as u64)).collect();
    let mut c = cfg();
    c.lead = true;
    let (_, s0) = run(2, 5, jobs.clone(), c);
    c.carry = true;
    let (r, s1) = run(2, 5, jobs.clone(), c);
    assert!(r.iter().all(|x| x.done && matches!(x.outcome, 1 | -1 | 2)));
    let (r2, _) = run(1, 24, jobs, c);
    for i in 0..24 {
        assert_eq!((r[i].outcome, r[i].len), (r2[i].outcome, r2[i].len), "job {i}");
    }
    assert!(s1.carried > 0 && s1.forks < s0.forks, "{} carried, forks {} vs {}", s1.carried, s1.forks, s0.forks);
}

/// `run` through `advance_shared`: one observation buffer, value rows written from its end backwards.
fn run_shared(threads: usize, n_roots: usize, jobs: Vec<(u32, u64)>, cfg: SearchCfg) -> (Vec<JobResult>, SearchStats, usize) {
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default()), (scenario(14, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let mut eng = SearchEngine::new(scen, jobs, n_roots, cfg, threads, false).unwrap();
    let cap = eng.shared_rows();
    let (pc, vc) = eng.max_rows();
    assert!(cap < pc + vc);
    let (mut ob, mut pm, mut pk, mut pu, mut vk) = (vec![0f32; cap * OBS_SIZE], vec![0u8; cap * ACTION_SPACE], vec![0u8; cap], vec![0f32; cap], vec![0u8; cap]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; cap * stride], vec![0f32; cap]);
    let (mut np, mut nv) = eng.advance_shared(None, None, &mut ob, &mut pm, &mut pk, &mut pu, &mut vk).unwrap();
    let (mut cycles, mut peak) = (0, 0);
    while np + nv > 0 {
        assert!(np + nv <= cap);
        peak = peak.max(np + nv);
        for r in 0..np {
            answer(&ob[r * OBS_SIZE..(r + 1) * OBS_SIZE], &pm[r * ACTION_SPACE..(r + 1) * ACTION_SPACE], cfg.m, &mut pol[r * stride..(r + 1) * stride]);
        }
        for r in 0..nv {
            let row = cap - 1 - r;
            val[r] = (hash(&ob[row * OBS_SIZE..(row + 1) * OBS_SIZE]) % 1000) as f32 / 2000.0 - 0.25;
        }
        let pa = pol[..np * stride].to_vec();
        let va = val[..nv].to_vec();
        (np, nv) = eng.advance_shared(Some(&pa), Some(&va), &mut ob, &mut pm, &mut pk, &mut pu, &mut vk).unwrap();
        cycles += 1;
        assert!(cycles < 100_000, "the engine does not terminate");
    }
    assert!(eng.finished());
    (eng.results().to_vec(), eng.stats(), peak)
}

#[test]
fn shared_request_buffer_matches_separate_buffers() {
    let jobs: Vec<(u32, u64)> = (0..24).map(|i| ((i % 2) as u32, 900 + i as u64)).collect();
    let mut c = cfg();
    for (lead, carry, strat) in [(false, false, false), (true, true, true)] {
        c.lead = lead;
        c.carry = carry;
        c.strat = strat;
        let (r1, s1) = run(3, 6, jobs.clone(), c);
        let (r2, s2, peak) = run_shared(3, 6, jobs.clone(), c);
        assert!(peak > 0);
        for i in 0..24 {
            assert_eq!((r1[i].outcome, r1[i].len), (r2[i].outcome, r2[i].len), "job {i}");
            assert_eq!(r1[i].hp_lost.to_bits(), r2[i].hp_lost.to_bits(), "job {i}");
        }
        assert_eq!((s1.policy_rows, s1.value_rows, s1.sim_steps, s1.searched), (s2.policy_rows, s2.value_rows, s2.sim_steps, s2.searched));
    }
}

// ---- Gumbel root (Gumbel-top-k candidates + sequential halving) ----

/// Root logits of the stand-in network: a hash of (observation, action) for every legal action, the network's -1e9 mask elsewhere.
fn root_logits(obs: &[f32], mask: &[u8], out: &mut [f32]) {
    let h = hash(obs);
    for a in 0..ACTION_SPACE {
        out[a] = if mask[a] > 0 { ((h ^ (a as u64).wrapping_mul(0x9E3779B97F4A7C15)) % 4000) as f32 / 1000.0 } else { -1e9 };
    }
}

/// `run` in Gumbel mode: every real-fight policy row (`pol_kind & 1 == 0`) is also answered with root logits. Returns the recorded moves too.
fn run_gumbel(threads: usize, n_roots: usize, jobs: Vec<(u32, u64)>, cfg: SearchCfg, shared: bool) -> (Vec<JobResult>, SearchStats, Vec<Vec<MoveRec>>) {
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default()), (scenario(14, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let nj = jobs.len();
    let mut eng = SearchEngine::new(scen, jobs, n_roots, cfg, threads, true).unwrap();
    let cap = eng.shared_rows();
    let (pc, vc) = if shared { (cap, cap) } else { eng.max_rows() };
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; pc * stride], vec![0f32; vc]);
    #[allow(clippy::too_many_arguments)]
    fn step(eng: &mut SearchEngine, shared: bool, pa: Option<&[f32]>, va: Option<&[f32]>, ra: Option<&[f32]>, po: &mut [f32], pm: &mut [u8], pk: &mut [u8], pu: &mut [f32], vo: &mut [f32], vk: &mut [u8]) -> (usize, usize) {
        if shared {
            eng.advance_shared_root(pa, va, ra, po, pm, pk, pu, vk).unwrap()
        } else {
            eng.advance_root(pa, va, ra, po, pm, pk, pu, vo, vk).unwrap()
        }
    }
    let (mut np, mut nv) = step(&mut eng, shared, None, None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk);
    let mut cycles = 0;
    while np + nv > 0 {
        let mut root = Vec::new();
        for r in 0..np {
            let (o, m) = (&po[r * OBS_SIZE..(r + 1) * OBS_SIZE], &pm[r * ACTION_SPACE..(r + 1) * ACTION_SPACE]);
            answer(o, m, cfg.m, &mut pol[r * stride..(r + 1) * stride]);
            if pk[r] & 1 == 0 {
                let mut l = vec![0f32; ACTION_SPACE];
                root_logits(o, m, &mut l);
                root.extend(l);
            }
        }
        for r in 0..nv {
            let o = if shared { &po[(cap - 1 - r) * OBS_SIZE..(cap - r) * OBS_SIZE] } else { &vo[r * OBS_SIZE..(r + 1) * OBS_SIZE] };
            val[r] = (hash(o) % 1000) as f32 / 2000.0 - 0.25;
        }
        let pa = pol[..np * stride].to_vec();
        let va = val[..nv].to_vec();
        (np, nv) = step(&mut eng, shared, Some(&pa), Some(&va), Some(&root), &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk);
        cycles += 1;
        assert!(cycles < 100_000, "the engine does not terminate");
    }
    assert!(eng.finished());
    let moves = (0..nj).map(|j| eng.moves(j).to_vec()).collect();
    (eng.results().to_vec(), eng.stats(), moves)
}

fn gcfg() -> SearchCfg {
    SearchCfg { m: 3, k: 4, root: RootMode::Gumbel, gm: 8, gn: 24, leaf_turns: 1, roll_cap: 60, lead: false, carry: false, strat: false, ..SearchCfg::default() }
}

#[test]
fn halving_plan_spends_the_budget_in_ceil_log2_phases() {
    assert_eq!(halving_plan(16, 160), vec![(16, 2), (8, 5), (4, 10), (2, 24)]);
    assert_eq!(halving_plan(2, 160), vec![(2, 80)]);
    assert_eq!(halving_plan(5, 160), vec![(5, 10), (3, 17), (2, 29)]);
    assert!(halving_plan(1, 160).is_empty());
    for m in 2..=MAX_M {
        for n in [m, 2 * m, 40, 160, 640] {
            let p = halving_plan(m, n);
            assert_eq!(p.len(), (m as f64).log2().ceil() as usize, "m {m}");
            assert_eq!(p[0].0, m);
            assert_eq!(p.last().unwrap().0, 2);
            assert!(p.windows(2).all(|w| w[1].0 == w[0].0.div_ceil(2)));
            let used: usize = p.iter().map(|&(s, v)| s * v).sum();
            if n >= m * p.len() {
                assert!(used <= n && used + 2 > n, "m {m} n {n}: {used}");
            }
        }
    }
    // the slot pool holds the largest phase of any candidate count
    let c = SearchCfg { root: RootMode::Gumbel, gm: 16, gn: 160, ..SearchCfg::default() };
    assert_eq!(c.slots(), 160);
    assert_eq!(SearchCfg { root: RootMode::TopM, m: 5, k: 32, ..SearchCfg::default() }.slots(), 160);
}

#[test]
fn gumbel_top_1_samples_the_softmax() {
    // Gumbel-max: argmax(g + logits) is a sample of softmax(logits)
    let logits = [0.0f32, 1.0, -1.0, 0.5];
    let z: f32 = logits.iter().map(|x| x.exp()).sum();
    let mut s = 12345u64;
    let mut cnt = [0usize; 4];
    let n = 40_000;
    for _ in 0..n {
        let g: Vec<f32> = logits
            .iter()
            .map(|&l| {
                s = s.wrapping_add(0x9E3779B97F4A7C15);
                let mut x = s;
                x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
                x ^= x >> 31;
                gumbel(((x >> 40) as f32 + 0.5) / (1u64 << 24) as f32) + l
            })
            .collect();
        cnt[top_k(&g, 1)[0]] += 1;
    }
    for i in 0..4 {
        let (f, p) = (cnt[i] as f32 / n as f32, logits[i].exp() / z);
        assert!((f - p).abs() < 0.01, "action {i}: {f} vs {p}");
    }
    assert_eq!(top_k(&[0.5, 2.0, 2.0, -1.0], 3), vec![1, 2, 0]);
}

/// Sequential halving with a fixed fake value (each future of candidate j returns q[j]): the candidate with the best value is kept through every phase and
/// played even with the lowest prior, when the value gap outweighs the prior gap on the sigma scale; with equal values the prior (g + logits) decides.
#[test]
fn sequential_halving_keeps_the_best_under_a_fixed_fake_value() {
    let sig = Sigma { lo: -1.0, hi: 1.5, c_visit: 50.0, c_scale: 0.1 };
    for (m, n) in [(16usize, 160usize), (8, 64), (5, 40), (2, 10), (3, 9)] {
        for best in 0..m {
            // g + logits decreasing with j: candidate m - 1 has the lowest prior
            let gl: Vec<f32> = (0..m).map(|j| 1.0 - 0.1 * j as f32).collect(); // a prior gap of up to 1.5 vs a value gap worth >= 3.1 on the sigma scale
            let q: Vec<f32> = (0..m).map(|j| if j == best { 1.0 } else { -0.5 + 0.01 * j as f32 }).collect();
            let mut h = Halving::new(gl, n);
            loop {
                let (v, _) = h.phase_futures();
                for j in 0..m {
                    if h.alive[j] {
                        h.add(j, q[j] * v as f32, v as u32);
                    }
                }
                if !h.next_phase(&sig) {
                    break;
                }
                assert!(h.alive[best], "m {m} best {best}: eliminated after phase {}", h.phase);
                assert_eq!(h.alive.iter().filter(|&&a| a).count(), h.plan[h.phase].0);
            }
            assert_eq!(h.best(&sig), best, "m {m} n {n}");
            // the last survivors have the most futures, and the futures add up to the plan
            assert_eq!(h.n[best], h.max_n());
            assert_eq!(h.n.iter().map(|&x| x as usize).sum::<usize>(), h.plan.iter().map(|&(s, v)| s * v).sum::<usize>());
        }
        // equal values: the best g + logits wins
        let gl: Vec<f32> = (0..m).map(|j| (j as f32 * 0.37).sin()).collect();
        let mut h = Halving::new(gl.clone(), n);
        loop {
            let (v, _) = h.phase_futures();
            for j in 0..m {
                if h.alive[j] {
                    h.add(j, 0.2 * v as f32, v as u32);
                }
            }
            if !h.next_phase(&sig) {
                break;
            }
        }
        assert_eq!(h.best(&sig), top_k(&gl, 1)[0]);
    }
}

#[test]
fn gumbel_search_finishes_reproducibly_and_records_the_improved_policy() {
    let jobs: Vec<(u32, u64)> = (0..16).map(|i| ((i % 2) as u32, 3000 + i as u64)).collect();
    for (lead, strat) in [(false, false), (true, true)] {
        let mut c = gcfg();
        c.lead = lead;
        c.strat = strat;
        let (r1, s1, m1) = run_gumbel(2, 5, jobs.clone(), c, false);
        assert!(r1.iter().all(|x| x.done && matches!(x.outcome, 1 | -1 | 2)));
        assert_eq!(s1.illegal, 0);
        assert!(s1.searched > 0);
        assert_eq!(s1.g_rank.iter().sum::<u64>(), s1.searched);
        // candidates beyond the policy's top-M (m = 3) are tried: the prior's ranking no longer bounds the search
        assert!(s1.g_cand > 3 * s1.searched, "{} candidates over {} searches", s1.g_cand, s1.searched);
        // the schedule (threads, fights in flight, shared buffer) changes nothing: the job seed fixes the Gumbel sample and every future
        for (th, roots, shared) in [(1, 16, false), (4, 3, false), (3, 6, true)] {
            let (r2, s2, m2) = run_gumbel(th, roots, jobs.clone(), c, shared);
            for i in 0..16 {
                assert_eq!((r1[i].outcome, r1[i].len), (r2[i].outcome, r2[i].len), "job {i}");
                assert_eq!(r1[i].hp_lost.to_bits(), r2[i].hp_lost.to_bits(), "job {i}");
                assert_eq!(m1[i].len(), m2[i].len());
                for (a, b) in m1[i].iter().zip(&m2[i]) {
                    assert_eq!((a.action, a.opts, a.n), (b.action, b.opts, b.n));
                    assert_eq!(a.q.map(f32::to_bits), b.q.map(f32::to_bits));
                    assert_eq!(a.pi.map(f32::to_bits), b.pi.map(f32::to_bits));
                }
            }
            assert_eq!((s1.policy_rows, s1.value_rows, s1.sim_steps, s1.g_rank), (s2.policy_rows, s2.value_rows, s2.sim_steps, s2.g_rank));
        }
        let mut searched = 0;
        for mv in m1.iter().flatten().filter(|m| m.searched) {
            searched += 1;
            assert!(mv.gumbel);
            let nc = mv.legal.iter().filter(|&&l| l).count();
            assert!(nc >= 2 && nc <= c.gm);
            let opts = &mv.opts[..nc];
            assert!((1..nc).all(|i| !opts[..i].contains(&opts[i])), "candidates are sampled without replacement: {opts:?}");
            // futures follow the halving plan: every candidate tried, the action played among those with the most futures (the last survivors)
            let plan = halving_plan(nc, c.gn);
            let total: usize = plan.iter().map(|&(s, v)| s * v).sum();
            assert_eq!(mv.n[..nc].iter().map(|&x| x as usize).sum::<usize>(), total);
            let mx = *mv.n[..nc].iter().max().unwrap();
            assert_eq!(mx as usize, plan.iter().map(|p| p.1).sum::<usize>());
            let jp = opts.iter().position(|&o| o == mv.action).expect("the action played is a candidate");
            assert_eq!(mv.n[jp], mx);
            assert!(mv.q[..nc].iter().all(|q| q.is_finite()) && mv.v.is_finite());
            // pi' over the candidates: positive, at most 1 in total, and pi'(a) / p(a) ordered like adv (softmax of logits + adv)
            let s: f32 = mv.pi[..nc].iter().sum();
            assert!(s > 0.0 && s <= 1.0 + 1e-4, "{s}");
            for i in 0..nc {
                for j in 0..nc {
                    if mv.adv[i] > mv.adv[j] + 1e-4 {
                        assert!(mv.pi[i] / mv.p[i] > mv.pi[j] / mv.p[j]);
                    }
                }
            }
        }
        assert_eq!(searched as u64, s1.searched);
    }
}

#[test]
fn gumbel_mode_needs_the_root_logits() {
    let jobs: Vec<(u32, u64)> = (0..2).map(|i| (0u32, 11 + i as u64)).collect();
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let mut eng = SearchEngine::new(scen, jobs, 2, gcfg(), 1, false).unwrap();
    let (pc, vc) = eng.max_rows();
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let (np, nv) = eng.advance(None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
    assert!(np > 0);
    let pa = vec![0f32; np * (2 * gcfg().m + 1)];
    let va = vec![0f32; nv];
    assert!(eng.advance(Some(&pa), Some(&va), &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).is_err());
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    assert!(SearchEngine::new(scen, vec![(0, 1)], 1, SearchCfg { gm: 1, ..gcfg() }, 1, false).is_err());
}

fn run_recorded(threads: usize, n_roots: usize, scen: Vec<(Scenario, ScenarioExtras)>, jobs: Vec<(u32, u64)>, cfg: SearchCfg) -> SearchEngine {
    let mut eng = SearchEngine::new(scen, jobs, n_roots, cfg, threads, true).unwrap();
    let (pc, vc) = eng.max_rows();
    let (mut po, mut pm, mut pk, mut pu, mut vo, mut vk) = (vec![0f32; pc * OBS_SIZE], vec![0u8; pc * ACTION_SPACE], vec![0u8; pc], vec![0f32; pc], vec![0f32; vc * OBS_SIZE], vec![0u8; vc]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; pc * stride], vec![0f32; vc]);
    let (mut np, mut nv) = eng.advance(None, None, &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
    let mut cycles = 0;
    while np + nv > 0 {
        for r in 0..np {
            answer(&po[r * OBS_SIZE..(r + 1) * OBS_SIZE], &pm[r * ACTION_SPACE..(r + 1) * ACTION_SPACE], cfg.m, &mut pol[r * stride..(r + 1) * stride]);
        }
        for r in 0..nv {
            val[r] = (hash(&vo[r * OBS_SIZE..(r + 1) * OBS_SIZE]) % 1000) as f32 / 2000.0 - 0.25;
        }
        let pa = pol[..np * stride].to_vec();
        let va = val[..nv].to_vec();
        (np, nv) = eng.advance(Some(&pa), Some(&va), &mut po, &mut pm, &mut pk, &mut pu, &mut vo, &mut vk).unwrap();
        cycles += 1;
        assert!(cycles < 100_000, "the engine does not terminate");
    }
    assert!(eng.finished());
    eng
}

/// The search's `terminal` reward of a finished fight (win 1 + 0.5 x HP fraction, loss -1; aborted 0), None while it runs.
fn end_reward(cx: &Combat) -> Option<f32> {
    if cx.missing.is_some() {
        Some(0.0)
    } else if sts2env::looped(cx) {
        Some(-1.0)
    } else if cx.overflow != 0 {
        Some(0.0)
    } else if cx.stage == Stage::Over {
        let me = cx.cr(0);
        Some(if cx.outcome == Outcome::Victory { 1.0 + 0.5 * (me.hp as f32 / me.max_hp.max(1) as f32) } else { -1.0 })
    } else {
        None
    }
}

/// Plays `first` on `cx` and then the stand-in policy's play-out moves (`answer`: the hash of the observation the engine would write, among the legal
/// actions in index order; forced moves as the engine plays them) to the end of the fight. Returns the final reward.
fn continue_with_policy(mut cx: Combat, first: Action) -> f32 {
    let mut act = first;
    for _ in 0..100_000 {
        assert!(cx.step(act), "an action from the legal set was refused");
        if let Some(r) = end_reward(&cx) {
            return r;
        }
        let mut buf = ActionBuf::new();
        let mut playable = 0u16;
        cx.legal_actions_ex(&mut buf, &mut playable);
        assert!(!buf.is_empty());
        if let Some(a) = forced_action(&cx, &buf) {
            act = a;
            continue;
        }
        let mut o = vec![0f32; OBS_SIZE];
        cx.observe_ex(&mut o, Some(playable));
        cx.sync_overflow();
        let mut mask = vec![0u8; ACTION_SPACE];
        for a in buf.iter() {
            mask[a.index()] = 1;
        }
        let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| mask[a] > 0).collect();
        act = Action::from_index(legal[(hash(&o) % legal.len() as u64) as usize]).unwrap();
    }
    panic!("the continuation does not end");
}

/// Searches every job with one future per option played to the fight's end, then checks every tried option's estimate against the real fight's
/// continuation (the true state at that decision, the option, then the same policy). Returns (options whose estimate equals it exactly, options tried).
fn playouts_vs_true_future(clairvoyant: bool) -> (usize, usize) {
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default()), (scenario(14, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let jobs: Vec<(u32, u64)> = (0..8).map(|i| ((i % 2) as u32, 4200 + i as u64)).collect();
    let mut c = cfg();
    c.k = 1;
    c.leaf_turns = u32::MAX;
    c.roll_cap = 1_000_000;
    c.max_steps = 100_000;
    c.clairvoyant = clairvoyant;
    let eng = run_recorded(3, 4, scen.clone(), jobs.clone(), c);
    let (mut same, mut tried) = (0, 0);
    for (j, &(si, seed)) in jobs.iter().enumerate() {
        let (sc, ex) = &scen[si as usize];
        let mut main = Combat::try_new_with(sc, ex).unwrap();
        main.reset_validated(sc, ex, seed, RngSet::from_run_seed_fast(seed)).unwrap();
        let moves = eng.moves(j);
        assert!(!moves.is_empty());
        for rec in moves {
            let mut buf = ActionBuf::new();
            let mut playable = 0u16;
            main.legal_actions_ex(&mut buf, &mut playable);
            if forced_action(&main, &buf).is_none() {
                // the engine wrote this decision's policy row (observation, then the overflow sync) before playing it
                main.sync_overflow();
            }
            if rec.searched {
                for o in 0..c.m {
                    if !rec.legal[o] {
                        continue;
                    }
                    let truth = continue_with_policy(main.clone(), Action::from_index(rec.opts[o] as usize).unwrap());
                    tried += 1;
                    same += (truth.to_bits() == rec.q[o].to_bits()) as usize;
                }
            }
            assert!(main.step(Action::from_index(rec.action as usize).unwrap()), "the recorded fight does not replay");
        }
    }
    (same, tried)
}

#[test]
fn clairvoyant_playout_is_the_real_continuation() {
    // diagnostic flag: with one future that is a copy of the true state, every option's play-out IS the real fight's continuation under the same policy
    let (same, tried) = playouts_vs_true_future(true);
    assert!(tried > 20, "{tried} options tried");
    assert_eq!(same, tried, "{} of {tried} play-outs differ from the real continuation", tried - same);
    // the default (determinized futures) resamples the hidden information: the same check fails on some options
    let (same0, tried0) = playouts_vs_true_future(false);
    assert!(tried0 > 20 && same0 < tried0, "{same0} of {tried0} determinized play-outs equal the real continuation");
}

#[test]
fn clairvoyant_search_finishes_reproducibly_and_changes_estimates() {
    // with every other option on (shared prefix, carried lines, stratified futures)
    let jobs: Vec<(u32, u64)> = (0..24).map(|i| ((i % 2) as u32, 700 + i as u64)).collect();
    let mut c = cfg();
    c.lead = true;
    c.carry = true;
    c.strat = true;
    c.clairvoyant = true;
    let (r1, s1) = run(2, 5, jobs.clone(), c);
    let (r2, _) = run(1, 24, jobs.clone(), c);
    assert!(r1.iter().all(|x| x.done && matches!(x.outcome, 1 | -1 | 2)));
    assert_eq!(s1.illegal, 0);
    assert!(s1.searched > 0 && s1.lead_branch > 0);
    for i in 0..24 {
        assert_eq!((r1[i].outcome, r1[i].len), (r2[i].outcome, r2[i].len), "job {i}");
        assert_eq!(r1[i].hp_lost.to_bits(), r2[i].hp_lost.to_bits(), "job {i}");
    }
    // the first searched decision of a job stands at the same true state with or without the flag: its estimates differ only through the futures
    let scen = vec![(scenario(10, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default()), (scenario(14, ids::encounter::NIBBITS_WEAK), ScenarioExtras::default())];
    let first_q = |e: &SearchEngine, j: usize| e.moves(j).iter().find(|m| m.searched).map(|m| m.q.map(f32::to_bits));
    let e1 = run_recorded(2, 5, scen.clone(), jobs.clone(), c);
    c.clairvoyant = false;
    let e0 = run_recorded(2, 5, scen, jobs, c);
    assert!((0..24).any(|j| first_q(&e0, j) != first_q(&e1, j)), "the flag changed no estimate");
}
