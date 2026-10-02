//! Potions that apply powers (to the player, one enemy, or all hittable enemies) and the other simple effects.

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `PowerCmd.Apply<Power>(target, PowerVar<Power>.BaseValue, Owner.Creature, null)`.
fn apply_var_power(cx: &mut Combat, potion: u16, power: u16, target: Cid) {
    let v = cx.potion_power_var(potion, power);
    cx.apply_power(power, target, Dec::int(v as i64), PLAYER, NO);
}

// Potions whose whole effect is `PowerCmd.Apply<T>(target, PowerVar<T>)` (DEX, Focus, Intangible, Gigantification, Plating,
// Thorns, Buffer, Ritual, Poison, Doom, Regen, Flex, Speed).
listener!(DexterityPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::DEXTERITY_POWER, target);
        Flow::Done
    }
});

listener!(FocusPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::FOCUS_POWER, target);
        Flow::Done
    }
});

listener!(GhostInAJar {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::INTANGIBLE_POWER, target);
        Flow::Done
    }
});

listener!(GigantificationPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::GIGANTIFICATION_POWER, target);
        Flow::Done
    }
});

listener!(HeartOfIron {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::PLATING_POWER, target);
        Flow::Done
    }
});

listener!(LiquidBronze {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::THORNS_POWER, target);
        Flow::Done
    }
});

listener!(LuckyTonic {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::BUFFER_POWER, target);
        Flow::Done
    }
});

listener!(MazalethsGift {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::RITUAL_POWER, target);
        Flow::Done
    }
});

listener!(PoisonPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::POISON_POWER, target);
        Flow::Done
    }
});

listener!(PotionOfDoom {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::DOOM_POWER, target);
        Flow::Done
    }
});

listener!(RegenPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::REGEN_POWER, target);
        Flow::Done
    }
});

listener!(FlexPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::FLEX_POTION_POWER, target);
        Flow::Done
    }
});

listener!(SpeedPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::SPEED_POTION_POWER, target);
        Flow::Done
    }
});

// Strength and Dexterity (+1 each).
listener!(FyshOil {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        apply_var_power(cx, potion, ids::power::STRENGTH_POWER, target);
        apply_var_power(cx, potion, ids::power::DEXTERITY_POWER, target);
        Flow::Done
    }
});

// Draw 1, then +3 Clarity.
listener!(Clarity {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        apply_var_power(cx, potion, ids::power::CLARITY_POWER, target);
        Flow::Done
    }
});

// +1 energy, then 3 Radiance.
listener!(RadiantTincture {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Energy);
        cx.gain_energy(n);
        apply_var_power(cx, potion, ids::power::RADIANCE_POWER, target);
        Flow::Done
    }
});

// Block now and the same block next turn.
listener!(ShipInABottle {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let b = cx.potion_var(potion, VarKind::Block);
        cx.gain_block(target, Dec::int(b as i64), ValueProp::UNPOWERED, NO);
        cx.apply_power(ids::power::BLOCK_NEXT_TURN_POWER, target, Dec::int(b as i64), PLAYER, NO);
        Flow::Done
    }
});

// The next card(s) are played twice (applied by the player to themselves).
listener!(Duplicator {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: Cid, _phase: u8) -> Flow {
        cx.apply_power(ids::power::DUPLICATION_POWER, target, Dec::ONE, target, NO);
        Flow::Done
    }
});

// Retain the hand for `Repeat` turns.
listener!(StableSerum {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Repeat);
        cx.apply_power(ids::power::RETAIN_HAND_POWER, target, Dec::int(n as i64), PLAYER, NO);
        Flow::Done
    }
});

// 4 Shrink (the DamageDecrease var is the power's own constant).
listener!(BeetleJuice {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Repeat);
        cx.apply_power(ids::power::SHRINK_POWER, target, Dec::int(n as i64), PLAYER, NO);
        Flow::Done
    }
});

// Demise (Named var `Demise`, id `NAMED_DEMISE`).
listener!(PowderedDemise {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_named_var(potion, crate::content::gen_cards::var_name::DEMISE);
        cx.apply_power(ids::power::DEMISE_POWER, target, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});

// All hittable enemies: Weak, then Vulnerable. (The C# reads the two vars swapped; both are 1.)
listener!(PotionOfBinding {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let vuln = cx.potion_power_var(potion, ids::power::VULNERABLE_POWER);
        let weak = cx.potion_power_var(potion, ids::power::WEAK_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(vuln as i64), PLAYER, NO);
        cx.apply_power_to_hittable_enemies(ids::power::VULNERABLE_POWER, Dec::int(weak as i64), PLAYER, NO);
        Flow::Done
    }
});

// Temporary -7 Strength on every hittable enemy.
listener!(ShacklingPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_power_var(potion, ids::power::STRENGTH_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::SHACKLING_POTION_POWER, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});
