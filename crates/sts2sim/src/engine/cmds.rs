//! The command toolbox card / relic / power code is written against (`CardCmd`, `PlayerCmd`, `PowerCmd` wrappers).

use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
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
        let mut list: ArrayVec<u16, 256> = ArrayVec::new(); // (256: Splash concatenates four character pools)
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

    /// `ListExtensions.UnstableShuffle(list, rng)` over cards with the given stream (Fisher-Yates, `n - 1` draws).
    pub fn unstable_shuffle_cards(&mut self, list: &mut [CardIdx], stream: crate::state::RngStream) {
        self.rng_stream_mut(stream).shuffle(list);
    }

    /// `ListExtensions.StableShuffle(list, rng)`: sorts with `CardModel.CompareTo` (.NET introsort: ties are permuted),
    /// then `UnstableShuffle`.
    pub fn stable_shuffle_cards(&mut self, list: &mut [CardIdx], stream: crate::state::RngStream) {
        let cards = &self.cards;
        crate::sort::intro_sort(list, |a, b| Combat::card_cmp(cards, a, b));
        self.rng_stream_mut(stream).shuffle(list);
    }

    /// `CardFactory.GetForCombat` (`rng.NextItem` per card, with replacement) over the generatable cards of `pool`.
    pub fn get_for_combat(&mut self, pool: &[u16], count: usize) -> ArrayVec<CardIdx, 16> {
        self.get_for_combat_where(pool, count, |_| true)
    }

    /// `CardFactory.GetForCombat` over `pool.Where(extra)` (the call site's own filter, e.g. Metamorphosis: Attacks).
    pub fn get_for_combat_where(&mut self, pool: &[u16], count: usize, extra: impl Fn(&crate::defs::CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 256> = ArrayVec::new(); // 256: Splash concatenates four character pools
        for &id in pool {
            let d = crate::content::card_def(id);
            if !d.multiplayer_only && extra(d) && d.can_be_generated_in_combat && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event) {
                list.push(id);
            }
        }
        let mut out = ArrayVec::new();
        if list.is_empty() {
            return out;
        }
        for _ in 0..count {
            let i = self.rng.combat_card_generation.next_int_range(0, list.len() as i32) as usize;
            if let Some(c) = self.new_card(list[i], 0) {
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
        self.set_star_cost_this_turn(c, 0);
    }

    /// `PlayerCmd.GainGold`: `Hook.ModifyGoldGained` (threaded over the run-level listeners), then `Gold += (int)amount`
    /// when positive.
    pub fn gain_gold(&mut self, n: i32) {
        let mut v = Dec::int(n as i64);
        if self.listen.has(hookbit::modify_gold_gained) {
            let snap = self.snapshot(Mask::bit(hookbit::modify_gold_gained));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = crate::content::listener(&e.me).modify_gold_gained(self, e.me, v);
                }
            }
        }
        if v > Dec::ZERO {
            self.gold = self.gold.saturating_add(v.trunc());
            self.dispatch_u(hookbit::after_gold_gained, |cx, me, l| l.after_gold_gained(cx, me));
        }
    }

    /// Loses up to `n` gold; returns the amount actually lost (Debt, Thievery: `min(n, gold)`).
    pub fn lose_gold(&mut self, n: i32) -> i32 {
        let l = n.clamp(0, self.gold);
        self.gold -= l;
        l
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
        copy.flags |= cflag::IS_CLONE;
        copy.deck_idx = NO;
        self.cards[idx as usize] = copy;
        self.listen |= crate::content::card_mask(copy.id);
        self.listen_cards |= crate::content::card_mask(copy.id);
        Some(idx)
    }

    /// `CardPileCmd.AddGeneratedCardToCombat`: a brand-new card enters `pile` (hand-full redirect applies).
    pub fn add_generated_card(&mut self, c: CardIdx, pile: PileType, pos: CardPilePosition) -> bool {
        // CardGeneratedEntry(creator): everything generated during the player's side is player-created (callers whose C# passes a
        // null creator while the player acts, e.g. enemy powers reacting to a hit, use `add_generated_card_as(.., false)`).
        let by_player = self.side == Side::Player;
        self.add_generated_card_as(c, pile, pos, by_player)
    }

    /// `AddGeneratedCardToCombat(card, pile, creator, pos)` with an explicit `creator != null`.
    pub fn add_generated_card_as(&mut self, c: CardIdx, pile: PileType, pos: CardPilePosition, by_player: bool) -> bool {
        self.hist_card_generated(c, by_player);
        let ok = self.move_card(c, pile, pos);
        if ok {
            self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, c, by_player));
        }
        ok
    }

    // (`CardCmd.Discard` / `DiscardAndDraw`, including the Sly auto-plays, is `Combat::discard_cards` in `autoplay.rs`.)

    // ---- ironclad_b1 helpers ----------------------------------------------------------------------------------

    /// Value of the card's named dynamic var (`DynamicVar(\"Name\", v)`), `name` = `gen_cards::var_name::*`.
    pub fn card_named_var(&self, c: CardIdx, name: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in crate::content::card_def(card.id).vars {
            if v.kind == crate::defs::VarKind::Named && v.arg == name {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    /// Exact (decimal) value of the card's Damage var including permanent growth (`dmg_bonus`).
    pub fn card_damage_dec(&self, c: CardIdx) -> Dec {
        let card = &self.cards[c as usize];
        let mut v = Dec::int(0);
        for d in crate::content::card_def(card.id).vars {
            if d.kind == crate::defs::VarKind::Damage {
                v = Dec::int(d.base as i64 + d.up as i64 * card.upgrade as i64);
            }
        }
        v + Dec::frac(card.dmg_bonus as i64, 4)
    }

    /// Adds `amount` (decimal) to the card's Damage var permanently (`DynamicVars.Damage.BaseValue += amount`).
    pub fn add_card_damage(&mut self, c: CardIdx, amount: Dec) {
        let milli = (amount * Dec::int(10_000)).trunc();
        let card = &mut self.cards[c as usize];
        card.dmg_bonus = card.dmg_bonus.saturating_add(milli);
    }

    /// `CardPileCmd.Draw(ctx, player)` for a single card: the drawn card, or `None` (empty piles / full hand / ending).
    pub fn draw_one(&mut self) -> Option<CardIdx> {
        let before = self.player.hand.len();
        if self.draw_cards(1, false) == 0 {
            return None;
        }
        if self.player.hand.len() > before { self.player.hand.last() } else { None }
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
        // A forced selection (`!RequireManualConfirmation && |L| <= min`) returns the pile's own order; only the screen
        // shown to the player sorts the draw pile.
        let forced = min == max && cands.len() <= min as usize;
        if pile == PileType::Draw && !forced {
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
        if self.auto_select {
            return Ask::Resolved(self.auto_selected(&cands, 1));
        }
        self.begin_decision(DecisionSource::Options, purpose, if can_skip { 0 } else { 1 }, 1, cands, false, can_skip);
        Ask::Pending
    }

    /// `VakuuCardSelector.GetSelectedCards`: `options.Take(maxSelect)` (the selector Whispering Earring pushes while it
    /// auto-plays the hand; `Combat::auto_select`).
    fn auto_selected(&self, cands: &ArrayVec<CardIdx, 64>, max: usize) -> ArrayVec<CardIdx, 16> {
        let mut v = ArrayVec::new();
        for &c in cands.iter().take(max).take(16) {
            v.push(c);
        }
        v
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
        if self.auto_select {
            return Ask::Resolved(self.auto_selected(&cands, max as usize));
        }
        self.begin_decision(source, purpose, min, max, cands, manual, can_skip);
        Ask::Pending
    }

    fn begin_decision(&mut self, source: DecisionSource, purpose: u16, min: u8, max: u8, cands: ArrayVec<CardIdx, 64>, confirm_required: bool, can_skip: bool) {
        self.decision_seq += 1;
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

    /// `CardCmd.ApplySingleTurnSly`.
    pub fn apply_single_turn_sly(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_SLY;
    }

    /// `CardCmd.ApplySingleTurnRetain`.
    pub fn apply_single_turn_retain(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_RETAIN;
    }

    /// `PowerCmd.Remove<T>(creature)`: removes the creature's power `id` if present.
    pub fn remove_power_by_id(&mut self, c: Cid, id: u16) {
        if let Some(uid) = self.cr(c).power(id).map(|p| p.uid) {
            self.remove_power(c, uid);
        }
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

}
