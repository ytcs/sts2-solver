use crate::engine::calc_with;
use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, HKind, RunResult, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

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

fn calculated(cx: &Combat, c: CardIdx, extra: VarKind, multiplier: i32) -> i32 {
    cx.card_var(c, VarKind::CalcBase) + cx.card_var(c, extra) * multiplier
}

pub(crate) fn discard_suspended(cx: &mut Combat, cards: &[CardIdx]) -> bool {
    cx.discard_cards(cards, 0) == RunResult::Suspended
}

fn discard_then(cx: &mut Combat, cards: &[CardIdx], after: u8) -> Flow {
    if discard_suspended(cx, cards) {
        Flow::Suspend(after)
    } else {
        Flow::Done
    }
}

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
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return;
        }
        let n = cx.hist.skills_finished_this_turn as i32;
        cx.add_cost_this_turn(card, -n, false);
    }
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

listener!(Skewer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let x = cx.x_value(p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(x));
        Flow::Done
    }
});

listener!(Ricochet {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Random).hits(hits));
        Flow::Done
    }
});

listener!(PreciseCut {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut in_hand = cx.player.hand.len() as i32;
        if cx.card_pile_type(p.card) == PileType::Hand {
            in_hand -= 1;
        }
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, -in_hand);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        let mut in_hand = cx.player.hand.len() as i32;
        if cx.card_pile_type(card) == PileType::Hand {
            in_hand -= 1;
        }
        Some(calc_with(cx, card, -in_hand))
    }
});

listener!(MementoMori {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hist_count_this_turn(HKind::CardDiscarded, |_| true) as i32;
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, n);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        Some(calc_with(cx, card, cx.hist_count_this_turn(HKind::CardDiscarded, |_| true) as i32))
    }
});

listener!(Murder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hist_total(HKind::CardDrawn) as i32;
        let dmg = calculated(cx, p.card, VarKind::ExtraDamage, n);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        Some(calc_with(cx, card, cx.hist_total(HKind::CardDrawn) as i32))
    }
});

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

listener!(LeadingStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        let n = cx.card_named_var(p.card, var_name::SHIVS);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

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
            let _ = cx.auto_play(c, p.target, AutoPlayType::Default, false);
        }
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        let shivs = cx.player.exhaust.iter().filter(|&&c| cx.card_def(c).tags & tag::SHIV != 0).count() as i32;
        Some(crate::engine::calc_extra_with(cx, card, shivs))
    }
});

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
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        let poison: i32 = cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).map(|&e| cx.power_amount(e, ids::power::POISON_POWER)).sum();
        Some(crate::engine::calc_extra_with(cx, card, poison))
    }
});

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

listener!(Prepared {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards_nosuspend(n, false);
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
