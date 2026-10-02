//! Status cards of the Act 2 "Hive" bosses (hive_b slice): Knowledge Demon's Curse of Knowledge choices (Disintegration,
//! MindRot, Sloth, WasteAway: their effect is applied by the monster when chosen, `OnChosen`) and The Insatiable's
//! FranticEscape (stats from gen_cards.rs).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;

listener!(Disintegration {});
listener!(MindRot {});
listener!(Sloth {});
listener!(WasteAway {});

// FranticEscape (cost 1, Self): +1 to the owner's Sandpit countdown, and the card costs 1 more for the rest of the combat.
listener!(FranticEscape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        // GetSandpitEnemy: the first enemy holding a SandpitPower; its instance targeting the owner.
        let enemy = cx.enemies.iter().copied().find(|&e| cx.has_power(e, ids::power::SANDPIT_POWER));
        if let Some(e) = enemy {
            if let Some(uid) = cx.cr(e).power(ids::power::SANDPIT_POWER).map(|s| s.uid) {
                cx.modify_power_amount(e, uid, Dec::int(1), e, p.card);
            }
        }
        cx.add_cost_this_combat(p.card, 1, false);
        Flow::Done
    }
});
