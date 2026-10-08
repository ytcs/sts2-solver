use crate::defs::*;
use crate::ids;

crate::listener!(Osty {});

pub static OSTY_DEF: MonsterDef = MonsterDef {
    id: ids::monster::OSTY,
    hp: |_| (1, 1),
    initial: 0,
    on_spawn: None,
    nodes: &[MonsterNode::Move { id: "NOTHING_MOVE", perform: |_, _| {}, intents: &[], follow_up: 0, must_perform_once: false }],
};
