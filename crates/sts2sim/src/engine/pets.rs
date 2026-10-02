//! Pets (Osty): creation, summon / revive, max-HP primitives (spec 05 §6, `OstyCmd`, `PlayerCmd.AddPet`).
//!
//! Osty is an ally-side `Creature` (`is_pet`, `owner = PLAYER`) whose monster model is `Osty`. It is never removed from
//! the combat when it dies (`DieForYouPower.ShouldCreatureBeRemovedFromCombatAfterDeath`), so a dead Osty stays in
//! `allies` and is revived in place by the next `summon`.

use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// `Player.Osty`: the player's Osty creature, alive or dead (`None` if never summoned this combat).
    #[inline]
    pub fn osty(&self) -> Option<Cid> {
        self.allies
            .iter()
            .copied()
            .find(|&c| self.cr(c).is_pet && self.cr(c).owner == PLAYER && self.cr(c).monster.id == ids::monster::OSTY)
    }

    /// `Player.IsOstyAlive`.
    #[inline]
    pub fn is_osty_alive(&self) -> bool {
        self.osty().map_or(false, |o| self.cr(o).is_alive())
    }

    /// `Player.IsOstyMissing`.
    #[inline]
    pub fn is_osty_missing(&self) -> bool {
        !self.is_osty_alive()
    }

    /// Living Osty or `NO`.
    #[inline]
    pub fn living_osty(&self) -> Cid {
        match self.osty() {
            Some(o) if self.cr(o).is_alive() => o,
            _ => NO,
        }
    }

    /// `PlayerCmd.AddPet<Osty>`: creates the pet creature (HP 1/1, no `niche` draw: only enemies roll HP) on the player side.
    fn add_osty_creature(&mut self) -> Option<Cid> {
        let cid = (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).in_combat)?;
        self.listen |= content::monster_mask(ids::monster::OSTY);
        let mut cr = Creature::default();
        cr.active = true;
        cr.in_combat = true;
        cr.side = Side::Player;
        cr.is_pet = true;
        cr.owner = PLAYER;
        cr.hp = 1;
        cr.max_hp = 1;
        cr.monster = MonsterState { id: ids::monster::OSTY, ..Default::default() };
        self.creatures[cid as usize] = cr;
        self.allies.push(cid);
        Some(cid)
    }

    /// `OstyCmd.Summon` (spec 05 §6.2): `Hook.ModifySummonAmount` (threaded), then summon / grow Osty.
    pub fn summon(&mut self, amount: i32) {
        let mut amt = Dec::int(amount as i64);
        if self.listen.has(hookbit::modify_summon_amount) && self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_summon_amount));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    amt = content::listener(&e.me).modify_summon_amount(self, e.me, amt);
                }
            }
        }
        let amount = amt.trunc();
        if amount == 0 {
            return;
        }
        if self.is_osty_alive() {
            let o = self.osty().unwrap();
            self.gain_max_hp(o, Dec::int(amount as i64));
        } else {
            let existing = self.osty();
            let reviving = existing.is_some();
            let osty = match existing {
                Some(o) => o,
                None => {
                    let Some(o) = self.add_osty_creature() else { return };
                    self.apply_power(ids::power::DIE_FOR_YOU_POWER, o, Dec::ONE, NO, NO);
                    o
                }
            };
            self.set_max_hp(osty, Dec::int(amount as i64));
            self.heal(osty, Dec::int(amount as i64));
            if reviving {
                self.dispatch_g(hookbit::after_osty_revived, |cx, me, l| l.after_osty_revived(cx, me, osty));
            }
        }
        self.dispatch_g(hookbit::after_summon, |cx, me, l| l.after_summon(cx, me, Dec::int(amount as i64)));
    }
}
