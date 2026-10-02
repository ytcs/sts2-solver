//! Card play pipeline (spec 03 §4-5): playability, resource spend, `OnPlayWrapper`, result piles.

use crate::content;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// `CardModel.IsValidTarget`.
    pub fn is_valid_target(&self, c: CardIdx, t: Cid) -> bool {
        let tt = self.card_def(c).target;
        if t == NO {
            return tt != TargetType::AnyEnemy && tt != TargetType::AnyAlly;
        }
        let cr = self.cr(t);
        if !cr.in_combat || cr.is_dead() {
            return false;
        }
        match tt {
            TargetType::AnyEnemy => cr.side == Side::Enemy,
            TargetType::AnyAlly => cr.side == Side::Player,
            _ => false,
        }
    }

    /// `CardModel.CanPlay` (spec 03 §4).
    pub fn can_play(&self, c: CardIdx) -> bool {
        let kws = self.card_keywords(c);
        if kws & kw::UNPLAYABLE != 0 {
            return false;
        }
        let d = self.card_def(c);
        if !d.x_cost {
            let e = self.card_cost(c, true).max(0);
            if e > self.player.energy {
                return false;
            }
        }
        if d.star_cost >= 0 && self.card_star_cost(c).max(0) > self.player.stars {
            return false;
        }
        if d.target == TargetType::AnyAlly {
            return false; // single-player: NoLivingAllies
        }
        // Hook.ShouldPlay (AND), then the card's own IsPlayable.
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::should_play));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_play(self, e.me, c) {
                    return false;
                }
            }
        }
        content::listener(&Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 }).is_playable(self, c)
    }

    /// Manual play of the hand card at `hand_pos` (`PlayCardAction.ExecuteAction`). Returns false if illegal.
    pub fn play_card(&mut self, hand_pos: usize, target: Cid) -> bool {
        if self.stage != Stage::AwaitAction || self.player.phase != Phase::Play {
            return false;
        }
        let Some(c) = self.player.hand.get(hand_pos) else { return false };
        if !self.can_play(c) || !self.is_valid_target(c, target) {
            return false;
        }
        // ---- SpendResources: card is still in hand ----
        let (energy_to_spend, stars_to_spend) = self.spend_resources(c);
        let play = CardPlay {
            card: c,
            target,
            is_auto: false,
            play_index: 0,
            play_count: 1,
            result_pile: PileType::Discard,
            energy_spent: energy_to_spend,
            stars_spent: stars_to_spend,
            energy_value: energy_to_spend,
        };
        self.begin_play(play);
        true
    }

    /// `CardModel.SpendResources`: captures X, spends the energy and stars (`AfterEnergySpent` / `AfterStarsSpent`).
    /// Returns `(energy, stars)` spent. The card is not moved.
    pub fn spend_resources(&mut self, c: CardIdx) -> (i32, i32) {
        let d = self.card_def(c);
        let energy_to_spend = if d.x_cost { self.player.energy } else { self.card_cost(c, true).max(0) };
        let stars_to_spend = if d.star_cost >= 0 { self.card_star_cost(c).max(0) } else { 0 };
        if d.x_cost {
            self.cards[c as usize].x_value = energy_to_spend as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if energy_to_spend > 0 {
            self.player.energy -= energy_to_spend;
        }
        self.dispatch_g(hookbit::after_energy_spent, |cx, me, l| l.after_energy_spent(cx, me, c, energy_to_spend));
        if stars_to_spend > 0 {
            self.player.stars = (self.player.stars - stars_to_spend).max(0);
            self.dispatch_g(hookbit::after_stars_spent, |cx, me, l| l.after_stars_spent(cx, me, stars_to_spend));
        }
        (energy_to_spend, stars_to_spend)
    }

    /// `CardModel.OnPlayWrapper` steps 1-8 (spec 03 §5.1); then runs the replay loop.
    fn begin_play(&mut self, mut play: CardPlay) {
        let c = play.card;
        if play.is_auto {
            // 2 (auto). `CardPileCmd.Add(card, Play, Bottom)` from wherever the card is (full Add semantics).
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        } else {
            // 2. move to the Play pile (AddDuringManualCardPlay)
            self.player.hand.remove_value(c);
            self.player.play.push(c);
            self.cards[c as usize].pile = PileType::Play as u8;
            let old = PileType::Hand;
            self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, c, old));
        }
        // 4. result location (consumes ExhaustOnNextPlay)
        let kws = self.card_keywords(c);
        let flags = self.cards[c as usize].flags;
        let d = self.card_def(c);
        let result = if flags & cflag::IS_DUPE != 0 || d.ctype == CardType::Power {
            PileType::None
        } else if flags & cflag::EXHAUST_ON_NEXT_PLAY != 0 || kws & kw::EXHAUST != 0 {
            self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
            PileType::Exhaust
        } else {
            PileType::Discard
        };
        // 5. Hook.ModifyCardPlayResultLocation (guarded, threaded; the pile is the only part modelled).
        let result = self.modify_card_play_result_location(c, play.is_auto, result);
        // 6. play count: (replay + 1), threaded through Hook.ModifyCardPlayCount (+ AfterModifyingCardPlayCount).
        let count = self.generate_play_count_for(c, play.target);
        play.result_pile = result;
        play.play_count = count;
        // 7. owner dead: the play is abandoned
        if self.cr(PLAYER).is_dead() {
            return;
        }
        // 8
        self.player.effect_depth += 1;
        self.play_ctx = Some(PlayCtx { play, step: PlayStep::Before, count, result, queue: Default::default(), queue_exhaust: false });
        self.run_play();
    }

    /// `Hook.ShouldPlay` (AND over guarded listeners).
    fn should_play_hooks(&self, c: CardIdx) -> bool {
        if self.hooks_enabled() && self.listen.has(hookbit::should_play) {
            let snap = self.snapshot(Mask::bit(hookbit::should_play));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_play(self, e.me, c) {
                    return false;
                }
            }
        }
        true
    }

    /// `CardModel.ResolveEnergyXValue` (`Hook.ModifyXValue` has no content yet): the captured X.
    pub fn resolve_energy_x(&self, c: CardIdx) -> i32 {
        self.cards[c as usize].x_value as i32
    }

    /// `CardModel.MoveToResultPileWithoutPlaying` after `CardPileCmd.Add(card, Play)` (spec 03 §5.2).
    pub fn move_to_result_pile_without_playing(&mut self, c: CardIdx) {
        self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        if self.card_pile_type(c) == PileType::Play {
            let flags = self.cards[c as usize].flags;
            if flags & cflag::IS_DUPE != 0 {
                self.remove_card_from_combat(c);
            } else if flags & cflag::EXHAUST_ON_NEXT_PLAY != 0 || self.card_keywords(c) & kw::EXHAUST != 0 {
                self.exhaust_card(c, false);
            } else {
                self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
            }
        }
    }

    /// `CardCmd.AutoPlay(card, null)` (spec 03 §5.2). Returns `true` when the (nested) play is suspended on a decision:
    /// the caller must then return `Flow::Suspend(phase)` so that it is resumed once the nested play has finished.
    /// A decision raised from a context that cannot resume (a hook outside any card play) is flagged as unfaithful.
    pub fn auto_play(&mut self, c: CardIdx) -> bool {
        self.auto_play_ex(c, NO, false)
    }

    /// `CardCmd.AutoPlay(card, target, Default, skipXCapture)`: an explicit target skips the random pick for `AnyEnemy` cards.
    /// X cards capture the whole energy value (not spent) unless `skip_x_capture`. A card that suspends inside a hook (no
    /// parent play) is resumed through `Combat::pending_hook` (set by the hook before calling this).
    pub fn auto_play_ex(&mut self, c: CardIdx, explicit_target: Cid, skip_x_capture: bool) -> bool {
        if self.is_over_or_ending() || self.cr(PLAYER).is_dead() {
            return false;
        }
        if self.card_keywords(c) & kw::UNPLAYABLE != 0 || !self.should_play_hooks(c) {
            self.move_to_result_pile_without_playing(c);
            return false;
        }
        let d = self.card_def(c);
        let mut target = explicit_target;
        match d.target {
            TargetType::AnyEnemy => {
                if target == NO {
                    let hittable = self.hittable_enemies();
                    if hittable.is_empty() {
                        self.move_to_result_pile_without_playing(c);
                        return false;
                    }
                    let i = self.rng.combat_targets.next_int_range(0, hittable.len() as i32) as usize;
                    target = hittable[i];
                }
            }
            TargetType::AnyAlly => {
                // single player: no other living player
                self.move_to_result_pile_without_playing(c);
                return false;
            }
            _ => {}
        }
        if d.x_cost && !skip_x_capture {
            // CapturedXValue = current energy (not spent)
            self.cards[c as usize].x_value = self.player.energy as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        let energy_value = if d.x_cost { self.player.energy } else { self.card_cost(c, true).max(0) };
        if self.card_pile_type(c) == PileType::None {
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        }
        // Hook.BeforeCardAutoPlayed: no content yet.
        let play = CardPlay {
            card: c,
            target,
            is_auto: true,
            play_index: 0,
            play_count: 1,
            result_pile: PileType::Discard,
            energy_spent: 0,
            stars_spent: 0,
            energy_value,
        };
        // The caller's own play (if any) is parked while the nested play runs synchronously.
        let parent = self.play_ctx.take();
        let saved_base = self.play_base;
        let depth = self.play_stack.len();
        self.play_base = depth as u8;
        self.begin_play(play);
        self.play_base = saved_base;
        if self.play_ctx.is_none() {
            self.play_ctx = parent; // nested play completed
            false
        } else {
            match parent {
                // (plays nested deeper than the parent were pushed first: the parent goes below them)
                Some(par) => self.play_stack.insert(depth, par),
                None => {
                    // a hook that started this auto-play resumes through `pending_hook`; otherwise nobody can resume it
                    if self.pending_hook.is_none() {
                        self.flag_missing(Kind::Card, self.cards[c as usize].id);
                    }
                }
            }
            true
        }
    }

    /// `CardPileCmd.AutoPlayFromDrawPile(count, position, forceExhaust)` called from a card's `on_play`: pulls the cards into
    /// the Play pile (all of them first), then auto-plays them in order. Returns `Done`, or `Suspend(resume_phase)` when a
    /// nested card needs a decision; the card must then call [`Combat::continue_auto_play`] in `resume_phase`.
    pub fn auto_play_from_draw_pile(&mut self, count: i32, pos: CardPilePosition, force_exhaust: bool, resume_phase: u8) -> Flow {
        if self.is_over_or_ending() {
            return Flow::Done;
        }
        let mut q: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
        for _ in 0..count {
            self.shuffle_if_necessary();
            let card = match pos {
                CardPilePosition::Bottom => self.player.draw.last(),
                CardPilePosition::Top => self.player.draw.first(),
                CardPilePosition::Random => {
                    let n = self.player.draw.len();
                    if n == 0 { None } else { Some(self.player.draw[self.rng.combat_card_selection.next_int_range(0, n as i32) as usize]) }
                }
            };
            let Some(card) = card else { break };
            q.push(card);
            self.move_card(card, PileType::Play, CardPilePosition::Bottom);
        }
        if let Some(ctx) = self.play_ctx.as_mut() {
            ctx.queue = q;
            ctx.queue_exhaust = force_exhaust;
        }
        self.continue_auto_play(resume_phase)
    }

    /// Plays the remaining queued cards of [`Combat::auto_play_from_draw_pile`].
    pub fn continue_auto_play(&mut self, resume_phase: u8) -> Flow {
        loop {
            let Some(ctx) = self.play_ctx.as_mut() else { return Flow::Done };
            if ctx.queue.is_empty() {
                return Flow::Done;
            }
            let c = ctx.queue.remove(0);
            let exhaust = ctx.queue_exhaust;
            if self.cr(PLAYER).is_dead() {
                if let Some(ctx) = self.play_ctx.as_mut() {
                    ctx.queue.clear();
                }
                return Flow::Done;
            }
            if exhaust {
                self.cards[c as usize].flags |= cflag::EXHAUST_ON_NEXT_PLAY;
            } else {
                self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
            }
            if self.auto_play(c) {
                return Flow::Suspend(resume_phase);
            }
        }
    }

    /// `Hook.ModifyCardPlayResultLocation`.
    fn modify_card_play_result_location(&self, c: CardIdx, is_auto: bool, mut pile: PileType) -> PileType {
        if self.hooks_enabled() && self.listen.has(hookbit::modify_card_play_result_location) {
            let snap = self.snapshot(Mask::bit(hookbit::modify_card_play_result_location));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    pile = content::listener(&e.me).modify_card_play_result_location(self, e.me, c, is_auto, pile);
                }
            }
        }
        pile
    }

    /// `CardModel.GeneratePlayCount`: `BaseReplayCount + 1`, threaded through `Hook.ModifyCardPlayCount` (guarded); the
    /// models that changed it are then told via `AfterModifyingCardPlayCount` (ThrowingAxe marks itself used).
    pub fn generate_play_count(&mut self, c: CardIdx) -> u8 {
        self.generate_play_count_for(c, NO)
    }

    /// `generate_play_count` with the play's target (what `ModifyCardPlayCount` listeners receive).
    pub fn generate_play_count_for(&mut self, c: CardIdx, target: Cid) -> u8 {
        let mut count_i = self.cards[c as usize].base_replay.saturating_add(1) as i32;
        if self.listen.has(hookbit::modify_card_play_count) && self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_card_play_count));
            let mut mods: super::damage::Mods = super::damage::Mods::new();
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    let n = content::listener(&e.me).modify_card_play_count(self, e.me, c, target, count_i);
                    if n != count_i {
                        mods.push(e.me);
                    }
                    count_i = n;
                }
            }
            if !mods.is_empty() {
                self.dispatch_g(hookbit::after_modifying_card_play_count, |cx, me, l| {
                    if mods.iter().any(|m| m.kind == me.kind && m.idx == me.idx && m.owner == me.owner) {
                        l.after_modifying_card_play_count(cx, me, c);
                    }
                });
            }
        }
        count_i.clamp(0, 255) as u8
    }

    /// Advances the in-flight card play until it finishes or needs a decision.
    pub fn run_play(&mut self) {
        loop {
            if self.play_ctx.is_none() {
                // A nested auto-play finished after a decision: continue the outer card at its resume phase.
                if self.play_stack.len() > self.play_base as usize {
                    let top = self.play_stack.len() - 1;
                    self.play_ctx = Some(self.play_stack.remove(top));
                } else {
                    return;
                }
            }
            let Some(mut ctx) = self.play_ctx else { return };
            let c = ctx.play.card;
            match ctx.step {
                PlayStep::Before => {
                    if self.is_over_or_ending() {
                        self.finish_play();
                        continue;
                    }
                    let p = ctx.play;
                    self.dispatch_g(hookbit::before_card_played, |cx, me, l| l.before_card_played(cx, me, &p));
                    self.hist.cards_played_this_turn += 1;
                    match self.card_def(c).ctype {
                        CardType::Attack => self.hist.attacks_played_this_turn += 1,
                        CardType::Skill => self.hist.skills_played_this_turn += 1,
                        _ => {}
                    }
                    ctx.step = PlayStep::OnPlay(0);
                    self.play_ctx = Some(ctx);
                }
                PlayStep::OnPlay(phase) => {
                    let me = Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 };
                    let p = ctx.play;
                    let depth = self.play_stack.len();
                    match content::listener(&me).on_play(self, &p, phase) {
                        Flow::Done => {
                            if self.play_stack.len() > depth {
                                // A hook (not this card's own effect) auto-played a card that is now waiting for a decision,
                                // but this effect cannot pause mid-way (e.g. inside `draw_cards`): not faithful, so flag it.
                                self.flag_missing(Kind::Card, self.cards[c as usize].id);
                            }
                            // (re-read: the effect may have changed the ctx, e.g. its auto-play queue)
                            if let Some(mut ctx) = self.play_ctx {
                                ctx.step = PlayStep::After;
                                self.play_ctx = Some(ctx);
                            }
                        }
                        Flow::Suspend(next) => {
                            if self.play_stack.len() > depth {
                                // A nested auto-play is waiting for the decision; this card resumes at `next` afterwards.
                                // (its entry sits at `depth`: everything deeper was pushed by plays nested inside it)
                                self.play_stack[depth].step = PlayStep::OnPlay(next);
                            } else if let Some(mut ctx) = self.play_ctx {
                                ctx.step = PlayStep::OnPlay(next);
                                self.play_ctx = Some(ctx);
                            }
                            self.stage = Stage::AwaitChoice;
                            return;
                        }
                    }
                }
                PlayStep::After => {
                    if self.cr(PLAYER).is_dead() {
                        self.finish_play();
                        continue;
                    }
                    // Enchantment.OnPlay / Affliction.OnPlay — no content yet.
                    if self.in_progress {
                        let p = ctx.play;
                        self.dispatch_u(hookbit::after_card_played, |cx, me, l| l.after_card_played(cx, me, &p));
                    }
                    ctx.play.play_index += 1;
                    if ctx.play.play_index < ctx.count {
                        ctx.step = PlayStep::Before;
                        self.play_ctx = Some(ctx);
                    } else {
                        self.finish_play();
                        continue;
                    }
                }
            }
        }
    }

    /// Steps 10-12 of `OnPlayWrapper`: depth--, move the card to its result pile, clean up cost modifiers.
    fn finish_play(&mut self) {
        let Some(ctx) = self.play_ctx.take() else { return };
        let c = ctx.play.card;
        self.player.effect_depth = self.player.effect_depth.saturating_sub(1);
        if self.card_pile_type(c) == PileType::Play {
            match ctx.result {
                PileType::None => self.remove_card_from_combat(c),
                PileType::Exhaust => self.exhaust_card(c, false),
                p => {
                    self.move_card(c, p, CardPilePosition::Bottom);
                }
            }
        }
        // 12. remove WhenPlayed local cost modifiers (after the card has moved).
        let card = &mut self.cards[c as usize];
        let mut kept: crate::util::ArrayVec<CostMod, 3> = crate::util::ArrayVec::new();
        for m in card.mods.iter() {
            if m.expire & EXPIRE_WHEN_PLAYED == 0 {
                kept.push(*m);
            }
        }
        card.mods = kept;
        card.flags &= !cflag::X_CAPTURED;
        // CheckForEmptyHand(owner) closes `OnPlayWrapper`; manual plays get it from `after_action`.
        if ctx.play.is_auto {
            self.check_for_empty_hand();
        }
    }
}
