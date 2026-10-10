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
