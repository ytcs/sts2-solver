//! Relic counters / saved properties from the bridge's JSON: the fight-start scenario (`convert::scenario_ex`) and a mid-fight
//! state (`convert::obs_relics` + `Combat::sync_relics`, what `sts2.Sim.sync` runs).
use serde_json::json;
use sts2sim::ids;
use sts2sim::{Action, Combat};

fn combat(pen_nib: serde_json::Value) -> Combat {
    let deck: Vec<_> = (0..10).map(|_| json!({"id": "STRIKE_IRONCLAD", "upgrade": 0})).collect();
    let sc = json!({
        "character": "IRONCLAD", "seed": "relic-sync", "encounter": "NIBBITS_WEAK", "hp": 80, "max_hp": 80, "ascension": 0,
        "deck": deck, "relics": [{"id": "BURNING_BLOOD"}, pen_nib], "potions": [],
    });
    let (sc, ex) = sts2diff::convert::scenario_ex(&sc).unwrap();
    Combat::try_new_with(&sc, &ex).unwrap()
}

/// Damage the first Strike in hand deals to the first enemy.
fn strike(cx: &mut Combat) -> i32 {
    let e = cx.enemies[0];
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike in hand");
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: pos as u8, target: e }));
    hp - cx.cr(e).hp
}

#[test]
fn fight_start_scenario_carries_pen_nib_at_nine() {
    let mut cx = combat(json!({"id": "PEN_NIB", "props": {"AttacksPlayed": 9}, "counter": 9}));
    assert_eq!(cx.player.relics[1].counter, 9);
    assert_eq!(strike(&mut cx), 12, "the 10th attack doubled");
    assert_eq!(strike(&mut cx), 6);
}

#[test]
fn bridge_state_relics_sync_pen_nib_to_nine() {
    let mut cx = combat(json!({"id": "PEN_NIB"}));
    assert_eq!(strike(&mut cx), 6);
    assert_eq!(cx.player.relics[1].counter, 1);
    // the game's state says 9 attacks (e.g. attacks the simulator did not see): the next one is doubled
    let state = json!({"relics": [{"id": "BURNING_BLOOD"}, {"id": "PEN_NIB", "props": {"AttacksPlayed": 9}, "counter": 9}, {"id": "NOT_A_RELIC"}]});
    let obs = sts2diff::convert::obs_relics(&state["relics"]);
    assert_eq!(obs.len(), 2, "unknown ids are left out");
    let rep = cx.sync_relics(&obs);
    assert_eq!((rep.changed, rep.unpaired), (1, 0));
    assert_eq!(cx.player.relics[1].counter, 9);
    assert_eq!(strike(&mut cx), 12);
    assert_eq!(strike(&mut cx), 6);
}

#[test]
fn string_props_are_ignored() {
    // Pael's Legion-style cosmetic string properties never reach the simulator
    let obs = sts2diff::convert::obs_relics(&json!([{"id": "PEN_NIB", "props": {"AttacksPlayed": 3, "Skin": "b"}}]));
    assert_eq!(obs[0].props, vec![("AttacksPlayed".to_string(), 3)]);
}
