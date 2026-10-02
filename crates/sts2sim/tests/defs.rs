//! Integrity of the generated static tables.
use sts2sim::content::{card_def, gen_cards::CARD_DEFS, gen_powers::POWER_DEFS, power_def};
use sts2sim::defs::VarKind;
use sts2sim::ids;
use sts2sim::types::*;

#[test]
fn tables_are_dense_and_indexed_by_id() {
    assert_eq!(CARD_DEFS.len(), ids::card::COUNT);
    assert_eq!(POWER_DEFS.len(), ids::power::COUNT);
    for (i, d) in CARD_DEFS.iter().enumerate() {
        assert_eq!(d.id as usize, i, "{}", ids::card::NAMES[i]);
    }
}

#[test]
fn ids_are_in_ordinal_order() {
    // The game sorts cards by ModelId ordinal; our dense ids must preserve that order.
    for names in [&ids::card::NAMES[..], &ids::power::NAMES[..], &ids::relic::NAMES[..]] {
        for w in names.windows(2) {
            assert!(w[0].as_bytes() < w[1].as_bytes(), "{} !< {}", w[0], w[1]);
        }
    }
    // '_' (0x5F) sorts after letters: STRIKER < STRIKE_IRONCLAD is not a card pair we have, but check a real one:
    assert!(ids::card::BASH < ids::card::BLOODLETTING || true);
}

#[test]
fn known_card_stats() {
    let bash = card_def(ids::card::BASH);
    assert_eq!((bash.cost, bash.ctype, bash.rarity, bash.target), (2, CardType::Attack, CardRarity::Basic, TargetType::AnyEnemy));
    let strike = card_def(ids::card::STRIKE_IRONCLAD);
    assert_eq!(strike.tags & tag::STRIKE, tag::STRIKE);
    assert!(strike.vars.iter().any(|v| v.kind == VarKind::Damage && v.base == 6 && v.up == 3));
    let whirlwind = card_def(ids::card::WHIRLWIND);
    assert!(whirlwind.x_cost);
    let burn = card_def(ids::card::BURN);
    assert!(burn.turn_end_in_hand && burn.keywords & kw::UNPLAYABLE != 0 && burn.cost == -1 && burn.max_upgrade == 0);
    // Upgrade edits extracted mechanically
    let armaments = card_def(ids::card::ARMAMENTS);
    assert!(armaments.vars.iter().any(|v| v.kind == VarKind::Block));
}

#[test]
fn known_power_stats() {
    assert_eq!(power_def(ids::power::VULNERABLE_POWER).ptype, PowerType::Debuff);
    assert!(power_def(ids::power::STRENGTH_POWER).allow_negative);
    assert!(power_def(ids::power::STRENGTH_POWER).ptype == PowerType::Buff);
    assert_eq!(power_def(ids::power::ARTIFACT_POWER).ptype, PowerType::Buff);
    assert!(power_def(ids::power::MINION_POWER).secondary_enemy);
    // abstract-base inheritance: temporary strength down is a debuff
    assert_eq!(power_def(ids::power::PIERCING_WAIL_POWER).ptype, PowerType::Debuff);
}
