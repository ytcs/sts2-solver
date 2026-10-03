//! Replays the frozen fuzz findings of `oracle/regression/` (scenario + truncated real-game trace, see
//! `tools/mk_regression.py`) against the simulator: every pair must match step for step.
use sts2diff::diff::{self, Verdict};

#[test]
fn oracle_regression_traces_match() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/regression");
    let mut n = 0;
    let mut bad = vec![];
    let mut entries: Vec<_> = std::fs::read_dir(&dir).expect("oracle/regression").filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(".scenario.json") {
            let trace = p.with_file_name(format!("{stem}.jsonl"));
            match diff::replay(&p.to_string_lossy(), &trace.to_string_lossy(), 3, true) {
                Ok(Verdict::Match) => n += 1,
                Ok(_) => bad.push(format!("{stem}: mismatch/unimplemented")),
                Err(e) => bad.push(format!("{stem}: {e}")),
            }
        }
    }
    assert!(bad.is_empty(), "regression traces diverge: {bad:?}");
    assert!(n > 0, "no regression traces found");
}
