//! Regent subsystems: stars (`PlayerCmd.GainStars` & co), star costs (`CardModel.*StarCost*`), Forge / Sovereign Blade
//! (`ForgeCmd`) and a synchronous `CardCmd.AutoPlay` (spec 03 §5.2-5.3, spec 05 §7-8).

use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

/// `CanonicalStarCost` marker for `HasStarCostX` cards (see `tools/gen_defs.py`).
pub const STAR_COST_X: i8 = -2;

impl Combat {
    // ---- stars -----------------------------------------------------------------------------------------------------

    /// `PlayerCombatState.Stars` setter side effects: `History.StarsModified(delta)` (only on change).
    pub(crate) fn set_stars_internal(&mut self, new: i32) {
        let old = self.player.stars;
        if new != old {
            self.player.stars = new;
            if new > old {
                self.hist.stars_gained_this_turn = self.hist.stars_gained_this_turn.saturating_add((new - old) as i16);
            }
        }
    }

    /// `PlayerCmd.GainStars`: no-op while the combat is ending; `Hook.ShouldGainStars` has no implementer in this build.
    pub fn gain_stars(&mut self, amount: i32) {
        if self.is_ending() || amount < 0 {
            return;
        }
        let new = (self.player.stars as i64 + amount as i64).clamp(0, i32::MAX as i64) as i32;
        self.set_stars_internal(new);
        self.dispatch_g(hookbit::after_stars_gained, |cx, me, l| l.after_stars_gained(cx, me, amount));
    }

    /// `PlayerCmd.LoseStars`: no-op while the combat is ending; no hooks.
    pub fn lose_stars(&mut self, amount: i32) {
        if self.is_ending() || amount < 0 {
            return;
        }
        self.set_stars_internal((self.player.stars - amount).max(0));
    }

    // ---- star costs ------------------------------------------------------------------------------------------------

    #[inline]
    pub fn card_has_star_cost_x(&self, c: CardIdx) -> bool {
        self.card_def(c).star_cost == STAR_COST_X
    }

    /// `CardModel.BaseStarCost` (-1 = none; star-X cards have canonical cost -1).
    #[inline]
    pub fn card_base_star_cost(&self, c: CardIdx) -> i32 {
        let s = self.card_def(c).star_cost;
        if s < 0 { -1 } else { s as i32 }
    }

    /// `CardModel.CurrentStarCost`: the last temporary star cost, except that a temporary 0 never gives a card
    /// without a star cost one.
    pub fn card_current_star_cost(&self, c: CardIdx) -> i32 {
        let base = self.card_base_star_cost(c);
        match self.cards[c as usize].star_mods.last() {
            Some(m) => {
                if m.amount == 0 && base < 0 {
                    base
                } else {
                    m.amount as i32
                }
            }
            None => base,
        }
    }

    /// `CardModel.GetStarCostWithModifiers`: X cards cost all current stars; otherwise `Hook.ModifyStarCost` in combat piles.
    pub fn card_star_cost(&self, c: CardIdx) -> i32 {
        if self.card_has_star_cost_x(c) {
            return self.player.stars;
        }
        let cur = self.card_current_star_cost(c);
        if self.card_in_combat_pile(c) { self.modify_star_cost(c, cur) } else { cur }
    }

    /// `Hook.ModifyStarCost` (threaded `TryModifyStarCost`, guarded; skipped for negative costs).
    fn modify_star_cost(&self, c: CardIdx, cost: i32) -> i32 {
        if cost < 0 || !self.listen.has(hookbit::try_modify_star_cost) || !self.hooks_enabled() {
            return cost;
        }
        let mut v = Dec::int(cost as i64);
        let snap = self.snapshot(Mask::bit(hookbit::try_modify_star_cost));
        for e in snap.iter() {
            if self.still_live(&e.me) {
                if let Some(nv) = content::listener(&e.me).try_modify_star_cost(self, e.me, c, v) {
                    v = nv;
                }
            }
        }
        v.trunc()
    }

    fn add_temp_star_cost(&mut self, c: CardIdx, cost: i32, expire: u8) {
        let m = CostMod { amount: cost as i8, relative: false, reduce_only: false, expire };
        let mods = &mut self.cards[c as usize].star_mods;
        if mods.len() >= 2 {
            mods.remove(0);
        }
        mods.push(m);
    }

    /// `CardModel.SetStarCostUntilPlayed`.
    pub fn set_star_cost_until_played(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, EXPIRE_WHEN_PLAYED);
    }
    /// `CardModel.SetStarCostThisTurn` (cleared at end of turn and when played).
    pub fn set_star_cost_this_turn(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED);
    }
    /// `CardModel.SetStarCostThisCombat`.
    pub fn set_star_cost_this_combat(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, 0);
    }

    /// Drops temporary star costs flagged `flag` (`EXPIRE_END_OF_TURN` / `EXPIRE_WHEN_PLAYED`).
    pub(crate) fn clear_star_mods(&mut self, c: CardIdx, flag: u8) {
        let card = &mut self.cards[c as usize];
        if card.star_mods.is_empty() {
            return;
        }
        let mut kept: crate::util::ArrayVec<CostMod, 2> = crate::util::ArrayVec::new();
        for m in card.star_mods.iter() {
            if m.expire & flag == 0 {
                kept.push(*m);
            }
        }
        card.star_mods = kept;
    }

    /// `CardModel.ResolveStarXValue` (`Hook.ModifyXValue` has no star-relevant implementer besides ChemicalX).
    pub fn resolve_star_x_value(&self, c: CardIdx) -> i32 {
        if !self.card_has_star_cost_x(c) {
            return 0;
        }
        self.cards[c as usize].x_value as i32
    }

    // ---- Forge -----------------------------------------------------------------------------------------------------

    /// Sovereign Blades in the player's combat piles (`Player.PlayerCombatState.AllCards.OfType<SovereignBlade>()`,
    /// pile order Hand, Draw, Discard, Exhaust, Play), duplicates excluded.
    fn sovereign_blades(&self, include_exhausted: bool) -> crate::util::ArrayVec<CardIdx, 32> {
        let mut out = crate::util::ArrayVec::new();
        let p = &self.player;
        for (pile, is_exhaust) in [(&p.hand, false), (&p.draw, false), (&p.discard, false), (&p.exhaust, true), (&p.play, false)] {
            if is_exhaust && !include_exhausted {
                continue;
            }
            for &c in pile.iter() {
                let card = &self.cards[c as usize];
                if card.id == ids::card::SOVEREIGN_BLADE && card.flags & cflag::IS_DUPE == 0 {
                    out.push(c);
                }
            }
        }
        out
    }

    /// `CardModel.TargetType` including dynamic overrides (Sovereign Blade hits all enemies under Seeking Edge).
    #[inline]
    pub fn card_target_type(&self, c: CardIdx) -> TargetType {
        let card = &self.cards[c as usize];
        if card.id == ids::card::SOVEREIGN_BLADE && self.has_power(PLAYER, ids::power::SEEKING_EDGE_POWER) {
            return TargetType::AllEnemies;
        }
        content::card_def(card.id).target
    }

    /// Value of the card's generic named var (`DynamicVar("Name", v)`; `name` = `gen_cards::var_name::*`).
    pub fn regent_named_var(&self, c: CardIdx, name: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in content::card_def(card.id).vars {
            if v.kind == crate::defs::VarKind::Named && v.arg == name {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    /// `SovereignBlade.AddDamage`.
    pub fn blade_add_damage(&mut self, c: CardIdx, amount: i32) {
        let k = &mut self.cards[c as usize];
        k.counter[0] = (k.counter[0] as i32 + amount).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    /// `ForgeCmd.Forge` (spec 05 §8): a Sovereign Blade is created in hand if none exists outside the exhaust pile, then
    /// every blade (exhausted ones included) gains `amount` damage, then `Hook.AfterForge`.
    pub fn forge(&mut self, amount: i32) {
        if self.is_over_or_ending() {
            return;
        }
        if self.sovereign_blades(false).is_empty() {
            if let Some(b) = self.new_card(ids::card::SOVEREIGN_BLADE, 0) {
                self.add_generated_card(b, PileType::Hand, CardPilePosition::Bottom);
            }
        }
        let all = self.sovereign_blades(true);
        for &b in all.iter() {
            self.blade_add_damage(b, amount);
        }
        self.dispatch_g(hookbit::after_forge, |cx, me, l| l.after_forge(cx, me, amount));
    }

    /// Base damage of a card including per-instance growth kept in `counter[0]` (Sovereign Blade forge, Kingly Punch).
    pub fn card_base_damage(&self, c: CardIdx) -> i32 {
        let card = &self.cards[c as usize];
        let extra = if card.id == ids::card::SOVEREIGN_BLADE || card.id == ids::card::KINGLY_PUNCH { card.counter[0] as i32 } else { 0 };
        self.card_var(c, crate::defs::VarKind::Damage) + extra
    }

    /// `Player.PlayerCombatState.AllCards`: hand, draw, discard, exhaust, play pile order.
    pub fn player_combat_cards(&self) -> crate::util::ArrayVec<CardIdx, MAX_CARDS> {
        let mut out = crate::util::ArrayVec::new();
        let p = &self.player;
        for pile in [&p.hand, &p.draw, &p.discard, &p.exhaust, &p.play] {
            for &c in pile.iter() {
                out.push(c);
            }
        }
        out
    }

    /// `CardCmd.Transform(original, replacement)` for one card in a combat pile (CardCmd.cs L374-490): the replacement takes
    /// the original's index, counts as generated by the player, and the original leaves the combat.
    pub fn regent_transform(&mut self, orig: CardIdx, new_id: u16, upgrade: u8) -> Option<CardIdx> {
        if self.is_ending() {
            return None;
        }
        let pile = self.card_pile_type(orig);
        if pile == PileType::None || pile == PileType::Deck {
            return None;
        }
        let idx = self.pile(pile).position(orig)?;
        let new = self.new_card(new_id, upgrade)?;
        self.pile_mut(pile).remove(idx);
        self.pile_mut(pile).insert(idx, new);
        self.cards[new as usize].pile = pile as u8;
        // History.CardGenerated(creator = owner), AfterCardEnteredCombat, AfterCardChangedPiles(pile, pile).
        self.cards_generated_by_player = self.cards_generated_by_player.saturating_add(1);
        self.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, new));
        self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, new, pile));
        self.gen_creator_player = true;
        self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, new));
        self.gen_creator_player = false;
        // original.RemoveFromState()
        let o = &mut self.cards[orig as usize];
        o.pile = PileType::None as u8;
        o.flags |= cflag::REMOVED;
        Some(new)
    }

    /// `CombatState.CreateCard<T>(owner)` x `count` + `CardPileCmd.AddGeneratedCardsToCombat(.., Hand, owner)`.
    pub fn create_regent_cards_in_hand(&mut self, id: u16, count: i32) {
        if count <= 0 || self.is_over_or_ending() {
            return;
        }
        let mut created: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
        for _ in 0..count.min(16) {
            if let Some(c) = self.new_card(id, 0) {
                created.push(c);
            }
        }
        for &c in created.iter() {
            self.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }

    // ---- auto-play from hooks -------------------------------------------------------------------------------------

    /// `CardPileCmd.AutoPlayFromDrawPile(1, Top, false)` run from a HOOK (no enclosing card play): pulls the top card into
    /// the Play pile and auto-plays it (`Combat::auto_play`; a decision it raises is flagged as unfaithful there).
    pub fn auto_play_top_from_hook(&mut self) {
        self.shuffle_if_necessary();
        let Some(c) = self.player.draw.first() else { return };
        self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        if self.cr(PLAYER).is_dead() {
            return;
        }
        self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
        self.auto_play(c);
    }
}
