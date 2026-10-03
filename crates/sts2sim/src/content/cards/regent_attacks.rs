//! Regent attack cards (bodies follow the decompiled `Models/Cards/<Class>.cs` `OnPlay`).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, HKind, Results, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn dmg(cx: &Combat, c: CardIdx) -> i32 {
    cx.card_base_damage(c)
}
/// `DamageCmd.Attack(Damage).FromCard(card).Targeting(target)`.
fn single(cx: &mut Combat, p: &CardPlay) -> Results {
    let d = dmg(cx, p.card);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)))
}
/// `...TargetingAllOpponents(CombatState)`.
fn all(cx: &mut Combat, p: &CardPlay) -> Results {
    let d = dmg(cx, p.card);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::AllOpponents))
}
fn apply_var(cx: &mut Combat, p: &CardPlay, power: u16, target: Cid) {
    let n = cx.card_power_var(p.card, power);
    cx.apply_power(power, target, Dec::int(n as i64), PLAYER, p.card);
}

listener!(StrikeRegent {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
});

// Weak then Vulnerable on the target after the hit.
listener!(FallingStar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        apply_var(cx, p, ids::power::WEAK_POWER, p.target);
        apply_var(cx, p, ids::power::VULNERABLE_POWER, p.target);
        Flow::Done
    }
});

listener!(Comet {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        apply_var(cx, p, ids::power::WEAK_POWER, p.target);
        apply_var(cx, p, ids::power::VULNERABLE_POWER, p.target);
        Flow::Done
    }
});

listener!(GammaBlast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        apply_var(cx, p, ids::power::WEAK_POWER, p.target);
        apply_var(cx, p, ids::power::VULNERABLE_POWER, p.target);
        Flow::Done
    }
});

listener!(MeteorShower {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        all(cx, p);
        let w = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(w as i64), PLAYER, p.card);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::VULNERABLE_POWER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Devastate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
});

listener!(AstralPulse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::AllOpponents).hits(2));
        Flow::Done
    }
});

listener!(CelestialMight {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

listener!(SevenStars {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::AllOpponents).hits(hits));
        Flow::Done
    }
});

// Draws Cards next turn after the hit.
listener!(GuidingStar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.apply_power(ids::power::DRAW_CARDS_NEXT_TURN_POWER, PLAYER, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(SolarStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let n = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(n);
        Flow::Done
    }
});

listener!(ShiningStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let n = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(n);
        Flow::Done
    }
    // A played Shining Strike goes to the TOP of the draw pile instead of the discard pile.
    fn get_result_location_for_card_play(&self, _cx: &Combat, _me: Me, _card: CardIdx, base: CardLocation) -> CardLocation {
        if base.pile == PileType::Discard { CardLocation::new(PileType::Draw, CardPilePosition::Top) } else { base }
    }
});

// Gains Stars if the hit killed the target.
listener!(KnockoutBlow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let r = single(cx, p);
        if r.iter().any(|x| x.killed) {
            let n = cx.card_var(p.card, VarKind::Stars);
            cx.gain_stars(n);
        }
        Flow::Done
    }
});

listener!(Hegemony {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.apply_power(ids::power::ENERGY_NEXT_TURN_POWER, PLAYER, Dec::int(e as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Each time it is drawn its cost drops by 1 for the rest of the combat.
listener!(KinglyKick {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if card as u16 == me.idx {
            // EnergyCost.AddThisCombat(-1): combat-long relative modifier (folded into one entry).
            let c = &mut cx.cards[card as usize];
            if let Some(last) = c.mods.as_mut_slice().last_mut() {
                if last.relative && !last.reduce_only && last.expire == 0 {
                    last.amount = last.amount.saturating_sub(1);
                    return;
                }
            }
            c.mods.push(CostMod { amount: -1, relative: true, reduce_only: false, expire: 0 });
        }
    }
});

// Each time it is drawn its damage grows by Increase (kept in `counter[0]`).
listener!(KinglyPunch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if card as u16 == me.idx {
            let inc = cx.card_named_var(card, crate::content::gen_cards::var_name::INCREASE);
            cx.blade_add_damage(card, inc);
        }
    }
});

// Exhausted copies replay themselves at the start of every turn.
listener!(Bombardment {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
    fn after_auto_pre_play_phase_entered_early(&self, cx: &mut Combat, me: Me) {
        let c = me.idx as CardIdx;
        if cx.card_pile_type(c) == PileType::Exhaust {
            let _ = cx.auto_play(c, NO, AutoPlayType::Default, false);
        }
    }
});

listener!(CollisionCourse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        cx.create_regent_cards_in_hand(ids::card::DEBRIS, 1);
        Flow::Done
    }
});

listener!(CrashLanding {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        all(cx, p);
        let n = MAX_HAND as i32 - cx.player.hand.len() as i32;
        cx.create_regent_cards_in_hand(ids::card::DEBRIS, n);
        Flow::Done
    }
});

// 5 + Extra * (cards with a star cost anywhere in the player's combat piles, itself included).
listener!(CrescentSpear {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let base = cx.card_var(p.card, VarKind::CalcBase);
        let extra = cx.card_var(p.card, VarKind::ExtraDamage);
        let all_cards = cx.player_combat_cards();
        let n = all_cards.iter().filter(|&&c| cx.card_def(c).star_cost != -1).count() as i32;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, base + extra * n, Targeting::Single(p.target)));
        Flow::Done
    }
    // `CalculatedDamageVar.Calculate(target)` read generically (Thrash exhausting this card).
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        let all_cards = cx.player_combat_cards();
        let n = if cx.in_progress { all_cards.iter().filter(|&&c| cx.card_def(c).star_cost != -1).count() as i64 } else { 0 };
        Some(Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * n))
    }
});

// 5 + Extra * (cards the player has generated this combat).
listener!(Supermassive {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let base = cx.card_var(p.card, VarKind::CalcBase);
        let extra = cx.card_var(p.card, VarKind::ExtraDamage);
        let n = cx.hist_log.generated_by_player as i32;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, base + extra * n, Targeting::Single(p.target)));
        Flow::Done
    }
    // `CalculatedDamageVar.Calculate(target)` read generically (Thrash exhausting this card).
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        let n = if cx.in_progress { cx.hist_log.generated_by_player as i64 } else { 0 };
        Some(Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * n))
    }
});

// Hits = Skills played this turn.
listener!(LunarBlast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        // CardPlaysFinished (Skill, this turn): a nested play (Beat Down) is not finished yet
        let hits = cx.hist.skills_finished_this_turn as i32;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

// Hits = stars gained this turn.
listener!(Radiate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        let hits = cx.stars_gained_this_turn();
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::AllOpponents).hits(hits));
        Flow::Done
    }
});

// X-star cost: random targets, one hit per star spent.
listener!(Stardust {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        let hits = cx.resolve_star_x_value(p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Random).hits(hits));
        Flow::Done
    }
});

// X-cost: hits = X (doubled when X >= Energy var).
listener!(HeavenlyDrill {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = dmg(cx, p.card);
        let mut n = cx.x_value(p.card);
        if n >= cx.card_var(p.card, VarKind::Energy) {
            n *= 2;
        }
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)).hits(n));
        Flow::Done
    }
});

// Hit, then a colorless card in hand is cloned Repeat times into the hand.
listener!(HeirloomHammer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                single(cx, p);
                let colorless = |cx2: &Combat, c: CardIdx| -> bool { is_colorless(cx2.cards[c as usize].id) };
                match cx.ask_hand(ids::card::HEIRLOOM_HAMMER, 1, 1, colorless) {
                    crate::engine::Ask::Resolved(cards) => {
                        clone_selection(cx, p, cards.first());
                        Flow::Done
                    }
                    crate::engine::Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let sel = cx.choice.cards.first();
                clone_selection(cx, p, sel);
                Flow::Done
            }
        }
    }
});

// `CardModel.VisualCardPool.IsColorless`: the Colorless, Event and Token pools, except the Event cards that override
// `VisualCardPool` to look like a character's card (Caltrops, Clash, Dual Wield, Distraction, Hello World, Entrench, Rip and Tear,
// Rebound, Stack, Outmaneuver).
fn is_colorless(id: u16) -> bool {
    use crate::content::gen_pools as p;
    use crate::ids::card as c;
    const EVENT_WITH_CHARACTER_LOOK: [u16; 10] =
        [c::CALTROPS, c::CLASH, c::DUAL_WIELD, c::DISTRACTION, c::HELLO_WORLD, c::ENTRENCH, c::RIP_AND_TEAR, c::REBOUND, c::STACK, c::OUTMANEUVER];
    if EVENT_WITH_CHARACTER_LOOK.contains(&id) {
        return false;
    }
    p::COLORLESS.contains(&id) || p::EVENT.contains(&id) || p::TOKEN.contains(&id)
}

fn clone_selection(cx: &mut Combat, p: &CardPlay, sel: Option<CardIdx>) {
    if let Some(s) = sel {
        let n = cx.card_var(p.card, VarKind::Repeat);
        for _ in 0..n {
            if let Some(c) = cx.clone_card(s) {
                cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
        }
    }
}

// Forge amount grows with the player's earlier powered hits on the target this turn.
listener!(BeatIntoShape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let r = single(cx, p);
        let base = cx.card_var(p.card, VarKind::CalcBase);
        let extra = cx.card_var(p.card, VarKind::CalcExtra);
        let total_hits = cx.hist_count_this_turn(HKind::DamageReceived, |e| e.actor == p.target && e.other == PLAYER && ValueProp(e.props).is_powered()) as i32;
        // CalculatedForge = base + extra * total hits; then minus extra * this attack's own results.
        let amount = base + extra * total_hits - r.len() as i32 * extra;
        cx.forge(amount);
        Flow::Done
    }
});

listener!(WroughtInWar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});

// Strength loss until end of turn on every enemy that was hittable before the attack.
listener!(CrushUnder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let enemies = cx.hittable_enemies();
        all(cx, p);
        let n = cx.card_named_var(p.card, crate::content::gen_cards::var_name::STRENGTH_LOSS);
        for &e in enemies.iter() {
            cx.apply_power(ids::power::CRUSH_UNDER_POWER, e, Dec::int(n as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});

listener!(DyingStar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let enemies = cx.hittable_enemies();
        all(cx, p);
        let n = cx.card_named_var(p.card, crate::content::gen_cards::var_name::STRENGTH_LOSS);
        for &e in enemies.iter() {
            cx.apply_power(ids::power::DYING_STAR_POWER, e, Dec::int(n as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});
