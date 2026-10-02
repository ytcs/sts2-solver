//! TOKEN pool cards (Shiv, Soul, Fuel, ...). The four `KnowledgeDemon.IChoosable` status cards (Disintegration,
//! Mind Rot, Sloth, Waste Away) only act through `OnChosen` in the Knowledge Demon event (outside combat), so
//! inside a combat they are inert.

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(Disintegration {});
listener!(MindRot {});
listener!(Sloth {});
listener!(WasteAway {});

listener!(Fuel {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

// NOTE for the merge: the Ironclad-B1 branch also defines `GiantRock` (identical single-target attack); keep one.
listener!(GiantRock {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(Luminesce {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(MinionDiveBomb {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(MinionSacrifice {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(MinionStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// With Fan of Knives the Shiv hits every enemy (its TargetType becomes AllEnemies).
// TODO(fidelity): the dynamic TargetType (AnyEnemy -> AllEnemies) is not reflected in `legal_actions`.
listener!(Shiv {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let t = if cx.has_power(PLAYER, ids::power::FAN_OF_KNIVES_POWER) { Targeting::AllOpponents } else { Targeting::Single(p.target) };
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, t));
        Flow::Done
    }
});

listener!(Soul {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Sovereign Blade (Regent token). Forge adds damage (`AddDamage`) / sets the repeat count (`SetRepeats`); this port
// keeps those two mutations in `card.counter` (`[0]` = extra damage, `[1]` = repeat count, 0 = the default 1).
// Dynamic TargetType (SeekingEdge -> all enemies) and the Parry block bonus follow the C# on_play.
pub fn sovereign_blade_add_damage(cx: &mut Combat, c: CardIdx, amount: i32) {
    cx.cards[c as usize].counter[0] += amount as i16;
}
pub fn sovereign_blade_set_repeats(cx: &mut Combat, c: CardIdx, n: i32) {
    cx.cards[c as usize].counter[1] = n as i16;
}

listener!(SovereignBlade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let card = p.card;
        let dmg = cx.card_var(card, VarKind::Damage) + cx.cards[card as usize].counter[0] as i32;
        let hits = match cx.cards[card as usize].counter[1] {
            0 => cx.card_var(card, VarKind::Repeat),
            n => n as i32,
        };
        let t = if cx.has_power(PLAYER, ids::power::SEEKING_EDGE_POWER) { Targeting::AllOpponents } else { Targeting::Single(p.target) };
        cx.execute_attack(&Attack::from_card(PLAYER, card, dmg, t).hits(hits));
        let parry = cx.power_amount(PLAYER, ids::power::PARRY_POWER);
        if parry > 0 {
            // CalculatedBlockVar: base 0 + extra 1 * parry amount.
            cx.gain_block(PLAYER, Dec::int(parry as i64), ValueProp::MOVE, card);
        }
        Flow::Done
    }
});

// Osty's attack: nothing happens if Osty is missing or dead.
listener!(SweepingGaze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = (1..MAX_CREATURES as u8).find(|&i| {
            let c = cx.cr(i);
            c.active && c.in_combat && c.is_pet && c.owner == PLAYER && c.is_alive()
        });
        if let Some(o) = osty {
            let dmg = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(o, p.card, dmg, Targeting::Random));
        }
        Flow::Done
    }
});
