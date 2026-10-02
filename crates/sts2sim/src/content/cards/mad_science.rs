//! Mad Science (the TinkerTime event card): its card type (Attack / Skill / Power), target and effect are per-instance
//! `[SavedProperty]` values (`TinkerTimeType`, `TinkerTimeRider`).
//!
//! Scenario props are read in the sorted key order of the JSON object (`TinkerTimeRider`, `TinkerTimeType`), i.e. `Card::counter`
//! = `[rider, type]`; with a single prop (`TinkerTimeRider` omitted = `None`) `counter[0]` is the type. See `decode`.
//! `Combat::card_def` returns `variant(..)` for this card so every `ctype` / `target` query sees the instance's type.

use crate::dec::Dec;
use crate::defs::{CardDef, VarKind};
use crate::content::gen_cards::var_name;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use std::sync::LazyLock;

// `TinkerTime.RiderEffect`
const SAPPING: i16 = 1;
const VIOLENCE: i16 = 2;
const CHOKING: i16 = 3;
const ENERGIZED: i16 = 4;
const WISDOM: i16 = 5;
const CHAOS: i16 = 6;
const EXPERTISE: i16 = 7;
const CURIOUS: i16 = 8;
const IMPROVEMENT: i16 = 9;

/// `(TinkerTimeType as CardType, TinkerTimeRider)` from the card's saved props.
pub fn decode(counter: [i16; 2]) -> (i16, i16) {
    if counter[1] != 0 { (counter[1], counter[0]) } else { (counter[0], 0) }
}

static VARIANTS: LazyLock<[CardDef; 3]> = LazyLock::new(|| {
    let base = crate::content::gen_cards::CARD_DEFS[ids::card::MAD_SCIENCE as usize];
    let mut attack = base;
    attack.ctype = CardType::Attack;
    attack.target = TargetType::AnyEnemy;
    let mut skill = base;
    skill.ctype = CardType::Skill;
    skill.target = TargetType::Self_;
    let mut power = base;
    power.ctype = CardType::Power;
    power.target = TargetType::Self_;
    [attack, skill, power]
});

/// The card definition for a Mad Science with these saved props (`Type` / `TargetType` follow `TinkerTimeType`).
pub fn variant(counter: [i16; 2]) -> &'static CardDef {
    match decode(counter).0 {
        2 => &VARIANTS[1],
        3 => &VARIANTS[2],
        _ => &VARIANTS[0],
    }
}

listener!(MadScience {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let (ty, rider) = decode(cx.cards[p.card as usize].counter);
        let dec = |n: i32| Dec::int(n as i64);
        match ty {
            1 => {
                let hits = if rider == VIOLENCE { cx.card_named_var(p.card, var_name::VIOLENCE_HITS) } else { 1 };
                let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target)).hits(hits);
                a.damage = cx.card_damage_dec(p.card);
                cx.execute_attack(&a);
            }
            2 => {
                let b = cx.card_var(p.card, VarKind::Block);
                cx.gain_block(PLAYER, dec(b), ValueProp::MOVE, p.card);
            }
            3 => match rider {
                EXPERTISE => {
                    let s = cx.card_named_var(p.card, var_name::EXPERTISE_STRENGTH);
                    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, dec(s), PLAYER, p.card);
                    let d = cx.card_named_var(p.card, var_name::EXPERTISE_DEXTERITY);
                    cx.apply_power(ids::power::DEXTERITY_POWER, PLAYER, dec(d), PLAYER, p.card);
                }
                CURIOUS => {
                    let r = cx.card_named_var(p.card, var_name::CURIOUS_REDUCTION);
                    cx.apply_power(ids::power::CURIOUS_POWER, PLAYER, dec(r), PLAYER, p.card);
                }
                IMPROVEMENT => {
                    cx.apply_power(ids::power::IMPROVEMENT_POWER, PLAYER, dec(1), PLAYER, p.card);
                }
                _ => {}
            },
            _ => {}
        }
        // `Sapping || Choking..Chaos` riders run after the main effect (Violence is handled by the hit count above).
        if rider == SAPPING || (CHOKING..=CHAOS).contains(&rider) {
            match rider {
                SAPPING => {
                    let w = cx.card_named_var(p.card, var_name::SAPPING_WEAK);
                    cx.apply_power(ids::power::WEAK_POWER, p.target, dec(w), PLAYER, p.card);
                    let v = cx.card_named_var(p.card, var_name::SAPPING_VULNERABLE);
                    cx.apply_power(ids::power::VULNERABLE_POWER, p.target, dec(v), PLAYER, p.card);
                }
                CHOKING => {
                    let c = cx.card_named_var(p.card, var_name::CHOKING_DAMAGE);
                    cx.apply_power(ids::power::STRANGLE_POWER, p.target, dec(c), PLAYER, p.card);
                }
                ENERGIZED => {
                    let e = cx.card_named_var(p.card, var_name::ENERGIZED_ENERGY);
                    cx.gain_energy(e);
                }
                WISDOM => {
                    let n = cx.card_named_var(p.card, var_name::WISDOM_CARDS);
                    cx.draw_cards(n, false);
                }
                _ => {
                    // Chaos: one random card from the whole character pool, free this turn, into the hand
                    let pool = cx.character_pool();
                    let cards = cx.get_distinct_for_combat(pool, 1, |_| true);
                    if let Some(c) = cards.first() {
                        cx.set_to_free_this_turn(c);
                        cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                }
            }
        }
        Flow::Done
    }
});
