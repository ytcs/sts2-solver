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

    /// `Owner.GetRelic<T>() != null` (e.g. `VulnerablePower` checks the dealer's PaperPhrog, `WeakPower` the target's PaperKrane).
    pub fn has_relic(&self, id: u16) -> bool {
        self.player.relics.iter().any(|r| r.id == id)
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

    /// `PlayerCmd.GainGold`: `Hook.ModifyGoldGained` (threaded over the run-level listeners), then `Gold += (int)amount`
    /// when positive.
    pub fn gain_gold(&mut self, amount: i32) {
        let mut v = Dec::int(amount as i64);
        let snap = self.snapshot(Mask::bit(hookbit::modify_gold_gained));
        for e in snap.iter() {
            if self.still_live(&e.me) {
                v = content::listener(&e.me).modify_gold_gained(self, e.me, v);
            }
        }
        if v > Dec::ZERO {
            self.gold += v.trunc();
        }
    }

    /// `PlayerCmd.LoseGold`: floors at 0.
    pub fn lose_gold(&mut self, amount: i32) {
        self.gold = (self.gold - amount).max(0);
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

    /// `CreatureCmd.LoseBlock(c, amount, remover)`: lowers block (floor 0); `AfterBlockBroken` when it reaches 0.
    pub fn lose_block(&mut self, c: Cid, amount: i32) {
        if self.is_over_or_ending() || self.cr(c).is_dead() || amount <= 0 {
            return;
        }
        let before = self.cr(c).block;
        self.cr_mut(c).block = (before - amount).max(0);
        if before > 0 && self.cr(c).block <= 0 {
            self.dispatch_u(hookbit::after_block_broken, |cx, me, l| l.after_block_broken(cx, me, c, NO));
        }
    }

    /// `dealer == Owner.Creature || dealer == Owner.Osty`.
    pub fn is_owner_or_osty(&self, dealer: Cid) -> bool {
        if dealer == PLAYER {
            return true;
        }
        dealer != NO && {
            let d = self.cr(dealer);
            d.is_pet && d.owner == PLAYER && d.monster.id == crate::ids::monster::OSTY
        }
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

    /// `CardModel.CostsEnergyOrStars(includeGlobalModifiers: true)`.
    pub fn costs_energy_or_stars(&self, c: CardIdx) -> bool {
        let d = self.card_def(c);
        if !d.x_cost && self.card_cost(c, true) > 0 {
            return true;
        }
        self.card_star_cost(c) > 0
    }

    /// `List.StableShuffle(rng.CombatCardSelection)`: sort by (id, upgrade) like `CardModel.CompareTo`, then an
    /// `UnstableShuffle` (n-1 draws). Returns the shuffled list.
    pub fn stable_shuffle_selection(&mut self, list: &mut [CardIdx]) {
        let cards = &self.cards;
        crate::sort::intro_sort(list, |a, b| {
            let (ca, cb) = (&cards[*a as usize], &cards[*b as usize]);
            if ca.id != cb.id {
                return if ca.id < cb.id { -1 } else { 1 };
            }
            (ca.upgrade as i32 - cb.upgrade as i32).signum()
        });
        self.rng.combat_card_selection.shuffle(list);
    }

    /// `Rng.CombatCardSelection.NextItem(items)`: one draw when non-empty, none otherwise.
    pub fn select_item(&mut self, items: &[CardIdx]) -> Option<CardIdx> {
        if items.is_empty() {
            return None;
        }
        let i = self.rng.combat_card_selection.next_int_range(0, items.len() as i32) as usize;
        Some(items[i])
    }

    /// `EnergyCost.SetThisCombat(cost)`: an absolute cost modifier that lasts the whole combat. It overrides every earlier
    /// modifier, so those are dropped (keeps the modifier list within its fixed capacity).
    pub fn set_cost_this_combat(&mut self, c: CardIdx, cost: i32) {
        let canonical = self.card_def(c).cost;
        if cost == 0 && canonical < 0 {
            return;
        }
        let card = &mut self.cards[c as usize];
        card.mods.clear();
        card.mods.push(CostMod { amount: cost as i8, relative: false, reduce_only: false, expire: 0 });
    }

    /// `EnergyCost.AddUntilPlayed(amount)`: relative modifier until the card is played.
    pub fn add_cost_until_played(&mut self, c: CardIdx, amount: i32) {
        if amount != 0 {
            self.cards[c as usize].mods.push(CostMod { amount: amount as i8, relative: true, reduce_only: false, expire: EXPIRE_WHEN_PLAYED });
        }
    }

    /// `CardModel.CreateDupe`: a clone flagged `IsDupe` (removed from combat after its play) without the Exhaust keyword.
    pub fn create_dupe(&mut self, c: CardIdx) -> Option<CardIdx> {
        let d = self.clone_card(c)?;
        let card = &mut self.cards[d as usize];
        card.flags |= cflag::IS_DUPE;
        card.kw_remove |= kw::EXHAUST;
        card.kw_add &= !kw::EXHAUST;
        Some(d)
    }

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
