// FoolPotionPool.
use crate::content::relics::fool_pool::tagged_character_cards;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

listener!(BottledSong {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        let mut list: ArrayVec<CardIdx, 3> = ArrayVec::new();
        for t in [tag::WAH, tag::CHO, tag::KI] {
            let pool = tagged_character_cards(t);
            if let Some(c) = cx.get_distinct_for_combat(pool.as_slice(), 1, |_| true).first() {
                cx.set_to_free_this_turn(c);
                list.push(c);
            }
        }
        for &c in list.iter() {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
        Flow::Done
    }
});

listener!(CrystallineEuphoria {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        let mut joys: ArrayVec<CardIdx, 16> = ArrayVec::new();
        for _ in 0..n {
            if let Some(c) = cx.new_card(ids::card::JOY, 0) {
                joys.push(c);
            }
        }
        for &c in joys.iter() {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
        let room = MAX_HAND as i32 - cx.player.hand.len() as i32;
        let mut sads: ArrayVec<CardIdx, MAX_HAND> = ArrayVec::new();
        for _ in 0..room.max(0) {
            if let Some(c) = cx.new_card(ids::card::SADNESS, 0) {
                sads.push(c);
            }
        }
        for &c in sads.iter() {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
        Flow::Done
    }
});

listener!(PrecisionPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, target: Cid, _phase: u8) -> Flow {
        let v = cx.potion_power_var(potion, ids::power::AIM_POWER);
        cx.apply_power(ids::power::AIM_POWER, target, Dec::int(v as i64), PLAYER, NO);
        Flow::Done
    }
});
