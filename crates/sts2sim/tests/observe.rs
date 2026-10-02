//! The observation must contain exactly what a human can see: perturbing hidden state must not change it.
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(seed: u64) -> Scenario {
    let mut deck = vec![];
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    for id in [ids::card::BASH, ids::card::SHRUG_IT_OFF, ids::card::POMMEL_STRIKE, ids::card::TWIN_STRIKE, ids::card::HEADBUTT, ids::card::ARMAMENTS] {
        deck.push(DeckCard { id, upgrade: 0 });
    }
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![ids::potion::BLOCK_POTION],
        rng: RngSet::from_run_seed(seed),
    }
}

fn obs(cx: &Combat) -> Vec<f32> {
    let mut v = vec![0f32; OBS_SIZE];
    assert_eq!(cx.observe(&mut v), OBS_SIZE);
    v
}

/// Plays a few real turns with a simple policy, returning a mid-fight state.
fn midfight(seed: u64) -> Combat {
    let mut cx = Combat::new(&scenario(seed));
    let mut buf = engine::ActionBuf::new();
    for _ in 0..14 {
        if cx.stage == Stage::Over {
            break;
        }
        cx.legal_actions(&mut buf);
        // prefer playing cards over ending the turn; for decisions take the first pick / confirm
        let a = buf.iter().copied().find(|a| !matches!(a, Action::EndTurn | Action::DiscardPotion { .. })).unwrap_or(Action::EndTurn);
        assert!(cx.step(a));
    }
    cx
}

#[test]
fn hidden_state_does_not_leak() {
    for seed in 0..40 {
        let cx = midfight(seed);
        if cx.stage == Stage::Over {
            continue;
        }
        let base = obs(&cx);

        // 1. permute the draw-pile order
        let mut a = cx;
        let mut rng = Rng::new(seed ^ 0xABCD);
        rng.shuffle(a.player.draw.as_mut_slice());
        assert_eq!(obs(&a), base, "draw order leaked (seed {seed})");

        // 2. rewrite every RNG stream
        let mut b = cx;
        b.rng = RngSet::from_run_seed(seed.wrapping_add(777));
        assert_eq!(obs(&b), base, "RNG state leaked (seed {seed})");

        // 3. hidden monster AI internals (current node / log) other than the visible intent + performed history
        let mut c = cx;
        for &e in cx.enemies.iter() {
            c.creatures[e as usize].monster.ever_logged = !0;
            c.creatures[e as usize].monster.log = [3; 8];
            c.creatures[e as usize].monster.log_len = 99;
        }
        assert_eq!(obs(&c), base, "monster log leaked (seed {seed})");
    }
}

#[test]
fn visible_changes_do_change_the_observation() {
    let cx = midfight(3);
    let base = obs(&cx);
    let mut a = cx;
    a.cr_mut(PLAYER).hp -= 1;
    assert_ne!(obs(&a), base);
    let mut b = cx;
    if b.player.hand.len() >= 2 {
        let (x, y) = (b.player.hand[0], b.player.hand[1]);
        if b.cards[x as usize].id != b.cards[y as usize].id {
            b.player.hand[0] = y;
            b.player.hand[1] = x;
            assert_ne!(obs(&b), base, "hand order is visible");
        }
    }
    let mut c = cx;
    if c.player.discard.len() >= 2 {
        let (x, y) = (c.player.discard[0], c.player.discard[1]);
        if c.cards[x as usize].id != c.cards[y as usize].id {
            c.player.discard[0] = y;
            c.player.discard[1] = x;
            assert_ne!(obs(&c), base, "discard order is visible");
        }
    }
}

#[test]
fn intent_damage_reflects_modifiers() {
    let mut cx = Combat::new(&scenario(1));
    let e = cx.enemies[0];
    let base = cx.intent_damage(e, 12);
    assert_eq!(base, 12);
    cx.apply_power(ids::power::VULNERABLE_POWER, PLAYER, sts2sim::dec::Dec::int(1), e, NO);
    assert_eq!(cx.intent_damage(e, 12), 18); // 12 * 1.5
    cx.apply_power(ids::power::STRENGTH_POWER, e, sts2sim::dec::Dec::int(2), e, NO);
    assert_eq!(cx.intent_damage(e, 12), 21); // (12+2) * 1.5
}
