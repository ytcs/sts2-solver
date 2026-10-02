//! Silent cards, second half of the `SILENT` pool array (INFINITE_BLADES .. WRAITH_FORM). Ported from the decompiled
//! `OnPlay` / hook bodies; stats come from `gen_cards.rs`.
//!
//! NOT in this file (ported by the first-half Silent file because the starter deck needs them): `StrikeSilent`,
//! `Neutralize`, `Survivor`.

use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, HKind, RunResult, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- shared helpers -----------------------------------------------------------------------------------------------------

/// Phase that simply finishes (used after a discard whose Sly auto-play suspended).
const DONE: u8 = 99;

fn block_from_var(cx: &mut Combat, p: &CardPlay) -> Dec {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card)
}

fn attack_single(cx: &mut Combat, p: &CardPlay) {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
}

fn apply_power_var(cx: &mut Combat, p: &CardPlay, power: u16, target: Cid) {
    let v = cx.card_power_var(p.card, power);
    cx.apply_power(power, target, Dec::int(v as i64), PLAYER, p.card);
}

/// `CalculatedDamageVar.Calculate` / `CalculatedBlockVar.Calculate`: `CalculationBase + CalculationExtra * multiplier`.
fn calculated(cx: &Combat, c: CardIdx, extra: VarKind, multiplier: i32) -> i32 {
    cx.card_var(c, VarKind::CalcBase) + cx.card_var(c, extra) * multiplier
}

/// `CardCmd.Discard(cards)`: true if a Sly auto-play it triggered suspended on a decision (the one place that adapts to
/// the engine's `discard_cards` return convention).
pub(crate) fn discard_suspended(cx: &mut Combat, cards: &[CardIdx]) -> bool {
    cx.discard_cards(cards, 0) == RunResult::Suspended
}

/// `CardCmd.Discard(cards)` then continue at `after` if a Sly auto-play suspended.
fn discard_then(cx: &mut Combat, cards: &[CardIdx], after: u8) -> Flow {
    if discard_suspended(cx, cards) {
        Flow::Suspend(after)
    } else {
        Flow::Done
    }
}

// ---- A: powers that just apply a power --------------------------------------------------------------------------------------

listener!(InfiniteBlades {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::INFINITE_BLADES_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(MasterPlanner {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::MASTER_PLANNER_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(NoxiousFumes {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::POISON_PER_TURN);
        cx.apply_power(ids::power::NOXIOUS_FUMES_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(PhantomBlades {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::PHANTOM_BLADES_POWER, PLAYER);
        Flow::Done
    }
});

listener!(SerpentForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::SERPENT_FORM_POWER, PLAYER);
        Flow::Done
    }
});

listener!(Shadowmeld {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::POWER);
        cx.apply_power(ids::power::SHADOWMELD_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Sneaky {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::SNEAKY_POWER, PLAYER);
        Flow::Done
    }
});

listener!(Speedster {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::SPEEDSTER_POWER, PLAYER);
        Flow::Done
    }
});

listener!(ToolsOfTheTrade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::TOOLS_OF_THE_TRADE_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(Tracking {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::TRACKING_POWER, PLAYER, Dec::int(50), PLAYER, p.card);
        Flow::Done
    }
});

listener!(WellLaidPlans {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::WELL_LAID_PLANS_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(WraithForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::INTANGIBLE_POWER, PLAYER);
        apply_power_var(cx, p, ids::power::WRAITH_FORM_POWER, PLAYER);
        Flow::Done
    }
});

// ---- B: simple attacks -----------------------------------------------------------------------------------------------------

listener!(Slice {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        Flow::Done
    }
});

listener!(Pinpoint {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        Flow::Done
    }
    // Entering combat (not as a clone): costs 1 less this turn for every Skill already played this turn.
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return;
        }
        let n = cx.hist.skills_finished_this_turn as i32;
        cx.add_cost_this_turn(card, -n, false);
    }
    // Every Skill played makes it 1 cheaper this turn.
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Skill {
            cx.add_cost_this_turn(me.idx as CardIdx, -1, false);
        }
    }
});

listener!(Pounce {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        cx.apply_power(ids::power::FREE_SKILL_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(Predator {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        cx.apply_power(ids::power::DRAW_CARDS_NEXT_TURN_POWER, PLAYER, Dec::int(2), PLAYER, p.card);
        Flow::Done
    }
});

listener!(PoisonedStab {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::POISON_POWER, p.target);
        Flow::Done
    }
});

listener!(SuckerPunch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::WEAK_POWER, p.target);
        Flow::Done
    }
});

listener!(Suppress {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::WEAK_POWER, p.target);
        Flow::Done
    }
});

listener!(Strangle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::STRANGLE_POWER, p.target);
        Flow::Done
    }
});

// X hits.
listener!(Skewer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let x = cx.x_value(p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(x));
        Flow::Done
    }
});

// Random enemy per hit.
listener!(Ricochet {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Random).hits(hits));
        Flow::Done
    }
});

// 13 (+3) damage, minus 2 for every other card in hand.
listener!(PreciseCut {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        // The card sits in the Play pile, so the whole hand counts.
        let mut in_hand = cx.player.hand.len() as i32;
        if cx.card_pile_type(p.card) == PileType::Hand {
            in_hand -= 1;
        }
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, -in_hand);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

// Base + extra per card discarded this turn.
listener!(MementoMori {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hist_count_this_turn(HKind::CardDiscarded, |_| true) as i32;
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, n);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

// Base + extra per card drawn this combat.
listener!(Murder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hist_total(HKind::CardDrawn) as i32;
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, n);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

// Attack, then (if it killed a creature that triggers Fatal) a card reward + a marker power (the reward is outside combat).
listener!(TheHunt {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let fatal = cx.all_powers_trigger_fatal(p.target);
        let results = cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        if fatal && results.iter().any(|r| r.killed) {
            cx.apply_power(ids::power::THE_HUNT_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        }
        Flow::Done
    }
});

// Attack, then 2 Shivs into the hand (one `CreateInHand` call each). `CardsVar("Shivs", 2)`: the generated table has
// the unnamed-var parser's 0 for it, so the count is spelled out here.
listener!(LeadingStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        for _ in 0..2 {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

// ---- C: skills with a target -------------------------------------------------------------------------------------------------

listener!(LegSweep {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        apply_power_var(cx, p, ids::power::WEAK_POWER, p.target);
        Flow::Done
    }
});

listener!(Snakebite {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::POISON_POWER, p.target);
        Flow::Done
    }
});

// X: -X Strength and +X Weak on the target (+1 when upgraded).
listener!(Malaise {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut x = cx.x_value(p.card);
        if cx.cards[p.card as usize].upgrade > 0 {
            x += 1;
        }
        cx.apply_power(ids::power::STRENGTH_POWER, p.target, Dec::int(-(x as i64)), PLAYER, p.card);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(x as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Auto-play every Shiv in the exhaust pile (upgrading them first when upgraded) at the target.
listener!(KnifeTrap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut shivs: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.exhaust.iter() {
            if cx.card_def(c).tags & tag::SHIV != 0 {
                shivs.push(c);
            }
        }
        let upgraded = cx.cards[p.card as usize].upgrade > 0;
        for &c in shivs.iter() {
            if upgraded {
                cx.upgrade_in_combat(c);
            }
            // (Shivs never raise a decision, so the nested play never suspends.)
            let _ = cx.auto_play(c, p.target, AutoPlayType::Default, false);
        }
        Flow::Done
    }
});

// ---- D: block / draw / energy skills ----------------------------------------------------------------------------------------

listener!(Untouchable {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        Flow::Done
    }
});

listener!(Reflex {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Tactician {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

// Block = base (0) + extra (1) * total Poison on living enemies.
listener!(Mirage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut poison = 0;
        for &e in cx.enemies.iter() {
            if cx.cr(e).is_alive() {
                poison += cx.power_amount(e, ids::power::POISON_POWER);
            }
        }
        let block = calculated(cx, p.card, VarKind::CalcExtra, poison);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

// Poison on every hittable enemy, then trigger each enemy's Poison once.
listener!(Outbreak {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::POISON_POWER);
        let targets = cx.hittable_enemies();
        for &t in targets.iter() {
            cx.apply_power(ids::power::POISON_POWER, t, Dec::int(v as i64), PLAYER, p.card);
        }
        let targets = cx.hittable_enemies();
        for &t in targets.iter() {
            if let Some(uid) = cx.cr(t).power(ids::power::POISON_POWER).map(|pw| pw.uid) {
                poison_trigger(cx, t, uid);
            }
        }
        Flow::Done
    }
});

/// `PoisonPower.Trigger()`: `min(Amount, 1 + Accelerant of living opponents)` ticks of (current amount) unblockable damage,
/// each followed by a decrement while the owner lives.
fn poison_trigger(cx: &mut Combat, owner: Cid, uid: u16) {
    let Some(i) = cx.power_idx(owner, uid) else { return };
    let amount = cx.cr(owner).powers[i].amount;
    let owner_side = cx.cr(owner).side;
    let mut accelerant = 0;
    for c in 0..MAX_CREATURES {
        let cr = cx.cr(c as Cid);
        if cr.in_combat && cr.is_alive() && cr.side != owner_side {
            accelerant += cr.power_amount(ids::power::ACCELERANT_POWER);
        }
    }
    let iterations = amount.min(1 + accelerant);
    for _ in 0..iterations {
        let Some(i) = cx.power_idx(owner, uid) else { break };
        let cur = cx.cr(owner).powers[i].amount;
        cx.damage(&[owner], Dec::int(cur as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
        if cx.cr(owner).is_alive() {
            cx.decrement_power(owner, uid);
        }
    }
}

// Strength loss (temporary) on every hittable enemy.
listener!(PiercingWail {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::STRENGTH_LOSS);
        let targets = cx.hittable_enemies();
        for &t in targets.iter() {
            cx.apply_power(ids::power::PIERCING_WAIL_POWER, t, Dec::int(v as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});

// ---- E: card-pile effects ------------------------------------------------------------------------------------------------------

// Draw, then discard the same number of cards (choice).
listener!(Prepared {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards(n, false);
                match cx.ask_hand(ids::card::PREPARED, n as u8, n as u8, |_, _| true) {
                    Ask::Resolved(cards) => discard_then(cx, cards.as_slice(), DONE),
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            1 => {
                let cards = cx.choice.cards;
                discard_then(cx, cards.as_slice(), DONE)
            }
            _ => Flow::Done,
        }
    }
});

// Choose a card from the hand: it is duplicated into the hand at the start of the next turn (Nightmare power, 3 copies).
listener!(Nightmare {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let selected = match phase {
            0 => match cx.ask_hand(ids::card::NIGHTMARE, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => cards.first(),
                Ask::Pending => return Flow::Suspend(1),
            },
            _ => cx.choice.cards.first(),
        };
        if let Some(sel) = selected {
            if let Some(uid) = cx.apply_power(ids::power::NIGHTMARE_POWER, PLAYER, Dec::int(3), PLAYER, p.card) {
                // NightmarePower.SetSelectedCard: a clone with its affliction cleared, parked in the arena (no pile).
                if let Some(clone) = cx.clone_card(sel) {
                    cx.cards[clone as usize].affliction = 0;
                    cx.cards[clone as usize].affliction_amount = 0;
                    if let Some(i) = cx.power_idx(PLAYER, uid) {
                        cx.cr_mut(PLAYER).powers[i].aux = clone as i32 + 1;
                    }
                }
            }
        }
        Flow::Done
    }
});

// Discard the whole hand, then Double Damage at the start of the next turn.
listener!(ShadowStep {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let hand = cx.player.hand;
                if discard_suspended(cx, hand.as_slice()) {
                    return Flow::Suspend(1);
                }
                cx.apply_power(ids::power::SHADOW_STEP_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
                Flow::Done
            }
            1 => {
                cx.apply_power(ids::power::SHADOW_STEP_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
                Flow::Done
            }
            _ => Flow::Done,
        }
    }
});

// Discard the whole hand, then create that many Shivs (upgraded when this card is).
// `counter[0]` carries the hand size across a Sly-triggered suspension.
listener!(StormOfSteel {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let n;
        match phase {
            0 => {
                let hand = cx.player.hand;
                n = hand.len() as i32;
                cx.cards[p.card as usize].counter[0] = n as i16;
                if discard_suspended(cx, hand.as_slice()) {
                    return Flow::Suspend(1);
                }
            }
            1 => n = cx.cards[p.card as usize].counter[0] as i32,
            _ => return Flow::Done,
        }
        cx.cards[p.card as usize].counter[0] = 0;
        let shivs = cx.create_shivs_in_hand(n);
        if cx.cards[p.card as usize].upgrade > 0 {
            for &s in shivs.iter() {
                cx.upgrade_in_combat(s);
            }
        }
        Flow::Done
    }
});

// Create Shivs; every play makes this card 1 cheaper for the rest of the combat.
listener!(UpMySleeve {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        cx.add_cost_this_combat(p.card, -1, false);
        Flow::Done
    }
});
