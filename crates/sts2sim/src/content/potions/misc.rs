use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

fn heal_percent(cx: &mut Combat, potion: u16, target: Cid) {
    let pct = cx.potion_named_var(potion, crate::content::gen_cards::var_name::HEAL_PERCENT) as i64;
    let amount = Dec::frac(cx.cr(target).max_hp as i64 * pct, 2);
    cx.heal(target, amount);
}

listener!(BloodPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        heal_percent(cx, potion, target);
        Flow::Done
    }
});

listener!(Ambergris {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        heal_percent(cx, potion, target);
        if cx.in_progress {
            cx.apply_power(ids::power::AMBERGRIS_POWER, target, Dec::ONE, PLAYER, NO);
        }
        Flow::Done
    }
});

listener!(FruitJuice {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::MaxHp);
        cx.gain_max_hp(target, Dec::int(n as i64));
        Flow::Done
    }
});

listener!(Fortifier {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: Cid, _phase: u8) -> Flow {
        let b = cx.cr(target).block() as i64 * 2;
        cx.gain_block(target, Dec::int(b), ValueProp::UNPOWERED, NO);
        Flow::Done
    }
});

listener!(CureAll {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let e = cx.potion_var(potion, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(StarPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Stars);
        cx.gain_stars(n);
        Flow::Done
    }
});

listener!(PotionOfCapacity {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Repeat);
        cx.add_orb_slots(n);
        Flow::Done
    }
});

listener!(BoneBrew {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Summon);
        cx.summon(n);
        Flow::Done
    }
});

listener!(KingsCourage {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Forge);
        cx.forge(n);
        Flow::Done
    }
});

listener!(DistilledChaos {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        if phase == 0 {
            let n = cx.potion_var(potion, VarKind::Repeat);
            if cx.auto_play_from_draw_pile(n, CardPilePosition::Top, false) == crate::engine::RunResult::Suspended {
                return Flow::Suspend(1);
            }
        }
        Flow::Done
    }
});

listener!(EntropicBrew {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        while cx.has_open_potion_slots() {
            let Some(p) = cx.create_random_potion(false) else { break };
            if !cx.try_procure_potion(p) {
                break;
            }
        }
        Flow::Done
    }
});

listener!(FairyInABottle {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: Cid, _phase: u8) -> Flow {
        let amount = Dec::frac(cx.cr(target).max_hp as i64 * 3, 1).max(Dec::ONE);
        cx.heal(target, amount);
        Flow::Done
    }
    fn should_die(&self, _cx: &Combat, _me: Me, creature: Cid) -> bool {
        creature != PLAYER
    }
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        cx.use_potion_now(me.idx as usize, creature);
    }
});

listener!(FoulPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        if !cx.in_progress {
            return Flow::Done;
        }
        let d = cx.potion_var(potion, VarKind::Damage);
        let mut targets: ArrayVec<Cid, MAX_CREATURES> = ArrayVec::new();
        for &c in cx.allies.iter().chain(cx.enemies.iter()) {
            if !cx.cr(c).is_pet {
                targets.push(c);
            }
        }
        cx.damage(targets.as_slice(), Dec::int(d as i64), ValueProp::UNPOWERED, PLAYER, NO);
        Flow::Done
    }
});

listener!(PotionShapedRock {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let d = cx.potion_var(potion, VarKind::Damage);
        cx.damage(&[target], Dec::int(d as i64), ValueProp::UNPOWERED, PLAYER, NO);
        Flow::Done
    }
});
