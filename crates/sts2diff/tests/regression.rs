use std::path::PathBuf;
use sts2diff::diff::{replay, Verdict};

#[test]
fn oracle_regressions() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/regression");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".scenario.json")).map(str::to_string)).collect())
        .unwrap_or_default();
    names.sort();
    let mut bad = vec![];
    for n in &names {
        let sc = dir.join(format!("{n}.scenario.json"));
        let tr = dir.join(format!("{n}.jsonl"));
        match replay(sc.to_str().unwrap(), tr.to_str().unwrap(), 4, false) {
            Ok(Verdict::Match) => {}
            other => bad.push(format!("{n}: {other:?}")),
        }
    }
    assert!(bad.is_empty(), "regression mismatches: {bad:?}");
}

#[test]
fn touch_of_orobas_saved_relic_ids_round_trip() {
    let props = serde_json::json!({"StarterRelic": "RELIC.BURNING_BLOOD", "UpgradedRelic": "RELIC.BLACK_BLOOD"});
    let sc = serde_json::json!({"encounter": "NIBBITS_WEAK", "character": "IRONCLAD", "hp": 80, "seed": "orobas", "deck": vec!["STRIKE_IRONCLAD"; 5],
        "relics": ["BLACK_BLOOD", {"id": "TOUCH_OF_OROBAS", "props": props}], "potions": []});
    let cx = sts2sim::Combat::new(&sts2diff::convert::scenario(&sc).expect("Touch of Orobas scenario builds"));
    assert_eq!(sts2diff::snapshot::snapshot(&cx)["relics"][1]["props"], props);
    let real = serde_json::json!([{"id": "BLACK_BLOOD"}, {"id": "TOUCH_OF_OROBAS", "props": props}]);
    let mut synced = cx.clone();
    assert_eq!(synced.sync_relics(&sts2diff::convert::obs_relics(&real)).changed, 0);
}

#[test]
fn toy_box_wax_relics_build_and_melted_drop() {
    let sc = serde_json::json!({"encounter": "NIBBITS_WEAK", "character": "IRONCLAD", "hp": 80, "seed": "wax", "deck": vec!["STRIKE_IRONCLAD"; 5],
        "relics": ["BURNING_BLOOD", {"id": "VAJRA", "props": {"IsWax": true}}, {"id": "ANCHOR", "props": {"IsWax": true, "IsMelted": true}}], "potions": []});
    let cx = sts2sim::Combat::new(&sts2diff::convert::scenario(&sc).expect("wax relic scenario builds"));
    let ids: Vec<String> = sts2diff::snapshot::snapshot(&cx)["relics"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(ids.len(), 2, "melted Anchor dropped: {ids:?}");
    let real = serde_json::json!([{"id": "BURNING_BLOOD"}, {"id": "VAJRA", "props": {"IsWax": true}}, {"id": "ANCHOR", "props": {"IsWax": true, "IsMelted": true}}]);
    let mut synced = cx.clone();
    let rep = synced.sync_relics(&sts2diff::convert::obs_relics(&real));
    assert_eq!((rep.changed, rep.unpaired, rep.unknown_props.len()), (0, 0, 0));
}

#[test]
fn thrash_keeps_its_exhaust_bonus_through_dampen() {
    use sts2sim::engine::{Action, ObsCard};
    use sts2sim::{dec::Dec, defs::VarKind, ids, state::PLAYER, types::NO, Combat};
    let sc = serde_json::json!({"encounter": "KNIGHTS_ELITE", "character": "IRONCLAD", "ascension": 10, "hp": 85, "seed": "thrash",
        "deck": [{"id": "THRASH", "upgrade": 1}, {"id": "STRIKE_IRONCLAD", "upgrade": 0}], "relics": [], "potions": []});
    let mut cx = Combat::new(&sts2diff::convert::scenario(&sc).expect("knights scenario builds"));
    let foes: Vec<_> = cx.enemies.iter().copied().collect();
    let thrash = (0..cx.n_cards).find(|&c| cx.cards[c as usize].id == ids::card::THRASH).unwrap() as u8;
    let hit = |cx: &mut Combat, e| {
        let pos = cx.player.hand.iter().position(|&c| c == thrash).unwrap() as u8;
        let hp = cx.cr(e).hp;
        assert!(cx.step(Action::PlayCard { hand_pos: pos, target: e }));
        hp - cx.cr(e).hp
    };
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(3), PLAYER, NO);
    assert_eq!(hit(&mut cx, foes[0]), 2 * (6 + 3));
    assert_eq!(cx.card_var(thrash, VarKind::Damage), 6 + 9);
    cx.apply_power(ids::power::DAMPEN_POWER, PLAYER, Dec::ONE, foes[2], NO);
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::ONE, PLAYER, NO);
    // Thrash.cs AfterDowngraded: canonical 4 + ExtraDamage (Strike 6 + Strength 3)
    assert_eq!((cx.cards[thrash as usize].upgrade, cx.card_var(thrash, VarKind::Damage)), (0, 4 + 9));
    cx.sync_hand(&[ObsCard { id: ids::card::THRASH, ..Default::default() }, ObsCard { id: ids::card::BASH, ..Default::default() }]);
    assert_eq!(hit(&mut cx, foes[1]), 34);
    cx.sync_hand(&[ObsCard { id: ids::card::THRASH, ..Default::default() }]);
    assert_eq!(hit(&mut cx, foes[2]), 58);
}
