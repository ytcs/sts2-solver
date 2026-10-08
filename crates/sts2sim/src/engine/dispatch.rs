//! Hook listener enumeration with the game's exact semantics (spec 02 §1): snapshot at the start of each pass,
//! liveness re-checked when each item is reached, "guarded" passes silent once combat is ending.

use crate::content;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// One listener of a snapshot. (It used to carry the listener's 32-byte hook mask; the few multi-bit passes query it
/// through [`Combat::has_hook`] instead, which keeps a snapshot at 12 bytes per listener.)
#[derive(Clone, Copy, Default)]
pub struct Entry {
    pub me: Me,
}

/// Worst case a hook can have this many listeners: every card instance of the arena with a listener plus its affliction /
/// enchantment, powers, relics, potions, monsters. Realistic fights stay far below; a fuller snapshot drops the surplus
/// listeners and raises the overflow flag (`ArrayVec`).
pub const SNAPSHOT_CAP: usize = 256;
pub type Snapshot = ArrayVec<Entry, SNAPSHOT_CAP>;

impl Combat {
    /// `L_combat` restricted to listeners whose hook mask intersects `m`, in the game's order (spec 02 §1.1).
    pub fn snapshot(&self, m: Mask) -> Snapshot {
        let mut s = Snapshot::new();
        self.snapshot_into(m, &mut s);
        s
    }

    /// [`Combat::snapshot`] into a caller-owned (empty) list. Hot paths use this: returning the 3 KB list by value makes the
    /// compiler copy it whole on every call, even when it is empty.
    #[inline(always)]
    pub fn snapshot_into(&self, m: Mask, s: &mut Snapshot) {
        // the "nobody listens" test inline at the call site (about half of the calls stop here), the scan out of line
        if self.listen.intersects(m) {
            self.snapshot_scan(m, s);
        }
    }

    #[inline(never)]
    fn snapshot_scan(&self, m: Mask, s: &mut Snapshot) {
        for &ci in self.allies.iter().chain(self.enemies.iter()) {
            let cr = &self.creatures[ci as usize];
            for p in cr.powers.iter() {
                let mask = content::power_mask(p.id);
                if mask.intersects(m) {
                    s.push(Entry { me: Me { kind: Kind::Power, owner: ci, idx: p.uid, id: p.id, amount: p.amount } });
                }
            }
            if !cr.is_player {
                let mask = content::monster_mask(cr.monster.id);
                if mask.intersects(m) {
                    s.push(Entry { me: Me { kind: Kind::Monster, owner: ci, idx: 0, id: cr.monster.id, amount: 0 } });
                }
            } else if self.player_hooks_active {
                let pl = &self.player;
                for (i, r) in pl.relics.iter().enumerate() {
                    let mask = content::relic_mask(r.id);
                    if mask.intersects(m) {
                        s.push(Entry { me: Me { kind: Kind::Relic, owner: ci, idx: i as u16, id: r.id, amount: r.counter } });
                    }
                }
                for (i, p) in pl.potions.iter().enumerate() {
                    if let Some(p) = p {
                        let mask = content::potion_mask(p.id);
                        if mask.intersects(m) {
                            s.push(Entry { me: Me { kind: Kind::Potion, owner: ci, idx: i as u16, id: p.id, amount: 0 } });
                        }
                    }
                }
                if !self.listen_cards.intersects(m) {
                    continue;
                }
                for pile in [&pl.hand, &pl.draw, &pl.discard, &pl.exhaust, &pl.play] {
                    for &c in pile.iter() {
                        let card = &self.cards[c as usize];
                        let mask = content::card_mask(card.id);
                        if mask.intersects(m) {
                            s.push(Entry { me: Me { kind: Kind::Card, owner: ci, idx: c as u16, id: card.id, amount: 0 } });
                        }
                        // card.Affliction (BEFORE the enchantment), then card.Enchantment (spec 02 §1.1)
                        if card.affliction != 0 {
                            let aid = (card.affliction - 1) as u16;
                            let mask = content::affliction_mask(aid);
                            if mask.intersects(m) {
                                s.push(Entry { me: Me { kind: Kind::Affliction, owner: ci, idx: c as u16, id: aid, amount: card.affliction_amount as i32 } });
                            }
                        }
                        if card.enchant != 0 {
                            let eid = (card.enchant - 1) as u16;
                            let mask = content::enchantment_mask(eid);
                            if mask.intersects(m) {
                                s.push(Entry { me: Me { kind: Kind::Enchantment, owner: ci, idx: c as u16, id: eid, amount: card.enchant_amount as i32 } });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether the listener `me` overrides hook `bit` (its static hook mask).
    #[inline]
    pub fn has_hook(&self, me: &Me, bit: u32) -> bool {
        let m = match me.kind {
            Kind::Power => content::power_mask(me.id),
            Kind::Monster => content::monster_mask(me.id),
            Kind::Relic => content::relic_mask(me.id),
            Kind::Potion => content::potion_mask(me.id),
            Kind::Card => content::card_mask(me.id),
            Kind::Affliction => content::affliction_mask(me.id),
            Kind::Enchantment => content::enchantment_mask(me.id),
            Kind::Orb => Mask::EMPTY,
        };
        m.has(bit)
    }

    /// `CombatState.Contains(item)` evaluated when the item is reached.
    #[inline]
    pub fn still_live(&self, me: &Me) -> bool {
        match me.kind {
            // PowerModel: owner in a combat and (owner not a player or the player is active); MonsterModel: attached.
            Kind::Power => {
                let o = &self.creatures[me.owner as usize];
                o.in_combat && (!o.is_player || self.player_hooks_active)
            }
            Kind::Monster => self.creatures[me.owner as usize].in_combat,
            Kind::Relic | Kind::Potion | Kind::Orb => self.player_hooks_active,
            Kind::Card | Kind::Enchantment | Kind::Affliction => {
                self.player_hooks_active && self.cards[me.idx as usize].flags & cflag::REMOVED == 0
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
    #[inline(always)]
    pub fn dispatch_g(&mut self, bit: u32, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.listen.has(bit) || !self.hooks_enabled() {
            return;
        }
        self.dispatch_slow(bit, &mut f);
    }

    /// Guarded notification pass whose listeners may raise a decision (`Stage::AwaitChoice`, `hook_ctx` set): the pass stops
    /// right after such a listener and returns true; after the decision the same call continues with the listeners that
    /// follow it (`susp_after`). Used for the turn-start hooks (`BeforeHandDraw`, `AfterPlayerTurnStart`).
    pub fn dispatch_resumable(&mut self, bit: u32, f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) -> bool {
        if !self.pass_enter() {
            return false; // runaway-work safeguard tripped (`engine/budget.rs`)
        }
        let r = self.dispatch_resumable_in(bit, f);
        self.pass_exit();
        r
    }

    fn dispatch_resumable_in(&mut self, bit: u32, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) -> bool {
        let resume = match self.susp.iter().position(|p| p.bit == bit) {
            Some(i) => Some(self.susp.remove(i)),
            None => None,
        };
        if !self.listen.has(bit) || !self.hooks_enabled() {
            return false;
        }
        if let Some(p) = resume.filter(|p| p.full) {
            // Continue with the listeners that were still to come when the pass suspended (the list built at its start).
            for (k, me) in p.rest.iter().enumerate() {
                if self.still_live(me) {
                    f(self, *me, content::listener(me));
                    if self.stage == Stage::AwaitChoice {
                        self.suspend_pass(bit, *me, 0, p.rest.as_slice()[k + 1..].iter().copied());
                        return true;
                    }
                }
            }
            return false;
        }
        let snap = self.snapshot(Mask::bit(bit));
        let mut start = 0;
        if let Some(p) = resume {
            // The suspended listener may have removed itself (a power): then the next one now sits at its old index.
            let last = p.me;
            start = match snap.iter().position(|e| e.me.kind == last.kind && e.me.idx == last.idx && e.me.owner == last.owner && e.me.id == last.id) {
                Some(i) => i + 1,
                None => (p.pos as usize).min(snap.len()),
            };
        }
        for (i, e) in snap.iter().enumerate().skip(start) {
            if self.still_live(&e.me) {
                f(self, e.me, content::listener(&e.me));
                if self.stage == Stage::AwaitChoice {
                    self.suspend_pass(bit, e.me, i as u8, snap.as_slice()[i + 1..].iter().map(|x| x.me));
                    return true;
                }
            }
        }
        false
    }

    fn suspend_pass(&mut self, bit: u32, me: Me, pos: u8, rest: impl Iterator<Item = Me>) {
        let mut p = SuspPass { bit, me, pos, full: true, rest: ArrayVec::new() };
        for m in rest {
            if p.rest.len() >= 8 {
                p.full = false;
                break;
            }
            p.rest.push(m);
        }
        if self.susp.len() >= 3 {
            self.susp.remove(0);
        }
        self.susp.push(p);
    }

    /// Notification pass over the unguarded iterator (hooks that are part of the kill/death sequence).
    #[inline(always)]
    pub fn dispatch_u(&mut self, bit: u32, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.listen.has(bit) {
            return;
        }
        self.dispatch_slow(bit, &mut f);
    }

    /// The part of a notification pass that runs only when some model listens. Out of line (and `dyn`) on purpose: the 3 KB
    /// snapshot lives in this frame instead of in every caller's, so the many "nobody listens" call sites stay cheap.
    #[inline(never)]
    fn dispatch_slow(&mut self, bit: u32, f: &mut dyn FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.pass_enter() {
            return; // runaway-work safeguard tripped (`engine/budget.rs`)
        }
        let mut snap = Snapshot::new();
        self.snapshot_into(Mask::bit(bit), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) {
                f(self, e.me, content::listener(&e.me));
            }
        }
        self.pass_exit();
    }

    pub fn is_primary_enemy(&self, c: Cid) -> bool {
        let cr = &self.creatures[c as usize];
        debug_assert_eq!(cr.secondary, cr.powers.iter().any(|p| content::power_def(p.id).secondary_enemy), "stale Creature::secondary");
        cr.side == Side::Enemy && !cr.secondary
    }

    /// OR over `ShouldStopCombatFromEnding` (Adaptable, Infested, SteamEruption, Stock, Surprise); unguarded.
    pub fn should_stop_combat_from_ending(&self) -> bool {
        if !self.listen.has(hookbit::should_stop_combat_from_ending) {
            return false;
        }
        self.any_true(hookbit::should_stop_combat_from_ending, |cx, me, l| l.should_stop_combat_from_ending(cx, me))
    }

    /// OR over a predicate hook on the unguarded iterator (`ShouldTakeExtraTurn` etc. use `any_true_g`).
    #[inline(always)]
    pub fn any_true(&self, bit: u32, f: impl Fn(&Combat, Me, &'static dyn Listener) -> bool) -> bool {
        if !self.listen.has(bit) {
            return false;
        }
        self.any_true_slow(bit, &f)
    }

    #[inline(never)]
    fn any_true_slow(&self, bit: u32, f: &dyn Fn(&Combat, Me, &'static dyn Listener) -> bool) -> bool {
        let mut snap = Snapshot::new();
        self.snapshot_into(Mask::bit(bit), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) && f(self, e.me, content::listener(&e.me)) {
                return true;
            }
        }
        false
    }

    /// OR over a predicate hook on the guarded iterator.
    pub fn any_true_g(&self, bit: u32, f: impl Fn(&Combat, Me, &'static dyn Listener) -> bool) -> bool {
        self.listen.has(bit) && self.hooks_enabled() && self.any_true(bit, f)
    }

    /// AND over a predicate hook (unguarded): the first model answering `false` — the "preventer" — is returned.
    #[inline(always)]
    pub fn first_veto(&self, bit: u32, f: impl Fn(&Combat, Me, &'static dyn Listener) -> bool) -> Option<Me> {
        if !self.listen.has(bit) {
            return None;
        }
        self.first_veto_slow(bit, &f)
    }

    #[inline(never)]
    fn first_veto_slow(&self, bit: u32, f: &dyn Fn(&Combat, Me, &'static dyn Listener) -> bool) -> Option<Me> {
        let mut snap = Snapshot::new();
        self.snapshot_into(Mask::bit(bit), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) && !f(self, e.me, content::listener(&e.me)) {
                return Some(e.me);
            }
        }
        None
    }

    /// AND over a predicate hook on the guarded iterator (every dispatch is a no-op, i.e. `None`, once combat is ending).
    pub fn first_veto_g(&self, bit: u32, f: impl Fn(&Combat, Me, &'static dyn Listener) -> bool) -> Option<Me> {
        if !self.listen.has(bit) || !self.hooks_enabled() {
            return None;
        }
        self.first_veto(bit, f)
    }

    /// `participants.Contains(creature)` of the side-turn hooks: on the player side only the player creature (pets are not
    /// participants of the turn-end hooks), on the enemy side every enemy.
    pub fn is_turn_participant(&self, side: Side, c: Cid) -> bool {
        match side {
            Side::Player => c == PLAYER,
            Side::Enemy => self.enemies.contains(c),
        }
    }

    /// Calls `f` on `me`'s listener if it is still a listener (`Hook.After*(…, modifier)` for a single model).
    pub fn notify_one(&mut self, me: Me, f: impl FnOnce(&mut Combat, Me, &'static dyn Listener)) {
        if self.still_live(&me) {
            f(self, me, content::listener(&me));
        }
    }
}
