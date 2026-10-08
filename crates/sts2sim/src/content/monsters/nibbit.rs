use crate::dec::Dec;
use crate::defs::*;
use crate::engine::Attack;
use crate::ids;
use crate::state::*;
use crate::types::*;

crate::listener!(Nibbit {});

mod nibbit {
    use super::*;
    pub fn butt_damage(cx: &Combat) -> i32 {
        asc::val(asc::DEADLY_ENEMIES, cx.ascension, 13, 12)
    }
    pub fn slice_block(cx: &Combat) -> i32 {
        asc::val(asc::TOUGH_ENEMIES, cx.ascension, 6, 5)
    }
    pub fn slice_damage(cx: &Combat) -> i32 {
        asc::val(asc::DEADLY_ENEMIES, cx.ascension, 7, 6)
    }
    pub fn hiss_strength(cx: &Combat) -> i32 {
        asc::val(asc::DEADLY_ENEMIES, cx.ascension, 3, 2)
    }
    pub fn butt(cx: &mut Combat, me: u8) {
        let d = butt_damage(cx);
        cx.execute_attack(&Attack::from_monster(me, d));
    }
    pub fn slice(cx: &mut Combat, me: u8) {
        let d = slice_damage(cx);
        cx.execute_attack(&Attack::from_monster(me, d));
        let b = slice_block(cx);
        cx.gain_block(me, Dec::int(b as i64), ValueProp::MOVE, NO);
    }
    pub fn hiss(cx: &mut Combat, me: u8) {
        let s = hiss_strength(cx);
        cx.apply_power(ids::power::STRENGTH_POWER, me, Dec::int(s as i64), me, NO);
    }
}

pub static NIBBIT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::NIBBIT,
    hp: |a| if a >= asc::TOUGH_ENEMIES { (44, 48) } else { (42, 46) },
    initial: 0,
    on_spawn: None,
    nodes: &[
        MonsterNode::Cond {
            id: "INIT_MOVE",
            arms: &[
                (1, |cx, c| cx.cr(c).monster.vars[0] != 0),
                (3, |cx, c| cx.cr(c).monster.vars[0] == 0 && cx.cr(c).monster.vars[1] == 0),
                (2, |cx, c| cx.cr(c).monster.vars[0] == 0 && cx.cr(c).monster.vars[1] != 0),
            ],
        },
        MonsterNode::Move {
            id: "BUTT_MOVE",
            perform: nibbit::butt,
            intents: &[Intent::Attack { damage: |cx, _| nibbit::butt_damage(cx), hits: |_, _| 1 }],
            follow_up: 2,
            must_perform_once: false,
        },
        MonsterNode::Move {
            id: "SLICE_MOVE",
            perform: nibbit::slice,
            intents: &[Intent::Attack { damage: |cx, _| nibbit::slice_damage(cx), hits: |_, _| 1 }, Intent::Defend],
            follow_up: 3,
            must_perform_once: false,
        },
        MonsterNode::Move { id: "HISS_MOVE", perform: nibbit::hiss, intents: &[Intent::Buff], follow_up: 1, must_perform_once: false },
    ],
};
