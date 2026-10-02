//! Vertical slice: Ironclad starter deck vs a lone Nibbit (NibbitsWeak).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(seed: u64, ascension: u8) -> Scenario {
    let mut deck = vec![];
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    deck.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0 }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

#[test]
fn opening_state() {
    let cx = Combat::new(&scenario(1, 0));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.hand.len(), 5);
    assert_eq!(cx.player.draw.len(), 5);
    assert_eq!(cx.player.energy, 3);
    assert_eq!(cx.enemies.len(), 1);
    let e = cx.enemies[0];
    // Nibbit alone opens with BUTT; HP in 42..=46 at A0.
    assert!((42..=46).contains(&cx.cr(e).max_hp));
    assert_eq!(cx.cr(e).monster.next_move, 1);
}

#[test]
fn initial_draw_order_matches_manual_shuffle() {
    // Reproduce the shuffle by hand with the same stream and compare the hand.
    let sc = scenario(7, 0);
    let mut rng = Rng::named(7, "shuffle");
    let mut order: Vec<usize> = (0..10).collect();
    rng.shuffle(&mut order);
    let cx = Combat::new(&sc);
    // The deck was [S,S,S,S,S,D,D,D,D,B]; hand = first five of the shuffled order.
    let want: Vec<u16> = order[..5]
        .iter()
        .map(|&i| if i < 5 { ids::card::STRIKE_IRONCLAD } else if i < 9 { ids::card::DEFEND_IRONCLAD } else { ids::card::BASH })
        .collect();
    let got: Vec<u16> = cx.player.hand.iter().map(|&c| cx.cards[c as usize].id).collect();
    assert_eq!(got, want);
}

#[test]
fn strike_damage_and_block() {
    let mut cx = Combat::new(&scenario(3, 0));
    let e = cx.enemies[0];
    let hp0 = cx.cr(e).hp;
    // Find a Strike and a Defend in hand.
    let pos = |cx: &Combat, id: u16| cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id);
    if let Some(p) = pos(&cx, ids::card::STRIKE_IRONCLAD) {
        assert!(cx.step(Action::PlayCard { hand_pos: p as u8, target: e }));
        assert_eq!(cx.cr(e).hp, hp0 - 6);
        assert_eq!(cx.player.energy, 2);
        assert_eq!(cx.player.discard.len(), 1);
    }
    if let Some(p) = pos(&cx, ids::card::DEFEND_IRONCLAD) {
        assert!(cx.step(Action::PlayCard { hand_pos: p as u8, target: NO }));
        assert_eq!(cx.cr(PLAYER).block, 5);
    }
}

#[test]
fn vulnerable_weak_pipeline_matches_spec_example() {
    // Spec 02 §3.6: Strike(6) + Strength 3, target Vulnerable, dealer Weak => ((6+3)*1.5)*0.75 = 10.125.
    let mut cx = Combat::new(&scenario(5, 0));
    let e = cx.enemies[0];
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(3), PLAYER, NO);
    cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(2), PLAYER, NO);
    cx.apply_power(ids::power::WEAK_POWER, PLAYER, Dec::int(1), e, NO);
    let (v, _) = cx.modify_damage(e, PLAYER, Dec::int(6), ValueProp::MOVE, NO);
    assert_eq!(v, Dec::frac(10125, 3));
    // Block 5 absorbs 5; Block -= (int)5 ; unblocked 5.125 -> 5 HP lost.
    cx.cr_mut(e).block = 5;
    let hp0 = cx.cr(e).hp;
    let r = cx.damage(&[e], Dec::int(6), ValueProp::MOVE, PLAYER, NO);
    assert_eq!(r[0].unblocked, 5);
    assert_eq!(cx.cr(e).hp, hp0 - 5);
    assert_eq!(cx.cr(e).block, 0);
    // Block 11: block loses (int)10.125 = 10 => 1 left; fully blocked.
    cx.cr_mut(e).block = 11;
    let hp1 = cx.cr(e).hp;
    let r = cx.damage(&[e], Dec::int(6), ValueProp::MOVE, PLAYER, NO);
    assert_eq!(cx.cr(e).block, 1);
    assert_eq!(cx.cr(e).hp, hp1);
    assert!(r[0].fully_blocked);
}

#[test]
fn full_fight_terminates_and_is_deterministic() {
    // Play greedily: Bash first, then Strikes, then Defends; end turn when nothing is playable.
    fn run(seed: u64) -> (Outcome, i32, i32) {
        let mut cx = Combat::new(&scenario(seed, 0));
        let mut guard = 0;
        while cx.stage != Stage::Over && guard < 500 {
            guard += 1;
            let e = cx.enemies.first().unwrap_or(NO);
            let mut played = false;
            for pref in [ids::card::BASH, ids::card::STRIKE_IRONCLAD, ids::card::DEFEND_IRONCLAD] {
                if let Some(p) = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == pref) {
                    let c = cx.player.hand[p];
                    if cx.can_play(c) {
                        let t = if cx.card_def(c).target == TargetType::AnyEnemy { e } else { NO };
                        assert!(cx.step(Action::PlayCard { hand_pos: p as u8, target: t }));
                        played = true;
                        break;
                    }
                }
            }
            if !played {
                assert!(cx.step(Action::EndTurn));
            }
        }
        assert_eq!(cx.stage, Stage::Over, "fight did not finish");
        (cx.outcome, cx.cr(PLAYER).hp, cx.round)
    }
    let a = run(11);
    let b = run(11);
    assert_eq!(a, b);
    assert_eq!(a.0, Outcome::Victory);
    // Burning Blood heals 6 after victory (capped at max HP).
    assert!(a.1 > 0 && a.1 <= 80);
    for s in 0..50 {
        let r = run(s);
        assert_eq!(r.0, Outcome::Victory, "seed {s}");
    }
}
