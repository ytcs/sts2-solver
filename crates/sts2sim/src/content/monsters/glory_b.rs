use super::ovg_util::*;
use crate::content::powers::glory_b::dampen_add_caster;
use crate::defs::*;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

pub static FLAIL_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FLAIL_KNIGHT,
    hp: |a| hp(a, (108, 108), (101, 101)),
    initial: 2,
    on_spawn: None,
    nodes: &[
        mv("WAR_CHANT", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 3), &[Intent::Buff], 3),
        mv(
            "FLAIL_MOVE",
            |cx, me| {
                let d = a9(cx, 10, 9);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 10, 9), |_, _| 2)],
            3,
        ),
        mv(
            "RAM_MOVE",
            |cx, me| {
                let d = a9(cx, 17, 15);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 15))],
            3,
        ),
        rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).max_repeats(2), Branch::new(2).max_repeats(2)]),
    ],
};

pub static SPECTRAL_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SPECTRAL_KNIGHT,
    hp: |a| hp(a, (97, 97), (93, 93)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("HEX", |cx, me| power_player(cx, me, ids::power::HEX_POWER, 2), &[Intent::Debuff], 1),
        mv(
            "SOUL_SLASH",
            |cx, me| {
                let d = a9(cx, 17, 15);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 15))],
            3,
        ),
        mv(
            "SOUL_FLAME",
            |cx, me| {
                let d = a9(cx, 4, 3);
                atk_n(cx, me, d, 3)
            },
            &[multi(|cx, _| a9(cx, 4, 3), |_, _| 3)],
            3,
        ),
        rand("RAND", &[Branch::new(1).max_repeats(2), Branch::new(2).cannot_repeat()]),
    ],
};

fn magi_block(cx: &Combat) -> i32 {
    a8(cx, 9, 5)
}
pub static MAGI_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::MAGI_KNIGHT,
    hp: |a| hp(a, (89, 89), (82, 82)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "POWER_SHIELD_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk(cx, me, d);
                let b = magi_block(cx);
                block(cx, me, b);
            },
            &[attack(|cx, _| a9(cx, 7, 6)), Intent::Defend],
            1,
        ),
        mv(
            "DAMPEN_MOVE",
            |cx, me| {
                if !cx.has_power(PLAYER, ids::power::DAMPEN_POWER) {
                    cx.apply_power(ids::power::DAMPEN_POWER, PLAYER, crate::dec::Dec::ONE, me, NO);
                }
                dampen_add_caster(cx, PLAYER, me);
            },
            &[Intent::Debuff],
            2,
        ),
        mv(
            "RAM_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 11, 10))],
            3,
        ),
        mv(
            "PREP_MOVE",
            |cx, me| {
                let b = magi_block(cx);
                block(cx, me, b)
            },
            &[Intent::Defend],
            4,
        ),
        mv(
            "MAGIC_BOMB",
            |cx, me| {
                let d = a9(cx, 40, 35);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 40, 35))],
            2,
        ),
    ],
};

pub static MECHA_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::MECHA_KNIGHT,
    hp: |a| hp(a, (320, 320), (300, 300)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ARTIFACT_POWER, 3)),
    nodes: &[
        mv(
            "CHARGE_MOVE",
            |cx, me| {
                let d = a9(cx, 30, 25);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 30, 25))],
            3,
        ),
        mv(
            "HEAVY_CLEAVE_MOVE",
            |cx, me| {
                let d = a9(cx, 40, 35);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 40, 35))],
            3,
        ),
        mv(
            "WINDUP_MOVE",
            |cx, me| {
                block(cx, me, 15);
                power_self(cx, me, ids::power::STRENGTH_POWER, 5);
            },
            &[Intent::Defend, Intent::Buff],
            1,
        ),
        mv(
            "FLAMETHROWER_MOVE",
            |cx, me| {
                let d = a9(cx, 12, 8);
                atk(cx, me, d);
                cx.add_status_cards(ids::card::BURN, PileType::Hand, 4, CardPilePosition::Bottom);
            },
            &[attack(|cx, _| a9(cx, 12, 8)), Intent::StatusCard],
            2,
        ),
    ],
};

pub static SOUL_NEXUS_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SOUL_NEXUS,
    hp: |a| hp(a, (254, 254), (234, 234)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "SOUL_BURN_MOVE",
            |cx, me| {
                let d = a9(cx, 31, 29);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 31, 29))],
            3,
        ),
        mv(
            "MAELSTROM_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk_n(cx, me, d, 4)
            },
            &[multi(|cx, _| a9(cx, 7, 6), |_, _| 4)],
            3,
        ),
        mv(
            "DRAIN_LIFE_MOVE",
            |cx, me| {
                let d = a9(cx, 19, 18);
                atk(cx, me, d);
                power_player(cx, me, ids::power::VULNERABLE_POWER, 2);
                power_player(cx, me, ids::power::WEAK_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 19, 18)), Intent::DebuffStrong],
            3,
        ),
        rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).cannot_repeat(), Branch::new(2).cannot_repeat()]),
    ],
};

fn weak_tackle(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 16, 14);
    atk(cx, me, d)
}
pub static TORCH_HEAD_AMALGAM_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TORCH_HEAD_AMALGAM,
    hp: |a| hp(a, (211, 211), (199, 199)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::MINION_POWER, 1)),
    nodes: &[
        mv(
            "STRONG_TACKLE_MOVE",
            |cx, me| {
                let d = a9(cx, 32, 26);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 32, 26))],
            1,
        ),
        mv(
            "TACKLE_2_MOVE",
            |cx, me| {
                let d = a9(cx, 22, 18);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 22, 18))],
            2,
        ),
        mv("BEAM_MOVE", |cx, me| atk_n(cx, me, 8, 3), &[multi(|_, _| 8, |_, _| 3)], 3),
        mv("TACKLE_3_MOVE", weak_tackle, &[attack(|cx, _| a9(cx, 16, 14))], 4),
        mv("TACKLE_4_MOVE", weak_tackle, &[attack(|cx, _| a9(cx, 16, 14))], 2),
    ],
};

const Q_BURN_BRIGHT: u8 = 2;
const Q_ENRAGE: u8 = 7;
fn amalgam_alive(cx: &Combat, me: Cid) -> bool {
    cx.cr(me).monster.vars[0] == 0
}
pub static QUEEN_DEF: MonsterDef = MonsterDef {
    id: ids::monster::QUEEN,
    hp: |a| hp(a, (419, 419), (400, 400)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("PUPPET_STRINGS_MOVE", |cx, me| power_player(cx, me, ids::power::CHAINS_OF_BINDING_POWER, 3), &[Intent::CardDebuff], 1),
        mv(
            "YOU_ARE_MINE_MOVE",
            |cx, me| {
                power_player(cx, me, ids::power::FRAIL_POWER, 99);
                power_player(cx, me, ids::power::WEAK_POWER, 99);
                power_player(cx, me, ids::power::VULNERABLE_POWER, 99);
            },
            &[Intent::Debuff],
            4,
        ),
        mv(
            "BURN_BRIGHT_FOR_ME_MOVE",
            |cx, me| {
                let mates = cx.enemies.clone();
                for &t in mates.iter() {
                    if t != me {
                        power_self_to(cx, me, t, ids::power::STRENGTH_POWER, 1);
                    }
                }
                block(cx, me, 20);
            },
            &[Intent::Buff, Intent::Defend],
            3,
        ),
        cond("BURN_BRIGHT_FOR_ME_BRANCH", &[(Q_BURN_BRIGHT, |cx, c| amalgam_alive(cx, c)), (5, |cx, c| !amalgam_alive(cx, c))]),
        cond("YOURE_MINE_NOW_BRANCH", &[(Q_BURN_BRIGHT, |cx, c| amalgam_alive(cx, c)), (5, |cx, c| !amalgam_alive(cx, c))]),
        mv(
            "OFF_WITH_YOUR_HEAD_MOVE",
            |cx, me| {
                let d = a9(cx, 4, 3);
                atk_n(cx, me, d, 5)
            },
            &[multi(|cx, _| a9(cx, 4, 3), |_, _| 5)],
            6,
        ),
        mv(
            "EXECUTION_MOVE",
            |cx, me| {
                let d = a9(cx, 18, 15);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 18, 15))],
            Q_ENRAGE,
        ),
        mv("ENRAGE_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 5),
    ],
};

fn power_self_to(cx: &mut Combat, src: Cid, target: Cid, power: u16, amount: i32) {
    cx.apply_power(power, target, crate::dec::Dec::int(amount as i64), src, NO);
}

crate::listener!(Queen {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, _was_removal_prevented: bool) {
        if cx.cr(creature).monster.id == ids::monster::TORCH_HEAD_AMALGAM && cx.cr(me.owner).is_alive() {
            cx.creatures[me.owner as usize].monster.vars[0] = 1;
            if cx.cr(me.owner).monster.next_move == Q_BURN_BRIGHT {
                cx.set_move_immediate(me.owner, Q_ENRAGE, false);
            }
        }
    }
});

fn ts_respawn(cx: &mut Combat, me: Cid) {
    cx.creatures[me as usize].monster.vars[0] += 1;
    let respawns = cx.cr(me).monster.vars[0];
    if let Some(p) = cx.cr(me).power(ids::power::ADAPTABLE_POWER) {
        let uid = p.uid;
        if let Some(i) = cx.power_idx(me, uid) {
            cx.cr_mut(me).powers[i].aux = 0;
        }
    }
    let revive = |cx: &mut Combat, hp: i32| {
        cx.set_max_hp(me, crate::dec::Dec::int(hp as i64));
        cx.heal(me, crate::dec::Dec::int(hp as i64));
    };
    match respawns {
        1 => {
            let h = a8(cx, 212, 200);
            revive(cx, h);
            power_self(cx, me, ids::power::PAINFUL_STABS_POWER, 1);
        }
        2 => {
            let h = a8(cx, 313, 300);
            revive(cx, h);
            power_self(cx, me, ids::power::NEMESIS_POWER, 1);
            for pid in [ids::power::ADAPTABLE_POWER, ids::power::PAINFUL_STABS_POWER] {
                if let Some(p) = cx.cr(me).power(pid) {
                    let uid = p.uid;
                    cx.remove_power(me, uid);
                }
            }
        }
        _ => {}
    }
}
fn ts_claw_hits(cx: &Combat, me: Cid) -> i32 {
    3 + cx.cr(me).monster.vars[1]
}
pub static TEST_SUBJECT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TEST_SUBJECT,
    hp: |a| hp(a, (111, 111), (100, 100)),
    initial: 1,
    on_spawn: Some(|cx, me| {
        power_self(cx, me, ids::power::ADAPTABLE_POWER, 1);
        let e = a9(cx, 3, 2);
        power_self(cx, me, ids::power::ENRAGE_POWER, e);
    }),
    nodes: &[
        mv_once("RESPAWN_MOVE", ts_respawn, &[Intent::Heal, Intent::Buff], 7),
        mv(
            "BITE_MOVE",
            |cx, me| {
                let d = a9(cx, 22, 20);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 22, 20))],
            2,
        ),
        mv(
            "SKULL_BASH_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d);
                power_player(cx, me, ids::power::VULNERABLE_POWER, 1);
            },
            &[attack(|cx, _| a9(cx, 16, 14)), Intent::Debuff],
            1,
        ),
        mv(
            "MULTI_CLAW_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                let n = ts_claw_hits(cx, me);
                atk_n(cx, me, d, n);
                cx.creatures[me as usize].monster.vars[1] += 1;
            },
            &[multi(|cx, _| a9(cx, 11, 10), ts_claw_hits)],
            3,
        ),
        mv(
            "PHASE3_LACERATE_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                atk_n(cx, me, d, 3)
            },
            &[multi(|cx, _| a9(cx, 11, 10), |_, _| 3)],
            5,
        ),
        mv("BIG_POUNCE", |cx, me| atk(cx, me, 45), &[attack(|_, _| 45)], 6),
        mv(
            "BURNING_GROWL_MOVE",
            |cx, me| {
                let n = a9(cx, 5, 3);
                cx.add_status_cards(ids::card::BURN, PileType::Discard, n, CardPilePosition::Bottom);
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[Intent::StatusCard, Intent::Buff],
            4,
        ),
        cond("REVIVE_BRANCH", &[(3, |cx, c| cx.cr(c).monster.vars[0] < 2), (4, |cx, c| cx.cr(c).monster.vars[0] >= 2)]),
    ],
};

fn wither_amount(cx: &Combat) -> i32 {
    a9(cx, 2, 1)
}
fn fake_upgrade(cx: &mut Combat, card: CardIdx) {
    cx.cards[card as usize].dmg_bonus += 3 * 10_000;
}
pub static AEONGLASS_DEF: MonsterDef = MonsterDef {
    id: ids::monster::AEONGLASS,
    hp: |a| hp(a, (535, 535), (512, 512)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        power_self(cx, me, ids::power::WITHERING_PRESENCE_POWER, 6);
        power_self(cx, me, ids::power::ARTIFACT_POWER, 3);
    }),
    nodes: &[
        mv(
            "EBB_MOVE",
            |cx, me| {
                let d = a9(cx, 26, 22);
                atk(cx, me, d);
                block(cx, me, 33);
            },
            &[attack(|cx, _| a9(cx, 26, 22)), Intent::Defend],
            1,
        ),
        mv(
            "EYE_LASERS_MOVE",
            |cx, me| {
                let d = a9(cx, 12, 11);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 12, 11), |_, _| 2)],
            2,
        ),
        mv(
            "INCREASING_INTENSITY_MOVE",
            |cx, me| {
                let cards = cx.all_combat_cards();
                for &c in cards.iter() {
                    if cx.cards[c as usize].id == ids::card::WITHER {
                        fake_upgrade(cx, c);
                    }
                }
                cx.creatures[me as usize].monster.vars[1] += 1;
                let n = wither_amount(cx);
                status_to_discard(cx, ids::card::WITHER, n);
                let s = a9(cx, 4, 3) + cx.cr(me).monster.vars[0];
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
                cx.creatures[me as usize].monster.vars[0] += 1;
            },
            &[Intent::StatusCard, Intent::Buff],
            0,
        ),
    ],
};

crate::listener!(Aeonglass {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx, _added_by_player: bool) {
        if cx.cards[card as usize].id != ids::card::WITHER {
            return;
        }
        for _ in 0..cx.cr(me.owner).monster.vars[1] {
            fake_upgrade(cx, card);
        }
    }
});

fn barrage(cx: &mut Combat, me: Cid, d: i32, str_gain: i32) {
    atk_n(cx, me, d, 2);
    power_self(cx, me, ids::power::STRENGTH_POWER, str_gain);
}
pub static THE_ADVERSARY_MK_ONE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_ADVERSARY_MK_ONE,
    hp: |_| (100, 100),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ARTIFACT_POWER, 0)),
    nodes: &[
        mv("SMASH_MOVE", |cx, me| atk(cx, me, 12), &[attack(|_, _| 12)], 1),
        mv("BEAM_MOVE", |cx, me| atk(cx, me, 15), &[attack(|_, _| 15)], 2),
        mv("BARRAGE_MOVE", |cx, me| barrage(cx, me, 8, 2), &[multi(|_, _| 8, |_, _| 2), Intent::Buff], 0),
    ],
};
pub static THE_ADVERSARY_MK_TWO_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_ADVERSARY_MK_TWO,
    hp: |_| (200, 200),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ARTIFACT_POWER, 1)),
    nodes: &[
        mv("BASH_MOVE", |cx, me| atk(cx, me, 13), &[attack(|_, _| 13)], 1),
        mv("FLAME_BEAM_MOVE", |cx, me| atk(cx, me, 16), &[attack(|_, _| 16)], 2),
        mv("BARRAGE_MOVE", |cx, me| barrage(cx, me, 9, 3), &[multi(|_, _| 9, |_, _| 2), Intent::Buff], 0),
    ],
};
pub static THE_ADVERSARY_MK_THREE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_ADVERSARY_MK_THREE,
    hp: |_| (300, 300),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ARTIFACT_POWER, 2)),
    nodes: &[
        mv("CRASH_MOVE", |cx, me| atk(cx, me, 15), &[attack(|_, _| 15)], 1),
        mv("FLAME_BEAM_MOVE", |cx, me| atk(cx, me, 18), &[attack(|_, _| 18)], 2),
        mv("BARRAGE_MOVE", |cx, me| barrage(cx, me, 10, 4), &[multi(|_, _| 10, |_, _| 2), Intent::Buff], 0),
    ],
};
