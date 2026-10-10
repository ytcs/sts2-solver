use std::collections::HashMap;
use std::sync::LazyLock;

use serde_json::Value;
use sts2sim::ids;
use sts2sim::rng::{deterministic_hash, Rng};
use sts2sim::state::RngSet;
use sts2sim::{DeckCard, DeckExtra, RelicInit, Scenario, ScenarioExtras};

fn strip(id: &str) -> &str {
    id.split_once('.').map_or(id, |(_, r)| r)
}

type Index = LazyLock<HashMap<&'static str, u16>>;

fn index(names: &'static [&'static str]) -> HashMap<&'static str, u16> {
    let mut m = HashMap::with_capacity(names.len());
    for (i, n) in names.iter().enumerate() {
        m.entry(*n).or_insert(i as u16);
    }
    m
}

static IDX_CARD: Index = LazyLock::new(|| index(&ids::card::NAMES));
static IDX_RELIC: Index = LazyLock::new(|| index(&ids::relic::NAMES));
static IDX_POTION: Index = LazyLock::new(|| index(&ids::potion::NAMES));
static IDX_ENCOUNTER: Index = LazyLock::new(|| index(&ids::encounter::NAMES));
static IDX_ENCHANTMENT: Index = LazyLock::new(|| index(&ids::enchantment::NAMES));

fn find(names: &Index, id: &str, what: &str) -> Result<u16, String> {
    names.get(strip(id)).copied().ok_or_else(|| format!("unknown {what} id {id}"))
}

fn id_of(v: &Value) -> &str {
    v.as_str().or_else(|| v["id"].as_str()).unwrap_or("")
}

fn relic_model(s: &str) -> Option<i32> {
    IDX_RELIC.get(s.strip_prefix("RELIC.")?).map(|&i| i as i32 + 1)
}

fn relic_init(id: u16, props: &Value) -> Result<RelicInit, String> {
    let l = sts2sim::content::relic_listener(id);
    let (counter, flags, aux) = l.meta_initial();
    let mut st = sts2sim::state::Relic { id, counter, flags, aux };
    if let Some(obj) = props.as_object() {
        let defs = l.meta_props();
        for (k, v) in obj {
            let d = defs.iter().find(|d| d.name == k).ok_or_else(|| format!("relic {} has no modelled saved property {k}", ids::relic::NAMES[id as usize]))?;
            if !d.lit.is_empty() {
                continue;
            }
            let n = if d.relic {
                match v.as_str() {
                    Some(s) => relic_model(s).ok_or_else(|| format!("relic prop {k}: unknown relic {s}"))? as i64,
                    None => 0,
                }
            } else {
                v.as_i64().or_else(|| v.as_bool().map(|b| b as i64)).ok_or_else(|| format!("relic prop {k}: expected int/bool"))?
            };
            st.set_prop(defs, k, n as i32);
        }
    }
    Ok(RelicInit { id, counter: st.counter, flags: st.flags, aux: st.aux })
}

pub fn obs_relics(relics: &Value) -> Vec<sts2sim::engine::ObsRelic> {
    let Some(a) = relics.as_array() else { return vec![] };
    a.iter()
        .filter_map(|r| {
            let id = find(&IDX_RELIC, id_of(r), "relic").ok()?;
            let props = r["props"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_i64().or_else(|| v.as_bool().map(|b| b as i64)).or_else(|| v.as_str().and_then(relic_model).map(|n| n as i64)).map(|n| (k.clone(), n as i32)))
                        .collect()
                })
                .unwrap_or_default();
            Some(sts2sim::engine::ObsRelic { id, props, counter: r["counter"].as_i64().map(|c| c as i32) })
        })
        .collect()
}

pub fn scenario(v: &Value) -> Result<Scenario, String> {
    scenario_ex(v).map(|(s, _)| s)
}

pub fn scenario_ex(v: &Value) -> Result<(Scenario, ScenarioExtras), String> {
    let character = match v["character"].as_str().unwrap_or("IRONCLAD") {
        "IRONCLAD" => 0,
        "SILENT" => 1,
        "DEFECT" => 2,
        "NECROBINDER" => 3,
        "REGENT" => 4,
        c => return Err(format!("unknown character {c}")),
    };
    let seed_str = v["seed"].as_str().map(str::to_string).or_else(|| v["seed"].as_i64().map(|i| i.to_string())).unwrap_or_else(|| "1".into());
    let run_seed = deterministic_hash(&seed_str);
    let mut rng = RngSet::from_run_seed(run_seed);
    if let Some(obj) = v["rng"].as_object() {
        for (name, st) in obj {
            let r = Rng::from_state(
                st["counter"].as_i64().unwrap_or(0) as i32,
                [st["s0"].as_u64().unwrap_or(0), st["s1"].as_u64().unwrap_or(0), st["s2"].as_u64().unwrap_or(0), st["s3"].as_u64().unwrap_or(0)],
            );
            match name.as_str() {
                "shuffle" => rng.shuffle = r,
                "combat_card_generation" => rng.combat_card_generation = r,
                "combat_potion_generation" => rng.combat_potion_generation = r,
                "combat_card_selection" => rng.combat_card_selection = r,
                "combat_energy_costs" => rng.combat_energy_costs = r,
                "combat_targets" => rng.combat_targets = r,
                "monster_ai" => rng.monster_ai = r,
                "niche" => rng.niche = r,
                "combat_orbs" => rng.combat_orbs = r,
                _ => {}
            }
        }
    }
    let mut deck = vec![];
    let mut extras = ScenarioExtras::default();
    extras.gold = v["gold"].as_i64().unwrap_or(99) as i32;
    extras.act = v["act"].as_u64().unwrap_or(0) as u8;
    for c in v["deck"].as_array().ok_or("scenario needs an explicit deck")? {
        let mut x = DeckExtra::default();
        if let Some(e) = c.get("enchantment") {
            let eid = if e.is_string() { e.as_str().unwrap() } else { e["id"].as_str().unwrap_or("") };
            x.enchant = find(&IDX_ENCHANTMENT, eid, "enchantment")? as u8 + 1;
            x.enchant_amount = e["amount"].as_i64().unwrap_or(1) as i16;
        }
        if let Some(p) = c["props"].as_object() {
            for (i, v) in p.values().filter_map(|v| v.as_i64().or_else(|| v.as_bool().map(|b| b as i64))).take(2).enumerate() {
                x.props[i] = v as i16;
            }
        }
        extras.deck.push(x);
        deck.push(DeckCard { id: find(&IDX_CARD, id_of(c), "card")?, upgrade: c["upgrade"].as_u64().unwrap_or(0) as u8 });
    }
    let mut relics = vec![];
    for r in v["relics"].as_array().unwrap_or(&vec![]) {
        relics.push(relic_init(find(&IDX_RELIC, id_of(r), "relic")?, &r["props"])?);
    }
    let mut potions = vec![];
    for p in v["potions"].as_array().unwrap_or(&vec![]) {
        potions.push(find(&IDX_POTION, id_of(p), "potion")?);
    }
    let hp = v["hp"].as_i64().unwrap_or(80) as i32;
    Ok((Scenario {
        run_seed,
        total_floor: v["total_floor"].as_i64().unwrap_or(1) as i32,
        character,
        ascension: v["ascension"].as_u64().unwrap_or(0) as u8,
        encounter: find(&IDX_ENCOUNTER, v["encounter"].as_str().ok_or("encounter missing")?, "encounter")?,
        max_hp: v["max_hp"].as_i64().map(|x| x as i32).unwrap_or(hp),
        hp,
        max_energy: v["max_energy"].as_i64().unwrap_or(3) as i32,
        orb_slots: v["base_orb_slots"].as_u64().unwrap_or(if character == 2 { 3 } else { 0 }) as u8,
        potion_slots: v["max_potion_slots"].as_u64().unwrap_or(3) as u8,
        deck,
        relics,
        potions,
        rng,
    }, extras))
}
