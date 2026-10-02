//! Hook interface. Every card / relic / power / potion / monster / enchantment is a `Listener`: a zero-sized type
//! overriding only the hooks it uses. The `listener!` macro derives the static hook mask from the overridden
//! methods, so dispatch can skip non-listeners with one bit test (most hooks have no listener in a given fight).
//!
//! Hook names, signatures and aggregation follow `docs/spec/02-hooks-damage-powers.md` §2.

use crate::dec::Dec;
use crate::engine::Attack;
use crate::state::*;
use crate::types::*;


/// Hook-presence bitset (192 hooks max).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Mask(pub [u64; 3]);

impl Mask {
    pub const EMPTY: Mask = Mask([0; 3]);
    #[inline(always)]
    pub const fn bit(n: u32) -> Mask {
        let mut m = [0u64; 3];
        m[(n / 64) as usize] = 1u64 << (n % 64);
        Mask(m)
    }
    #[inline(always)]
    pub const fn or(self, o: Mask) -> Mask {
        Mask([self.0[0] | o.0[0], self.0[1] | o.0[1], self.0[2] | o.0[2]])
    }
    #[inline(always)]
    pub const fn intersects(self, o: Mask) -> bool {
        (self.0[0] & o.0[0]) | (self.0[1] & o.0[1]) | (self.0[2] & o.0[2]) != 0
    }
    #[inline(always)]
    pub const fn has(self, n: u32) -> bool {
        self.0[(n / 64) as usize] >> (n % 64) & 1 != 0
    }
}
impl core::ops::BitOr for Mask {
    type Output = Mask;
    #[inline(always)]
    fn bitor(self, o: Mask) -> Mask {
        self.or(o)
    }
}
impl core::ops::BitOrAssign for Mask {
    #[inline(always)]
    fn bitor_assign(&mut self, o: Mask) {
        *self = self.or(o)
    }
}

/// Identity of the listening model (what the C# code calls `this`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Me {
    pub kind: Kind,
    /// Creature that owns it (powers / monsters) — `PLAYER` for relics, potions, cards, orbs.
    pub owner: Cid,
    /// Power `uid` / relic index / potion slot / card index / orb index.
    pub idx: u16,
    /// Content id (`ids::power::*`, `ids::relic::*`, ...).
    pub id: u16,
    /// Power amount at snapshot time (fallback if the power is removed mid-dispatch).
    pub amount: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Power,
    Relic,
    Potion,
    Card,
    Enchantment,
    Affliction,
    Monster,
    Orb,
}

/// Arguments of a damage query (`Hook.ModifyDamage*`).
#[derive(Clone, Copy, Debug)]
pub struct DmgQ {
    pub target: Cid,
    pub dealer: Cid,
    pub card: CardIdx,
    pub props: ValueProp,
    /// Running value at the time this listener is consulted.
    pub amount: Dec,
}

/// Arguments of a block query (`Hook.ModifyBlock*`).
#[derive(Clone, Copy, Debug)]
pub struct BlockQ {
    pub target: Cid,
    pub card: CardIdx,
    pub props: ValueProp,
    pub amount: Dec,
}

/// `CardPlay`.
#[derive(Clone, Copy, Debug)]
pub struct CardPlay {
    pub card: CardIdx,
    pub target: Cid,
    pub is_auto: bool,
    pub play_index: u8,
    pub play_count: u8,
    pub result_pile: PileType,
    pub energy_spent: i32,
    pub stars_spent: i32,
    /// `Resources.EnergyValue`: the energy cost of the play (== `energy_spent` for manual plays; for auto-plays the
    /// cost the card would have had, while `energy_spent` is 0).
    pub energy_value: i32,
}

/// Result of resumable effect code (`on_play`): finished, or suspended waiting for a decision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    Done,
    /// Waiting for a choice; resume the same function with this phase afterwards.
    Suspend(u8),
}

#[allow(unused_variables)]
pub trait Listener: Sync {
    // ---- queries (aggregated) --------------------------------------------------------------------------
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        Dec::ZERO
    }
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        Dec::ONE
    }
    fn modify_damage_cap(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        Dec::MAX
    }
    fn modify_block_additive(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        Dec::ZERO
    }
    fn modify_block_multiplicative(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        Dec::ONE
    }
    /// HP-loss passes (threaded): BeforeOsty{,Late}, AfterOsty{,Late}.
    fn modify_hp_lost_before_osty(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Dec {
        amount
    }
    fn modify_hp_lost_before_osty_late(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Dec {
        amount
    }
    fn modify_hp_lost_after_osty(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Dec {
        amount
    }
    fn modify_hp_lost_after_osty_late(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Dec {
        amount
    }
    /// `ShouldClearBlock` — AND over listeners.
    fn should_clear_block(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// `ShouldAllowHitting` — AND.
    fn should_allow_hitting(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// `ModifyMaxEnergy` (threaded).
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }
    /// `ModifyHandDraw` (threaded).
    fn modify_hand_draw(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }
    /// `ShouldPlayerResetEnergy` — AND.
    fn should_player_reset_energy(&self, cx: &Combat, me: Me) -> bool {
        true
    }
    /// `ShouldFlush` — AND.
    fn should_flush(&self, cx: &Combat, me: Me) -> bool {
        true
    }
    /// `ShouldPlay` — AND.
    fn should_play(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        true
    }
    /// `TryModifyEnergyCostInCombat` pass 1 / pass 2 ("Late": free-cost effects). `None` = unchanged.
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        None
    }
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        None
    }
    /// `ModifyPowerAmountGiven` additive / multiplicative passes.
    fn modify_power_amount_given_additive(&self, cx: &Combat, me: Me, power_id: u16, giver: Cid, amount: Dec, target: Cid, card: CardIdx) -> Dec {
        Dec::ZERO
    }
    fn modify_power_amount_given_multiplicative(&self, cx: &Combat, me: Me, power_id: u16, giver: Cid, amount: Dec, target: Cid, card: CardIdx) -> Dec {
        Dec::ONE
    }
    /// `TryModifyPowerAmountReceived` (threaded): `Some(new)` = this listener modified the amount.
    fn try_modify_power_amount_received(&self, cx: &Combat, me: Me, power_id: u16, target: Cid, amount: Dec, applier: Cid) -> Option<Dec> {
        None
    }
    /// Card logic: `IsPlayable`.
    fn is_playable(&self, cx: &Combat, card: CardIdx) -> bool {
        true
    }

    // ---- notifications ---------------------------------------------------------------------------------
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {}
    fn before_combat_start_late(&self, cx: &mut Combat, me: Me) {}
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {}
    fn after_combat_victory_early(&self, cx: &mut Combat, me: Me) {}
    fn after_combat_victory(&self, cx: &mut Combat, me: Me) {}
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn after_side_turn_start_late(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn before_side_turn_end_very_early(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn before_side_turn_end_early(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn after_side_turn_end_late(&self, cx: &mut Combat, me: Me, side: Side) {}
    fn after_block_cleared(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    fn after_preventing_block_clear(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {}
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {}
    fn after_energy_spent(&self, cx: &mut Combat, me: Me, card: CardIdx, amount: i32) {}
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {}
    fn after_hand_emptied(&self, cx: &mut Combat, me: Me) {}
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {}
    fn after_auto_post_play_phase_entered(&self, cx: &mut Combat, me: Me) {}
    fn before_flush(&self, cx: &mut Combat, me: Me) {}
    fn before_flush_late(&self, cx: &mut Combat, me: Me) {}
    fn after_flush(&self, cx: &mut Combat, me: Me) {}
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {}
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {}
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {}
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {}
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, from_hand_draw: bool) {}
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, card: CardIdx, by_ethereal: bool) {}
    fn after_card_discarded(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    fn before_potion_used(&self, cx: &mut Combat, me: Me, potion: u16, target: Cid) {}
    fn after_potion_used(&self, cx: &mut Combat, me: Me, potion: u16, target: Cid) {}
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    fn after_card_changed_piles(&self, cx: &mut Combat, me: Me, card: CardIdx, old: PileType) {}
    fn after_shuffle(&self, cx: &mut Combat, me: Me) {}
    fn after_block_gained(&self, cx: &mut Combat, me: Me, creature: Cid, amount: Dec) {}
    fn after_block_broken(&self, cx: &mut Combat, me: Me, target: Cid, breaker: Cid) {}
    fn after_current_hp_changed(&self, cx: &mut Combat, me: Me, creature: Cid, delta: i32) {}
    fn before_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid) {}
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, dealer: Cid) {}
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, unblocked: i32, props: ValueProp) {}
    fn after_modifying_damage_amount(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    fn after_modifying_hp_lost_before_osty(&self, cx: &mut Combat, me: Me) {}
    fn after_modifying_hp_lost_after_osty(&self, cx: &mut Combat, me: Me) {}
    fn before_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, amount: Dec, target: Cid, applier: Cid) {}
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, amount: i32) {}
    fn after_modifying_power_amount_given(&self, cx: &mut Combat, me: Me, power_id: u16) {}
    fn after_modifying_power_amount_received(&self, cx: &mut Combat, me: Me, power_id: u16) {}
    /// Called on the power itself before it is attached.
    fn before_applied(&self, cx: &mut Combat, me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {}
    fn after_applied(&self, cx: &mut Combat, me: Me) {}
    fn after_removed(&self, cx: &mut Combat, me: Me, old_owner: Cid) {}
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid) {}

    // ---- card logic ------------------------------------------------------------------------------------
    /// `CardModel.OnPlay`. Resumable: called with `phase = 0`, and again with the returned phase after a decision.
    fn on_play(&self, cx: &mut Combat, play: &CardPlay, phase: u8) -> Flow {
        Flow::Done
    }
    /// `PotionModel.OnUse` (resumable like `on_play`). `target` is the chosen creature (the player for self-targeted
    /// potions, `NO` for AoE).
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, phase: u8) -> Flow {
        Flow::Done
    }
    /// `CardModel.OnTurnEndInHand`.
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {}

    // ---- relic-driven additions (appended; see content/relics) ---------------------------------------------
    /// `Hook.AfterRoomEntered` (run-level iterator, no combat-ending guard): fires once at combat setup, before the
    /// enemies' first `RollMove` and before `BeforeCombatStart`. Only relics/potions override it.
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {}
    fn after_player_turn_start_late(&self, cx: &mut Combat, me: Me) {}
    fn after_energy_reset_late(&self, cx: &mut Combat, me: Me) {}
    fn after_auto_pre_play_phase_entered_late(&self, cx: &mut Combat, me: Me) {}
    /// `AfterPreventingDraw` — told to the first listener whose `should_draw` vetoed the draw.
    fn after_preventing_draw(&self, cx: &mut Combat, me: Me) {}
    fn after_modifying_hand_draw(&self, cx: &mut Combat, me: Me) {}
    /// `ModifyXValue` (threaded int).
    fn modify_x_value(&self, cx: &Combat, me: Me, card: CardIdx, value: i32) -> i32 {
        value
    }
    fn should_die_late(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// `Hook.AfterCreatureAddedToCombat` (unguarded): mid-combat spawns (`Combat::notify_creature_added`).
    fn after_creature_added_to_combat(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    /// `AfterModifyingBlockAmount` — only for models that changed the block amount (`card` = source card or `NO`).
    fn after_modifying_block_amount(&self, cx: &mut Combat, me: Me, amount: Dec, card: CardIdx) {}
    fn after_stars_spent(&self, cx: &mut Combat, me: Me, amount: i32) {}
    fn after_stars_gained(&self, cx: &mut Combat, me: Me, amount: i32) {}
    fn try_modify_star_cost(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        None
    }
    /// `Hook.AfterDiedToDoom` (unguarded): the creatures Doom just killed (`Combat::notify_died_to_doom`).
    fn after_died_to_doom(&self, cx: &mut Combat, me: Me, creatures: &[Cid]) {}
    /// Continuation of a hook that suspended (`Combat::suspend_hook`): called after its decision / the card play it started
    /// finished. `phase` is whatever the hook stored.
    fn hook_resume(&self, cx: &mut Combat, me: Me, phase: u8) {}
    /// `ModifyGoldGained` (threaded, run-level) — Ectoplasm.
    fn modify_gold_gained(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }

    // ---- relic state metadata (static dispatch by relic id; NOT hooks, no mask bit) ------------------------
    /// The relic's `[SavedProperty]` list: how the oracle dumps / injects its persistent state (`Relic::{counter,aux,flags}`).
    fn meta_props(&self) -> &'static [PropDef] {
        &[]
    }
    /// `ShowCounter ? DisplayAmount : none` evaluated in the current combat state (the oracle dumps it as `counter`).
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        None
    }
    /// Fresh relic instance state (C# field initialisers that are not zero / false), applied before injecting props.
    fn meta_initial(&self) -> (i32, u8, i32) {
        (0, 0, 0)
    }
    // ---- appended hooks (ironclad_a1) ------------------------------------------------------------------
    /// `ShouldDraw(player, fromHandDraw)` — AND over guarded listeners, checked once per `Draw` call.
    fn should_draw(&self, cx: &Combat, me: Me, from_hand_draw: bool) -> bool {
        true
    }
    /// `AfterCardDrawnEarly` — runs for every listener before the `AfterCardDrawn` pass (Hellraiser).
    fn after_card_drawn_early(&self, cx: &mut Combat, me: Me, card: CardIdx, from_hand_draw: bool) {}
    /// `ModifyCardPlayResultLocation` (threaded over the pile type; position stays Bottom) — Corruption.
    fn modify_card_play_result_location(&self, cx: &Combat, me: Me, card: CardIdx, is_auto: bool, pile: PileType) -> PileType {
        pile
    }
    /// `PowerModel.ShouldOwnerDeathTriggerFatal` — AND over the dying creature's powers (Minion, Reattach).
    fn should_owner_death_trigger_fatal(&self, cx: &Combat, me: Me) -> bool {
        true
    }
    // ---- potions / death prevention / play count (added by the potions work; append-only) -------------------------
    /// `ShouldProcurePotion` — AND (Sozu).
    fn should_procure_potion(&self, cx: &Combat, me: Me, potion: u16) -> bool {
        true
    }
    fn after_potion_procured(&self, cx: &mut Combat, me: Me, potion: u16) {}
    fn after_potion_discarded(&self, cx: &mut Combat, me: Me, potion: u16) {}
    /// `ShouldDie` — AND over run + combat listeners; the first listener returning false is the "preventer".
    fn should_die(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// `AfterPreventingDeath` — called on the preventer only.
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    /// `ModifyCardPlayCount` (threaded, DuplicationPower / Burst-like effects).
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, target: Cid, count: i32) -> i32 {
        count
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    /// `AfterPowerAmountChanged(power, amount, applier, cardSource)` with the full argument set (the plain
    /// `after_power_amount_changed` only knows the power id). Dispatched right after it.
    fn after_power_amount_changed_full(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {}
}

/// The arguments of `AfterPowerAmountChanged`: `power` is identified by (`target`, `uid`).
#[derive(Clone, Copy, Debug)]
pub struct PowerChange {
    pub power_id: u16,
    pub target: Cid,
    pub uid: u16,
    /// The change (delta), not the new total.
    pub amount: i32,
    pub applier: Cid,
    pub card: CardIdx,
}

/// Statically derived hook mask of a listener type.
pub trait HasMask {
    const MASK: Mask;
}

/// Bit index of every hook (must list every `Listener` method that content may override).
#[allow(non_upper_case_globals)]
pub mod hookbit {
    macro_rules! bits { ($($n:ident),* $(,)?) => { bits!(@ 0u32; $($n),*); }; (@ $i:expr; $h:ident $(, $t:ident)*) => { pub const $h: u32 = $i; bits!(@ $i + 1; $($t),*); }; (@ $i:expr;) => { pub const COUNT: u32 = $i; }; }
    bits!(
        modify_damage_additive,
        modify_damage_multiplicative,
        modify_damage_cap,
        modify_block_additive,
        modify_block_multiplicative,
        modify_hp_lost_before_osty,
        modify_hp_lost_before_osty_late,
        modify_hp_lost_after_osty,
        modify_hp_lost_after_osty_late,
        should_clear_block,
        should_allow_hitting,
        modify_max_energy,
        modify_hand_draw,
        should_player_reset_energy,
        should_flush,
        should_play,
        try_modify_energy_cost_in_combat,
        try_modify_energy_cost_in_combat_late,
        modify_power_amount_given_additive,
        modify_power_amount_given_multiplicative,
        try_modify_power_amount_received,
        is_playable,
        before_combat_start,
        before_combat_start_late,
        after_combat_end,
        after_combat_victory_early,
        after_combat_victory,
        before_side_turn_start,
        after_side_turn_start,
        after_side_turn_start_late,
        before_side_turn_end_very_early,
        before_side_turn_end_early,
        before_side_turn_end,
        after_side_turn_end,
        after_side_turn_end_late,
        after_block_cleared,
        after_preventing_block_clear,
        after_player_turn_start,
        after_energy_reset,
        after_energy_spent,
        before_hand_draw,
        after_hand_emptied,
        after_auto_pre_play_phase_entered,
        after_auto_post_play_phase_entered,
        before_flush,
        before_flush_late,
        after_flush,
        before_attack,
        after_attack,
        before_card_played,
        after_card_played,
        after_card_drawn,
        after_card_exhausted,
        after_card_discarded,
        before_potion_used,
        after_potion_used,
        on_use_potion,
        after_card_generated_for_combat,
        after_card_entered_combat,
        after_card_changed_piles,
        after_shuffle,
        after_block_gained,
        after_block_broken,
        after_current_hp_changed,
        before_damage_received,
        after_damage_received,
        after_damage_given,
        after_modifying_damage_amount,
        after_modifying_hp_lost_before_osty,
        after_modifying_hp_lost_after_osty,
        before_power_amount_changed,
        after_power_amount_changed,
        after_modifying_power_amount_given,
        after_modifying_power_amount_received,
        before_applied,
        after_applied,
        after_removed,
        after_death,
        on_play,
        on_turn_end_in_hand,
        after_room_entered,
        after_player_turn_start_late,
        after_energy_reset_late,
        after_auto_pre_play_phase_entered_late,
        after_preventing_draw,
        after_modifying_hand_draw,
        modify_x_value,
        should_die_late,
        after_creature_added_to_combat,
        after_modifying_block_amount,
        after_stars_spent,
        after_stars_gained,
        try_modify_star_cost,
        modify_gold_gained,
        should_draw,
        after_card_drawn_early,
        modify_card_play_result_location,
        should_owner_death_trigger_fatal,
        after_died_to_doom,
        should_procure_potion,
        after_potion_procured,
        after_potion_discarded,
        should_die,
        after_preventing_death,
        modify_card_play_count,
        after_modifying_card_play_count,
        after_power_amount_changed_full,
    );
    // `Listener::meta_*` are static metadata, not hooks: they only need a (never dispatched) bit so `listener!` can name them.
    // The static-metadata pseudo bits sit at the very top of the 192-bit mask: real hooks must stay below them.
    const _: () = assert!(COUNT <= 188, "too many hooks: the `meta_*` / `hook_resume` pseudo bits start at 188");
    pub const hook_resume: u32 = 188;
    pub const meta_props: u32 = 191;
    pub const meta_display: u32 = 190;
    pub const meta_initial: u32 = 189;
}

/// Declares a listener: a unit struct implementing `Listener` for just the listed hooks and deriving its mask.
///
/// ```ignore
/// listener!(StrengthPower {
///     fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec { ... }
/// });
/// ```
#[macro_export]
macro_rules! listener {
    ($name:ident { $( fn $f:ident ( $($args:tt)* ) $(-> $ret:ty)? $body:block )* }) => {
        pub struct $name;
        impl $crate::hooks::HasMask for $name {
            const MASK: $crate::hooks::Mask = $crate::hooks::Mask::EMPTY $( .or($crate::hooks::Mask::bit($crate::hooks::hookbit::$f)) )*;
        }
        impl $crate::hooks::Listener for $name {
            $( fn $f ( $($args)* ) $(-> $ret)? $body )*
        }
    };
}

/// `fn meta_props(&self) -> &'static [PropDef] { relic_props![PropDef::int("TurnsSeen", Slot::Counter)] }`
#[macro_export]
macro_rules! relic_props {
    ($($e:expr),* $(,)?) => {{
        const P: &[$crate::state::PropDef] = &[$($e),*];
        P
    }};
}
