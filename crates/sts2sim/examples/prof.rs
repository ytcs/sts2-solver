//! Deterministic workload for instruction-count profiling (`valgrind --tool=callgrind`).
//! `prof fights [N]`: greedy fights (starter vs Nibbits), `Combat::new` per fight.
//! `prof reset [N]`: the same fights on one reused combat (`Combat::reset_validated`), i.e. the batch-env path.
//! `prof env [N]`: random masked-policy episodes with observation + legal-action generation every step (what a batch env does).
//! Prints a checksum so optimisations can be verified bit-identical.
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(seed: u64) -> Scenario {
    let mut deck = vec![];
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    deck.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    deck.push(DeckCard { id: ids::card::ASCENDERS_BANE, upgrade: 0 });
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

fn greedy(cx: &mut Combat) -> u64 {
    let mut steps = 0u64;
    while cx.stage != Stage::Over {
        let e = cx.enemies.first().unwrap_or(NO);
        let mut done = false;
        for pos in 0..cx.player.hand.len() {
            let c = cx.player.hand[pos];
            if cx.can_play(c) {
                let t = if cx.card_def(c).target == TargetType::AnyEnemy { e } else { NO };
                cx.step(Action::PlayCard { hand_pos: pos as u8, target: t });
                done = true;
                break;
            }
        }
        if !done {
            cx.step(Action::EndTurn);
        }
        steps += 1;
    }
    steps ^ ((cx.cr(0).hp as u64) << 32)
}

/// Dense action index of the original layout (16 creature slots), so checksums stay comparable across `MAX_CREATURES` changes.
fn stable_index(a: Action) -> u64 {
    let t = |t: u8| if t == NO { 16u64 } else { t as u64 };
    match a {
        Action::EndTurn => 0,
        Action::PlayCard { hand_pos, target } => 1 + hand_pos as u64 * 17 + t(target),
        Action::UsePotion { slot, target } => 1 + 10 * 17 + slot as u64 * 17 + t(target),
        Action::DiscardPotion { slot } => 1 + 10 * 17 + 4 * 17 + slot as u64,
        Action::Pick { idx } => 1 + 10 * 17 + 4 * 17 + 4 + idx as u64,
        Action::Confirm => 1 + 10 * 17 + 4 * 17 + 4 + 64,
    }
}

fn env_episode(seed: u64, obs: &mut [f32]) -> u64 {
    let mut cx = Combat::new(&scenario(seed));
    let mut pol = Rng::new(seed ^ 0x55);
    let mut h = 0u64;
    let mut steps = 0;
    while cx.stage != Stage::Over && steps < 2000 {
        cx.observe(obs);
        let mut buf = engine::ActionBuf::new();
        cx.legal_actions(&mut buf);
        let k = buf.len();
        let a = *buf.iter().nth(pol.next_int(k as i32) as usize).unwrap();
        cx.step(a);
        let mut o = 0u64;
        for (i, v) in obs.iter().enumerate() {
            o = o.wrapping_mul(0x100000001b3).wrapping_add(v.to_bits() as u64 + i as u64);
        }
        h = h.wrapping_mul(31).wrapping_add(o ^ stable_index(a));
        steps += 1;
    }
    h ^ steps
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or("fights".into());
    let n: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let mut sum = 0u64;
    let mut obs = vec![0f32; OBS_SIZE];
    let sc = scenario(0);
    let mut cx = Combat::new(&sc);
    for i in 0..n {
        sum = sum.wrapping_mul(1000003).wrapping_add(match mode.as_str() {
            "fights" => greedy(&mut Combat::new(&scenario(i))),
            "reset" => {
                // what a batch env does: one combat per slot, re-initialised in place for every episode
                let ex = ScenarioExtras::default();
                cx.reset_validated(&sc, &ex, 0, RngSet::from_run_seed_fast(i)).unwrap();
                greedy(&mut cx)
            }
            _ => env_episode(i, &mut obs),
        });
    }
    println!("checksum {sum:#x}");
}
