//! Small card-effect helpers shared by the colorless / curse / status / token content.

use crate::dec::Dec;
use crate::defs::*;
use crate::sort::intro_sort;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// Which run-level RNG stream a helper draws from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stream {
    Shuffle,
    CardSelection,
    CardGeneration,
    Targets,
}

impl Combat {
    /// `ValueProp` flags of the card's first dynamic var of `kind` (`DamageVar.Props` / `BlockVar.Props`).
    pub fn card_var_props(&self, c: CardIdx, kind: VarKind) -> ValueProp {
        for v in crate::content::card_def(self.cards[c as usize].id).vars {
            if v.kind == kind {
                return ValueProp(v.props);
            }
        }
        ValueProp::NONE
    }

    /// Value of the card's generic named var (`DynamicVar("Name", v)`; `name` = `gen_cards::var_name::*`).
    pub fn named_var(&self, c: CardIdx, name: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in crate::content::card_def(card.id).vars {
            if v.kind == VarKind::Named && v.arg == name {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    /// `CreatureCmd.Damage(ctx, owner, card.DynamicVars.<Damage|HpLoss>, card, null)`: the player damages themselves
    /// with a card-sourced hit (dealer = card owner).
    pub fn self_damage_from_card(&mut self, c: CardIdx, amount: Dec, props: ValueProp) {
        self.damage(&[PLAYER], amount, props, PLAYER, c);
    }

    /// `PlayerCmd.LoseEnergy`: no-op once combat is ending; energy floors at 0.
    pub fn lose_energy(&mut self, n: i32) {
        if n <= 0 || self.is_ending() {
            return;
        }
        self.player.energy = (self.player.energy - n).max(0);
    }

    /// `PlayerCmd.GainGold` (`ModifyGoldGained` / `AfterGoldGained` run-level hooks: no content yet).
    pub fn gain_gold(&mut self, n: i32) {
        if n > 0 {
            self.gold += n;
        }
    }

    /// `PlayerCmd.LoseGold`: floors at 0.
    pub fn lose_gold(&mut self, n: i32) {
        self.gold = (self.gold - n).max(0);
    }

    /// `CardEnergyCost.AddThisCombat(amount)`: relative modifier that lasts the whole combat. Consecutive
    /// combat-long relative modifiers are folded into one (the cost chain is plain addition, so this is equivalent
    /// and keeps the fixed-capacity modifier list from overflowing).
    pub fn add_energy_cost_this_combat(&mut self, c: CardIdx, amount: i8) {
        if amount == 0 {
            return;
        }
        let card = &mut self.cards[c as usize];
        if let Some(last) = card.mods.as_mut_slice().last_mut() {
            if last.relative && !last.reduce_only && last.expire == 0 {
                last.amount = last.amount.saturating_add(amount);
                return;
            }
        }
        card.mods.push(CostMod { amount, relative: true, reduce_only: false, expire: 0 });
    }

    /// `Shiv.CreateInHand` / `combatState.CreateCard<T>` x n + `CardPileCmd.AddGeneratedCardsToCombat(.., Hand)`.
    pub fn create_cards_in_hand(&mut self, id: u16, count: i32) {
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

    /// `PowerModel.SetAmount`: sets the amount directly (no hooks, no removal at 0).
    pub fn set_power_amount(&mut self, c: Cid, uid: u16, amount: i32) {
        if let Some(i) = self.power_idx(c, uid) {
            self.cr_mut(c).powers[i].amount = amount;
        }
    }

    fn rng_stream(&mut self, s: Stream) -> &mut crate::rng::Rng {
        match s {
            Stream::Shuffle => &mut self.rng.shuffle,
            Stream::CardSelection => &mut self.rng.combat_card_selection,
            Stream::CardGeneration => &mut self.rng.combat_card_generation,
            Stream::Targets => &mut self.rng.combat_targets,
        }
    }

    /// `ListExtensions.UnstableShuffle(list, rng)` (Fisher-Yates, `n-1` draws) over card handles.
    pub fn unstable_shuffle_cards(&mut self, s: Stream, list: &mut [CardIdx]) {
        self.rng_stream(s).shuffle(list);
    }

    /// `ListExtensions.StableShuffle(list, rng)`: `List.Sort()` by (Id, upgrade level) with .NET's IntroSort, THEN the
    /// Fisher-Yates shuffle (spec 03 §7.3).
    pub fn stable_shuffle_cards(&mut self, s: Stream, list: &mut [CardIdx]) {
        {
            let cards = &self.cards;
            intro_sort(list, |a, b| {
                let (ca, cb) = (&cards[*a as usize], &cards[*b as usize]);
                if ca.id != cb.id {
                    return if ca.id < cb.id { -1 } else { 1 };
                }
                (ca.upgrade as i32 - cb.upgrade as i32).signum()
            });
        }
        self.rng_stream(s).shuffle(list);
    }

    /// `CardFactory.GetForCombat(player, pool.Where(extra), count, CombatCardGeneration)`: `count` draws WITH
    /// replacement (`NextItem`: one draw each) over the generatable cards of `pool`.
    pub fn get_for_combat_where(&mut self, pool: &[u16], count: usize, extra: impl Fn(&CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 256> = ArrayVec::new(); // (256: Splash concatenates four character pools)
        for &id in pool {
            let d = crate::content::card_def(id);
            if extra(d) && !d.multiplayer_only && d.can_be_generated_in_combat && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event) {
                list.push(id);
            }
        }
        let mut out = ArrayVec::new();
        if list.is_empty() {
            return out;
        }
        for _ in 0..count {
            let i = self.rng.combat_card_generation.next_int_range(0, list.len() as i32) as usize;
            if let Some(c) = self.new_card(list[i], 0) {
                out.push(c);
            }
        }
        out
    }
}
