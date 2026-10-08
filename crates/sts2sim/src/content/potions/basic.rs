use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(BlockPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let b = cx.potion_var(potion, VarKind::Block);
        cx.gain_block(target, Dec::int(b as i64), ValueProp::UNPOWERED, NO);
        Flow::Done
    }
});

listener!(StrengthPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_power_var(potion, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, target, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});

listener!(SwiftPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(EnergyPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Energy);
        cx.gain_energy(n);
        Flow::Done
    }
});

listener!(WeakPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_power_var(potion, ids::power::WEAK_POWER);
        cx.apply_power(ids::power::WEAK_POWER, target, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});

listener!(VulnerablePotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_power_var(potion, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, target, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});

listener!(FirePotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let d = cx.potion_var(potion, VarKind::Damage);
        cx.damage(&[target], Dec::int(d as i64), ValueProp::UNPOWERED, PLAYER, NO);
        Flow::Done
    }
});

listener!(ExplosiveAmpoule {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let d = cx.potion_var(potion, VarKind::Damage);
        let targets = cx.hittable_enemies();
        cx.damage(targets.as_slice(), Dec::int(d as i64), ValueProp::UNPOWERED, PLAYER, NO);
        Flow::Done
    }
});
