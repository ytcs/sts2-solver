//! The observation must contain exactly what a human can see: perturbing hidden state must not change it.
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::types::CardPilePosition;
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
        assert!(obs(&a) == base, "draw order leaked (seed {seed})");

        // 1b. permute the discard and exhaust orders (a player only knows what is in the piles, not their order)
        let mut a2 = cx;
        rng.shuffle(a2.player.discard.as_mut_slice());
        rng.shuffle(a2.player.exhaust.as_mut_slice());
        assert!(obs(&a2) == base, "discard/exhaust order leaked (seed {seed})");

        // 2. rewrite every RNG stream
        let mut b = cx;
        b.rng = RngSet::from_run_seed(seed.wrapping_add(777));
        assert!(obs(&b) == base, "RNG state leaked (seed {seed})");

        // 3. hidden monster AI internals (current node / log) other than the visible intent + performed history
        let mut c = cx;
        for &e in cx.enemies.iter() {
            c.creatures[e as usize].monster.ever_logged = !0;
            c.creatures[e as usize].monster.log = [3; 8];
            c.creatures[e as usize].monster.log_len = 99;
        }
        assert!(obs(&c) == base, "monster log leaked (seed {seed})");
    }
}

#[test]
fn visible_changes_do_change_the_observation() {
    let cx = midfight(3);
    let base = obs(&cx);
    let mut a = cx;
    a.cr_mut(PLAYER).hp -= 1;
    assert!(obs(&a) != base);
    let mut b = cx;
    if b.player.hand.len() >= 2 {
        let (x, y) = (b.player.hand[0], b.player.hand[1]);
        if b.cards[x as usize].id != b.cards[y as usize].id {
            b.player.hand[0] = y;
            b.player.hand[1] = x;
            assert!(obs(&b) != base, "hand order is visible");
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

/// A pile-selection screen must not reveal the pile order: two states that differ only in the (hidden) order of the
/// discard pile present the same candidates, and clicking the same displayed position picks an equivalent card.
#[test]
fn pile_selection_screen_does_not_reveal_pile_order() {
    use sts2sim::engine::Ask;
    let mut cx = Combat::new(&scenario(3));
    // fill the discard pile with distinguishable cards in a known order
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    for id in [ids::card::TWIN_STRIKE, ids::card::BASH, ids::card::DEFEND_IRONCLAD, ids::card::STRIKE_IRONCLAD, ids::card::SHRUG_IT_OFF] {
        let c = cx.new_card(id, 0).unwrap();
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    let mut a = cx;
    let mut b = cx;
    let mut rng = Rng::new(99);
    rng.shuffle(b.player.discard.as_mut_slice());
    for c in [&mut a, &mut b] {
        assert!(matches!(c.ask_pile(0, PileType::Discard, 1, 1, |_, _| true), Ask::Pending));
        c.stage = Stage::AwaitChoice;
    }
    assert!(obs(&a) == obs(&b), "pile screen leaked the pile order");
    // the displayed list is sorted by what is visible, not by pile order
    let ids_a: Vec<u16> = {
        let d = a.decision.unwrap();
        let view = a.decision_view(&d);
        (0..view.len()).map(|k| a.cards[d.cands[view[k] as usize] as usize].id).collect()
    };
    let mut sorted = ids_a.clone();
    sorted.sort_by_key(|&id| (sts2sim::content::card_def(id).rarity, id));
    assert_eq!(ids_a, sorted);
}
