//! Helpers the relic implementations are written against (thin wrappers over `PlayerCmd` / `CreatureCmd` / `PotionCmd`
//! semantics that no card needed before).

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

    // ---- creatures ---------------------------------------------------------------------------------------------------

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
