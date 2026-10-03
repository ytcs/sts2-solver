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

// (FranticEscape lives in status.rs.)
