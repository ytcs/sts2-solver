//! Act 1b (Underdocks) rules that are easy to get wrong: stun follow-ups, death-triggered spawns, revive-style bosses.
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn combat(encounter: u16, seed: u64) -> Combat {
    let mut deck = vec![];
    for _ in 0..10 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    Combat::new(&Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter,
        max_hp: 999,
        hp: 999,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    })
}

fn move_id(cx: &Combat, e: Cid) -> &'static str {
    let def = content::monster_def(cx.cr(e).monster.id);
    let nm = cx.cr(e).monster.next_move;
    if nm == engine::STUN_NODE {
        return "STUNNED";
    }
    match &def.nodes[nm as usize] {
        defs::MonsterNode::Move { id, .. } => id,
        _ => "?",
    }
}

fn strike_at(cx: &mut Combat, target: Cid) {
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike in hand");
    assert!(cx.step(Action::PlayCard { hand_pos: pos as u8, target }));
}

#[test]
fn ravenous_slug_is_stunned_and_resumes_its_interrupted_move() {
    let mut cx = combat(ids::encounter::CORPSE_SLUGS_NORMAL, 3);
    assert_eq!(cx.enemies.len(), 3);
    let (a, b) = (cx.enemies[0], cx.enemies[1]);
    let pending_b = move_id(&cx, b);
    let str_before = cx.power_amount(b, ids::power::STRENGTH_POWER);
    cx.cr_mut(a).hp = 1;
    strike_at(&mut cx, a);
    assert!(!cx.enemies.contains(a));
    // the survivors are stunned (their pending move is replaced) and gained Ravenous Strength (5 at A10)
    assert_eq!(move_id(&cx, b), "STUNNED");
    assert_eq!(cx.power_amount(b, ids::power::STRENGTH_POWER), str_before + 5);
    // end the turn: the stun is performed, next turn the interrupted move is rolled again (no RNG draw for the move)
    assert!(cx.step(Action::EndTurn));
    assert_eq!(move_id(&cx, b), pending_b);
}

#[test]
fn gremlin_merc_death_spawns_sneaky_then_fat_and_keeps_combat_open() {
    let mut cx = combat(ids::encounter::GREMLIN_MERC_NORMAL, 5);
    let merc = cx.enemies[0];
    assert_eq!(cx.cr(merc).monster.id, ids::monster::GREMLIN_MERC);
    cx.cr_mut(merc).hp = 1;
    strike_at(&mut cx, merc);
    assert_eq!(cx.stage, Stage::AwaitAction, "combat must stay open (SurprisePower)");
    let ids_now: Vec<u16> = cx.enemies.iter().map(|&e| cx.cr(e).monster.id).collect();
    assert_eq!(ids_now, vec![ids::monster::SNEAKY_GREMLIN, ids::monster::FAT_GREMLIN]);
    // spawned during the player's turn: both are rolled immediately (SPAWNED_MOVE = stun intent)
    for &e in cx.enemies.iter() {
        assert_eq!(move_id(&cx, e), "SPAWNED_MOVE");
    }
}

#[test]
fn waterfall_giant_revives_into_about_to_blow_and_explodes() {
    let mut cx = combat(ids::encounter::WATERFALL_GIANT_BOSS, 7);
    let g = cx.enemies[0];
    // first move: PRESSURIZE builds Steam Eruption 20; one enemy turn passes
    assert!(cx.step(Action::EndTurn));
    let steam = cx.power_amount(g, ids::power::STEAM_ERUPTION_POWER);
    assert_eq!(steam, 20);
    cx.cr_mut(g).hp = 1;
    strike_at(&mut cx, g);
    assert_eq!(cx.stage, Stage::AwaitAction, "combat stays open while the Giant is about to blow");
    assert_eq!(cx.cr(g).hp, 999_999_999);
    assert_eq!(move_id(&cx, g), "ABOUT_TO_BLOW_MOVE");
    // ABOUT_TO_BLOW (stun) then EXPLODE (death blow of `steam` damage); the Giant dies on its own move
    assert!(cx.step(Action::EndTurn));
    assert_eq!(move_id(&cx, g), "EXPLODE_MOVE");
    assert_eq!(cx.power_amount(g, ids::power::STEAM_ERUPTION_POWER), 0);
    let hp = cx.cr(PLAYER).hp;
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
    assert!(cx.cr(PLAYER).hp <= hp);
}

#[test]
fn living_fog_bomb_dies_with_the_fog() {
    let mut cx = combat(ids::encounter::LIVING_FOG_NORMAL, 11);
    let fog = cx.enemies[0];
    // ADVANCED_GAS, then BLOAT summons a GasBomb into the first free slot (bomb1 < livingFog)
    assert!(cx.step(Action::EndTurn));
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.enemies.len(), 2);
    assert_eq!(cx.cr(cx.enemies[0]).monster.id, ids::monster::GAS_BOMB, "bomb slots sort before the fog");
    assert_eq!(cx.cr(cx.enemies[1]).monster.id, ids::monster::LIVING_FOG);
    cx.cr_mut(fog).hp = 1;
    strike_at(&mut cx, fog);
    assert_eq!(cx.stage, Stage::Over, "the Minion bomb does not keep combat open");
    assert_eq!(cx.outcome, Outcome::Victory);
}
