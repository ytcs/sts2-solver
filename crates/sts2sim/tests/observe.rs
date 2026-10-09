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

#[cfg(feature = "obs_v3")]
mod v3 {
    use super::*;
    use sts2sim::dec::Dec;
    use sts2sim::observe::{CARDX_F, CX_DERIVED, CX_F, OBS_POWERS, OBS_SIZE_V2};

    fn sec(name: &str) -> usize {
        observe::layout().iter().find(|s| s.0 == name).unwrap().1
    }

    /// creature block k: 0 player, 1 + i enemy i, 9 Osty
    fn cx_block(v: &[f32], k: usize) -> &[f32] {
        &v[sec("creature_x") + k * CX_F..sec("creature_x") + (k + 1) * CX_F]
    }

    fn derived(v: &[f32], k: usize) -> Vec<i32> {
        cx_block(v, k)[OBS_POWERS + 2..OBS_POWERS + 2 + CX_DERIVED].iter().map(|&x| x as i32).collect()
    }

    fn silent(deck: &[u16], relics: &[u16]) -> Combat {
        let mut sc = scenario(7);
        sc.character = 1;
        sc.deck = deck.iter().map(|&id| DeckCard { id, upgrade: 0 }).collect();
        sc.relics = relics.iter().map(|&id| RelicInit { id, ..Default::default() }).collect();
        sc.potions = vec![];
        Combat::new(&sc)
    }

    #[test]
    fn v2_prefix_is_the_v2_observation() {
        for seed in 0..20 {
            let cx = midfight(seed);
            let full = obs(&cx);
            let mut v2 = vec![0f32; OBS_SIZE_V2];
            assert_eq!(cx.observe_v2(&mut v2, None), OBS_SIZE_V2);
            assert!(full[..OBS_SIZE_V2] == v2[..], "seed {seed}");
        }
    }

    #[test]
    fn doom_threshold_is_explicit() {
        let mut cx = Combat::new(&scenario(1));
        let e = cx.enemies[0];
        let hp = cx.cr(e).hp;
        let mut a = cx.clone();
        a.apply_power(ids::power::DOOM_POWER, e, Dec::int(hp as i64 - 1), PLAYER, NO);
        let d = derived(&obs(&a), 1);
        assert_eq!((d[0], d[1], d[6], d[9]), (-1, 0, 1, 0), "doom hp-1: {d:?}");
        cx.apply_power(ids::power::DOOM_POWER, e, Dec::int(hp as i64), PLAYER, NO);
        let d = derived(&obs(&cx), 1);
        assert_eq!((d[0], d[1], d[6], d[9]), (0, 1, 0, 1), "doom = hp: {d:?}");
    }

    #[test]
    fn poison_projection_counts_accelerant_and_fumes() {
        let mut cx = silent(&[ids::card::DEADLY_POISON; 6], &[]);
        let e = cx.enemies[0];
        cx.apply_power(ids::power::POISON_POWER, e, Dec::int(10), PLAYER, NO);
        let d = derived(&obs(&cx), 1);
        assert_eq!((d[2], d[3]), (10, 10 + 9 + 8), "poison 10: {d:?}");
        cx.apply_power(ids::power::ACCELERANT_POWER, PLAYER, Dec::int(1), PLAYER, NO);
        let d = derived(&obs(&cx), 1);
        assert_eq!((d[2], d[3]), (10 + 9, 19 + 8 + 7 + 6 + 5), "poison 10 + accelerant 1: {d:?}");
        cx.apply_power(ids::power::ACCELERANT_POWER, PLAYER, Dec::int(1), PLAYER, NO);
        let d = derived(&obs(&cx), 1);
        assert_eq!(d[2], 10 + 9 + 8, "accelerant 2: {d:?}");
        cx.apply_power(ids::power::NOXIOUS_FUMES_POWER, PLAYER, Dec::int(2), PLAYER, NO);
        let d = derived(&obs(&cx), 1);
        // ticks: 10 9 8 -> 7 (+2 = 9): 9 8 7 -> 6 (+2 = 8): 8 7 6
        assert_eq!((d[2], d[3]), (27, 27 + 24 + 21), "accelerant 2 + fumes 2: {d:?}");
        let hp = cx.cr(e).hp;
        assert_eq!(d[4], (27 >= hp) as i32);
        assert_eq!(d[6], hp - 27);
    }

    #[test]
    fn intangible_lands_on_its_side_and_caps_damage() {
        let cx = Combat::new(&scenario(2));
        let e = cx.enemies[0];
        let base = obs(&cx);
        let hits = {
            let o = sec("enemies");
            let ef = observe::ENEMY_F;
            (0..cx.enemies.len())
                .flat_map(|k| (0..3).map(move |j| (k, j)))
                .map(|(k, j)| {
                    let b = o + k * ef + 8 + OBS_POWERS * 3 + j * 3;
                    if base[b] == 1.0 && base[o + k * ef + 6] == 1.0 { base[b + 2] as i32 } else { 0 }
                })
                .sum::<i32>()
        };
        let mut p = cx.clone();
        p.apply_power(ids::power::INTANGIBLE_POWER, PLAYER, Dec::int(1), PLAYER, NO);
        let d = derived(&obs(&p), 0);
        assert_eq!(d[7], hits, "player intangible: incoming = one per hit");
        let bomb_hand = {
            let h = sec("hand_tgt");
            base[h..h + observe::OBS_MAX_ENEMIES].to_vec()
        };
        let mut q = cx.clone();
        q.apply_power(ids::power::INTANGIBLE_POWER, e, Dec::int(1), e, NO);
        let v = obs(&q);
        let h = sec("hand_tgt");
        for k in 0..sts2sim::state::MAX_HAND {
            let x = v[h + k * observe::OBS_MAX_ENEMIES];
            assert!(x <= 1.0, "enemy intangible caps the per-target preview of hand card {k}: {x}");
        }
        assert!(bomb_hand.iter().any(|&x| x > 1.0) || cx.player.hand.iter().all(|&c| cx.card_preview(c).damage == 0));
        // the player block shows no intangible-specific change from an enemy's intangible
        assert_eq!(derived(&v, 0), derived(&base, 0));
    }

    #[test]
    fn vulnerable_target_shows_in_the_per_target_preview() {
        let mut cx = Combat::new(&scenario(2));
        let e = cx.enemies[0];
        let Some(k) = (0..cx.player.hand.len()).find(|&k| cx.card_preview(cx.player.hand[k]).damage > 0) else { return };
        let h = sec("hand_tgt") + k * observe::OBS_MAX_ENEMIES;
        let before = obs(&cx)[h];
        cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(2), PLAYER, NO);
        let after = obs(&cx)[h];
        assert_eq!(after, (before * 1.5).floor(), "vulnerable x1.5 on the target");
    }

    #[test]
    fn hidden_second_numbers_show() {
        let mut cx = Combat::new(&scenario(3));
        let uid = cx.apply_power(ids::power::THE_BOMB_POWER, PLAYER, Dec::int(3), PLAYER, NO).unwrap();
        cx.power_mut(PLAYER, uid).unwrap().aux = 40;
        let v = obs(&cx);
        let i = cx.cr(PLAYER).powers.iter().position(|p| p.id == ids::power::THE_BOMB_POWER).unwrap();
        assert_eq!(cx_block(&v, 0)[i], 40.0, "The Bomb's damage");
        let c = cx.player.hand[0];
        let uid = cx.apply_power(ids::power::NIGHTMARE_POWER, PLAYER, Dec::int(3), PLAYER, NO).unwrap();
        cx.power_mut(PLAYER, uid).unwrap().aux = c as i32 + 1;
        let v = obs(&cx);
        let b = cx_block(&v, 0);
        assert_eq!((b[OBS_POWERS] as u16, b[OBS_POWERS + 1] as u8), (cx.cards[c as usize].id + 1, cx.cards[c as usize].upgrade), "Nightmare's card");
    }

    #[test]
    fn pile_extras_and_relic_state_show_without_leaking_order() {
        let mut cx = midfight(5);
        for k in 0..6 {
            let c = cx.new_card(ids::card::BASH, (k % 2) as u8).unwrap();
            cx.cards[c as usize].counter[0] = k as i16 + 1;
            cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
        let v = obs(&cx);
        let x = &v[sec("discard_x")..sec("discard_x") + observe::PILEX_N * observe::PILEX_F];
        let shown: Vec<i32> = x.chunks(observe::PILEX_F).filter(|r| r[0] > 0.0).map(|r| r[6] as i32).collect();
        assert_eq!(shown, vec![1, 3, 5, 2, 4, 6], "sorted by (id, upgrade, ..., counter)");
        let mut rng = Rng::new(3);
        let mut a = cx.clone();
        rng.shuffle(a.player.discard.as_mut_slice());
        assert!(obs(&a) == v, "pile extras leaked the order");

        let mut sc = scenario(1);
        sc.relics.push(RelicInit { id: ids::relic::LIZARD_TAIL, ..Default::default() });
        let mut cx = Combat::new(&sc);
        let i = cx.player.relics.as_slice().iter().filter(|r| sts2sim::relic_mask::OBSERVED[r.id as usize]).position(|r| r.id == ids::relic::LIZARD_TAIL).unwrap();
        let at = sec("relic_x") + i * observe::RELICX_F;
        assert_eq!(obs(&cx)[at], 0.0);
        let j = cx.player.relics.as_slice().iter().position(|r| r.id == ids::relic::LIZARD_TAIL).unwrap();
        cx.player.relics.as_mut_slice()[j].set_flag(0, true);
        assert_eq!(obs(&cx)[at], 1.0, "Lizard Tail used");
    }

    #[test]
    fn card_effects_name_the_power_and_its_amount() {
        let cx = silent(&[ids::card::DEADLY_POISON; 6], &[]);
        let v = obs(&cx);
        let x = &v[sec("hand_x")..sec("hand_x") + CARDX_F];
        assert_eq!(x[..5].iter().map(|&f| f as i32).collect::<Vec<_>>(), vec![2, 2, ids::power::POISON_POWER as i32 + 1, 5, 2], "Deadly Poison: skill, common, poison 5 on one enemy");
        let cx = silent(&[ids::card::DEADLY_POISON; 6], &[ids::relic::SNECKO_SKULL]);
        let v = obs(&cx);
        assert_eq!(v[sec("hand_x") + 3], 6.0, "Snecko Skull: poison given +1");
        let cx = silent(&[ids::card::WRAITH_FORM; 6], &[]);
        let v = obs(&cx);
        let x: Vec<i32> = v[sec("hand_x")..sec("hand_x") + CARDX_F].iter().map(|&f| f as i32).collect();
        let effs = [(x[2], x[3], x[4]), (x[5], x[6], x[7])];
        assert!(effs.contains(&(ids::power::INTANGIBLE_POWER as i32 + 1, 2, 1)) && effs.contains(&(ids::power::WRAITH_FORM_POWER as i32 + 1, 1, 1)), "{effs:?}");
    }
}
