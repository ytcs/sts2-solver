// The Fool's tarot tokens (`TarotCards.All`, created by Mythic Form via `Combat::random_tarot`).
use crate::content::gen_cards::var_name;
use crate::content::gen_pools as gp;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, RunResult, Targeting, VALID_ORBS};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

const DONE: u8 = 99;

fn upgraded(cx: &Combat, c: CardIdx) -> bool {
    cx.cards[c as usize].upgrade > 0
}

fn room_in_hand(cx: &Combat) -> i32 {
    MAX_HAND as i32 - cx.player.hand.len() as i32
}

// `StableShuffle(CombatCardSelection).FirstOrDefault()` over the playable hand cards, else over the whole hand.
fn the_fool_pick(cx: &mut Combat) -> Option<CardIdx> {
    let mut playable: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
    for &c in cx.player.hand.iter() {
        if cx.card_keywords(c) & kw::UNPLAYABLE == 0 {
            playable.push(c);
        }
    }
    cx.stable_shuffle_cards(playable.as_mut_slice(), RngStream::CombatCardSelection);
    if let Some(c) = playable.first() {
        return Some(c);
    }
    let mut all: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
    for &c in cx.player.hand.iter() {
        all.push(c);
    }
    cx.stable_shuffle_cards(all.as_mut_slice(), RngStream::CombatCardSelection);
    all.first()
}

listener!(TheFool {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let mut i = phase as i32;
        // the loop bound reads DynamicVars.Cards each pass: an autoplayed Apotheosis upgrades The Fool mid-loop
        while i < cx.card_var(p.card, VarKind::Cards) {
            if let Some(c) = the_fool_pick(cx) {
                if cx.auto_play(c, NO, AutoPlayType::Default, false) == RunResult::Suspended {
                    return Flow::Suspend((i + 1) as u8);
                }
            }
            i += 1;
        }
        Flow::Done
    }
});

listener!(TheMagician {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        cx.add_orb_slots(n);
        for _ in 0..n {
            let i = cx.rng.combat_orbs.next_int_range(0, VALID_ORBS.len() as i32) as usize;
            cx.channel_orb(VALID_ORBS[i]);
        }
        Flow::Done
    }
});

listener!(TheHighPriestess {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(TheEmpress {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = Dec::int(cx.card_named_var(p.card, var_name::POWER) as i64);
        cx.apply_power(ids::power::WEAK_POWER, p.target, n, PLAYER, p.card);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, n, PLAYER, p.card);
        Flow::Done
    }
});

listener!(TheEmperor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(TheHierophant {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards).max(0) as usize;
        let cards = cx.get_distinct_for_combat(&gp::COLORLESS, n, |_| true);
        for &c in cards.iter() {
            // `EnergyCost.AddThisTurnOrUntilPlayed(-1)`
            cx.push_cost_mod(c, CostMod::new(-1, true, false, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(TheLovers {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        // `UnlockState.CharacterCardPools` in `ModelDb.AllCharacters` order, minus the owner's pool.
        let pools: [(u8, &[u16]); 6] = [(0, &gp::IRONCLAD), (1, &gp::SILENT), (4, &gp::REGENT), (3, &gp::NECROBINDER), (2, &gp::DEFECT), (5, &gp::FOOL)];
        let mut all: ArrayVec<u16, 512> = ArrayVec::new();
        for (ch, pool) in pools {
            if ch != cx.character {
                for &id in pool {
                    all.push(id);
                }
            }
        }
        let count = room_in_hand(cx).max(0) as usize;
        let cards = cx.get_for_combat(all.as_slice(), count);
        if upgraded(cx, p.card) {
            for &c in cards.iter() {
                cx.upgrade_in_combat(c);
            }
        }
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(TheChariot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
        Flow::Done
    }
});

listener!(Strength {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// `CardSelectCmd.FromHandForDiscard(1)`: a Curse is exhausted, anything else discarded; then draw.
listener!(TheHermit {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let picked = match phase {
            0 => match cx.ask_hand(ids::card::THE_HERMIT, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => cards,
                Ask::Pending => return Flow::Suspend(1),
            },
            1 => cx.choice.cards,
            _ => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards(n, false);
                return Flow::Done;
            }
        };
        for &c in picked.iter() {
            if cx.card_def(c).ctype == CardType::Curse {
                cx.exhaust_card(c, false);
            } else if cx.discard_cards(&[c], 0) == RunResult::Suspended {
                return Flow::Suspend(DONE);
            }
        }
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(WheelOfFortune {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        if phase != 0 {
            return Flow::Done;
        }
        let n = cx.card_var(p.card, VarKind::Cards);
        let hand = cx.player.hand;
        if !hand.is_empty() {
            if cx.discard_cards(hand.as_slice(), n) == RunResult::Suspended {
                return Flow::Suspend(DONE);
            }
        } else {
            cx.draw_cards(n, false);
        }
        Flow::Done
    }
});

listener!(Justice {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(TheHangedMan {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        cx.damage(&[PLAYER], Dec::int(loss as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE), PLAYER, p.card);
        cx.apply_power(ids::power::EXTRA_TURN_POWER, PLAYER, Dec::ONE, PLAYER, NO);
        Flow::Done
    }
});

listener!(Death {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        cx.apply_power(ids::power::DOOM_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Temperance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

// Exhausts the hand, then auto-plays as many cards from the top of the draw pile (The Devil+ upgrades each first).
listener!(TheDevil {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        if phase != 0 {
            return Flow::Done;
        }
        let hand = cx.player.hand;
        for &c in hand.iter() {
            cx.exhaust_card(c, false);
        }
        let up = upgraded(cx, p.card);
        if cx.auto_play_from_draw_pile_with(hand.len() as i32, CardPilePosition::Top, false, up) == RunResult::Suspended {
            return Flow::Suspend(1);
        }
        Flow::Done
    }
});

// CalculatedDamage = CalcBase + ExtraDamage * `RunState.TotalFloor`.
listener!(TheTower {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = crate::engine::calc_with(cx, p.card, cx.total_floor);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d.trunc(), Targeting::Single(p.target)));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_with(cx, card, cx.total_floor))
    }
});

listener!(TheStar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.apply_power(ids::power::ENERGY_NEXT_TURN_POWER, PLAYER, Dec::int(e as i64), PLAYER, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.apply_power(ids::power::DRAW_CARDS_NEXT_TURN_POWER, PLAYER, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(TheMoon {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let shivs = cx.create_shivs_in_hand(room_in_hand(cx));
        if upgraded(cx, p.card) {
            for &s in shivs.iter() {
                cx.upgrade_in_combat(s);
            }
        }
        Flow::Done
    }
});

listener!(TheSun {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        // `AddToCombatAndPreview<Soot>(owner, Discard, 1, creator: owner)`
        if cx.cr(PLAYER).is_alive() {
            if let Some(c) = cx.new_card(ids::card::SOOT, 0) {
                cx.add_generated_card_by(c, PileType::Discard, CardPilePosition::Bottom, true);
            }
        }
        Flow::Done
    }
});

listener!(Judgement {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let t = p.target;
        if cx.cr(t).is_alive() && cx.cr(t).hp() <= cx.card_named_var(p.card, var_name::THRESHOLD) {
            cx.set_current_hp(t, Dec::ZERO);
        }
        Flow::Done
    }
});

listener!(TheWorld {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let count = cx.card_var(p.card, VarKind::Cards).min(room_in_hand(cx)).max(0) as usize;
        let mut list: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in cx.player.discard.iter() {
            list.push(c);
        }
        cx.unstable_shuffle_cards(list.as_mut_slice(), RngStream::CombatCardSelection);
        let mut picked: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in list.iter().take(count) {
            picked.push(c);
        }
        cx.add_cards_to_pile(picked.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        Flow::Done
    }
});
