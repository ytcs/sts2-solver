//! Potion belt, potion generation (`PotionFactory`, `PotionCmd.TryToProcure`), death prevention (`Hook.ShouldDie` /
//! `AfterPreventingDeath`, Fairy in a Bottle) and the small card/HP commands the potions need.

use crate::content;
use crate::defs::*;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// Character potion pools (`<Character>PotionPool.GetUnlockedPotions` with every epoch revealed), in epoch order.
fn character_potions(character: u8) -> &'static [u16] {
    use ids::potion as p;
    static IRONCLAD: [u16; 3] = [p::BLOOD_POTION, p::SOLDIERS_STEW, p::ASHWATER];
    static SILENT: [u16; 3] = [p::POISON_POTION, p::GHOST_IN_A_JAR, p::CUNNING_POTION];
    static DEFECT: [u16; 3] = [p::FOCUS_POTION, p::ESSENCE_OF_DARKNESS, p::POTION_OF_CAPACITY];
    static NECROBINDER: [u16; 3] = [p::POTION_OF_DOOM, p::POT_OF_GHOULS, p::BONE_BREW];
    static REGENT: [u16; 3] = [p::STAR_POTION, p::COSMIC_CONCOCTION, p::KINGS_COURAGE];
    match character {
        0 => &IRONCLAD,
        1 => &SILENT,
        2 => &DEFECT,
        3 => &NECROBINDER,
        _ => &REGENT,
    }
}

/// `SharedPotionPool.GenerateAllPotions` (array order; the Potion1/Potion2 epoch potions are included because
/// the oracle's `UnlockState.all` reveals every epoch).
static SHARED_POTIONS: [u16; 45] = {
    use ids::potion as p;
    [
        p::ATTACK_POTION,
        p::BEETLE_JUICE,
        p::BLESSING_OF_THE_FORGE,
        p::BLOCK_POTION,
        p::BOTTLED_POTENTIAL,
        p::CLARITY,
        p::COLORLESS_POTION,
        p::CURE_ALL,
        p::DEXTERITY_POTION,
        p::DISTILLED_CHAOS,
        p::DROPLET_OF_PRECOGNITION,
        p::DUPLICATOR,
        p::ENERGY_POTION,
        p::ENTROPIC_BREW,
        p::EXPLOSIVE_AMPOULE,
        p::FAIRY_IN_A_BOTTLE,
        p::FIRE_POTION,
        p::FLEX_POTION,
        p::FORTIFIER,
        p::FRUIT_JUICE,
        p::FYSH_OIL,
        p::GAMBLERS_BREW,
        p::GIGANTIFICATION_POTION,
        p::HEART_OF_IRON,
        p::LIQUID_BRONZE,
        p::LIQUID_MEMORIES,
        p::LUCKY_TONIC,
        p::MAZALETHS_GIFT,
        p::OROBIC_ACID,
        p::POTION_OF_BINDING,
        p::POWDERED_DEMISE,
        p::POWER_POTION,
        p::RADIANT_TINCTURE,
        p::REGEN_POTION,
        p::SHACKLING_POTION,
        p::SHIP_IN_A_BOTTLE,
        p::SKILL_POTION,
        p::SNECKO_OIL,
        p::SPEED_POTION,
        p::STABLE_SERUM,
        p::STRENGTH_POTION,
        p::SWIFT_POTION,
        p::TOUCH_OF_INSANITY,
        p::VULNERABLE_POTION,
        p::WEAK_POTION,
    ]
};

impl Combat {
    // ---- belt ---------------------------------------------------------------------------------------------------

    /// `Player.HasOpenPotionSlots`.
    pub fn has_open_potion_slots(&self) -> bool {
        (0..self.player.potion_slots as usize).any(|i| self.player.potions[i].is_none())
    }

    /// `Player.AddPotionInternal(potion, -1)`: first empty slot below the slot count.
    pub fn add_potion_internal(&mut self, id: u16) -> bool {
        for i in 0..(self.player.potion_slots as usize).min(MAX_POTIONS) {
            if self.player.potions[i].is_none() {
                self.player.potions[i] = Some(Potion { id });
                self.listen |= content::potion_mask(id);
                if !content::potion_implemented(id) {
                    self.flag_missing(Kind::Potion, id);
                }
                return true;
            }
        }
        false
    }

    /// `PotionCmd.TryToProcure`: `Hook.ShouldProcurePotion` (AND), add to the first free slot, `AfterPotionProcured`.
    pub fn try_procure_potion(&mut self, id: u16) -> bool {
        if self.listen.has(hookbit::should_procure_potion) {
            let snap = self.snapshot(Mask::bit(hookbit::should_procure_potion));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_procure_potion(self, e.me, id) {
                    return false;
                }
            }
        }
        if !self.add_potion_internal(id) {
            return false;
        }
        self.dispatch_u(hookbit::after_potion_procured, |cx, me, l| l.after_potion_procured(cx, me, id));
        true
    }

    // ---- PotionFactory --------------------------------------------------------------------------------------------

    /// `PotionFactory.GetPotionOptions` (character pool ++ shared pool), optionally restricted to potions that can be
    /// generated in combat.
    pub fn potion_options(&self, in_combat: bool) -> ArrayVec<u16, 64> {
        let mut v = ArrayVec::new();
        for &p in character_potions(self.character).iter().chain(SHARED_POTIONS.iter()) {
            if !in_combat || content::potion_def(p).can_be_generated_in_combat {
                v.push(p);
            }
        }
        v
    }

    /// `PotionFactory.CreateRandomPotion{In,OutOf}Combat` with the `CombatPotionGeneration` stream: one float picks the
    /// rarity bucket, one `NextItem` over the options of that rarity picks the potion.
    pub fn create_random_potion(&mut self, in_combat: bool) -> Option<u16> {
        let options = self.potion_options(in_combat);
        let f = self.rng.combat_potion_generation.next_float();
        let rarity = if f <= 0.1f32 {
            PotionRarity::Rare
        } else if f <= 0.35f32 {
            PotionRarity::Uncommon
        } else {
            PotionRarity::Common
        };
        let mut bucket: ArrayVec<u16, 64> = ArrayVec::new();
        for &p in options.iter() {
            if content::potion_def(p).rarity == rarity {
                bucket.push(p);
            }
        }
        if bucket.is_empty() {
            return None; // NextItem on an empty set draws nothing
        }
        let i = self.rng.combat_potion_generation.next_int_range(0, bucket.len() as i32) as usize;
        Some(bucket[i])
    }

    // ---- potion use (automatic path) ---------------------------------------------------------------------------------

    // ---- small commands --------------------------------------------------------------------------------------------

    /// `CardModel.SetToFreeThisCombat`: energy cost 0 for the rest of the combat, and `SetStarCostThisCombat(0)`.
    pub fn set_to_free_this_combat(&mut self, c: CardIdx) {
        if self.card_def(c).cost >= 0 {
            self.cards[c as usize].mods.push(CostMod { amount: 0, relative: false, reduce_only: false, expire: 0 });
        }
        self.set_star_cost_this_combat(c, 0);
    }

    /// `CardPileCmd.Add(IEnumerable<CardModel>, PileType, position)`: every card is moved first (hand-full redirect
    /// evaluated per card), then `AfterCardChangedPiles` fires for each moved card.
    pub fn add_cards_to_pile(&mut self, cards: &[CardIdx], pile: PileType, pos: CardPilePosition) {
        if cards.is_empty() || self.is_ending() {
            return;
        }
        let mut moved: ArrayVec<(CardIdx, PileType), 16> = ArrayVec::new();
        for &c in cards {
            if self.cards[c as usize].flags & cflag::REMOVED != 0 {
                continue;
            }
            let old = self.card_pile_type(c);
            let mut target = pile;
            if pile == PileType::Hand && self.player.hand.len() >= MAX_HAND {
                target = PileType::Discard;
            }
            if old != PileType::None {
                self.pile_mut(old).remove_value(c);
            }
            let n = self.pile(target).len();
            let index = match pos {
                CardPilePosition::Bottom => n,
                CardPilePosition::Top => 0,
                CardPilePosition::Random => self.rng.shuffle.next_int((n + 1) as i32) as usize,
            };
            self.pile_mut(target).insert(index, c);
            self.cards[c as usize].pile = target as u8;
            if old == PileType::None {
                self.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, c));
            }
            moved.push((c, old));
        }
        for &(c, old) in moved.iter() {
            if old != self.card_pile_type(c) {
                self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, c, old));
            }
        }
    }

    /// `CardPileCmd.AddGeneratedCardsToCombat`: per card `Add` + `AfterCardGeneratedForCombat`.
    pub fn add_generated_cards(&mut self, cards: &[CardIdx], pile: PileType, pos: CardPilePosition) {
        for &c in cards {
            self.add_generated_card(c, pile, pos);
        }
    }
}

// ---- Temporary{Strength,Dexterity,Focus}Power family (`ITemporaryPower`) -----------------------------------------------
//
// The three C# base classes are identical up to the inner power and the sign; a derived power (FlexPotionPower,
// SpeedPotionPower, ShacklingPotionPower, card powers, ...) just forwards its three hooks here:
//   before_applied                  -> `temp_before_applied`
//   after_power_amount_changed -> `temp_after_amount_changed`
//   after_side_turn_end             -> `temp_after_side_turn_end`
impl Combat {
    /// `BeforeApplied`: apply `sign * amount` of the inner power right away.
    pub fn temp_before_applied(&mut self, inner: u16, sign: i32, target: Cid, amount: crate::dec::Dec, applier: Cid, card: CardIdx) {
        self.apply_power(inner, target, crate::dec::Dec::int(sign as i64) * amount, applier, card);
    }

    /// `AfterPowerAmountChanged`: when this very power's amount changed by something other than its creation
    /// (`delta != Amount`), forward `sign * delta` to the inner power.
    pub fn temp_after_amount_changed(&mut self, me: Me, inner: u16, sign: i32, ch: &PowerChange) {
        if ch.target != me.owner || ch.uid != me.idx {
            return;
        }
        if ch.amount == self.power_amount(me.owner, me.id) {
            return;
        }
        self.apply_power(inner, me.owner, crate::dec::Dec::int((sign * ch.amount) as i64), ch.applier, ch.card);
    }

    /// `AfterSideTurnEnd`: at the end of the owner's side, remove the power, then take the inner power back.
    pub fn temp_after_side_turn_end(&mut self, me: Me, inner: u16, sign: i32, side: Side) {
        if self.cr(me.owner).side != side {
            return;
        }
        let amount = self.power_amount(me.owner, me.id);
        self.remove_power(me.owner, me.idx);
        self.apply_power(inner, me.owner, crate::dec::Dec::int((-sign * amount) as i64), me.owner, NO);
    }
}
