//! Necrobinder engine helpers: Doom, Osty attacks, small card/keyword utilities (`DoomPower.DoomKill`, spec 05 §9).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// Powers implementing `ITemporaryPower` (the `Temporary{Strength,Dexterity,Focus}Power` families).
pub fn is_temporary_power(id: u16) -> bool {
    use ids::power::*;
    matches!(
        id,
        FADE_POWER
            | MANGLE_POWER
            | HELICAL_DART_POWER
            | ENFEEBLING_TOUCH_POWER
            | FLEX_POTION_POWER
            | COORDINATE_POWER
            | DARK_SHACKLES_POWER
            | SPEED_POTION_POWER
            | HOTFIX_POWER
            | SYNCHRONIZE_POWER
            | FEEDING_FRENZY_POWER
            | MONARCHS_GAZE_STRENGTH_DOWN_POWER
            | DYING_STAR_POWER
            | PIERCING_WAIL_POWER
            | REPTILE_TRINKET_POWER
            | HYPERBEAM_FOCUS_DOWN_POWER
            | ANTICIPATE_POWER
            | CRUSH_UNDER_POWER
            | FOCUSED_STRIKE_POWER
            | SHACKLING_POTION_POWER
            | SETUP_STRIKE_POWER
    )
}

/// `ITemporaryPower.InternallyAppliedPower` of a temporary power (`None` if `id` is not one).
pub fn temporary_inner_power(id: u16) -> Option<u16> {
    use ids::power::*;
    if !is_temporary_power(id) {
        return None;
    }
    Some(match id {
        ANTICIPATE_POWER | FADE_POWER | HELICAL_DART_POWER | SPEED_POTION_POWER => DEXTERITY_POWER,
        HOTFIX_POWER | HYPERBEAM_FOCUS_DOWN_POWER | FOCUSED_STRIKE_POWER | SYNCHRONIZE_POWER => FOCUS_POWER,
        _ => STRENGTH_POWER,
    })
}

impl Combat {
    /// `CreatureAttackedEntry`s of this turn whose actor is Osty (the pet): Flatten / Fetch / Rattle.
    pub fn osty_attacks_this_turn(&self) -> usize {
        self.hist_count_this_turn(crate::engine::HKind::CreatureAttacked, |e| e.actor != NO && self.cr(e.actor).is_pet)
    }

    /// `Rng.CombatTargets.NextItem(HittableEnemies)`: one draw (even for a single enemy); none when nobody is hittable.
    pub fn random_hittable_enemy(&mut self) -> Option<Cid> {
        let h = self.hittable_enemies();
        if h.is_empty() {
            return None;
        }
        let i = self.rng.combat_targets.next_int_range(0, h.len() as i32) as usize;
        Some(h[i])
    }

    /// `DoomPower.GetDoomedCreatures(side)`: creatures on the side (list order) whose Doom amount reaches their HP.
    pub fn doomed_on_side(&self, side: Side) -> ArrayVec<Cid, MAX_CREATURES> {
        let mut out = ArrayVec::new();
        for &c in self.creatures_on(side).iter() {
            if self.is_doomed(c) {
                out.push(c);
            }
        }
        out
    }

    /// `DoomPower.IsOwnerDoomed`: `Owner.CurrentHp <= Amount` (false when the creature has no Doom).
    pub fn is_doomed(&self, c: Cid) -> bool {
        match self.cr(c).power(ids::power::DOOM_POWER) {
            Some(p) => self.cr(c).hp <= p.amount,
            None => false,
        }
    }

    /// `DoomPower.DoomKill`: kill each creature (normal `Kill`, one call per creature), then `AfterDiedToDoom`.
    pub fn doom_kill(&mut self, creatures: &[Cid]) {
        if creatures.is_empty() {
            return;
        }
        let mut list = [0u8; MAX_CREATURES];
        let n = creatures.len().min(MAX_CREATURES);
        list[..n].copy_from_slice(&creatures[..n]);
        for &c in &list[..n] {
            self.kill(&[c]);
        }
        let l = &list[..n];
        self.dispatch_u(hookbit::after_died_to_doom, |cx, me, lis| lis.after_died_to_doom(cx, me, l));
    }

    /// `CardCmd.ApplyKeyword`: adds a local keyword to a card instance.
    pub fn apply_keyword(&mut self, c: CardIdx, kw: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_add |= kw;
        card.kw_remove &= !kw;
    }

    /// A fresh Soul token (optionally upgraded) in no pile.
    pub fn new_soul(&mut self, upgraded: bool) -> Option<CardIdx> {
        let c = self.new_card(ids::card::SOUL, 0)?;
        if upgraded {
            self.upgrade_in_combat(c);
        }
        Some(c)
    }

    /// `Soul.Create(owner, n)` added to the draw pile at random positions (`AddGeneratedCardsToCombat(.., Draw, .., Random)`).
    pub fn add_souls_to_draw_pile(&mut self, n: i32, upgraded: bool) {
        for _ in 0..n.max(0) {
            if let Some(c) = self.new_soul(upgraded) {
                self.add_generated_card(c, PileType::Draw, CardPilePosition::Random);
            }
        }
    }

    /// Dec helper: `Dec::int` of an i32.
    #[inline]
    pub fn dint(v: i32) -> Dec {
        Dec::int(v as i64)
    }

    /// `content::power_listener` convenience used by cards that inspect a power's private state.
    #[inline]
    pub fn power_uid(&self, c: Cid, id: u16) -> Option<u16> {
        self.cr(c).power(id).map(|p| p.uid)
    }

    /// Whether the card is the given card id.
    #[inline]
    pub fn is_card(&self, c: CardIdx, id: u16) -> bool {
        c != NO && self.cards[c as usize].id == id
    }
}
