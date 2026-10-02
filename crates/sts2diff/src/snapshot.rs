//! Rust combat state -> JSON in the oracle's trace schema (only fields the simulator models).

use serde_json::{json, Map, Value};
use sts2sim::defs::{Intent, MonsterNode};
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn phase(p: Phase) -> &'static str {
    match p {
        Phase::None => "None",
        Phase::Start => "Start",
        Phase::AutoPrePlay => "AutoPrePlay",
        Phase::Play => "Play",
        Phase::AutoPostPlay => "AutoPostPlay",
        Phase::End => "End",
    }
}

fn powers(cx: &Combat, c: Cid) -> Value {
    Value::Array(
        cx.cr(c)
            .powers
            .iter()
            .map(|p| {
                let mut m = Map::new();
                m.insert("id".into(), json!(ids::power::NAMES[p.id as usize]));
                m.insert("amount".into(), json!(p.amount));
                if p.amount_on_turn_start != p.amount {
                    m.insert("amount_on_turn_start".into(), json!(p.amount_on_turn_start));
                }
                Value::Object(m)
            })
            .collect(),
    )
}

fn card(cx: &Combat, c: CardIdx, with_cost: bool) -> Value {
    let k = &cx.cards[c as usize];
    let mut m = Map::new();
    m.insert("id".into(), json!(ids::card::NAMES[k.id as usize]));
    m.insert("upgrade".into(), json!(k.upgrade));
    if with_cost {
        m.insert("cost".into(), json!(cx.card_cost(c, true).max(0)));
    }
    Value::Object(m)
}

fn pile(cx: &Combat, p: &Pile, with_cost: bool) -> Value {
    Value::Array(p.iter().map(|&c| card(cx, c, with_cost)).collect())
}

fn rng(r: &sts2sim::rng::Rng) -> Value {
    let s = r.state();
    json!({"counter": r.counter, "s0": s[0], "s1": s[1], "s2": s[2], "s3": s[3]})
}

fn enemy(cx: &Combat, e: Cid) -> Value {
    let cr = cx.cr(e);
    let ms = &cr.monster;
    let def = sts2sim::content::monster_def(ms.id);
    let mut m = Map::new();
    m.insert("id".into(), json!(ids::monster::NAMES[ms.id as usize]));
    m.insert("hp".into(), json!(cr.hp));
    m.insert("max_hp".into(), json!(cr.max_hp));
    m.insert("block".into(), json!(cr.block));
    m.insert("alive".into(), json!(cr.is_alive()));
    m.insert("powers".into(), powers(cx, e));
    if ms.next_move != NO {
        if let MonsterNode::Move { id, intents, .. } = &def.nodes[ms.next_move as usize] {
            m.insert("next_move".into(), json!(id));
            let mut list = vec![];
            for it in intents.iter() {
                list.push(match it {
                    Intent::Attack { damage, hits } => {
                        let d = cx.intent_damage(e, damage(cx, e));
                        let h = hits(cx, e);
                        json!({"type": "Attack", "damage": d, "hits": h, "total_damage": d * h})
                    }
                    Intent::Buff => json!({"type": "Buff"}),
                    Intent::Debuff => json!({"type": "Debuff"}),
                    Intent::DebuffStrong => json!({"type": "DebuffStrong"}),
                    Intent::Defend => json!({"type": "Defend"}),
                    Intent::Escape => json!({"type": "Escape"}),
                    Intent::Heal => json!({"type": "Heal"}),
                    Intent::Hidden => json!({"type": "Hidden"}),
                    Intent::Summon => json!({"type": "Summon"}),
                    Intent::Sleep => json!({"type": "Sleep"}),
                    Intent::Stun => json!({"type": "Stun"}),
                    Intent::StatusCard => json!({"type": "StatusCard"}),
                    Intent::CardDebuff => json!({"type": "CardDebuff"}),
                    Intent::DeathBlow => json!({"type": "DeathBlow"}),
                });
            }
            m.insert("intents".into(), Value::Array(list));
        }
    }
    Value::Object(m)
}

pub fn snapshot(cx: &Combat) -> Value {
    let me = cx.cr(PLAYER);
    let over = cx.stage == Stage::Over;
    let mut o = Map::new();
    o.insert("round".into(), json!(cx.round));
    o.insert("side".into(), json!(if cx.side == Side::Player { "player" } else { "enemy" }));
    o.insert("turn".into(), json!(cx.player.turn_number));
    o.insert("phase".into(), json!(phase(cx.player.phase)));
    o.insert("energy".into(), json!(cx.player.energy));
    o.insert("combat_in_progress".into(), json!(cx.in_progress));
    o.insert("combat_over".into(), json!(over));
    o.insert("player".into(), json!({"hp": me.hp, "max_hp": me.max_hp, "block": me.block, "alive": me.is_alive(), "powers": powers(cx, PLAYER)}));
    o.insert("enemies".into(), Value::Array(cx.enemies.iter().map(|&e| enemy(cx, e)).collect()));
    if !over {
        // Orbs: `{id, passive, evoke}` front first (numbers as integers: orb values are whole numbers).
        o.insert(
            "orbs".into(),
            Value::Array(
                cx.player
                    .orbs
                    .iter()
                    .map(|b| json!({"id": ids::orb::NAMES[b.kind as usize], "passive": cx.orb_passive_val(b).trunc(), "evoke": cx.orb_evoke_val(b).trunc()}))
                    .collect(),
            ),
        );
        o.insert("orb_capacity".into(), json!(cx.player.orb_capacity));
        o.insert("hand".into(), pile(cx, &cx.player.hand, true));
        o.insert("draw".into(), pile(cx, &cx.player.draw, false));
        o.insert("discard".into(), pile(cx, &cx.player.discard, false));
        o.insert("exhaust".into(), pile(cx, &cx.player.exhaust, false));
    }
    o.insert(
        "relics".into(),
        Value::Array(cx.player.relics.iter().map(|r| json!({"id": ids::relic::NAMES[r.id as usize]})).collect()),
    );
    o.insert(
        "potions".into(),
        Value::Array(cx.player.potions.iter().flatten().map(|p| json!({"id": ids::potion::NAMES[p.id as usize]})).collect()),
    );
    let r = &cx.rng;
    o.insert(
        "rng".into(),
        json!({"shuffle": rng(&r.shuffle), "combat_card_generation": rng(&r.combat_card_generation),
               "combat_potion_generation": rng(&r.combat_potion_generation), "combat_card_selection": rng(&r.combat_card_selection),
               "combat_energy_costs": rng(&r.combat_energy_costs), "combat_targets": rng(&r.combat_targets),
               "monster_ai": rng(&r.monster_ai), "niche": rng(&r.niche), "combat_orbs": rng(&r.combat_orbs)}),
    );
    Value::Object(o)
}
