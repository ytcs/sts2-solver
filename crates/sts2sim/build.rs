//! Generates the content registry: scans `src/content/{cards,powers,relics,potions,monsters,encounters}/*.rs` and wires every
//! item into the id-indexed registries, so adding content never edits a shared file.
//!
//! Conventions (class names are the game's C# class names, so ids resolve through the same slug rule as `ids.rs`):
//! * cards/powers/relics/potions: `listener!(ClassName { ... });` (use `{}` for no hooks) — registers the item.
//!   Card/power stat tables come from `gen_*.rs`.
//! * monsters: `pub static <SLUG>_DEF: MonsterDef = ...;` registers the monster definition; an optional
//!   `listener!(ClassName { ... });` supplies monster-model hooks (`AfterDeath`, ...).
//! * encounters: `pub fn spawn_<slug_lower>(rng: &mut Rng, ascension: u8) -> Spawns` registers an encounter composition.

use std::fmt::Write as _;
use std::{env, fs, path::Path};

fn slugify(name: &str) -> String {
    let b: Vec<char> = name.chars().collect();
    let (mut out, mut i, mut last_end) = (String::new(), 0usize, usize::MAX);
    while i < b.len() {
        if i + 1 < b.len() && b[i].is_ascii_alphanumeric() && b[i + 1].is_ascii_uppercase() {
            out.push(b[i]);
            out.push('_');
            out.push(b[i + 1]);
            i += 2;
            last_end = i;
            continue;
        }
        if i == last_end && i != 0 && b[i].is_ascii_uppercase() {
            out.push('_');
            out.push(b[i]);
            i += 1;
            last_end = i;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out.to_uppercase()
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".rs") && n != "mod.rs").collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn scan(src: &str, prefix: &str, suffix: &str) -> Vec<String> {
    // find `prefix IDENT suffix` occurrences (very small hand-rolled scanner; no regex dependency)
    let mut out = vec![];
    let mut rest = src;
    while let Some(p) = rest.find(prefix) {
        let after = &rest[p + prefix.len()..];
        let ident: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let tail = &after[ident.len()..];
        if !ident.is_empty() && tail.trim_start().starts_with(suffix) {
            out.push(ident);
        }
        rest = &rest[p + prefix.len()..];
    }
    out
}

fn main() {
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let content = Path::new(&root).join("src/content");
    println!("cargo:rerun-if-changed=src/content");
    let mut out = String::new();
    // module declarations (absolute paths: robust to where the generated file is included from)
    let mut cat_items: Vec<(String, Vec<(String, String)>)> = vec![]; // category -> (item, module path)
    let mut mon_defs: Vec<(String, String)> = vec![];
    let mut mon_listeners: Vec<(String, String)> = vec![];
    let mut enc: Vec<(String, String)> = vec![];
    for cat in ["cards", "powers", "relics", "potions", "monsters", "encounters"] {
        let dir = content.join(cat);
        writeln!(out, "pub mod {cat} {{").unwrap();
        let mut items = vec![];
        for f in files(&dir) {
            let stem = f.trim_end_matches(".rs");
            let path = dir.join(&f);
            writeln!(out, "    #[path = {:?}]\n    pub mod {stem};", path.to_string_lossy()).unwrap();
            let src = fs::read_to_string(&path).unwrap();
            println!("cargo:rerun-if-changed={}", path.display());
            match cat {
                "monsters" => {
                    for d in scan(&src, "pub static ", ": MonsterDef") {
                        let id = d.trim_end_matches("_DEF").to_string();
                        mon_defs.push((id, format!("{cat}::{stem}::{d}")));
                    }
                    for n in scan(&src, "listener!(", "{") {
                        mon_listeners.push((slugify(&n), format!("{cat}::{stem}::{n}")));
                    }
                }
                "encounters" => {
                    for n in scan(&src, "pub fn spawn_", "(") {
                        enc.push((n.to_uppercase(), format!("{cat}::{stem}::spawn_{n}")));
                    }
                }
                _ => {
                    for n in scan(&src, "listener!(", "{") {
                        items.push((slugify(&n), format!("{cat}::{stem}::{n}")));
                    }
                }
            }
        }
        writeln!(out, "}}").unwrap();
        cat_items.push((cat.to_string(), items));
    }
    let dup = |v: &[(String, String)], what: &str| {
        let mut seen = std::collections::HashSet::new();
        for (id, p) in v {
            if !seen.insert(id.clone()) {
                panic!("duplicate {what} registration for {id} ({p})");
            }
        }
    };
    for (cat, items) in &cat_items {
        dup(items, cat);
    }
    dup(&mon_defs, "monster def");
    dup(&enc, "encounter");
    let ids_mod = |cat: &str| match cat {
        "cards" => "card",
        "powers" => "power",
        "relics" => "relic",
        "potions" => "potion",
        _ => "",
    };
    for (cat, items) in &cat_items {
        let m = ids_mod(cat);
        if m.is_empty() {
            continue;
        }
        let ents: String = items.iter().map(|(id, p)| format!("    {id} => {p},\n")).collect();
        writeln!(out, "registry!({m}_listener, {m}_mask, {m}, &NO_LISTENER;\n{ents});").unwrap();
        writeln!(out, "registry!(impl {m}_implemented, {m};\n{ents});").unwrap();
    }
    // monsters
    let ents: String = mon_listeners.iter().map(|(id, p)| format!("    {id} => {p},\n")).collect();
    writeln!(out, "registry!(monster_listener, monster_mask, monster, &NO_LISTENER;\n{ents});").unwrap();
    writeln!(out, "pub fn monster_def(id: u16) -> &'static MonsterDef {{\n    match id {{").unwrap();
    for (id, p) in &mon_defs {
        writeln!(out, "        ids::monster::{id} => &{p},").unwrap();
    }
    writeln!(out, "        _ => panic!(\"unimplemented monster {{}}\", ids::monster::NAMES[id as usize]),\n    }}\n}}").unwrap();
    writeln!(out, "pub fn monster_implemented(id: u16) -> bool {{\n    matches!(id, {})\n}}", if mon_defs.is_empty() { "_ if false".to_string() } else { mon_defs.iter().map(|(i, _)| format!("ids::monster::{i}")).collect::<Vec<_>>().join(" | ") }).unwrap();
    // encounters
    writeln!(out, "pub fn encounter_spawns(id: u16, rng: &mut crate::rng::Rng, ascension: u8) -> Option<Spawns> {{\n    match id {{").unwrap();
    for (id, p) in &enc {
        writeln!(out, "        ids::encounter::{id} => Some({p}(rng, ascension)),").unwrap();
    }
    writeln!(out, "        _ => None,\n    }}\n}}").unwrap();
    writeln!(out, "pub fn encounter_implemented(id: u16) -> bool {{\n    matches!(id, {})\n}}", if enc.is_empty() { "_ if false".to_string() } else { enc.iter().map(|(i, _)| format!("ids::encounter::{i}")).collect::<Vec<_>>().join(" | ") }).unwrap();
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("registry.rs"), out).unwrap();
}
