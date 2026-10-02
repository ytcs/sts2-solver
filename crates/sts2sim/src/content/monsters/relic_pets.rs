//! Pets granted by relics (`Byrdpip`, `PaelsLegion` monster models): 9999 HP, hidden health bar, one no-op move that loops.
//! Added with `Combat::add_pet` by the relics' `BeforeCombatStart`.

use crate::defs::*;
use crate::ids;

pub static BYRDPIP_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BYRDPIP,
    hp: |_| (9999, 9999),
    initial: 0,
    on_spawn: None,
    nodes: &[MonsterNode::Move { id: "NOTHING_MOVE", perform: |_, _| {}, intents: &[], follow_up: 0, must_perform_once: false }],
};

pub static PAELS_LEGION_DEF: MonsterDef = MonsterDef {
    id: ids::monster::PAELS_LEGION,
    hp: |_| (9999, 9999),
    initial: 0,
    on_spawn: None,
    nodes: &[MonsterNode::Move { id: "NOTHING_MOVE", perform: |_, _| {}, intents: &[], follow_up: 0, must_perform_once: false }],
};
