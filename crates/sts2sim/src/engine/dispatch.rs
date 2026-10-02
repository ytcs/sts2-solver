//! Hook listener enumeration with the game's exact semantics (spec 02 §1): snapshot at the start of each pass,
//! liveness re-checked when each item is reached, "guarded" passes silent once combat is ending.

use crate::content;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

#[derive(Clone, Copy, Default)]
pub struct Entry {
    pub me: Me,
    pub mask: Mask,
}

pub type Snapshot = ArrayVec<Entry, 96>;

impl Combat {
    /// `L_combat` restricted to listeners whose hook mask intersects `m`, in the game's order (spec 02 §1.1).
    pub fn snapshot(&self, m: Mask) -> Snapshot {
        let mut s = Snapshot::new();
        if !self.listen.intersects(m) {
            return s;
        }
        for &ci in self.allies.iter().chain(self.enemies.iter()) {
            let cr = &self.creatures[ci as usize];
            for p in cr.powers.iter() {
                let mask = content::power_mask(p.id);
                if mask.intersects(m) {
                    s.push(Entry { me: Me { kind: Kind::Power, owner: ci, idx: p.uid, id: p.id, amount: p.amount }, mask });
                }
            }
            if !cr.is_player {
                let mask = content::monster_mask(cr.monster.id);
                if mask.intersects(m) {
                    s.push(Entry { me: Me { kind: Kind::Monster, owner: ci, idx: 0, id: cr.monster.id, amount: 0 }, mask });
                }
            } else if cr.is_alive() {
                let pl = &self.player;
                for (i, r) in pl.relics.iter().enumerate() {
                    let mask = content::relic_mask(r.id);
                    if mask.intersects(m) {
                        s.push(Entry { me: Me { kind: Kind::Relic, owner: ci, idx: i as u16, id: r.id, amount: r.counter }, mask });
                    }
                }
                for (i, p) in pl.potions.iter().enumerate() {
                    if let Some(p) = p {
                        let mask = content::potion_mask(p.id);
                        if mask.intersects(m) {
                            s.push(Entry { me: Me { kind: Kind::Potion, owner: ci, idx: i as u16, id: p.id, amount: 0 }, mask });
                        }
                    }
                }
                for pile in [&pl.hand, &pl.draw, &pl.discard, &pl.exhaust, &pl.play] {
                    for &c in pile.iter() {
                        let card = &self.cards[c as usize];
                        let mask = content::card_mask(card.id);
                        if mask.intersects(m) {
                            s.push(Entry { me: Me { kind: Kind::Card, owner: ci, idx: c as u16, id: card.id, amount: 0 }, mask });
                        }
                    }
                }
            }
        }
        s
    }

    /// `CombatState.Contains(item)` evaluated when the item is reached.
    #[inline]
    pub fn still_live(&self, me: &Me) -> bool {
        match me.kind {
            Kind::Power | Kind::Monster => self.creatures[me.owner as usize].in_combat,
            Kind::Relic | Kind::Potion | Kind::Orb => self.creatures[PLAYER as usize].is_alive(),
            Kind::Card | Kind::Enchantment | Kind::Affliction => {
                self.creatures[PLAYER as usize].is_alive() && self.cards[me.idx as usize].flags & cflag::REMOVED == 0
            }
        }
    }

    /// `CombatManager.IsEnding` — a live predicate, not a flag (spec 02 §1.2).
    pub fn is_ending(&self) -> bool {
        if !self.in_progress {
            return false;
        }
        if self.pending_loss {
            return true;
        }
        for &e in self.enemies.iter() {
            if self.creatures[e as usize].is_alive() && self.is_primary_enemy(e) {
                return false;
            }
        }
        !self.should_stop_combat_from_ending()
    }

    #[inline]
    pub fn is_over_or_ending(&self) -> bool {
        !self.in_progress || self.is_ending()
    }

    /// `Hook.IterateCombatHookListeners` guard: yields nothing once combat is over/ending (unless starting).
    #[inline]
    pub fn hooks_enabled(&self) -> bool {
        self.is_starting || !self.is_over_or_ending()
    }

    /// Notification pass over the guarded iterator.
    #[inline]
    pub fn dispatch_g(&mut self, bit: u32, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.listen.has(bit) || !self.hooks_enabled() {
            return;
        }
        self.dispatch_u(bit, &mut f);
    }

    /// Notification pass over the unguarded iterator (hooks that are part of the kill/death sequence).
    #[inline]
    pub fn dispatch_u(&mut self, bit: u32, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.listen.has(bit) {
            return;
        }
        let snap = self.snapshot(Mask::bit(bit));
        for e in snap.iter() {
            if self.still_live(&e.me) {
                f(self, e.me, content::listener(&e.me));
            }
        }
    }

    pub fn is_primary_enemy(&self, c: Cid) -> bool {
        let cr = &self.creatures[c as usize];
        cr.side == Side::Enemy && !cr.powers.iter().any(|p| content::power_def(p.id).secondary_enemy)
    }

    /// OR over `ShouldStopCombatFromEnding` (Adaptable, Infested, SteamEruption, Stock, Surprise).
    pub fn should_stop_combat_from_ending(&self) -> bool {
        // Dispatched directly (unguarded): it decides whether combat ends.
        if !self.listen.has(hookbit::should_stop_combat_from_ending) {
            return false;
        }
        let snap = self.snapshot(Mask::bit(hookbit::should_stop_combat_from_ending));
        for e in snap.iter() {
            if self.still_live(&e.me) && content::listener(&e.me).should_stop_combat_from_ending(self, e.me) {
                return true;
            }
        }
        false
    }
}
