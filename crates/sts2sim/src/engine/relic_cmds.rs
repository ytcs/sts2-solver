//! Helpers the relic implementations are written against (thin wrappers over `PlayerCmd` / `CreatureCmd` / `PotionCmd`
//! semantics that no card needed before).

use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    // ---- relic state access ----------------------------------------------------------------------------------------

    /// The relic instance behind a relic-listener identity (`Me::idx` = index in `player.relics`).
    #[inline(always)]
    pub fn rel(&self, me: Me) -> &Relic {
        &self.player.relics[me.idx as usize]
    }
    #[inline(always)]
    pub fn rel_mut(&mut self, me: Me) -> &mut Relic {
        &mut self.player.relics[me.idx as usize]
    }

    /// `Owner.PlayerCombatState.TurnNumber`.
    #[inline(always)]
    pub fn turn_number(&self) -> i32 {
        self.player.turn_number
    }

    // ---- player resources ----------------------------------------------------------------------------------------

    /// `PlayerCmd.LoseEnergy`: no-op when `amount <= 0` or the combat is ending; clamps at 0.
    pub fn lose_energy(&mut self, n: i32) {
        if n <= 0 || self.is_ending() {
            return;
        }
        self.player.energy = (self.player.energy - n).max(0);
    }

    /// `PlayerCmd.GainStars` (`ShouldGainStars` has no overriding model): +stars, then `AfterStarsGained`.
    pub fn gain_stars(&mut self, n: i32) {
        if self.is_ending() {
            return;
        }
        self.player.stars = (self.player.stars + n).max(0);
        self.dispatch_g(hookbit::after_stars_gained, |cx, me, l| l.after_stars_gained(cx, me, n));
    }

    /// `CardModel.GetStarCostWithModifiers` for a non-X star cost: canonical + upgrades, through `TryModifyStarCost`
    /// (single pass, skipped when the cost is < 0 or the card is outside the combat piles). `-1` = no star cost.
    pub fn card_star_cost(&self, c: CardIdx) -> i32 {
        let d = self.card_def(c);
        if d.star_cost < 0 {
            return -1;
        }
        let cost = d.star_cost as i32 + d.up_star_cost as i32 * self.cards[c as usize].upgrade as i32;
        if cost < 0 || !self.card_in_combat_pile(c) || !self.hooks_enabled() {
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

    /// `CardModel.ResolveEnergyXValue` after the captured value: `Hook.ModifyXValue` (ChemicalX).
    pub fn resolve_x_value(&self, c: CardIdx) -> i32 {
        let mut v = self.cards[c as usize].x_value as i32;
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_x_value));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = content::listener(&e.me).modify_x_value(self, e.me, c, v);
                }
            }
        }
        v
    }

    // ---- creatures ---------------------------------------------------------------------------------------------------

    /// `Hook.AfterCreatureAddedToCombat` for a creature that joined mid-combat (summons). Call it after the creature has
    /// been added and rolled its first move (`CreatureCmd.Add`).
    pub fn notify_creature_added(&mut self, c: Cid) {
        self.dispatch_u(hookbit::after_creature_added_to_combat, |cx, me, l| l.after_creature_added_to_combat(cx, me, c));
    }

    /// Alive enemies in list order (`GetOpponentsOf(player)` filtered by `IsAlive`).
    pub fn alive_enemies(&self) -> ArrayVec<Cid, MAX_CREATURES> {
        let mut o = ArrayVec::new();
        for &e in self.enemies.iter() {
            if self.cr(e).is_alive() {
                o.push(e);
            }
        }
        o
    }

    /// `CreatureCmd.Damage(HittableEnemies, amount, props, PLAYER)` — the unpowered relic damage (`DamageVar(n, Unpowered)`).
    pub fn damage_hittable_enemies(&mut self, amount: i32, props: ValueProp) {
        let targets = self.hittable_enemies();
        if !targets.is_empty() {
            self.damage(targets.as_slice(), Dec::int(amount as i64), props, PLAYER, NO);
        }
    }

    /// `Rng.CombatTargets.NextItem(HittableEnemies)` then `CreatureCmd.Damage(that enemy)`: one draw when any enemy is
    /// hittable.
    pub fn damage_random_hittable_enemy(&mut self, amount: i32, props: ValueProp) {
        let targets = self.hittable_enemies();
        if targets.is_empty() {
            return;
        }
        let k = self.rng.combat_targets.next_int_range(0, targets.len() as i32) as usize;
        let t = targets[k];
        self.damage(&[t], Dec::int(amount as i64), props, PLAYER, NO);
    }

    // ---- potions -----------------------------------------------------------------------------------------------------

    /// `Owner.Potions.Any()`.
    pub fn has_potions(&self) -> bool {
        self.player.potions.iter().any(|p| p.is_some())
    }

    /// `Player.HasOpenPotionSlots`.
    pub fn has_open_potion_slot(&self) -> bool {
        (0..self.player.potion_slots as usize).any(|i| self.player.potions[i].is_none())
    }

    /// `PotionCmd.TryToProcure`: `Hook.ShouldProcurePotion` (AND, run-level), then the first empty slot, then
    /// `AfterPotionProcured`. Returns whether the potion was obtained.
    pub fn procure_potion(&mut self, id: u16) -> bool {
        let snap = self.snapshot(Mask::bit(hookbit::should_procure_potion));
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_procure_potion(self, e.me, id) {
                return false;
            }
        }
        let Some(slot) = (0..self.player.potion_slots as usize).find(|&i| self.player.potions[i].is_none()) else {
            return false;
        };
        if !content::potion_implemented(id) {
            self.flag_missing(Kind::Potion, id);
        }
        self.player.potions[slot] = Some(Potion { id });
        self.listen |= content::potion_mask(id);
        self.dispatch_u(hookbit::after_potion_procured, |cx, me, l| l.after_potion_procured(cx, me, id));
        true
    }

    // ---- cards -------------------------------------------------------------------------------------------------------

    /// `CardCmd.ApplyKeyword(card, kw)`: local keyword added (no-op if the card already has it).
    pub fn apply_keyword(&mut self, c: CardIdx, k: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_add |= k;
        card.kw_remove &= !k;
    }

    /// `Owner.PlayerCombatState.AllCards`: hand, draw, discard, exhaust, play pile (piles in `AllPiles` order is
    /// Hand, Draw, Discard, Exhaust, Play).
    pub fn all_cards(&self) -> ArrayVec<CardIdx, MAX_CARDS> {
        let mut o = ArrayVec::new();
        let p = &self.player;
        for pile in [&p.hand, &p.draw, &p.discard, &p.exhaust, &p.play] {
            for &c in pile.iter() {
                o.push(c);
            }
        }
        o
    }
}
