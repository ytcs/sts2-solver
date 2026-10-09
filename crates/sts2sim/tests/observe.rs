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

fn midfight(seed: u64) -> Combat {
    let mut cx = Combat::new(&scenario(seed));
    let mut buf = engine::ActionBuf::new();
    for _ in 0..14 {
        if cx.stage == Stage::Over {
            break;
        }
        cx.legal_actions(&mut buf);
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

        let mut a = cx.clone();
        let mut rng = Rng::new(seed ^ 0xABCD);
        rng.shuffle(a.player.draw.as_mut_slice());
        assert!(obs(&a) == base, "draw order leaked (seed {seed})");

        let mut a2 = cx.clone();
        rng.shuffle(a2.player.discard.as_mut_slice());
        rng.shuffle(a2.player.exhaust.as_mut_slice());
        assert!(obs(&a2) == base, "discard/exhaust order leaked (seed {seed})");

        let mut b = cx.clone();
        b.rng = RngSet::from_run_seed(seed.wrapping_add(777));
        assert!(obs(&b) == base, "RNG state leaked (seed {seed})");

        let mut c = cx.clone();
        for &e in cx.enemies.iter() {
            c.creatures[e as usize].monster.ever_logged = !0;
            c.creatures[e as usize].monster.log = [3; 8];
            c.creatures[e as usize].monster.log_len = 99;
        }
        assert!(obs(&c) == base, "monster log leaked (seed {seed})");
    }
}

#[test]
fn big_pile_order_does_not_leak() {
    let mut cx = midfight(5);
    let ids_ = [ids::card::BASH, ids::card::TWIN_STRIKE, ids::card::SHRUG_IT_OFF, ids::card::ARMAMENTS, ids::card::POMMEL_STRIKE];
    for k in 0..80 {
        let c = cx.new_card(ids_[k % ids_.len()], (k % 2) as u8).unwrap();
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    assert!(cx.player.discard.len() > observe::OBS_MAX_PILE);
    let base = obs(&cx);
    let mut rng = Rng::new(17);
    for _ in 0..20 {
        let mut a = cx.clone();
        rng.shuffle(a.player.discard.as_mut_slice());
        assert!(obs(&a) == base, "a big pile's hidden order leaked");
    }
}

#[test]
fn enemy_moves_section_shows_the_pending_node() {
    let (off, size) = observe::layout().iter().find(|s| s.0 == "enemy_moves").map(|s| (s.1, s.2)).unwrap();
    assert_eq!(size, observe::ENEMY_MOVES_F);
    let mut cx = Combat::new(&scenario(4));
    let e = cx.enemies[0];
    let v = obs(&cx);
    let pending = cx.cr(e).monster.next_move;
    assert_eq!((v[off], v[off + 1]), ((pending + 1) as f32, 0.0));
    cx.stun(e, None, None);
    let v = obs(&cx);
    assert_eq!((v[off], v[off + 1]), (255.0, (pending + 1) as f32));
}

#[test]
fn visible_changes_do_change_the_observation() {
    let cx = midfight(3);
    let base = obs(&cx);
    let mut a = cx.clone();
    a.cr_mut(PLAYER).hp -= 1;
    assert!(obs(&a) != base);
    let mut b = cx.clone();
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
    assert_eq!(cx.intent_damage(e, 12), 18);
    cx.apply_power(ids::power::STRENGTH_POWER, e, sts2sim::dec::Dec::int(2), e, NO);
    assert_eq!(cx.intent_damage(e, 12), 21);
}

#[test]
fn pile_selection_screen_does_not_reveal_pile_order() {
    use sts2sim::engine::Ask;
    let mut cx = Combat::new(&scenario(3));
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    for id in [ids::card::TWIN_STRIKE, ids::card::BASH, ids::card::DEFEND_IRONCLAD, ids::card::STRIKE_IRONCLAD, ids::card::SHRUG_IT_OFF] {
        let c = cx.new_card(id, 0).unwrap();
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    let mut a = cx.clone();
    let mut b = cx.clone();
    let mut rng = Rng::new(99);
    rng.shuffle(b.player.discard.as_mut_slice());
    for c in [&mut a, &mut b] {
        assert!(matches!(c.ask_pile(0, PileType::Discard, 1, 1, |_, _| true), Ask::Pending));
        c.stage = Stage::AwaitChoice;
    }
    assert!(obs(&a) == obs(&b), "pile screen leaked the pile order");
    let ids_a: Vec<u16> = {
        let d = a.decision.unwrap();
        let view = a.decision_view(&d);
        (0..view.len()).map(|k| a.cards[d.cands[view[k] as usize] as usize].id).collect()
    };
    let mut sorted = ids_a.clone();
    sorted.sort_by_key(|&id| (sts2sim::content::card_def(id).rarity, id));
    assert_eq!(ids_a, sorted);
}

#[test]
fn layout_sections_tile_the_observation() {
    let mut off = 0;
    for (name, o, sz) in observe::layout().iter() {
        assert_eq!(*o, off, "section {name} starts where the previous one ended");
        off += sz;
    }
    assert_eq!(off, OBS_SIZE);
    let c: std::collections::HashMap<_, _> = observe::layout_consts().into_iter().collect();
    assert_eq!(c["OBS_SIZE"], OBS_SIZE);
    assert_eq!(c["ACTION_SPACE"], sts2sim::engine::ACTION_SPACE);
    assert_eq!(c["OFF_CONFIRM"] + 1, c["ACTION_SPACE"]);
    let cx = midfight(2);
    let mut big = vec![7f32; OBS_SIZE + 5];
    assert_eq!(cx.observe(&mut big), OBS_SIZE);
    assert!(big[OBS_SIZE..].iter().all(|&x| x == 7.0));
}

#[test]
fn determinize_changes_only_hidden_state() {
    let mut changed = 0;
    for seed in 0..40 {
        let cx = midfight(seed);
        if cx.stage == Stage::Over {
            continue;
        }
        let base = obs(&cx);
        let mut a = cx.clone();
        assert!(a.determinize(seed + 1));
        assert!(obs(&a) == base, "determinize changed what the agent sees (seed {seed})");
        let mut acts = sts2sim::engine::ActionBuf::new();
        let mut acts_a = sts2sim::engine::ActionBuf::new();
        cx.legal_actions(&mut acts);
        a.legal_actions(&mut acts_a);
        assert!(acts.as_slice() == acts_a.as_slice(), "legal actions changed (seed {seed})");
        changed += (0..cx.player.draw.len()).any(|k| a.player.draw[k] != cx.player.draw[k]) as u32;
        for _ in 0..30 {
            let mut v = sts2sim::engine::ActionBuf::new();
            a.legal_actions(&mut v);
            if v.is_empty() {
                break;
            }
            assert!(a.step(v[(seed as usize * 7) % v.len()]));
        }
    }
    assert!(changed > 10, "the draw order was resampled in only {changed} of 40 fights");
}
