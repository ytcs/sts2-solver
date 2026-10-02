//! Act 2 "Hive" elites and bosses (hive_b slice): smoke runs of every encounter plus the non-obvious rules
//! (Decimillipede Reattach, Entomancer's Personal Hive, Infested Prism's Tainted skills, Kaiser Crab, Knowledge Demon's
//! Curse of Knowledge decision in the middle of the enemy turn, The Insatiable's Sandpit). Fidelity itself is validated by
//! the oracle sweeps (`oracle/templates/hive_b_*.json`).
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(enc: u16, seed: u64, hp: i32) -> Scenario {
    let mut deck = vec![];
    for _ in 0..8 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
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

/// Plays the first playable card at the first hittable enemy, else ends the turn; decisions pick the first candidate.
fn greedy_step(cx: &mut Combat) {
    if cx.stage == Stage::AwaitChoice {
        assert!(cx.step(Action::Pick { idx: 0 }) || cx.step(Action::Confirm));
        return;
    }
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
        greedy_step(cx);
        steps += 1;
        assert!(steps < 4000, "fight does not terminate");
    }
}

const HIVE_B: [u16; 6] = [
    ids::encounter::DECIMILLIPEDE_ELITE,
    ids::encounter::ENTOMANCER_ELITE,
    ids::encounter::INFESTED_PRISMS_ELITE,
    ids::encounter::KAISER_CRAB_BOSS,
    ids::encounter::KNOWLEDGE_DEMON_BOSS,
    ids::encounter::THE_INSATIABLE_BOSS,
];

#[test]
fn every_hive_elite_and_boss_runs_to_the_end() {
    for &enc in HIVE_B.iter() {
        for seed in 0..6 {
            let mut cx = Combat::new(&scenario(enc, seed, 3000));
            play_out(&mut cx);
            assert!(cx.missing.is_none(), "unported content in {}: {:?}", ids::encounter::NAMES[enc as usize], cx.missing);
        }
    }
}

fn find_in_hand(cx: &Combat, id: u16) -> Option<u8> {
    cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id).map(|p| p as u8)
}

#[test]
fn decimillipede_segments_have_distinct_even_max_hp_and_reattach() {
    let mut cx = Combat::new(&scenario(ids::encounter::DECIMILLIPEDE_ELITE, 2, 2000));
    assert_eq!(cx.enemies.len(), 3);
    let hps: Vec<i32> = cx.enemies.iter().map(|&e| cx.cr(e).max_hp).collect();
    for (i, &h) in hps.iter().enumerate() {
        assert!(h % 2 == 0 && (46..=52).contains(&h), "segment max HP {h}");
        assert!(!hps[i + 1..].contains(&h), "segment max HPs must differ: {hps:?}");
        assert!(cx.has_power(cx.enemies[i], ids::power::REATTACH_POWER));
    }
    // kill the first segment: it stays in the enemy list, dead and unhittable, with the DEAD_MOVE pending
    let seg = cx.enemies[0];
    cx.cr_mut(seg).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: seg }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.enemies.len(), 3);
    assert!(cx.cr(seg).is_dead());
    assert!(!cx.hittable_enemies().contains(seg));
    assert_eq!(cx.move_view(seg).map(|m| m.0), Some("DEAD_MOVE"));
    // DEAD_MOVE this turn, REATTACH_MOVE next turn: it comes back with 25 HP and is hittable again
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.move_view(seg).map(|m| m.0), Some("REATTACH_MOVE"));
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(seg).hp, 25);
    assert!(cx.hittable_enemies().contains(seg));
}

#[test]
fn decimillipede_last_segment_death_ends_the_fight_even_with_others_dead() {
    let mut cx = Combat::new(&scenario(ids::encounter::DECIMILLIPEDE_ELITE, 4, 2000));
    let segs: Vec<Cid> = cx.enemies.iter().copied().collect();
    cx.cr_mut(segs[0]).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: segs[0] }));
    cx.cr_mut(segs[1]).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a second Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: segs[1] }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    cx.cr_mut(segs[2]).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a third Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: segs[2] }));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}

#[test]
fn entomancer_adds_dazed_to_the_draw_pile_when_hit() {
    let mut cx = Combat::new(&scenario(ids::encounter::ENTOMANCER_ELITE, 1, 2000));
    let e = cx.enemies[0];
    assert_eq!(cx.power_amount(e, ids::power::PERSONAL_HIVE_POWER), 1);
    let draw_before = cx.player.draw.len();
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
    assert_eq!(cx.player.draw.len(), draw_before + 1);
    assert!(cx.player.draw.iter().any(|&c| cx.cards[c as usize].id == ids::card::DAZED));
}

#[test]
fn infested_prism_taints_skills_and_tainted_plays_add_damage_taken() {
    let mut cx = Combat::new(&scenario(ids::encounter::INFESTED_PRISMS_ELITE, 1, 2000));
    let mut cx = cx;
    // make sure a Defend (Skill) is in hand: swap one into the first slot if needed
    let pos = match find_in_hand(&cx, ids::card::DEFEND_IRONCLAD) {
        Some(p) => p,
        None => {
            let d = cx.player.draw.iter().copied().find(|&c| cx.cards[c as usize].id == ids::card::DEFEND_IRONCLAD).expect("a Defend in the draw pile");
            let h = cx.player.hand[0];
            cx.player.draw.remove_value(d);
            cx.player.hand.remove_value(h);
            cx.player.hand.push(d);
            cx.player.draw.push(h);
            cx.cards[d as usize].pile = PileType::Hand as u8;
            cx.cards[h as usize].pile = PileType::Draw as u8;
            cx.player.hand.len() as u8 - 1
        }
    };
    let card = cx.player.hand[pos as usize];
    assert_eq!(cx.card_affliction(card), Some(ids::affliction::TAINTED));
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: NO }));
    // VitalSpark 3 at A10: +3 damage from every powered attack for the rest of the enemy turn
    assert_eq!(cx.power_amount(PLAYER, ids::power::TAINTED_POWER), 3);
    assert!(cx.step(Action::EndTurn));
    assert!(!cx.has_power(PLAYER, ids::power::TAINTED_POWER));
}

#[test]
fn kaiser_crab_surrounded_flips_when_targeting_the_other_crab_and_crab_rage_triggers() {
    let mut cx = Combat::new(&scenario(ids::encounter::KAISER_CRAB_BOSS, 1, 2000));
    let (crusher, rocket) = (cx.enemies[0], cx.enemies[1]);
    assert_eq!(cx.cr(crusher).monster.id, ids::monster::CRUSHER);
    assert_eq!(cx.cr(rocket).monster.id, ids::monster::ROCKET);
    let surrounded = cx.cr(PLAYER).power(ids::power::SURROUNDED_POWER).expect("Surrounded on the player").uid;
    assert_eq!(cx.power_aux(PLAYER, surrounded), 0, "facing right: the BackAttackLeft crab (Crusher) is behind");
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: rocket }));
    assert_eq!(cx.power_aux(PLAYER, surrounded), 0, "targeting the crab already in front does not flip");
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a second Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: crusher }));
    assert_eq!(cx.power_aux(PLAYER, surrounded), 1, "targeting the BackAttackLeft crab turns the player around");
    // killing Crusher enrages Rocket: Strength +6, 99 block, CrabRage consumed
    cx.cr_mut(crusher).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a third Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: crusher }));
    assert_eq!(cx.power_amount(rocket, ids::power::STRENGTH_POWER), 6);
    assert_eq!(cx.cr(rocket).block, 99);
    assert!(!cx.has_power(rocket, ids::power::CRAB_RAGE_POWER));
}

#[test]
fn knowledge_demon_curse_of_knowledge_asks_for_a_choice_in_the_enemy_turn() {
    let mut cx = Combat::new(&scenario(ids::encounter::KNOWLEDGE_DEMON_BOSS, 1, 2000));
    assert!(cx.step(Action::EndTurn));
    // the enemy turn is suspended inside CURSE_OF_KNOWLEDGE_MOVE: choose between Disintegration and Mind Rot
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let d = cx.decision.as_ref().expect("a pending decision");
    assert_eq!(d.cands.len(), 2);
    assert_eq!(cx.cards[d.cands[0] as usize].id, ids::card::DISINTEGRATION);
    assert_eq!(cx.cards[d.cands[1] as usize].id, ids::card::MIND_ROT);
    assert!(cx.step(Action::Pick { idx: 1 }));
    // the turn resumed: the player holds MindRot 1 and the next player turn started
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.power_amount(PLAYER, ids::power::MIND_ROT_POWER), 1);
    assert_eq!(cx.player.turn_number, 2);
    // MindRot: 5 - 1 cards drawn on the next hand draw (turn 2 was already drawn with the power in place)
    assert_eq!(cx.player.hand.len(), 4);
    assert_eq!(cx.cr(cx.enemies[0]).monster.vars[0], 1, "CurseOfKnowledgeCounter advanced");
}

#[test]
fn insatiable_sandpit_counts_down_and_kills_the_player() {
    let mut cx = Combat::new(&scenario(ids::encounter::THE_INSATIABLE_BOSS, 1, 5000));
    assert!(cx.step(Action::EndTurn)); // LIQUIFY_GROUND: Sandpit 4 + 6 FranticEscape
    let boss = cx.enemies[0];
    let sp = cx.cr(boss).power(ids::power::SANDPIT_POWER).expect("Sandpit on the Insatiable");
    // Sandpit ticks at the start of each enemy turn: 4 -> 3 on the same turn it was applied? (applied during the move,
    // i.e. after the tick), so it reads 4 now
    assert_eq!(sp.amount, 4);
    let frantic = cx.all_combat_cards().iter().filter(|&&c| cx.cards[c as usize].id == ids::card::FRANTIC_ESCAPE).count();
    assert_eq!(frantic, 6);
    for _ in 0..3 {
        assert!(cx.step(Action::EndTurn));
        assert_eq!(cx.stage, Stage::AwaitAction);
    }
    // 4th enemy turn start: 1 -> 0, the Sandpit is removed and swallows the player
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Defeat);
}
