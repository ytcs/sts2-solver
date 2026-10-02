//! The combat engine: `impl Combat` blocks split by subsystem.

mod action;
mod autoplay;
mod cmds;
mod creature;
mod cost;
mod damage;
mod death;
mod enchant;
mod energy;
mod history;
mod dispatch;
mod lifecycle;
mod monster;
mod necro;
mod pets;
mod piles;
mod potion_gen;
mod potions;
mod play;
mod powers;
mod regent;
mod relic_cmds;
mod turn;

pub use creature::DamageResult;
pub use necro::{is_temporary_power, temporary_inner_power};
pub use damage::{Attack, Mods, Results, Targeting};
pub use dispatch::*;
pub use cmds::Ask;
pub use action::{Action, ActionBuf, ACTION_SPACE};
pub use monster::{STUN_INTENTS, STUN_NODE};
pub use turn::BASE_HAND_DRAW;
pub use cost::CostMods;
pub use play::RunResult;
pub use history::{HKind, HistEntry, HistLog, HIST_CAP};
