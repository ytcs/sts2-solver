//! Engine extensions for effects that cannot be written as a straight-line `on_play`:
//!
//! * Hook-driven `AutoPlayFromDrawPile` (Mayhem) with a resumable queue (card effects use `auto_play_from_draw_pile` /
//!   `auto_play` of `play.rs`, whose nested plays park their caller on `play_stack`).
//! * Decisions raised from INSIDE a hook (Entropy at turn start, Stratagem during a reshuffle). The game simply awaits
//!   the choice in the middle of a hook dispatch; our hooks are plain function calls and cannot suspend. Instead the
//!   whole agent step is rolled back and re-executed: `step` snapshots the (Copy) state, a hook that needs a decision
//!   calls `hook_decision`, which records the decision and lets the step finish "blind". `step` then restores the
//!   snapshot, exposes the decision to the agent, and after it is answered re-runs the same action with the answer
//!   recorded in `ExtState::replay_answers`: every hook-decision site consumes its recorded answer in the same order,
//!   so the re-execution is bit-identical up to (and past) the decision. Only combats containing cards that can
//!   raise such decisions pay for the snapshot (`ExtState::unwind_enabled`).
//! * `CardCmd.TransformToRandom` (Entropy).

use super::cmds::Ask;
use super::{Action, Stream};
use crate::content::gen_pools as gp;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    // ---- hook-driven auto-play from the draw pile -------------------------------------------------------------

    /// `CardPileCmd.AutoPlayFromDrawPile(count, position, forceExhaust)` called from a HOOK (Mayhem), i.e. with no
    /// card play of its own to park (spec 03 §5.3): pick all cards first (moving each to the Play pile), then auto-play
    /// them one by one. Resumable: a card that suspends on a decision leaves the remaining ones in
    /// `ExtState::autoplay_queue`, which `resume_after_decision` continues.
    pub fn auto_play_from_draw_pile_hook(&mut self, count: i32, pos: CardPilePosition, force_exhaust: bool) {
        if self.is_over_or_ending() {
            return;
        }
        let mut cards: ArrayVec<CardIdx, 8> = ArrayVec::new();
        for _ in 0..count.min(8) {
            self.shuffle_if_necessary();
            let n = self.player.draw.len();
            if n == 0 {
                break;
            }
            let c = match pos {
                CardPilePosition::Top => self.player.draw[0],
                CardPilePosition::Bottom => self.player.draw[n - 1],
                CardPilePosition::Random => {
                    let i = self.rng.combat_card_selection.next_int_range(0, n as i32) as usize;
                    self.player.draw[i]
                }
            };
            cards.push(c);
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        }
        self.ext.autoplay_queue.clear();
        for &c in cards.iter() {
            self.ext.autoplay_queue.push((c, force_exhaust));
        }
        self.drain_autoplay_queue();
    }

    /// Auto-plays the queued cards until the queue is empty or one of them suspends on a decision.
    pub(crate) fn drain_autoplay_queue(&mut self) {
        self.ext.hook_autoplay = true;
        while !self.ext.autoplay_queue.is_empty() {
            let (c, force) = self.ext.autoplay_queue.remove(0);
            if self.cr(PLAYER).is_dead() {
                self.ext.autoplay_queue.clear();
                break;
            }
            if force {
                self.cards[c as usize].flags |= cflag::EXHAUST_ON_NEXT_PLAY;
            } else {
                self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
            }
            if self.auto_play(c) {
                break; // suspended: `resume_after_decision` continues the queue
            }
        }
        self.ext.hook_autoplay = false;
    }

    // ---- decisions raised from hooks ---------------------------------------------------------------------------------

    /// Wraps the result of an `ask_*` call made from inside a hook. `Resolved` passes through. A real decision is
    /// answered from the replay record if available; otherwise the current step is marked for rollback (the hook then
    /// sees an empty selection and must tolerate it) so the agent can be asked after the rollback.
    pub fn hook_decision(&mut self, a: Ask, kind: Kind, id: u16) -> Ask {
        match a {
            Ask::Resolved(c) => Ask::Resolved(c),
            Ask::Pending => {
                let Some(d) = self.decision.take() else { return Ask::Resolved(ArrayVec::new()) };
                let pos = self.ext.replay_pos as usize;
                if pos < self.ext.replay_answers.len() {
                    self.ext.replay_pos += 1;
                    return Ask::Resolved(self.ext.replay_answers[pos]);
                }
                if self.ext.unwind_enabled && !self.ext.unwind {
                    self.ext.unwind = true;
                    self.ext.unwind_decision = Some(d);
                } else if !self.ext.unwind {
                    // Content that raises hook decisions was not announced at combat start: not faithful.
                    self.flag_missing(kind, id);
                }
                Ask::Resolved(ArrayVec::new())
            }
        }
    }

    /// Applies an action. Returns false if it was illegal (state unchanged). Wraps `step_inner` with the rollback /
    /// replay protocol for decisions raised from hooks.
    pub fn step(&mut self, a: Action) -> bool {
        if self.ext.unwound.is_some() {
            return self.step_hook_answer(a);
        }
        self.step_unwindable(a)
    }

    fn step_unwindable(&mut self, a: Action) -> bool {
        if !self.ext.unwind_enabled {
            return self.step_inner(a);
        }
        self.ext.replay_pos = 0;
        let saved = *self;
        let ok = self.step_inner(a);
        if self.ext.unwind {
            let d = self.ext.unwind_decision.take();
            let (orig_decision, orig_stage) = (saved.decision, saved.stage);
            *self = saved;
            self.ext.unwind = false;
            self.ext.unwound = Some(UnwoundStep { action: a, orig_decision, orig_stage });
            self.decision = d;
            self.stage = Stage::AwaitChoice;
            return true;
        }
        self.ext.replay_answers.clear();
        ok
    }

    /// An agent click while a hook-raised decision is pending: when the selection completes, the recorded answer is
    /// stored and the original action is re-executed from its snapshot.
    fn step_hook_answer(&mut self, a: Action) -> bool {
        let ok = match a {
            Action::Pick { idx } => self.decision_pick(idx),
            Action::Confirm => self.decision_confirm(),
            _ => false,
        };
        if !ok {
            return false;
        }
        if self.decision.is_some() {
            return true; // selection still in progress
        }
        let h = self.ext.unwound.take().unwrap();
        self.decision = h.orig_decision;
        self.stage = h.orig_stage;
        self.step_unwindable(h.action);
        true
    }

    // ---- transform ---------------------------------------------------------------------------------------------------

    /// Pool a card belongs to (`CardModel.Pool`).
    fn pool_of(id: u16) -> &'static [u16] {
        let pools: [&'static [u16]; 11] = [&gp::IRONCLAD, &gp::SILENT, &gp::DEFECT, &gp::NECROBINDER, &gp::REGENT, &gp::COLORLESS, &gp::CURSE, &gp::STATUS, &gp::TOKEN, &gp::EVENT, &gp::QUEST];
        for p in pools {
            if p.contains(&id) {
                return p;
            }
        }
        &gp::COLORLESS
    }

    /// `CardCmd.TransformToRandom(original, rng)` in combat: `CardFactory.CreateRandomCardForTransform` (one `NextItem`
    /// draw over the candidates, spec 03 §6.6) and `CardCmd.Transform` (the replacement takes the original's place).
    pub fn transform_to_random(&mut self, original: CardIdx, stream: Stream) {
        if self.is_ending() {
            return;
        }
        let oid = self.cards[original as usize].id;
        let od = crate::content::card_def(oid);
        let pool: &[u16] = if od.ctype == CardType::Quest || matches!(od.rarity, CardRarity::Event | CardRarity::Ancient | CardRarity::Token) {
            &gp::COLORLESS
        } else {
            Self::pool_of(oid)
        };
        let mut options: ArrayVec<u16, 128> = ArrayVec::new();
        for &id in pool {
            let d = crate::content::card_def(id);
            let rarity_ok = matches!(od.rarity, CardRarity::Status | CardRarity::Curse) || matches!(d.rarity, CardRarity::Common | CardRarity::Uncommon | CardRarity::Rare);
            if rarity_ok && d.can_be_generated_in_combat && id != oid && !d.multiplayer_only {
                options.push(id);
            }
        }
        if options.is_empty() {
            return;
        }
        let rng = match stream {
            Stream::Shuffle => &mut self.rng.shuffle,
            Stream::CardSelection => &mut self.rng.combat_card_selection,
            Stream::CardGeneration => &mut self.rng.combat_card_generation,
            Stream::Targets => &mut self.rng.combat_targets,
        };
        let i = rng.next_int_range(0, options.len() as i32) as usize;
        let Some(r) = self.new_card(options[i], 0) else { return };
        self.transform_in_place(original, r);
        self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, r));
    }

    /// `CardCmd.Transform(original, replacement)` in a combat pile (spec 03 §6.6): the replacement (a fresh arena card in
    /// no pile) takes the original's index in its pile; the original leaves the combat.
    fn transform_in_place(&mut self, original: CardIdx, replacement: CardIdx) {
        let pile = self.card_pile_type(original);
        if pile == PileType::None {
            return;
        }
        let idx = self.pile(pile).position(original).unwrap_or(0);
        self.pile_mut(pile).remove_value(original);
        {
            let o = &mut self.cards[original as usize];
            o.pile = PileType::None as u8;
            o.flags |= cflag::REMOVED;
        }
        self.pile_mut(pile).insert(idx, replacement);
        self.cards[replacement as usize].pile = pile as u8;
        // History.CardGenerated; Hook.AfterCardEnteredCombat; Hook.AfterCardChangedPiles(replacement, pile)
        self.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, replacement));
        self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, replacement, pile));
    }
}
