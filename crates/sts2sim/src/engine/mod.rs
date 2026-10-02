//! The combat engine: `impl Combat` blocks split by subsystem.

mod action;
mod cmds;
mod creature;
mod damage;
mod dispatch;
mod monster;
mod orbs;
mod piles;
mod potions;
mod play;
mod powers;
mod turn;

pub use creature::DamageResult;
pub use damage::{Attack, Mods, Results, Targeting};
pub use dispatch::*;
pub use cmds::Ask;
pub use action::{Action, ActionBuf, ACTION_SPACE};
pub use turn::BASE_HAND_DRAW;
pub use orbs::VALID_ORBS;
