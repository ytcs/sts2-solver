#![recursion_limit = "512"]
//! Slay the Spire 2 combat simulator core.
//!
//! Design goals, in priority order: bit-exact fidelity with the game's combat rules, then raw
//! throughput and memory efficiency. All simulation state is plain data (`Clone` is a memcpy,
//! no heap allocation on the hot path) so tens of thousands of fights can run in parallel.

pub mod content;
pub mod dec;
pub mod defs;
pub mod engine;
pub mod hooks;
pub mod observe;
pub mod ids;
pub mod rng;
pub mod scenario;
pub mod sort;
pub mod state;
pub mod types;
pub mod util;

pub use engine::Action;
pub use scenario::{DeckCard, RelicInit, Scenario};
pub use state::{Combat, Stage};
