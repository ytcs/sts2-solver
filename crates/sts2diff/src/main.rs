//! `sts2diff` — differential test of the Rust simulator against traces recorded from the real game.
//!
//!   sts2diff run <scenario.json> <trace.jsonl> [--max N] [--quiet]   replay one trace, report the first mismatches
//!   sts2diff dir <dir>                                                 every `*.scenario.json` + matching `*.jsonl`
//!
//! The comparison walks the Rust snapshot (`snapshot::snapshot`) and requires every field it contains to equal the
//! oracle's, so fields the simulator does not model yet are simply absent from the comparison.

use sts2diff::diff;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") if args.len() >= 4 => {
            let max = args.iter().position(|a| a == "--max").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(8);
            let quiet = args.iter().any(|a| a == "--quiet");
            match diff::replay(&args[2], &args[3], max, quiet) {
                Ok(diff::Verdict::Match) => ExitCode::SUCCESS,
                Ok(diff::Verdict::Mismatch) => ExitCode::from(1),
                Ok(diff::Verdict::Unimplemented) => ExitCode::from(3),
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(2)
                }
            }
        }
        // Debug aid: the simulator's snapshot right after `Combat::new` (a decision pending at setup is listed).
        Some("show") if args.len() >= 3 => {
            let sv: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
            let sc = sts2diff::convert::scenario(&sv).unwrap();
            let cx = sts2sim::Combat::new(&sc);
            println!("{}", serde_json::to_string(&sts2diff::snapshot::snapshot(&cx)).unwrap());
            if let Some(d) = cx.decision.as_ref() {
                println!("PENDING DECISION: purpose {} min {} max {} cands {:?}", d.purpose, d.min, d.max, d.cands.iter().map(|&c| sts2sim::ids::card::NAMES[cx.cards[c as usize].id as usize]).collect::<Vec<_>>());
            }
            ExitCode::SUCCESS
        }
        Some("dir") if args.len() >= 3 => {
            let (mut ok, mut bad) = (0, 0);
            let mut entries: Vec<_> = std::fs::read_dir(&args[2]).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
            entries.sort();
            for p in entries {
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                if let Some(stem) = name.strip_suffix(".scenario.json") {
                    let trace = p.with_file_name(format!("{stem}.jsonl"));
                    match diff::replay(&p.to_string_lossy(), &trace.to_string_lossy(), 3, true) {
                        Ok(diff::Verdict::Match) => ok += 1,
                        _ => {
                            bad += 1;
                            println!("MISMATCH {stem}");
                        }
                    }
                }
            }
            println!("{ok} ok, {bad} mismatching");
            if bad == 0 { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        _ => {
            eprintln!("usage: sts2diff run <scenario.json> <trace.jsonl> [--max N] [--quiet] | sts2diff dir <dir>");
            ExitCode::from(2)
        }
    }
}

