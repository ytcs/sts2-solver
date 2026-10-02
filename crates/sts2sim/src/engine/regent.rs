//! Regent subsystems: Forge / Sovereign Blade (`ForgeCmd`) and card-creation helpers (spec 05 §7-8). Stars and star costs
//! live in `energy.rs`; auto-play, transform and the history in their own engine-core modules.

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

impl Combat {
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
        self.dispatch_g(hookbit::after_forge, |cx, me, l| l.after_forge(cx, me, Dec::int(amount as i64)));
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
}
