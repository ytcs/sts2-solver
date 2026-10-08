pub mod convert;
pub mod diff;
pub mod snapshot;

use serde_json::Value;

pub fn load_jsonl(path: &str) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).map_err(|e| format!("{path}: {e}"))).collect()
}
