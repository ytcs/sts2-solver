//! Hook interface. Every card / relic / power / potion / monster / enchantment is a `Listener`: a zero-sized type
//! overriding only the hooks it uses. The `listener!` macro derives the static hook mask from the overridden
//! methods, so dispatch can skip non-listeners with one bit test (most hooks have no listener in a given fight).
//!
//! Hook names, signatures and aggregation follow `docs/spec/02-hooks-damage-powers.md` §2.

use crate::dec::Dec;
use crate::engine::Attack;
use crate::state::*;
use crate::types::*;


/// Hook-presence bitset (256 hooks max).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Mask(pub [u64; 4]);

impl Mask {
    pub const EMPTY: Mask = Mask([0; 4]);
    #[inline(always)]
    pub const fn bit(n: u32) -> Mask {
        let mut m = [0u64; 4];
        m[(n / 64) as usize] = 1u64 << (n % 64);
        Mask(m)
    }
    #[inline(always)]
    pub const fn or(self, o: Mask) -> Mask {
        Mask([self.0[0] | o.0[0], self.0[1] | o.0[1], self.0[2] | o.0[2], self.0[3] | o.0[3]])
    }
    #[inline(always)]
    pub const fn intersects(self, o: Mask) -> bool {
        (self.0[0] & o.0[0]) | (self.0[1] & o.0[1]) | (self.0[2] & o.0[2]) | (self.0[3] & o.0[3]) != 0
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
    /// `Resources.EnergyValue` (`GetAmountToSpend()`: what the play would cost / the captured X; not spent for auto-play).
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
    /// `AfterPowerAmountChanged(power, amount, applier, cardSource)`.
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {}
    fn after_modifying_power_amount_given(&self, cx: &mut Combat, me: Me, power_id: u16) {}
    fn after_modifying_power_amount_received(&self, cx: &mut Combat, me: Me, power_id: u16) {}
    /// Called on the power itself before it is attached.
    fn before_applied(&self, cx: &mut Combat, me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {}
    fn after_applied(&self, cx: &mut Combat, me: Me) {}
    fn after_removed(&self, cx: &mut Combat, me: Me, old_owner: Cid) {}
    /// `AfterDeath(creature, wasRemovalPrevented, deathAnimLength)`: `was_removal_prevented` is true when `ShouldDie`
    /// vetoed the death (Fairy in a Bottle ...), false for a real death (spec 02 §5.3).
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {}

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

    // ---- engine-core additions (new hooks are appended here; spec 02 §2 / Appendix A) ------------------------------
    // Dispatch class in brackets: G = guarded iterator (silent once combat is ending), C = unguarded, R = run-level
    // iterator (== unguarded here: deck copies never listen in combat).

    // notifications
    /// [G] `BeforeBlockGained` — RAW amount (before modifiers).
    fn before_block_gained(&self, cx: &mut Combat, me: Me, creature: Cid, amount: Dec, props: ValueProp, card: CardIdx) {}
    /// [G] `AfterModifyingBlockAmount` — only models that changed the block value in `ModifyBlock`.
    fn after_modifying_block_amount(&self, cx: &mut Combat, me: Me, modified: Dec, card: CardIdx) {}
    /// [G] `BeforeCardAutoPlayed`.
    fn before_card_auto_played(&self, cx: &mut Combat, me: Me, card: CardIdx, target: Cid, kind: AutoPlayType) {}
    /// [G] first pass of `AfterCardDrawn` (Hellraiser); `after_card_drawn` is the second pass.
    fn after_card_drawn_early(&self, cx: &mut Combat, me: Me, card: CardIdx, from_hand_draw: bool) {}
    /// [C] second pass of `AfterCardPlayed`.
    fn after_card_played_late(&self, cx: &mut Combat, me: Me, play: &CardPlay) {}
    /// [R] second pass of `AfterCardChangedPiles`.
    fn after_card_changed_piles_late(&self, cx: &mut Combat, me: Me, card: CardIdx, old: PileType) {}
    /// [C] `AfterCreatureAddedToCombat` (summons).
    fn after_creature_added_to_combat(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    /// [C] `AfterDiedToDoom`.
    fn after_died_to_doom(&self, cx: &mut Combat, me: Me, creatures: &[Cid]) {}
    /// [G] second pass of `AfterEnergyReset`.
    fn after_energy_reset_late(&self, cx: &mut Combat, me: Me) {}
    /// [G] second pass of `BeforeHandDraw`.
    fn before_hand_draw_late(&self, cx: &mut Combat, me: Me) {}
    /// [G] `AfterPlayerTurnStart` first / third pass.
    fn after_player_turn_start_early(&self, cx: &mut Combat, me: Me) {}
    fn after_player_turn_start_late(&self, cx: &mut Combat, me: Me) {}
    /// [G] `AfterAutoPrePlayPhaseEntered` first / third pass.
    fn after_auto_pre_play_phase_entered_early(&self, cx: &mut Combat, me: Me) {}
    fn after_auto_pre_play_phase_entered_late(&self, cx: &mut Combat, me: Me) {}
    /// [G] `AfterPreventingDraw` — called on the single modifier returned by `ShouldDraw`.
    fn after_preventing_draw(&self, cx: &mut Combat, me: Me) {}
    /// [R] `BeforeDeath` — fires even if the death will be prevented.
    fn before_death(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    /// [R] `AfterPreventingDeath` — only the preventer (Fairy in a Bottle, Lizard Tail heal here).
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {}
    /// [G] `AfterTakingExtraTurn`.
    fn after_taking_extra_turn(&self, cx: &mut Combat, me: Me) {}
    /// [R] `AfterRoomEntered` — combat start, before the enemies' initial `RollMove` (spec 01 §3.5).
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {}
    /// [R] `AfterPotionDiscarded` / `AfterPotionProcured`.
    fn after_potion_discarded(&self, cx: &mut Combat, me: Me, potion: u16) {}
    fn after_potion_procured(&self, cx: &mut Combat, me: Me, potion: u16) {}
    /// [G] `AfterModifyingCardPlayCount` / `...ResultLocation` — only the models that modified the value.
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    fn after_modifying_card_play_result_location(&self, cx: &mut Combat, me: Me, card: CardIdx, loc: CardLocation) {}
    /// [G] `AfterModifyingEnergyGain` / `AfterModifyingHandDraw` — only the models that changed the (int) value.
    fn after_modifying_energy_gain(&self, cx: &mut Combat, me: Me) {}
    fn after_modifying_hand_draw(&self, cx: &mut Combat, me: Me) {}
    /// [R] second pass of `AfterDamageReceived`.
    fn after_damage_received_late(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, dealer: Cid) {}
    /// [G] stars: `AfterStarsGained` / `AfterStarsSpent`.
    fn after_stars_gained(&self, cx: &mut Combat, me: Me, amount: i32) {}
    fn after_stars_spent(&self, cx: &mut Combat, me: Me, amount: i32) {}
    /// [G] Regent / Necrobinder / Defect command hooks (declared for the character subsystems; the engine core does
    /// not dispatch them yet): `AfterForge`, `AfterSummon`, `AfterOstyRevived`, `AfterOrbChanneled`, `AfterOrbEvoked`,
    /// `AfterModifyingOrbPassiveTriggerCount`.
    fn after_forge(&self, cx: &mut Combat, me: Me, amount: Dec) {}
    fn after_summon(&self, cx: &mut Combat, me: Me, amount: Dec) {}
    fn after_osty_revived(&self, cx: &mut Combat, me: Me, osty: Cid) {}
    fn after_orb_channeled(&self, cx: &mut Combat, me: Me, orb: u16) {}
    fn after_orb_evoked(&self, cx: &mut Combat, me: Me, orb: u16) {}
    fn after_modifying_orb_passive_trigger_count(&self, cx: &mut Combat, me: Me, orb: u16) {}

    // value hooks
    /// [G] `ModifyAttackHitCount` (threaded int). No shipped model overrides it.
    fn modify_attack_hit_count(&self, cx: &Combat, me: Me, attack: &Attack, hits: i32) -> i32 {
        hits
    }
    /// [G] `ModifyCardPlayCount` (threaded int; recorded when changed).
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, target: Cid, count: i32) -> i32 {
        count
    }
    /// [G] `ModifyCardPlayResultLocation` (threaded; recorded when changed). `energy_value` = `ResourceInfo.EnergyValue`
    /// (the play's cost / captured X, also for auto-plays that spend nothing; FeralPower tests `energy_value > 0`).
    fn modify_card_play_result_location(&self, cx: &Combat, me: Me, card: CardIdx, is_auto: bool, energy_value: i32, loc: CardLocation) -> CardLocation {
        loc
    }
    /// `CalculatedDamageVar.Calculate(target)` of a card (`None` = the card has no calculated damage). Called directly
    /// on the card's listener (not a snapshot hook): cards whose damage is a `CalculatedDamageVar` should implement it so
    /// effects that read it generically (Thrash) agree with the card's own attack.
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        None
    }
    /// [C] `TryModifyKeywordsInCombat`: returns the new keyword set (threaded).
    fn try_modify_keywords_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, keywords: u8) -> u8 {
        keywords
    }
    /// [G] `ModifyEnergyGain` (threaded).
    fn modify_energy_gain(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }
    /// [G] `ModifyHandDrawLate` (threaded).
    fn modify_hand_draw_late(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }
    /// [G, exempt while starting] `ModifyShuffleOrder` — mutates the shuffled list in place.
    fn modify_shuffle_order(&self, cx: &Combat, me: Me, cards: &mut [CardIdx], is_initial_shuffle: bool) {}
    /// [C] `ModifyUnblockedDamageTarget` (threaded; DieForYou redirects to Osty).
    fn modify_unblocked_damage_target(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid) -> Cid {
        target
    }
    /// [G] `ModifyXValue` (threaded int).
    fn modify_x_value(&self, cx: &Combat, me: Me, card: CardIdx, value: i32) -> i32 {
        value
    }
    /// [G] `TryModifyStarCost` (threaded; skipped when the cost is negative).
    fn try_modify_star_cost(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        None
    }
    /// [G] `ModifySummonAmount` / `ModifyOrbValue` / `ModifyOrbPassiveTriggerCounts` (threaded).
    fn modify_summon_amount(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        amount
    }
    fn modify_orb_value(&self, cx: &Combat, me: Me, orb: u16, value: Dec) -> Dec {
        value
    }
    fn modify_orb_passive_trigger_counts(&self, cx: &Combat, me: Me, orb: u16, count: i32) -> i32 {
        count
    }

    // predicates
    /// [G] `ShouldAfflict` — AND.
    fn should_afflict(&self, cx: &Combat, me: Me, card: CardIdx, affliction: u8) -> bool {
        true
    }
    /// [G] `ShouldAllowTargeting` — AND.
    fn should_allow_targeting(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// [R] `ShouldDie` pass 1 / `ShouldDieLate` pass 2 — AND, first `false` is the preventer.
    fn should_die(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    fn should_die_late(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// [C] `ShouldCreatureBeRemovedFromCombatAfterDeath` — AND.
    fn should_creature_be_removed_from_combat_after_death(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        true
    }
    /// [C] `ShouldPowerBeRemovedOnDeath` — AND (Illusion).
    fn should_power_be_removed_on_death(&self, cx: &Combat, me: Me, owner: Cid, power_id: u16) -> bool {
        true
    }
    /// [G] `ShouldDraw` — AND, the vetoing model gets `after_preventing_draw`.
    fn should_draw(&self, cx: &Combat, me: Me, from_hand_draw: bool) -> bool {
        true
    }
    /// [G] `ShouldEtherealTrigger` — AND.
    fn should_ethereal_trigger(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        true
    }
    /// [G] `ShouldGainStars` — AND.
    fn should_gain_stars(&self, cx: &Combat, me: Me, amount: Dec) -> bool {
        true
    }
    /// [G] `ShouldPayExcessEnergyCostWithStars` — OR (default false).
    fn should_pay_excess_energy_cost_with_stars(&self, cx: &Combat, me: Me) -> bool {
        false
    }
    /// [R] `ShouldProcurePotion` — AND.
    fn should_procure_potion(&self, cx: &Combat, me: Me, potion: u16) -> bool {
        true
    }
    /// [C] `ShouldStopCombatFromEnding` — OR (Adaptable, Infested, SteamEruption, Stock, Surprise).
    fn should_stop_combat_from_ending(&self, cx: &Combat, me: Me) -> bool {
        false
    }
    /// [G] `ShouldTakeExtraTurn` — OR.
    fn should_take_extra_turn(&self, cx: &Combat, me: Me) -> bool {
        false
    }

    // ---- non-hook virtuals of the model base classes (called directly by the engine) --------------------------------
    /// `PowerModel.ShouldPowerBeRemovedAfterOwnerDeath` (default true).
    fn should_power_be_removed_after_owner_death(&self, cx: &Combat, me: Me) -> bool {
        true
    }
    /// `PowerModel.ShouldOwnerDeathTriggerFatal` (default true; false for Minion / Reattach).
    fn should_owner_death_trigger_fatal(&self, cx: &Combat, me: Me) -> bool {
        true
    }
    /// `MonsterModel.BeforeRemovedFromRoom`.
    fn before_removed_from_room(&self, cx: &mut Combat, me: Me) {}
    /// `MonsterModel.OnDieToDoom`.
    fn on_die_to_doom(&self, cx: &mut Combat, me: Me) {}

    // ---- enchantment / affliction virtuals (called directly by the engine, not hook dispatchers) -------------------
    /// `EnchantmentModel.EnchantDamageAdditive/Multiplicative(original, props)`.
    fn enchant_damage_additive(&self, cx: &Combat, me: Me, original: Dec, props: ValueProp) -> Dec {
        Dec::ZERO
    }
    fn enchant_damage_multiplicative(&self, cx: &Combat, me: Me, original: Dec, props: ValueProp) -> Dec {
        Dec::ONE
    }
    /// `EnchantBlockAdditive/Multiplicative(original)`.
    fn enchant_block_additive(&self, cx: &Combat, me: Me, original: Dec) -> Dec {
        Dec::ZERO
    }
    fn enchant_block_multiplicative(&self, cx: &Combat, me: Me, original: Dec) -> Dec {
        Dec::ONE
    }
    /// `EnchantmentModel.EnchantPlayCount(base)`.
    fn enchant_play_count(&self, cx: &Combat, me: Me, base: i32) -> i32 {
        base
    }
    /// `EnchantmentModel.ShouldStartAtBottomOfDrawPile` (Imbued).
    fn should_start_at_bottom_of_draw_pile(&self, cx: &Combat, me: Me) -> bool {
        false
    }
    /// `EnchantmentModel.OnEnchant` (card = the enchanted card).
    fn on_enchant(&self, cx: &mut Combat, me: Me, card: CardIdx) {}
    /// `EnchantmentModel.OnPlay` / `AfflictionModel.OnPlay` — run in the replay loop after the card's `OnPlay`.
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, play: &CardPlay) {}
    fn on_play_affliction(&self, cx: &mut Combat, me: Me, play: &CardPlay) {}
    /// `EnchantmentModel.CanEnchantCardType` / `CanEnchant` (default: the base rule + the type check).
    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        true
    }
    fn can_enchant(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, self.can_enchant_card_type(cx.card_def(card).ctype))
    }
    /// `AfflictionModel.CanAfflictCardType` / `CanAfflictUnplayableCards` / `IsStackable` / `CanAfflict(card)`.
    /// `PowerModel.InitInternalData()` as the initial value of `Power::aux` (per-instance private state; default 0).
    fn initial_power_aux(&self) -> i32 {
        0
    }
    fn can_afflict_card_type(&self, card_type: CardType) -> bool {
        true
    }
    fn can_afflict_unplayable_cards(&self) -> bool {
        true
    }
    fn affliction_is_stackable(&self) -> bool {
        false
    }
    fn can_afflict(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        cx.base_can_afflict(me, card)
    }

    /// [G] `ShouldPlay` with the auto-play type (`AutoPlayType.None` for manual plays / `CanPlay`). Content that only
    /// vetoes manual plays (Enthralled) overrides this instead of `should_play`; both are AND-ed.
    fn should_play_kind(&self, cx: &Combat, me: Me, card: CardIdx, kind: AutoPlayType) -> bool {
        true
    }
    /// `CardModel.GetResultLocationForCardPlay` override (ParticleWall, ShiningStrike, TheBall): `None` = the base rule.
    fn get_result_location_for_card_play(&self, cx: &mut Combat, me: Me, card: CardIdx) -> Option<CardLocation> {
        None
    }
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
    macro_rules! bits { ($($n:ident),* $(,)?) => { bits!(@ 0u32; $($n),*); }; (@ $i:expr; $h:ident $(, $t:ident)*) => { pub const $h: u32 = $i; bits!(@ $i + 1; $($t),*); }; (@ $i:expr;) => {}; }
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
        // ---- engine-core additions ----
        before_block_gained,
        after_modifying_block_amount,
        before_card_auto_played,
        after_card_drawn_early,
        after_card_played_late,
        after_card_changed_piles_late,
        after_creature_added_to_combat,
        after_died_to_doom,
        after_energy_reset_late,
        before_hand_draw_late,
        after_player_turn_start_early,
        after_player_turn_start_late,
        after_auto_pre_play_phase_entered_early,
        after_auto_pre_play_phase_entered_late,
        after_preventing_draw,
        before_death,
        after_preventing_death,
        after_taking_extra_turn,
        after_room_entered,
        after_potion_discarded,
        after_potion_procured,
        after_modifying_card_play_count,
        after_modifying_card_play_result_location,
        after_modifying_energy_gain,
        after_modifying_hand_draw,
        after_damage_received_late,
        after_stars_gained,
        after_stars_spent,
        after_forge,
        after_summon,
        after_osty_revived,
        after_orb_channeled,
        after_orb_evoked,
        after_modifying_orb_passive_trigger_count,
        modify_attack_hit_count,
        modify_card_play_count,
        modify_card_play_result_location,
        try_modify_keywords_in_combat,
        modify_energy_gain,
        modify_hand_draw_late,
        modify_shuffle_order,
        modify_unblocked_damage_target,
        modify_x_value,
        try_modify_star_cost,
        modify_summon_amount,
        modify_orb_value,
        modify_orb_passive_trigger_counts,
        should_afflict,
        should_allow_targeting,
        should_die,
        should_die_late,
        should_creature_be_removed_from_combat_after_death,
        should_power_be_removed_on_death,
        should_draw,
        should_ethereal_trigger,
        should_gain_stars,
        should_pay_excess_energy_cost_with_stars,
        should_procure_potion,
        should_stop_combat_from_ending,
        should_take_extra_turn,
        should_power_be_removed_after_owner_death,
        should_owner_death_trigger_fatal,
        before_removed_from_room,
        on_die_to_doom,
        enchant_damage_additive,
        enchant_damage_multiplicative,
        enchant_block_additive,
        enchant_block_multiplicative,
        enchant_play_count,
        should_start_at_bottom_of_draw_pile,
        on_enchant,
        on_play_enchantment,
        on_play_affliction,
        can_afflict,
        can_enchant_card_type,
        can_enchant,
        initial_power_aux,
        can_afflict_card_type,
        can_afflict_unplayable_cards,
        affliction_is_stackable,
        should_play_kind,
        get_result_location_for_card_play,
        calculated_damage,
    );
    // The mask has 256 bits.
    const _: () = assert!(get_result_location_for_card_play < 256);
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
