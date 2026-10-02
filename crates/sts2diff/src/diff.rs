//! Replay + compare.

use crate::{convert, load_jsonl, snapshot::snapshot};
use serde_json::Value;
use sts2sim::engine::ActionBuf;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

/// Every key of `rust` must be present and equal in `oracle`; arrays must match element-wise (and in length).
pub fn compare(path: &str, rust: &Value, oracle: &Value, out: &mut Vec<String>) {
    match (rust, oracle) {
        (Value::Object(r), Value::Object(o)) => {
            for (k, rv) in r {
                match o.get(k) {
                    Some(ov) => compare(&format!("{path}.{k}"), rv, ov, out),
                    None => out.push(format!("{path}.{k}: missing in oracle (rust = {rv})")),
                }
            }
        }
        (Value::Array(r), Value::Array(o)) => {
            if r.len() != o.len() {
                out.push(format!("{path}: length rust {} vs oracle {}  [rust {}] [oracle {}]", r.len(), o.len(), brief(rust), brief(oracle)));
                return;
            }
            for (i, (rv, ov)) in r.iter().zip(o).enumerate() {
                compare(&format!("{path}[{i}]"), rv, ov, out);
            }
        }
        _ => {
            if rust != oracle {
                out.push(format!("{path}: rust {rust} vs oracle {oracle}"));
            }
        }
    }
}

fn brief(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 160 { format!("{}…", &s[..160]) } else { s }
}

fn picks_of(v: &Value) -> Vec<u8> {
    let arr = if v.is_array() { v } else if v["picked"].is_array() { &v["picked"] } else if v["choose"].is_array() { &v["choose"] } else { return vec![] };
    arr.as_array().unwrap().iter().filter_map(|x| x.as_u64()).map(|x| x as u8).collect()
}

fn to_action(cx: &Combat, a: &Value) -> Result<Action, String> {
    if a.get("end_turn").is_some() {
        return Ok(Action::EndTurn);
    }
    if let Some(p) = a.get("play") {
        let target = match p["target"].as_u64() {
            Some(i) => *cx.enemies.as_slice().get(i as usize).ok_or("enemy target out of range")?,
            None => NO,
        };
        return Ok(Action::PlayCard { hand_pos: p["hand_pos"].as_u64().ok_or("hand_pos")? as u8, target });
    }
    if let Some(p) = a.get("use_potion") {
        let target = match p["target"].as_u64() {
            Some(i) => *cx.enemies.as_slice().get(i as usize).ok_or("enemy target out of range")?,
            None => NO,
        };
        return Ok(Action::UsePotion { slot: p["slot"].as_u64().ok_or("slot")? as u8, target });
    }
    Err(format!("unsupported action {a}"))
}

/// Replays one trace.
#[derive(PartialEq, Eq, Debug)]
pub enum Verdict {
    Match,
    Mismatch,
    /// The simulator used content that has no Rust implementation yet (the name is printed).
    Unimplemented,
}

fn missing_name(cx: &Combat) -> Option<String> {
    use sts2sim::hooks::Kind;
    cx.missing.map(|(k, id)| match k {
        Kind::Card => format!("card {}", sts2sim::ids::card::NAMES[id as usize]),
        Kind::Power => format!("power {}", sts2sim::ids::power::NAMES[id as usize]),
        Kind::Relic => format!("relic {}", sts2sim::ids::relic::NAMES[id as usize]),
        Kind::Potion => format!("potion {}", sts2sim::ids::potion::NAMES[id as usize]),
        Kind::Monster => format!("monster {}", sts2sim::ids::monster::NAMES[id as usize]),
        _ => format!("{k:?} {id}"),
    })
}

pub fn replay(scenario_path: &str, trace_path: &str, max_report: usize, quiet: bool) -> Result<Verdict, String> {
    let sv: Value = serde_json::from_str(&std::fs::read_to_string(scenario_path).map_err(|e| format!("{scenario_path}: {e}"))?).map_err(|e| e.to_string())?;
    let sc = convert::scenario(&sv)?;
    sc.validate().map_err(|e| format!("not implemented in the simulator: {e:?}"))?;
    let trace = load_jsonl(trace_path)?;
    let mut cx = Combat::new(&sc);
    let mut reported = 0;
    let mut ok = true;
    let mut buf = ActionBuf::new();
    for (i, rec) in trace.iter().enumerate() {
        if i > 0 {
            let act = to_action(&cx, &rec["action"])?;
            cx.legal_actions(&mut buf);
            if !buf.iter().any(|a| *a == act) {
                println!("step {i}: action {act:?} is NOT legal in the simulator (legal: {:?})", buf.as_slice());
                return Ok(Verdict::Mismatch);
            }
            if !cx.step(act) {
                println!("step {i}: simulator rejected {act:?}");
                return Ok(Verdict::Mismatch);
            }
            let choices: Vec<&Value> = rec["choices"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
            let mut ci = 0;
            while cx.stage == Stage::AwaitChoice {
                let Some(ch) = choices.get(ci) else {
                    println!("step {i}: simulator raised a decision but the oracle made no choice");
                    return Ok(Verdict::Mismatch);
                };
                ci += 1;
                for p in picks_of(ch) {
                    if !cx.step(Action::Pick { idx: p }) {
                        println!("step {i}: pick {p} rejected");
                        return Ok(Verdict::Mismatch);
                    }
                    if cx.stage != Stage::AwaitChoice {
                        break;
                    }
                }
                if cx.stage == Stage::AwaitChoice && !cx.step(Action::Confirm) {
                    println!("step {i}: decision still pending after the oracle's picks");
                    return Ok(Verdict::Mismatch);
                }
            }
        }
        if let Some(m) = missing_name(&cx) {
            println!("UNIMPLEMENTED {m} (step {i})");
            return Ok(Verdict::Unimplemented);
        }
        let mut diffs = vec![];
        compare("", &snapshot(&cx), rec, &mut diffs);
        if !diffs.is_empty() {
            ok = false;
            if !quiet {
                println!("step {i} (action {}): {} difference(s)", rec["action"], diffs.len());
                for d in diffs.iter().take(max_report) {
                    println!("    {d}");
                }
            }
            reported += 1;
            if reported >= 1 {
                break; // later steps are meaningless once state has diverged
            }
        }
    }
    if ok && !quiet {
        println!("OK: {} steps match ({trace_path})", trace.len());
    }
    Ok(if ok { Verdict::Match } else { Verdict::Mismatch })
}
