//! Act 3 "Glory" elites / bosses (Knights, Mecha Knight, Soul Nexus, Queen, Test Subject, Aeonglass): smoke runs plus the
//! non-obvious rules (Hex / Dampen / Chains of Binding / Withering Presence / Test Subject respawns / Queen enrage).
//! Fidelity itself is validated by the oracle sweeps (`oracle/templates/glory_b_*.json`).
use sts2sim::defs::VarKind;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(enc: u16, seed: u64, hp: i32, upgraded: bool) -> Scenario {
    let mut deck = vec![];
    for _ in 0..8 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: upgraded as u8 });
    }
    for _ in 0..2 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: enc,
        max_hp: hp,
        hp,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0 }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

fn greedy_step(cx: &mut Combat) {
    let target = cx.hittable_enemies().first().unwrap_or(NO);
    for pos in 0..cx.player.hand.len() {
        let c = cx.player.hand[pos];
        if cx.can_play(c) {
            let t = if cx.card_def(c).target == TargetType::AnyEnemy { target } else { NO };
            if cx.step(Action::PlayCard { hand_pos: pos as u8, target: t }) {
                return;
            }
        }
    }
    assert!(cx.step(Action::EndTurn));
}

fn play_out(cx: &mut Combat) {
    let mut steps = 0;
    while cx.stage != Stage::Over {
        assert_eq!(cx.stage, Stage::AwaitAction);
        greedy_step(cx);
        steps += 1;
        assert!(steps < 4000, "fight does not terminate");
        // The card arena is capped (`MAX_CARDS`): a very long Test Subject fight (a Wound per hit, Burns every 3rd turn)
        // can fill it; stop the smoke run before that.
        if cx.n_cards as usize + 12 >= MAX_CARDS {
            break;
        }
    }
}

const GLORY_B: [u16; 6] = [
    ids::encounter::KNIGHTS_ELITE,
    ids::encounter::MECHA_KNIGHT_ELITE,
    ids::encounter::SOUL_NEXUS_ELITE,
    ids::encounter::AEONGLASS_BOSS,
    ids::encounter::QUEEN_BOSS,
    ids::encounter::TEST_SUBJECT_BOSS,
];

#[test]
fn every_glory_b_encounter_runs_to_the_end() {
    for &enc in GLORY_B.iter() {
        for seed in 0..6 {
            let mut cx = Combat::new(&scenario(enc, seed, 4000, false));
            play_out(&mut cx);
            assert!(cx.missing.is_none(), "unported content in {}: {:?}", ids::encounter::NAMES[enc as usize], cx.missing);
        }
    }
}

fn find_in_hand(cx: &Combat, id: u16) -> Option<u8> {
    cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id).map(|p| p as u8)
}

/// Sets `target` to 1 HP and kills it with a Strike from the hand.
fn strike_kill(cx: &mut Combat, target: Cid) {
    cx.cr_mut(target).hp = 1;
    cx.cr_mut(target).block = 0;
    let pos = find_in_hand(cx, ids::card::STRIKE_IRONCLAD).expect("a Strike in hand");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target }));
}

#[test]
fn knights_enemy_order_is_creation_order_and_hex_makes_cards_ethereal_until_the_spectral_knight_dies() {
    let mut cx = Combat::new(&scenario(ids::encounter::KNIGHTS_ELITE, 2, 4000, false));
    let ids_in_order: Vec<u16> = cx.enemies.iter().map(|&e| cx.cr(e).monster.id).collect();
    assert_eq!(ids_in_order, vec![ids::monster::FLAIL_KNIGHT, ids::monster::SPECTRAL_KNIGHT, ids::monster::MAGI_KNIGHT]);
    assert!(cx.step(Action::EndTurn)); // the Spectral Knight opens with HEX
    assert!(cx.has_power(PLAYER, ids::power::HEX_POWER));
    for &c in cx.player.hand.iter() {
        assert_eq!(cx.card_affliction(c), Some(ids::affliction::HEXED));
        assert!(cx.card_keywords(c) & kw::ETHEREAL != 0);
    }
    let spectral = cx.enemies[1];
    strike_kill(&mut cx, spectral);
    assert!(!cx.has_power(PLAYER, ids::power::HEX_POWER));
    for c in cx.all_combat_cards().iter() {
        assert_eq!(cx.card_affliction(*c), None);
    }
}

#[test]
fn dampen_downgrades_upgraded_cards_until_the_magi_knight_dies() {
    let mut cx = Combat::new(&scenario(ids::encounter::KNIGHTS_ELITE, 4, 4000, true));
    // MagiKnight: POWER_SHIELD (turn 1) -> DAMPEN (turn 2)
    assert!(cx.step(Action::EndTurn));
    assert!(!cx.has_power(PLAYER, ids::power::DAMPEN_POWER));
    assert!(cx.step(Action::EndTurn));
    assert!(cx.has_power(PLAYER, ids::power::DAMPEN_POWER));
    let strikes = |cx: &Combat| -> Vec<u8> {
        cx.all_combat_cards().iter().filter(|&&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).map(|&c| cx.cards[c as usize].upgrade).collect()
    };
    assert_eq!(strikes(&cx).len(), 8);
    assert!(strikes(&cx).iter().all(|&u| u == 0), "every Strike+ was downgraded");
    let magi = cx.enemies[2];
    strike_kill(&mut cx, magi);
    assert!(!cx.has_power(PLAYER, ids::power::DAMPEN_POWER));
    assert!(strikes(&cx).iter().all(|&u| u == 1), "the upgrades come back with the Magi Knight's death");
}

#[test]
fn queen_enrages_when_the_amalgam_dies_during_burn_bright() {
    let mut cx = Combat::new(&scenario(ids::encounter::QUEEN_BOSS, 1, 4000, false));
    let ids_in_order: Vec<u16> = cx.enemies.iter().map(|&e| cx.cr(e).monster.id).collect();
    assert_eq!(ids_in_order, vec![ids::monster::TORCH_HEAD_AMALGAM, ids::monster::QUEEN]);
    assert!(cx.step(Action::EndTurn)); // PUPPET_STRINGS: Chains of Binding 3
    assert_eq!(cx.power_amount(PLAYER, ids::power::CHAINS_OF_BINDING_POWER), 3);
    // the first three cards drawn this turn are Bound; only one Bound card may be played
    let bound: Vec<usize> = (0..cx.player.hand.len()).filter(|&i| cx.card_affliction(cx.player.hand[i]) == Some(ids::affliction::BOUND)).collect();
    assert_eq!(bound.len(), 3);
    assert!(cx.can_play(cx.player.hand[bound[0]]));
    let amalgam = cx.enemies[0];
    let t = cx.hittable_enemies().first().unwrap_or(NO);
    assert_eq!(t, amalgam);
    assert!(cx.step(Action::PlayCard { hand_pos: bound[0] as u8, target: amalgam }));
    // after playing a Bound card, the other Bound cards are unplayable (one of them may have shifted left)
    let still_bound: Vec<u8> = (0..cx.player.hand.len() as u8).filter(|&i| cx.card_affliction(cx.player.hand[i as usize]) == Some(ids::affliction::BOUND)).collect();
    assert_eq!(still_bound.len(), 2);
    for i in still_bound {
        assert!(!cx.can_play(cx.player.hand[i as usize]));
    }
    assert!(cx.step(Action::EndTurn)); // YOU_ARE_MINE: Frail / Weak / Vulnerable 99
    assert_eq!(cx.power_amount(PLAYER, ids::power::VULNERABLE_POWER), 99);
    // the Queen now shows BURN_BRIGHT_FOR_ME; killing the Amalgam swaps it for ENRAGE
    let queen = cx.enemies[1];
    assert_eq!(cx.move_view(queen).map(|m| m.0), Some("BURN_BRIGHT_FOR_ME_MOVE"));
    let amalgam = cx.enemies[0];
    strike_kill(&mut cx, amalgam);
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.cr(queen).monster.vars[0], 1);
    assert_eq!(cx.move_view(queen).map(|m| m.0), Some("ENRAGE_MOVE"));
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(queen, ids::power::STRENGTH_POWER), 2);
    assert_eq!(cx.move_view(queen).map(|m| m.0), Some("OFF_WITH_YOUR_HEAD_MOVE"));
}

#[test]
fn test_subject_respawns_twice_with_new_hp_and_powers() {
    let mut cx = Combat::new(&scenario(ids::encounter::TEST_SUBJECT_BOSS, 3, 4000, false));
    let ts = cx.enemies[0];
    assert_eq!(cx.cr(ts).max_hp, 111);
    assert!(cx.has_power(ts, ids::power::ADAPTABLE_POWER) && cx.has_power(ts, ids::power::ENRAGE_POWER));
    strike_kill(&mut cx, ts);
    // dead but not removed: unhittable, combat goes on
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.cr(ts).is_dead() && cx.enemies.contains(ts) && !cx.hittable_enemies().contains(ts));
    assert!(cx.step(Action::EndTurn)); // RESPAWN_MOVE
    assert!(cx.cr(ts).is_alive());
    assert_eq!((cx.cr(ts).hp, cx.cr(ts).max_hp), (212, 212));
    assert!(cx.has_power(ts, ids::power::PAINFUL_STABS_POWER) && cx.has_power(ts, ids::power::ADAPTABLE_POWER));
    assert_eq!(cx.cr(ts).monster.vars[0], 1);
    strike_kill(&mut cx, ts);
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.step(Action::EndTurn));
    assert_eq!((cx.cr(ts).hp, cx.cr(ts).max_hp), (313, 313));
    assert!(cx.has_power(ts, ids::power::NEMESIS_POWER));
    assert!(!cx.has_power(ts, ids::power::ADAPTABLE_POWER) && !cx.has_power(ts, ids::power::PAINFUL_STABS_POWER));
    // third form: no more revives
    strike_kill(&mut cx, ts);
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}

#[test]
fn aeonglass_withers_every_sixth_card_and_upgrades_them_with_increasing_intensity() {
    let mut cx = Combat::new(&scenario(ids::encounter::AEONGLASS_BOSS, 5, 4000, false));
    let ae = cx.enemies[0];
    assert_eq!(cx.power_amount(ae, ids::power::WITHERING_PRESENCE_POWER), 6);
    let withers = |cx: &Combat| -> Vec<CardIdx> {
        cx.all_combat_cards().iter().copied().filter(|&c| cx.cards[c as usize].id == ids::card::WITHER).collect()
    };
    // six cards played over two turns -> one Wither (base Damage 3) lands in the hand
    for _ in 0..3 {
        let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).or_else(|| find_in_hand(&cx, ids::card::DEFEND_IRONCLAD)).unwrap();
        let t = if cx.card_def(cx.player.hand[pos as usize]).target == TargetType::AnyEnemy { ae } else { NO };
        assert!(cx.step(Action::PlayCard { hand_pos: pos, target: t }));
    }
    assert!(cx.step(Action::EndTurn));
    for _ in 0..3 {
        let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).or_else(|| find_in_hand(&cx, ids::card::DEFEND_IRONCLAD)).unwrap();
        let t = if cx.card_def(cx.player.hand[pos as usize]).target == TargetType::AnyEnemy { ae } else { NO };
        assert!(cx.step(Action::PlayCard { hand_pos: pos, target: t }));
    }
    let w = withers(&cx);
    assert_eq!(w.len(), 1);
    assert_eq!(cx.card_var(w[0], VarKind::Damage), 3);
    assert!(cx.step(Action::EndTurn)); // EYE_LASERS ran at the end of turn 2... the Wither in hand hurt at turn end
    assert!(cx.step(Action::EndTurn)); // turn 3 enemy phase: INCREASING_INTENSITY
    let w = withers(&cx);
    assert!(w.len() >= 3);
    for &c in w.iter() {
        assert_eq!(cx.card_var(c, VarKind::Damage), 6, "every Wither is fake-upgraded once (+3)");
    }
    assert_eq!(cx.cr(ae).monster.vars[1], 1);
    assert_eq!(cx.power_amount(ae, ids::power::STRENGTH_POWER), 4);
}

