//! Death / kill sequence (spec 02 §5.3, spec 01 §13.4): `CreatureCmd.Kill`, death preventers (Fairy in a Bottle, Lizard
//! Tail), removal rules, minions, player death, escape.

use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    /// `CombatManager.LoseCombat`: marks the combat as pending loss; processed at the next `CheckWinCondition`.
    pub fn lose_combat(&mut self) {
        if self.in_progress && !self.pending_loss {
            self.pending_loss = true;
        }
    }

    /// `CreatureCmd.Kill(creatures)`.
    pub fn kill(&mut self, victims: &[Cid]) {
        self.kill_ex(victims, false);
    }

    /// `CreatureCmd.Kill(creatures, force)`. `force` bypasses death preventers (Fairy in a Bottle ...).
    pub fn kill_ex(&mut self, victims: &[Cid], force: bool) {
        if victims.is_empty() {
            return;
        }
        let mut list: ArrayVec<Cid, MAX_CREATURES> = ArrayVec::new();
        for &v in victims {
            list.push(v);
        }
        for &v in list.iter() {
            self.kill_without_check(v, force, 0);
        }
        // Single player: every player dead => the combat is lost (applied at the next `CheckWinCondition`).
        if self.cr(PLAYER).is_dead() && self.in_progress {
            self.lose_combat();
        }
    }

    /// `KillWithoutCheckingWinCondition`.
    fn kill_without_check(&mut self, c: Cid, force: bool, recursion: u8) {
        if !self.cr(c).in_combat && c != PLAYER {
            return;
        }
        let hp = self.cr(c).hp;
        if hp > 0 {
            self.lose_hp_internal(c, Dec::int(hp as i64));
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, -hp));
        }
        self.dispatch_u(hookbit::before_death, |cx, me, l| l.before_death(cx, me, c));
        let mut preventer: Option<Me> = None;
        let dies = force || self.cr(c).max_hp <= 0 || {
            preventer = self.find_death_preventer(c);
            preventer.is_none()
        };
        if dies {
            let should_remove = self.cr(c).in_combat && self.should_creature_be_removed_after_death(c);
            self.dispatch_u(hookbit::after_death, |cx, me, l| l.after_death(cx, me, c, false));
            let side = self.cr(c).side;
            // teammates (alive, same side) evaluated after AfterDeath
            let mut teammates: ArrayVec<Cid, MAX_CREATURES> = ArrayVec::new();
            let pool: ArrayVec<Cid, MAX_CREATURES> = {
                let mut p = ArrayVec::new();
                let src = if side == Side::Player { self.allies.as_slice() } else { self.enemies.as_slice() };
                for &t in src {
                    p.push(t);
                }
                p
            };
            for &t in pool.iter() {
                if t != c && self.cr(t).is_alive() {
                    teammates.push(t);
                }
            }
            if should_remove && side == Side::Enemy && self.enemies.contains(c) {
                self.remove_creature_from_combat(c);
            }
            let is_primary = self.is_primary_enemy(c);
            // RemoveAllPowersAfterDeath + AfterRemoved on each removed power.
            let removed = self.remove_all_powers_after_death(c);
            for p in removed.iter() {
                let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
                content::listener(&me).after_removed(self, me, c);
            }
            if side == Side::Enemy {
                if is_primary && !teammates.is_empty() && teammates.iter().all(|&t| !self.is_primary_enemy(t)) {
                    self.kill(teammates.as_slice());
                }
            } else if c == PLAYER {
                // OrbQueue.Clear() (orbs gone, capacity zeroed); `if (player.IsOstyAlive) Kill(Osty)`; DeactivateHooks;
                // HandlePlayerDeath (single player: nothing — the combat is lost by the caller).
                self.player.orbs.clear();
                self.player.orb_slots = 0;
                if let Some(o) = self.osty() {
                    if self.cr(o).is_alive() {
                        self.kill(&[o]);
                    }
                }
                self.player_hooks_active = false;
            }
        } else {
            assert!(recursion < 10, "Combat is ending, but something is continually preventing the last creature from being killed!");
            self.dispatch_u(hookbit::after_death, |cx, me, l| l.after_death(cx, me, c, true));
            if let Some(p) = preventer {
                // Hook.AfterPreventingDeath: only if the preventer is still a listener.
                if self.still_live(&p) {
                    content::listener(&p).after_preventing_death(self, p, c);
                }
            }
            if self.cr(c).is_dead() {
                self.kill_without_check(c, force, recursion + 1);
            }
        }
    }

    /// `Hook.ShouldDie`: pass 1 `ShouldDie`, pass 2 `ShouldDieLate` (AND; the first `false` is the preventer).
    fn find_death_preventer(&self, c: Cid) -> Option<Me> {
        if !self.listen.has(hookbit::should_die) && !self.listen.has(hookbit::should_die_late) {
            return None;
        }
        for bit in [hookbit::should_die, hookbit::should_die_late] {
            if !self.listen.has(bit) {
                continue;
            }
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(bit), &mut snap);
            for e in snap.iter() {
                if !self.still_live(&e.me) {
                    continue;
                }
                let l = content::listener(&e.me);
                let ok = if bit == hookbit::should_die { l.should_die(self, e.me, c) } else { l.should_die_late(self, e.me, c) };
                if !ok {
                    return Some(e.me);
                }
            }
        }
        None
    }

    /// `Hook.ShouldCreatureBeRemovedFromCombatAfterDeath` (unguarded, AND).
    pub fn should_creature_be_removed_after_death(&self, c: Cid) -> bool {
        if !self.listen.has(hookbit::should_creature_be_removed_from_combat_after_death) {
            return true;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::should_creature_be_removed_from_combat_after_death), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_creature_be_removed_from_combat_after_death(self, e.me, c) {
                return false;
            }
        }
        true
    }

    /// `Hook.ShouldPowerBeRemovedOnDeath` (unguarded, AND).
    fn should_power_be_removed_on_death(&self, owner: Cid, power_id: u16) -> bool {
        if !self.listen.has(hookbit::should_power_be_removed_on_death) {
            return true;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::should_power_be_removed_on_death), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_power_be_removed_on_death(self, e.me, owner, power_id) {
                return false;
            }
        }
        true
    }

    /// `Creature.RemoveAllPowersAfterDeath`: strips every power except those that veto
    /// (`!ShouldPowerBeRemovedAfterOwnerDeath() || !Hook.ShouldPowerBeRemovedOnDeath(p)`); returns the removed ones.
    fn remove_all_powers_after_death(&mut self, c: Cid) -> ArrayVec<Power, MAX_POWERS> {
        let mut removed: ArrayVec<Power, MAX_POWERS> = ArrayVec::new();
        let mut kept: ArrayVec<Power, MAX_POWERS> = ArrayVec::new();
        let all = self.cr(c).powers;
        for p in all.iter() {
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            let keep = !content::listener(&me).should_power_be_removed_after_owner_death(self, me) || !self.should_power_be_removed_on_death(c, p.id);
            if keep {
                kept.push(*p);
            } else {
                removed.push(*p);
            }
        }
        self.cr_mut(c).powers = kept;
        self.sync_secondary(c);
        removed
    }

    /// `CombatManager.RemoveCreature` + `CombatState.RemoveCreature` for a dead / escaped enemy: the monster leaves
    /// the enemy list and stops listening. A monster that is mid-move is only detached when its move finishes
    /// (`perform_move`).
    pub fn remove_creature_from_combat(&mut self, c: Cid) {
        if !self.cr(c).is_player && self.cr(c).side == Side::Enemy {
            let me = Me { kind: Kind::Monster, owner: c, idx: 0, id: self.cr(c).monster.id, amount: 0 };
            content::listener(&me).before_removed_from_room(self, me);
        }
        if self.cr(c).monster.is_performing {
            return;
        }
        self.detach_creature(c);
    }

    /// `Hook.AfterDiedToDoom(creatures)` (unguarded): called by the Doom power after its `Kill`.
    pub fn after_died_to_doom(&mut self, creatures: &[Cid]) {
        if !self.listen.has(hookbit::after_died_to_doom) {
            return;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::after_died_to_doom), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) {
                content::listener(&e.me).after_died_to_doom(self, e.me, creatures);
            }
        }
    }

    /// Whether every power of `c` answers `ShouldOwnerDeathTriggerFatal() == true` (Feed / Hand of Greed / The Hunt ask
    /// this about the creature they killed; Minion and Reattach answer false).
    pub fn all_powers_trigger_fatal(&self, c: Cid) -> bool {
        self.cr(c).powers.iter().all(|p| {
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            content::listener(&me).should_owner_death_trigger_fatal(self, me)
        })
    }

    /// `Hook.ShouldAllowTargeting` (guarded AND). UI-only in the game (no shipped model overrides it).
    pub fn should_allow_targeting(&self, c: Cid) -> bool {
        self.first_veto_g(hookbit::should_allow_targeting, |cx, me, l| l.should_allow_targeting(cx, me, c)).is_none()
    }

    /// `CreatureCmd.Escape`: the creature leaves the combat without dying (powers stripped silently).
    pub fn escape(&mut self, c: Cid) {
        if self.cr(c).is_dead() || !self.cr(c).in_combat || !self.in_progress {
            return;
        }
        self.cr_mut(c).powers.clear();
        self.sync_secondary(c);
        // CombatManager.RemoveCreature (BeforeRemovedFromRoom) then CombatState.CreatureEscaped.
        let me = Me { kind: Kind::Monster, owner: c, idx: 0, id: self.cr(c).monster.id, amount: 0 };
        content::listener(&me).before_removed_from_room(self, me);
        self.escaped += 1;
        self.detach_creature(c);
    }
}
