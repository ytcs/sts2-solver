use crate::defs::*;
use crate::ids;

pub static BYRDPIP_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BYRDPIP,
    hp: |_| (9999, 9999),
    initial: 0,
    on_spawn: None,
    nodes: &[MonsterNode::Move { id: "NOTHING_MOVE", perform: |_, _| {}, intents: &[], follow_up: 0, must_perform_once: false }],
};

pub static BYRDINE_MONSTER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BYRDINE_MONSTER,
    hp: |_| (9999, 9999),
    initial: 0,
    on_spawn: None,
    nodes: &[MonsterNode::Move { id: "NOTHING_MOVE", perform: |_, _| {}, intents: &[], follow_up: 0, must_perform_once: false }],
};

pub static WUPPY_DEF: MonsterDef = MonsterDef {
    id: ids::monster::WUPPY,
    hp: |_| (50, 50),
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
