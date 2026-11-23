// The Fool's arrows: `ArrowCmd`, `ImpaledPower`'s arrow list, `MelodyCmd`, `TarotCards`.
use crate::dec::Dec;
use crate::engine::{Attack, HKind};
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

/// `CharacterModel.BaseArrowSlotCount`.
pub const fn base_arrow_slots(character: u8) -> usize {
    if character == crate::types::character::FOOL { 1 } else { 0 }
}

/// `ImpaledPower` `HitCounter` canonical value (`new DynamicVar("HitCounter", 3m)`).
pub const IMPALED_HITS: i32 = 3;
/// Power uid passed to the arrow hooks for an Impaled instance that never attached.
pub const DETACHED: u16 = u16::MAX;

/// `TarotCards.All`, in the game's order (`MythicFormPower` draws one with `CombatCardSelection.NextItem`).
pub static TAROT_CARDS: [u16; 22] = {
    use ids::card as c;
    [
        c::JUDGEMENT,
        c::THE_FOOL,
        c::THE_HIGH_PRIESTESS,
        c::THE_EMPEROR,
        c::THE_EMPRESS,
        c::THE_HANGED_MAN,
        c::THE_HIEROPHANT,
        c::THE_CHARIOT,
        c::JUSTICE,
        c::THE_MAGICIAN,
        c::THE_MOON,
        c::THE_TOWER,
        c::STRENGTH,
        c::DEATH,
        c::TEMPERANCE,
        c::THE_DEVIL,
        c::THE_HERMIT,
        c::THE_LOVERS,
        c::THE_STAR,
        c::THE_SUN,
        c::THE_WORLD,
        c::WHEEL_OF_FORTUNE,
    ]
};

impl Combat {
    /// `ArrowCmd.GainArrowSlot`.
    pub fn gain_arrow_slot(&mut self) {
        self.player.arrows.push(ArrowSlot::default());
    }

    /// `ArrowCmd.GetAvailableArrowCount` (0 outside combat).
    pub fn available_arrows(&self) -> i32 {
        self.player.arrows.iter().filter(|a| a.status == ArrowStatus::Retrieved).count() as i32
    }

    /// `ArrowCmd.PlayerHasAnyArrows`.
    pub fn has_any_arrows(&self) -> bool {
        self.next_arrow().is_some()
    }

    fn next_arrow(&self) -> Option<u8> {
        self.player.arrows.as_slice().iter().rposition(|a| a.status == ArrowStatus::Retrieved).map(|i| i as u8)
    }

    /// `ArrowCmd.CreatureIsImpaledByArrow`.
    pub fn is_impaled(&self, c: Cid) -> bool {
        self.has_power(c, ids::power::IMPALED_POWER)
    }

    /// `ArrowCmd.CreatureIsImpaledByPlayersArrow` (the only applier is the player).
    pub fn is_impaled_by_player(&self, c: Cid) -> bool {
        self.impaled_uid(c).is_some()
    }

    /// The player's Impaled instance on `c`.
    pub fn impaled_uid(&self, c: Cid) -> Option<u16> {
        self.cr(c).powers.iter().find(|p| p.id == ids::power::IMPALED_POWER && p.applier == PLAYER).map(|p| p.uid)
    }

    /// `ImpaledPower.Arrows.Count` of the player's instance on `c`.
    pub fn impaled_arrows(&self, c: Cid) -> usize {
        self.player.arrows.iter().filter(|a| a.impaled == c && !a.detached).count()
    }

    fn first_impaled_arrow(&self, c: Cid, detached: bool) -> Option<u8> {
        let mut best: Option<(u16, u8)> = None;
        for (i, a) in self.player.arrows.iter().enumerate() {
            if a.impaled == c && a.detached == detached && best.is_none_or(|(s, _)| a.seq < s) {
                best = Some((a.seq, i as u8));
            }
        }
        best.map(|(_, i)| i)
    }

    /// `ArrowSlot.DestroyAfterImpaled = v` (Quietus).
    pub fn set_arrow_destroy_after_impaled(&mut self, arrow: u8, v: bool) {
        self.player.arrows[arrow as usize].destroy_after_impaled = v;
    }

    /// `CardPile.Add`: a Shoot card entering a combat pile gives a character without arrows its first slot.
    #[inline]
    pub(crate) fn note_card_added_to_pile(&mut self, c: CardIdx) {
        if self.card_def(c).tags & tag::SHOOT != 0 && self.in_progress && base_arrow_slots(self.character) == 0 && self.player.arrows.is_empty() {
            self.gain_arrow_slot();
        }
    }

    /// `ArrowCmd.TryShootArrow`: marks the last available arrow Away (history `ArrowShot`), then `AfterArrowShot`
    /// (also when there was none). The slot index, or None.
    pub fn try_shoot_arrow(&mut self, source: CardIdx) -> Option<u8> {
        let arrow = self.next_arrow();
        if let Some(a) = arrow {
            self.player.arrows[a as usize].status = ArrowStatus::Away;
            self.hist_push(HKind::ArrowShot, PLAYER, NO, 0, source, 0, 0, 0, a);
        }
        let a = arrow.unwrap_or(NO);
        self.dispatch_g(hookbit::after_arrow_shot, |cx, me, l| l.after_arrow_shot(cx, me, a, source));
        arrow
    }

    /// `ArrowCmd.ShootArrowAtTarget(target, player, attackCommand, ...)`: nothing (not even the attack) without an
    /// arrow or with a dead target; else the attack, then the impale. True if an arrow was shot.
    pub fn shoot_arrow_attack(&mut self, target: Cid, attack: &Attack) -> bool {
        if self.cr(target).is_dead() {
            return false;
        }
        let Some(arrow) = self.try_shoot_arrow(attack.card) else { return false };
        let card = attack.card;
        self.impale_inner(target, arrow, card, NO, |cx| {
            cx.execute_attack(attack);
        });
        true
    }

    /// `ArrowCmd.ShootArrowAtTarget(target, player, context, card, actionToPerform)`.
    pub fn shoot_arrow_at(&mut self, target: Cid, source: CardIdx, action: impl FnOnce(&mut Combat)) -> bool {
        if self.cr(target).is_dead() {
            return false;
        }
        let Some(arrow) = self.try_shoot_arrow(source) else { return false };
        self.impale_target(target, arrow, source, action);
        true
    }

    /// `ArrowCmd.ImpaleTarget(target, arrow, actionToPerform, context, card)` (Rain Shot shoots first, then impales).
    pub fn impale_target(&mut self, target: Cid, arrow: u8, source: CardIdx, action: impl FnOnce(&mut Combat)) {
        self.impale_inner(target, arrow, source, source, action);
    }

    // `hook_source`: the attack-command overload passes no card to `AfterArrowImpaled`.
    fn impale_inner(&mut self, target: Cid, arrow: u8, source: CardIdx, hook_source: CardIdx, action: impl FnOnce(&mut Combat)) {
        if self.cr(target).is_dead() {
            return;
        }
        action(self);
        let uid = match self.impaled_uid(target) {
            Some(u) => Some(u),
            None => self.apply_power(ids::power::IMPALED_POWER, target, Dec::ONE, PLAYER, source),
        };
        let seq = self.player.arrow_seq;
        self.player.arrow_seq = seq.wrapping_add(1);
        let a = &mut self.player.arrows[arrow as usize];
        a.impaled = target;
        a.detached = uid.is_none();
        a.seq = seq;
        let puid = uid.unwrap_or(DETACHED);
        self.dispatch_g(hookbit::after_arrow_impaled, |cx, me, l| l.after_arrow_impaled(cx, me, target, puid, arrow, hook_source));
        let a = self.player.arrows[arrow as usize];
        if a.destroy_after_impaled {
            if a.status != ArrowStatus::Destroyed {
                if self.cr(target).is_alive() && a.impaled == target && a.detached == uid.is_none() {
                    self.player.arrows[arrow as usize].impaled = NO;
                    if let Some(u) = uid {
                        self.remove_impaled_if_no_arrows(target, u);
                    }
                }
                self.player.arrows[arrow as usize].status = ArrowStatus::Destroyed;
            }
        } else if self.cr(target).is_dead() && a.impaled == target && a.detached == uid.is_none() {
            self.dislodge_first(target, uid, source);
        }
    }

    /// `ArrowCmd.DislodgeArrow(target, player, context, source)`: the first arrow of the player's instance on
    /// `target`, then `RemoveIfNoArrows`. No-op if `target` is not impaled by the player (the game throws).
    pub fn dislodge_arrow_from(&mut self, target: Cid, source: CardIdx) {
        let Some(uid) = self.impaled_uid(target) else { return };
        self.dislodge_first(target, Some(uid), source);
        self.remove_impaled_if_no_arrows(target, uid);
    }

    /// `ImpaledPower.DislodgeAllArrows`.
    pub fn dislodge_all_arrows(&mut self, target: Cid, uid: u16, source: CardIdx) {
        while self.first_impaled_arrow(target, false).is_some() {
            if !self.tick() {
                return;
            }
            self.dislodge_first(target, Some(uid), source);
        }
    }

    /// `ArrowCmd.DislodgeArrow(combatState, impaledArrow, context, source)`: `RemoveFirstArrow`; a Quietus arrow is
    /// destroyed, any other runs `AfterArrowDislodged` and is retrieved.
    pub(crate) fn dislodge_first(&mut self, target: Cid, uid: Option<u16>, source: CardIdx) {
        let Some(arrow) = self.first_impaled_arrow(target, uid.is_none()) else { return };
        let a = &mut self.player.arrows[arrow as usize];
        a.impaled = NO;
        a.detached = false;
        if a.destroy_after_impaled {
            a.status = ArrowStatus::Destroyed;
            return;
        }
        let puid = uid.unwrap_or(DETACHED);
        self.dispatch_g(hookbit::after_arrow_dislodged, |cx, me, l| l.after_arrow_dislodged(cx, me, target, puid, arrow, source));
        let a = &mut self.player.arrows[arrow as usize];
        debug_assert!(a.status == ArrowStatus::Away, "RetrieveArrow on a {:?} arrow", a.status);
        if a.status == ArrowStatus::Away {
            a.status = ArrowStatus::Retrieved;
            a.destroy_after_impaled = false;
        }
    }

    /// `ImpaledPower.RemoveIfNoArrows`.
    pub fn remove_impaled_if_no_arrows(&mut self, target: Cid, uid: u16) {
        if self.impaled_arrows(target) == 0 && self.power_idx(target, uid).is_some() {
            self.remove_power(target, uid);
        }
    }

    /// `CreatureCmd.Escape`: every arrow in the escaping creature comes back first.
    pub(crate) fn dislodge_on_escape(&mut self, c: Cid) {
        if let Some(uid) = self.impaled_uid(c) {
            self.dislodge_all_arrows(c, uid, NO);
        }
    }

    /// `MelodyCmd.PlayedThisTurn(player, tag)` (`tag::WAH`, `CHO` or `KI`): a finished play this turn carried it.
    pub fn melody_played_this_turn(&self, t: u16) -> bool {
        debug_assert!(t == tag::WAH || t == tag::CHO || t == tag::KI, "not a song tag");
        self.hist.melody_finished_this_turn & t != 0
    }

    /// `MelodyCmd.MelodiesThisTurn`: distinct song tags among this turn's finished plays.
    pub fn melodies_this_turn(&self) -> i32 {
        (self.hist.melody_finished_this_turn & tag::MELODY).count_ones() as i32
    }

    /// `MelodyCmd.MelodyCardsPlayedThisTurn`: finished plays with any song tag.
    pub fn melody_cards_played_this_turn(&self) -> i32 {
        self.hist.melody_cards_finished_this_turn as i32
    }

    /// `MelodyCmd.IsFirstPlayThisTurn(cardPlay, tag)`: `play` is this turn's first started play with the tag.
    pub fn melody_is_first_play_this_turn(&self, play: &CardPlay, t: u16) -> bool {
        debug_assert!(t == tag::WAH || t == tag::CHO || t == tag::KI, "not a song tag");
        self.hist_log
            .iter()
            .find(|e| e.kind == HKind::CardPlayStarted && self.hist_this_turn(e) && self.card_def(e.card).tags & t != 0)
            .is_some_and(|e| e.val as u16 == play.serial)
    }

    /// `CombatCardSelection.NextItem(TarotCards.All.Where(IsAllowedInRun))` (all 22 are allowed single-player).
    pub fn random_tarot(&mut self) -> u16 {
        let k = self.rng.combat_card_selection.next_int_range(0, TAROT_CARDS.len() as i32) as usize;
        TAROT_CARDS[k]
    }

    /// `CardModel.GainsBlock || Enchantment.GainsBlock` (StretchPower), with the dynamic overrides.
    pub fn card_gains_block(&self, c: CardIdx) -> bool {
        let d = self.card_def(c);
        let card = &self.cards[c as usize];
        d.gains_block
            || (d.id == ids::card::MAD_SCIENCE && d.ctype == CardType::Skill)
            || (d.id == ids::card::SOVEREIGN_BLADE && self.card_in_combat_pile(c) && self.power_amount(PLAYER, ids::power::PARRY_POWER) > 0)
            || (card.enchant != 0 && (card.enchant - 1) as u16 == ids::enchantment::ADROIT)
    }

    /// The serial of the running play of `c` (the `cardPlay` a block gain from `c` belongs to), 0 if none.
    pub fn active_play_serial(&self, c: CardIdx) -> u16 {
        self.play_stack.iter().rev().find(|p| p.play.card == c).map_or(0, |p| p.play.serial)
    }
}
