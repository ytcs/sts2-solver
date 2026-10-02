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
        if d.star_cost > 0 && d.star_cost as i32 > self.player.stars {
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
        let d = self.card_def(c);
        let energy_to_spend = if d.x_cost { self.player.energy } else { self.card_cost(c, true).max(0) };
        let stars_to_spend = if d.star_cost > 0 { d.star_cost as i32 } else { 0 };
        if d.x_cost {
            self.cards[c as usize].x_value = energy_to_spend as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if energy_to_spend > 0 {
            self.player.energy -= energy_to_spend;
        }
        self.dispatch_g(hookbit::after_energy_spent, |cx, me, l| l.after_energy_spent(cx, me, c, energy_to_spend));
        if stars_to_spend > 0 {
            self.player.stars -= stars_to_spend;
        }
        let play = CardPlay {
            card: c,
            target,
            is_auto: false,
            play_index: 0,
            play_count: 1,
            result_pile: PileType::Discard,
            energy_spent: energy_to_spend,
            stars_spent: stars_to_spend,
        };
        self.begin_play(play);
        true
    }

    /// `CardModel.OnPlayWrapper` steps 1-8 (spec 03 §5.1); then runs the replay loop.
    fn begin_play(&mut self, mut play: CardPlay) {
        let c = play.card;
        // 2. move to the Play pile (AddDuringManualCardPlay)
        self.player.hand.remove_value(c);
        self.player.play.push(c);
        self.cards[c as usize].pile = PileType::Play as u8;
        let old = PileType::Hand;
        self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, c, old));
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
        // 5. Hook.ModifyCardPlayResultLocation — no content yet.
        // 6. play count: (replay + 1), Hook.ModifyCardPlayCount — no content yet.
        let count = self.cards[c as usize].base_replay.saturating_add(1);
        play.result_pile = result;
        play.play_count = count;
        // 7-8
        self.player.effect_depth += 1;
        self.play_ctx = Some(PlayCtx { play, step: PlayStep::Before, count, result });
        self.run_play();
    }

    /// Advances the in-flight card play until it finishes or needs a decision.
    pub fn run_play(&mut self) {
        loop {
            let Some(mut ctx) = self.play_ctx else { return };
            let c = ctx.play.card;
            match ctx.step {
                PlayStep::Before => {
                    if self.is_over_or_ending() {
                        self.finish_play();
                        return;
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
                    match content::listener(&me).on_play(self, &p, phase) {
                        Flow::Done => {
                            ctx.step = PlayStep::After;
                            self.play_ctx = Some(ctx);
                        }
                        Flow::Suspend(next) => {
                            ctx.step = PlayStep::OnPlay(next);
                            self.play_ctx = Some(ctx);
                            self.stage = Stage::AwaitChoice;
                            return;
                        }
                    }
                }
                PlayStep::After => {
                    if self.cr(PLAYER).is_dead() {
                        self.finish_play();
                        return;
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
                        return;
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
    }
}
