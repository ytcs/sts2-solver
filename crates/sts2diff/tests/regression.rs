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
