//! The command toolbox card / relic / power code is written against (`CardCmd`, `PlayerCmd`, `PowerCmd` wrappers).

use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::defs::VarKind;
use crate::util::ArrayVec;

/// Result of requesting a decision: either already resolved (empty / forced choice) or pending (the effect must
/// return `Flow::Suspend` and read `cx.choice` when resumed).
pub enum Ask {
    Resolved(ArrayVec<CardIdx, 16>),
    Pending,
}

impl Combat {
    /// Records that unported content was used (keeps the first).
    pub fn flag_missing(&mut self, kind: Kind, id: u16) {
        if self.missing.is_none() {
            self.missing = Some((kind, id));
        }
    }

    /// The character's card pool (`Owner.Character.CardPool`), in the game's array order.
    pub fn character_pool(&self) -> &'static [u16] {
        use crate::content::gen_pools as p;
        match self.character {
            0 => &p::IRONCLAD,
            1 => &p::SILENT,
            2 => &p::DEFECT,
            3 => &p::NECROBINDER,
            _ => &p::REGENT,
        }
    }

    /// `CardFactory.GetDistinctForCombat` (spec 03 §10.2): filter (generatable, not Basic/Ancient/Event, single-player),
    /// then a FULL `UnstableShuffle` of the candidates with `CombatCardGeneration` (n-1 draws regardless of `count`),
    /// take the first `count`, and instantiate them. `extra` is the call site's own `Where` filter.
    pub fn get_distinct_for_combat(&mut self, pool: &[u16], count: usize, extra: impl Fn(&crate::defs::CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 128> = ArrayVec::new();
        for &id in pool {
            let d = crate::content::card_def(id);
            if !d.multiplayer_only && extra(d) && d.can_be_generated_in_combat
                && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event)
            {
                list.push(id);
            }
        }
        self.rng.combat_card_generation.shuffle(list.as_mut_slice());
        let mut out = ArrayVec::new();
        for &id in list.iter().take(count) {
            if let Some(c) = self.new_card(id, 0) {
                out.push(c);
            }
        }
        out
    }

    /// `CardModel.SetToFreeThisTurn` (energy part): cost 0 until played or end of turn.
    pub fn set_to_free_this_turn(&mut self, c: CardIdx) {
        let canonical = self.card_def(c).cost;
        if canonical >= 0 {
            let card = &mut self.cards[c as usize];
            card.mods.push(CostMod { amount: 0, relative: false, reduce_only: false, expire: EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED });
        }
    }

    /// `PlayerCmd.GainEnergy` (`ModifyEnergyGain` hook not implemented yet).
    pub fn gain_energy(&mut self, n: i32) {
        if n > 0 {
            self.player.energy += n;
        }
    }

    /// `CombatState.HittableEnemies`: alive, attached, and `ShouldAllowHitting`.
    pub fn hittable_enemies(&self) -> ArrayVec<Cid, MAX_CREATURES> {
        let mut o = ArrayVec::new();
        for &e in self.enemies.iter() {
            if self.cr(e).is_alive() && self.cr(e).in_combat && self.should_allow_hitting(e) {
                o.push(e);
            }
        }
        o
    }

    /// `PowerCmd.Apply<T>` to every hittable enemy, sequentially in list order.
    pub fn apply_power_to_hittable_enemies(&mut self, id: u16, amount: Dec, applier: Cid, card: CardIdx) {
        let targets = self.hittable_enemies();
        for &t in targets.iter() {
            self.apply_power(id, t, amount, applier, card);
        }
    }

    /// `Rng.CombatCardSelection.NextItem(hand)`: one draw, even for a single card; none for an empty hand.
    pub fn random_hand_card(&mut self) -> Option<CardIdx> {
        let n = self.player.hand.len();
        if n == 0 {
            return None;
        }
        let i = self.rng.combat_card_selection.next_int_range(0, n as i32) as usize;
        Some(self.player.hand[i])
    }

    /// `CardCmd.Upgrade` in combat: only the combat card changes (no hooks).
    pub fn upgrade_in_combat(&mut self, c: CardIdx) {
        if self.cards[c as usize].upgrade < self.card_def(c).max_upgrade {
            self.upgrade_card(c);
        }
    }

    pub fn is_upgradable(&self, c: CardIdx) -> bool {
        self.cards[c as usize].upgrade < self.card_def(c).max_upgrade
    }

    /// `CardModel.CreateClone` + registration: copies live state (including active cost modifiers).
    pub fn clone_card(&mut self, c: CardIdx) -> Option<CardIdx> {
        if self.n_cards as usize >= MAX_CARDS {
            return None;
        }
        let idx = self.n_cards as CardIdx;
        self.n_cards += 1;
        let mut copy = self.cards[c as usize];
        copy.pile = PileType::None as u8;
        copy.flags &= !(cflag::EXHAUST_ON_NEXT_PLAY | cflag::REMOVED);
        copy.deck_idx = NO;
        self.cards[idx as usize] = copy;
        self.listen |= crate::content::card_mask(copy.id);
        Some(idx)
    }

    /// `CardPileCmd.AddGeneratedCardToCombat`: a brand-new card enters `pile` (hand-full redirect applies).
    pub fn add_generated_card(&mut self, c: CardIdx, pile: PileType, pos: CardPilePosition) -> bool {
        // History.CardGenerated — not tracked yet.
        let ok = self.move_card(c, pile, pos);
        if ok {
            self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, c));
        }
        ok
    }

    /// `CardCmd.Discard` for one card (Sly auto-play not implemented yet).
    pub fn discard_card(&mut self, c: CardIdx) {
        self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
        self.dispatch_g(hookbit::after_card_discarded, |cx, me, l| l.after_card_discarded(cx, me, c));
    }

    // ---- decisions ---------------------------------------------------------------------------------------------

    /// `CardSelectCmd.FromHand*` (spec 03 §9.2): candidates in hand order; auto-resolves forced choices.
    pub fn ask_hand(&mut self, purpose: u16, min: u8, max: u8, filter: impl Fn(&Combat, CardIdx) -> bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, 64> = ArrayVec::new();
        for &c in self.player.hand.iter() {
            if filter(self, c) {
                cands.push(c);
            }
        }
        self.raise(DecisionSource::Hand, purpose, min, max, cands, false)
    }

    /// `CardSelectCmd.FromCombatPile`: candidates in pile order, except the draw pile which is presented sorted by
    /// (rarity, id) — a stable sort — so its real order stays hidden (spec 03 §9.3).
    pub fn ask_pile(&mut self, purpose: u16, pile: PileType, min: u8, max: u8, filter: impl Fn(&Combat, CardIdx) -> bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, 64> = ArrayVec::new();
        for &c in self.pile(pile).iter() {
            if filter(self, c) {
                cands.push(c);
            }
        }
        if pile == PileType::Draw {
            let cards = &self.cards;
            let key = |c: &CardIdx| {
                let id = cards[*c as usize].id;
                (crate::content::card_def(id).rarity, id)
            };
            // stable insertion sort (List.OrderBy is stable)
            let sl = cands.as_mut_slice();
            for i in 1..sl.len() {
                let x = sl[i];
                let kx = key(&x);
                let mut j = i;
                while j > 0 && key(&sl[j - 1]) > kx {
                    sl[j] = sl[j - 1];
                    j -= 1;
                }
                sl[j] = x;
            }
        }
        self.raise(DecisionSource::Pile(pile), purpose, min, max, cands, false)
    }

    /// `CardSelectCmd.FromChooseACardScreen`: pick one of the (already generated) cards, optionally skip.
    pub fn ask_options(&mut self, purpose: u16, options: &[CardIdx], can_skip: bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, 64> = ArrayVec::new();
        for &c in options {
            cands.push(c);
        }
        if self.is_over_or_ending() || cands.is_empty() {
            return Ask::Resolved(ArrayVec::new());
        }
        self.begin_decision(DecisionSource::Options, purpose, if can_skip { 0 } else { 1 }, 1, cands, false, can_skip);
        Ask::Pending
    }

    /// Shared decision entry for `FromHand`/`FromCombatPile`: `RequireManualConfirmation = (min != max)`;
    /// `!manual && |L| <= min` auto-resolves with all candidates (no decision).
    fn raise(&mut self, source: DecisionSource, purpose: u16, min: u8, max: u8, cands: ArrayVec<CardIdx, 64>, can_skip: bool) -> Ask {
        if self.is_over_or_ending() || cands.is_empty() {
            return Ask::Resolved(ArrayVec::new());
        }
        let manual = min != max;
        if !manual && cands.len() <= min as usize {
            let mut all = ArrayVec::new();
            for &c in cands.iter().take(16) {
                all.push(c);
            }
            return Ask::Resolved(all);
        }
        self.begin_decision(source, purpose, min, max, cands, manual, can_skip);
        Ask::Pending
    }

    fn begin_decision(&mut self, source: DecisionSource, purpose: u16, min: u8, max: u8, cands: ArrayVec<CardIdx, 64>, confirm_required: bool, can_skip: bool) {
        self.decision = Some(Decision { source, min, max, cands, selected: ArrayVec::new(), confirm_required, can_skip, purpose });
    }

    /// Applies a `Pick` click to the pending decision. Returns false if illegal.
    pub(crate) fn decision_pick(&mut self, i: u8) -> bool {
        let Some(d) = self.decision.as_mut() else { return false };
        if i as usize >= d.cands.len() {
            return false;
        }
        if let Some(pos) = d.selected.position(i) {
            d.selected.remove(pos); // clicking a selected card deselects it
        } else {
            if d.selected.len() >= d.max as usize {
                d.selected.truncate(d.selected.len().saturating_sub(1)); // at max: most recent selection is replaced
            }
            d.selected.push(i);
        }
        let done = !d.confirm_required && d.selected.len() >= d.max as usize;
        if done {
            self.finish_decision();
        }
        true
    }

    /// `Confirm` (or skip when nothing is selected and skipping is allowed).
    pub(crate) fn decision_confirm(&mut self) -> bool {
        let Some(d) = self.decision.as_ref() else { return false };
        let n = d.selected.len();
        let ok = (d.confirm_required && n >= d.min as usize && n <= d.max as usize) || (d.can_skip && n == 0);
        if ok {
            self.finish_decision();
        }
        ok
    }

    fn finish_decision(&mut self) {
        let d = self.decision.take().unwrap();
        let mut ch = Choice::default();
        for &i in d.selected.iter() {
            ch.cards.push(d.cands[i as usize]);
        }
        self.choice = ch;
        self.stage = Stage::AwaitAction;
        self.resume_after_decision();
    }
}

// ---- Silent slice helpers ------------------------------------------------------------------------------------------------
impl Combat {
    /// `CardModel.IsSlyThisTurn`: Sly keyword or the single-turn flag.
    pub fn is_sly_this_turn(&self, c: CardIdx) -> bool {
        self.card_keywords(c) & kw::SLY != 0 || self.cards[c as usize].flags & cflag::SINGLE_TURN_SLY != 0
    }

    /// `CardCmd.ApplySingleTurnSly`.
    pub fn apply_single_turn_sly(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_SLY;
    }

    /// `CardCmd.ApplySingleTurnRetain`.
    pub fn apply_single_turn_retain(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_RETAIN;
    }

    /// `CardCmd.DiscardAndDraw` (spec 03 §5.4): discard all (in order), then draw, then auto-play each Sly card in the
    /// original order. Returns `Suspended` if a Sly auto-play suspended on a decision: the calling effect must return
    /// `Flow::Suspend(next)` for the rest of its body (remaining Sly cards are queued and run by the engine first).
    pub fn discard_cards(&mut self, cards: &[CardIdx], draw: i32) -> crate::engine::RunResult {
        use crate::engine::RunResult;
        if self.is_over_or_ending() || cards.is_empty() {
            return RunResult::Finished;
        }
        let mut sly: ArrayVec<CardIdx, MAX_HAND> = ArrayVec::new();
        for &c in cards {
            if self.is_sly_this_turn(c) {
                sly.push(c);
            }
            self.discard_card(c);
        }
        if draw > 0 {
            self.draw_cards_list(draw, false);
        }
        for i in 0..sly.len() {
            if self.auto_play(sly[i], NO) {
                let mut q: ArrayVec<CardIdx, 10> = ArrayVec::new();
                for j in (i + 1)..sly.len() {
                    q.push(sly[j]);
                }
                for &c in self.sly_queue.iter() {
                    q.push(c);
                }
                self.sly_queue = q;
                return RunResult::Suspended;
            }
        }
        RunResult::Finished
    }

    /// `CreatureCmd.LoseBlock`.
    pub fn lose_block(&mut self, c: Cid, amount: i32, remover: Cid) {
        if self.is_over_or_ending() || self.cr(c).is_dead() || amount <= 0 {
            return;
        }
        let before = self.cr(c).block;
        self.cr_mut(c).block = (before - amount).max(0);
        if before > 0 && self.cr(c).block <= 0 {
            self.dispatch_u(hookbit::after_block_broken, |cx, me, l| l.after_block_broken(cx, me, c, remover));
        }
    }

    /// `PowerCmd.Remove<T>(creature)`: removes the creature's power `id` if present.
    pub fn remove_power_by_id(&mut self, c: Cid, id: u16) {
        if let Some(uid) = self.cr(c).power(id).map(|p| p.uid) {
            self.remove_power(c, uid);
        }
    }

    /// Value of the card's explicitly named dynamic var (`DynamicVars["Name"]`, `gen_cards::var_name::*`).
    pub fn card_named_var(&self, c: CardIdx, name: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in crate::content::card_def(card.id).vars {
            if v.kind == VarKind::Named && v.arg == name {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    /// `Shiv.CreateInHand(owner, count, combatState)`: creates all cards first, then adds them one by one.
    pub fn create_shivs_in_hand(&mut self, count: i32) -> ArrayVec<CardIdx, MAX_HAND> {
        let mut shivs: ArrayVec<CardIdx, MAX_HAND> = ArrayVec::new();
        if count <= 0 || self.is_over_or_ending() {
            return shivs;
        }
        for _ in 0..count.min(MAX_HAND as i32) {
            if let Some(c) = self.new_card(crate::ids::card::SHIV, 0) {
                shivs.push(c);
            }
        }
        for i in 0..shivs.len() {
            let c = shivs[i];
            self.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        shivs
    }
}

// ---- Enchantments (Silent slice: only what Blade of Ink needs) ------------------------------------------------------------
impl Combat {
    /// `CardCmd.Enchant<T>(card, amount)` for a card without an enchantment (stored as `enchantment id + 1`).
    pub fn enchant_card(&mut self, c: CardIdx, enchantment: u16, amount: i32) {
        let card = &mut self.cards[c as usize];
        if card.enchant == 0 {
            card.enchant = (enchantment + 1) as u8;
            card.enchant_amount = amount as i16;
        } else if card.enchant as u16 == enchantment + 1 {
            card.enchant_amount += amount as i16;
        }
    }

    /// `EnchantmentModel.OnPlay` of the card's enchantment (after the card's own `OnPlay`, every replay).
    pub fn enchantment_on_play(&mut self, enchantment: u16, play: &CardPlay) {
        if enchantment == crate::ids::enchantment::INKY {
            // Inky: apply Weak (PowerVar 1) to the target, or to every hittable enemy for AllEnemies cards.
            let id = crate::ids::power::WEAK_POWER;
            if self.card_target_type(play.card) != TargetType::AllEnemies {
                self.apply_power(id, play.target, Dec::ONE, PLAYER, play.card);
            } else {
                self.apply_power_to_hittable_enemies(id, Dec::ONE, PLAYER, play.card);
            }
        } else {
            self.flag_missing(Kind::Enchantment, enchantment);
        }
    }
}
