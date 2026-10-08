//! Observation v2: the visible information v1 leaves out is present (`observe.rs` module doc). Every test reads `observe_v(.., 2)`
//! directly (tests run in parallel; none of them touches the process-wide version).
use sts2sim::dec::Dec;
use sts2sim::defs::VarKind;
use sts2sim::engine::{ActionBuf, Ask};
use sts2sim::ids;
use sts2sim::observe::{self, layout_consts_v, layout_v, obs_size, CARD_F_V2, OBS_POWERS, POWER_F_V2};
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

/// Indices into a v2 card entry.
const DMG: usize = 6;
const BLK: usize = 7;
const COUNT: usize = 12;
const AFF_AMT: usize = 13;
const REPLAY: usize = 14;

fn scenario(character: u8, deck: Vec<DeckCard>, relics: Vec<RelicInit>, seed: u64) -> Scenario {
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 3,
        potion_slots: 3,
        deck,
        relics,
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

fn ironclad(seed: u64) -> Combat {
    let mut deck: Vec<DeckCard> = (0..4).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    deck.extend((0..4).map(|_| DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 }));
    for id in [ids::card::BASH, ids::card::SHRUG_IT_OFF, ids::card::POMMEL_STRIKE, ids::card::TWIN_STRIKE, ids::card::HEADBUTT, ids::card::ARMAMENTS] {
        deck.push(DeckCard { id, upgrade: 0 });
    }
    Combat::new(&scenario(0, deck, vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }], seed))
}

/// A Necrobinder with Osty summoned (6/6).
fn necro(seed: u64) -> Combat {
    let mut deck: Vec<DeckCard> = (0..4).map(|_| DeckCard { id: ids::card::STRIKE_NECROBINDER, upgrade: 0 }).collect();
    deck.extend((0..4).map(|_| DeckCard { id: ids::card::DEFEND_NECROBINDER, upgrade: 0 }));
    deck.push(DeckCard { id: ids::card::UNLEASH, upgrade: 0 });
    let mut cx = Combat::new(&scenario(3, deck, vec![RelicInit { id: ids::relic::BOUND_PHYLACTERY, ..Default::default() }], seed));
    cx.summon(5);
    cx
}

fn set_hand(cx: &mut Combat, cards: &[(u16, u8)]) -> Vec<u8> {
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    cards
        .iter()
        .map(|&(id, up)| {
            let c = cx.new_card(id, up).unwrap();
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            c
        })
        .collect()
}

fn obs(cx: &Combat, ver: u8) -> Vec<f32> {
    let mut v = vec![0f32; obs_size(ver)];
    cx.observe_v(&mut v, None, ver);
    v
}

fn sec(ver: u8, name: &str) -> (usize, usize) {
    layout_v(ver).iter().find(|s| s.0 == name).map(|s| (s.1, s.2)).unwrap()
}

fn hand_entry(v: &[f32], k: usize) -> &[f32] {
    let o = sec(2, "hand").0 + k * CARD_F_V2;
    &v[o..o + CARD_F_V2]
}

fn osty_dmg(v: &[f32], k: usize) -> f32 {
    v[sec(2, "osty").0 + 4 + OBS_POWERS * POWER_F_V2 + k]
}

/// Every card with a calculated value: the v2 observation's damage / block / count (and Osty damage) are the simulator's own preview, in states
/// that move each multiplier (block, Strength, Poison, Doom, orbs, piles, Osty's HP, cards played ...).
#[test]
fn every_calculated_card_shows_its_preview() {
    let calc: Vec<u16> = (0..ids::card::COUNT as u16)
        .filter(|&id| content::card_def(id).vars.iter().any(|v| matches!(v.kind, VarKind::CalcDamage | VarKind::CalcBlock | VarKind::CalcExtra)))
        .collect();
    assert!(calc.len() >= 40, "{} calculated cards", calc.len());
    let mut nonzero = 0;
    for &id in &calc {
        for k in 0..4 {
            let mut cx = if k % 2 == 0 { ironclad(k as u64) } else { necro(k as u64) };
            let e = cx.enemies[0];
            cx.cr_mut(PLAYER).block = 5 * k as i32;
            if k > 0 {
                cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(k as i64), PLAYER, NO);
                cx.apply_power(ids::power::POISON_POWER, e, Dec::int(3 * k as i64), PLAYER, NO);
                cx.apply_power(ids::power::DOOM_POWER, e, Dec::int(11 * k as i64), PLAYER, NO);
                cx.apply_power(ids::power::PARRY_POWER, PLAYER, Dec::int(k as i64), PLAYER, NO);
            }
            for _ in 0..k {
                cx.channel_orb(ids::orb::LIGHTNING_ORB);
                let s = cx.new_card(ids::card::SOUL, 0).unwrap();
                cx.move_card(s, PileType::Exhaust, CardPilePosition::Bottom);
            }
            let hand = set_hand(&mut cx, &[(id, (k % 2) as u8), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::SHRUG_IT_OFF, 0)]);
            let v = obs(&cx, 2);
            let f = hand_entry(&v, 0);
            let p = cx.card_preview(hand[0]);
            let name = ids::card::NAMES[id as usize];
            assert_eq!(f[0], id as f32 + 1.0);
            assert_eq!((f[DMG], f[BLK], f[COUNT]), (p.damage as f32, p.block as f32, p.count as f32), "{name} state {k}: obs vs preview {p:?}");
            assert_eq!(osty_dmg(&v, 0), p.osty_damage.unwrap_or(0) as f32, "{name} state {k}: Osty damage");
            nonzero += (p.damage != 0 || p.block != 0 || p.count != 0 || p.osty_damage.unwrap_or(0) != 0) as usize;
        }
    }
    assert!(nonzero > calc.len() * 2, "previews mostly zero: {nonzero} of {}", calc.len() * 4);
}

/// Hand-checked values of the card text, against v1 (which shows 0 for calculated numbers and adds Strength to status damage).
#[test]
fn calculated_numbers_match_the_card_text() {
    let mut cx = ironclad(1);
    cx.cr_mut(PLAYER).block = 13;
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    let hand = set_hand(
        &mut cx,
        &[(ids::card::PERFECTED_STRIKE, 0), (ids::card::BODY_SLAM, 0), (ids::card::FLECHETTES, 0), (ids::card::EXPECT_A_FIGHT, 0), (ids::card::BURN, 0), (ids::card::NORMALITY, 0), (ids::card::SHRUG_IT_OFF, 0), (ids::card::DEFEND_IRONCLAD, 0)],
    );
    let v = obs(&cx, 2);
    let v1 = obs(&cx, 1);
    let strikes = cx.player_combat_cards().iter().filter(|&&c| cx.card_def(c).tags & tag::STRIKE != 0).count() as i32;
    assert!(strikes >= 5);
    // Perfected Strike: 6 + 2 per Strike, + 2 Strength
    assert_eq!(hand_entry(&v, 0)[DMG], (6 + 2 * strikes + 2) as f32);
    // Body Slam: damage = current block (+ Strength)
    assert_eq!(hand_entry(&v, 1)[DMG], (13 + 2) as f32);
    // Flechettes: one hit per Skill in hand (Expect a Fight, Shrug It Off, Defend)
    assert_eq!(hand_entry(&v, 2)[COUNT], 3.0);
    // Expect a Fight: CalculationBase + CalculationExtra x Strength
    let c = hand[3];
    assert_eq!(hand_entry(&v, 3)[BLK], (cx.card_var(c, VarKind::CalcBase) + cx.card_var(c, VarKind::CalcExtra) * 2) as f32);
    // Burn: unpowered damage gets no Strength (v1 added it)
    let burn = cx.card_var(hand[4], VarKind::Damage) as f32;
    assert_eq!(hand_entry(&v, 4)[DMG], burn);
    assert_eq!(v1[sec(1, "hand").0 + 4 * observe::CARD_F + DMG], burn + 2.0);
    // Normality: 3 cards left to play this turn
    assert_eq!(hand_entry(&v, 5)[COUNT], 3.0);
    // v1 shows none of the calculated numbers (Perfected Strike, Body Slam, Expect a Fight)
    for k in [0, 1, 3] {
        let o = sec(1, "hand").0 + k * observe::CARD_F;
        assert_eq!((v1[o + DMG], v1[o + BLK]), (0.0, 0.0), "v1 card {k}");
    }

    // Unleash: Osty attacks for 6 + Osty's HP, in the Osty damage field (v1: 0)
    let mut cx = necro(2);
    set_hand(&mut cx, &[(ids::card::UNLEASH, 0)]);
    let o = cx.osty().unwrap();
    assert_eq!(osty_dmg(&obs(&cx, 2), 0), (6 + cx.cr(o).hp) as f32);
    assert_eq!(obs(&cx, 1)[sec(1, "osty").0 + 4 + 2 * OBS_POWERS], 0.0);
}

#[test]
fn card_state_fields_and_single_turn_keywords() {
    let mut cx = ironclad(3);
    let hand = set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    cx.cards[hand[0] as usize].flags |= cflag::SINGLE_TURN_RETAIN;
    cx.cards[hand[1] as usize].flags |= cflag::SINGLE_TURN_SLY;
    cx.cards[hand[1] as usize].affliction = ids::affliction::BOUND as u8 + 1;
    cx.cards[hand[1] as usize].affliction_amount = 3;
    cx.cards[hand[0] as usize].base_replay = 2;
    let v = obs(&cx, 2);
    assert!(hand_entry(&v, 0)[4] as u8 & kw::RETAIN != 0, "single-turn Retain shows as Retain");
    assert!(hand_entry(&v, 1)[4] as u8 & kw::SLY != 0, "single-turn Sly shows as Sly");
    assert_eq!(hand_entry(&v, 1)[AFF_AMT], 3.0);
    assert_eq!(hand_entry(&v, 0)[REPLAY], 2.0);
    let v1 = obs(&cx, 1);
    assert_eq!(v1[sec(1, "hand").0 + 4] as u8 & kw::RETAIN, 0, "v1 unchanged");
}

/// The number a power's icon shows instead of its amount (`DisplayAmount`), in the third float of the power slot.
#[test]
fn power_display_numbers() {
    let mut cx = ironclad(4);
    for id in [ids::power::PANACHE_POWER, ids::power::AUTOMATION_POWER, ids::power::SURROUNDED_POWER] {
        cx.apply_power(id, PLAYER, Dec::int(2), PLAYER, NO);
    }
    let v = obs(&cx, 2);
    let (po, _) = sec(2, "player");
    let slots: Vec<(f32, f32, f32)> = (0..OBS_POWERS).map(|k| (v[po + 8 + 3 * k], v[po + 9 + 3 * k], v[po + 10 + 3 * k])).collect();
    let find = |id: u16| slots.iter().find(|s| s.0 == id as f32 + 1.0).copied().unwrap();
    assert_eq!(find(ids::power::PANACHE_POWER).2, 5.0); // 5 cards left
    assert_eq!(find(ids::power::AUTOMATION_POWER).2, 10.0); // 10 cards left
    assert_eq!(find(ids::power::SURROUNDED_POWER).2, 1.0); // facing right
    // Panache counts down from the second card played (the first is the one that applied it: `alreadyApplied`)
    set_hand(&mut cx, &[(ids::card::DEFEND_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    let v = obs(&cx, 2);
    let k = (0..OBS_POWERS).find(|&k| v[po + 8 + 3 * k] == ids::power::PANACHE_POWER as f32 + 1.0).unwrap();
    assert_eq!(v[po + 10 + 3 * k], 4.0);
}

/// A selection asked by a card in play shows its source and the card; one asked by a relic / potion / monster shows that kind and id.
#[test]
fn selection_source_and_card_in_play() {
    let mut cx = ironclad(5);
    let e = cx.enemies[0];
    for id in [ids::card::BASH, ids::card::TWIN_STRIKE] {
        let c = cx.new_card(id, 0).unwrap();
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    set_hand(&mut cx, &[(ids::card::HEADBUTT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.stage, Stage::AwaitChoice, "Headbutt asks for a discard-pile card");
    let v = obs(&cx, 2);
    let (so, _) = sec(2, "dec_source");
    assert_eq!((v[so], v[so + 1]), (1.0, ids::card::HEADBUTT as f32 + 1.0));
    let (pl, sz) = sec(2, "played");
    assert_eq!(sz, observe::PLAYED_F);
    assert_eq!(v[pl], ids::card::HEADBUTT as f32 + 1.0);
    assert!(v[pl + DMG] > 0.0, "the played card's damage preview");
    // no selection, no card in play: zeros
    let cx2 = ironclad(5);
    let v2 = obs(&cx2, 2);
    assert!(v2[so..so + 2].iter().chain(v2[pl..pl + sz].iter()).all(|&x| x == 0.0));
    // relic / potion / monster purposes
    for (purpose, kind, id) in [
        (purpose::relic(ids::relic::TOASTY_MITTENS), 3.0, ids::relic::TOASTY_MITTENS),
        (purpose::potion(ids::potion::BLOCK_POTION), 2.0, ids::potion::BLOCK_POTION),
        (purpose::monster(ids::monster::KNOWLEDGE_DEMON), 4.0, ids::monster::KNOWLEDGE_DEMON),
    ] {
        let mut c = ironclad(6);
        assert!(matches!(c.ask_hand(purpose, 1, 1, |_, _| true), Ask::Pending));
        c.stage = Stage::AwaitChoice;
        let v = obs(&c, 2);
        assert_eq!((v[so], v[so + 1]), (kind, id as f32 + 1.0));
    }
}

/// More than 16 candidates: v2 shows all of them (every pickable one), in the displayed order.
#[test]
fn selection_shows_every_candidate() {
    let mut cx = ironclad(7);
    let pool = [ids::card::BASH, ids::card::TWIN_STRIKE, ids::card::SHRUG_IT_OFF, ids::card::OFFERING, ids::card::IMPERVIOUS, ids::card::POMMEL_STRIKE];
    for k in 0..40 {
        let c = cx.new_card(pool[k % pool.len()], (k / pool.len() % 2) as u8).unwrap();
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    assert!(matches!(cx.ask_pile(0, PileType::Discard, 1, 1, |_, _| true), Ask::Pending));
    cx.stage = Stage::AwaitChoice;
    let d = cx.decision.unwrap();
    let n = d.cands.len();
    assert_eq!(n, 40);
    let mut acts = ActionBuf::new();
    cx.legal_actions(&mut acts);
    assert!(acts.iter().any(|a| matches!(a, Action::Pick { idx } if *idx as usize >= 16)), "picks past 16 are legal");
    let v = obs(&cx, 2);
    let c2: std::collections::HashMap<_, _> = layout_consts_v(2).into_iter().collect();
    assert_eq!(c2["OBS_MAX_CANDS"], 64);
    let (dec, _) = sec(2, "decision");
    let view = cx.decision_view(&d);
    for k in 0..64 {
        let id = v[dec + 8 + k * (CARD_F_V2 + 1)];
        if k < n.min(64) {
            assert_eq!(id, cx.cards[d.cands[view[k] as usize] as usize].id as f32 + 1.0, "candidate {k}");
        } else {
            assert_eq!(id, 0.0);
        }
    }
    // the rares at the end of the rarity-sorted screen are visible in v2, cut in v1
    let rare_shown = (16..n.min(64)).any(|k| content::card_def(cx.cards[d.cands[view[k] as usize] as usize].id).rarity as u8 >= 3);
    assert!(rare_shown);
    let v1 = obs(&cx, 1);
    let (dec1, _) = sec(1, "decision");
    assert_eq!(v1[dec1 + 8 + 15 * (observe::CARD_F + 1)], cx.cards[d.cands[view[15] as usize] as usize].id as f32 + 1.0);
}
