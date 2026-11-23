// The Fool's arrow cards (shots, retrievals, Aim and arrow powers).
use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{calc_extra_with, Ask, Attack, DamageResult, Results, Targeting, DMG_HOOK_CAP};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

fn shot(cx: &Combat, p: &CardPlay) -> Attack {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).from_arrow()
}

fn hit(cx: &mut Combat, p: &CardPlay) -> Results {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)))
}

fn apply_self(cx: &mut Combat, id: u16, amount: i32, p: &CardPlay) {
    cx.apply_power(id, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

/// `while (CreatureIsImpaledByPlayersArrow(target)) DislodgeArrow(target, ...)`.
fn dislodge_all_from(cx: &mut Combat, target: Cid, source: CardIdx) {
    while cx.is_impaled_by_player(target) {
        if !cx.tick() {
            return;
        }
        cx.dislodge_arrow_from(target, source);
    }
}

listener!(ArrowStab {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                hit(cx, p);
                if !cx.has_any_arrows() || cx.player.hand.len() >= MAX_HAND {
                    return Flow::Done;
                }
                let shoot = |cx: &Combat, c: CardIdx| cx.card_def(c).tags & tag::SHOOT != 0;
                match cx.ask_pile(ids::card::ARROW_STAB, PileType::Discard, 1, 1, shoot) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

listener!(BlossomShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        let card = p.card;
        cx.shoot_arrow_at(p.target, card, |cx| {
            let r = cx.execute_attack(&a);
            let n: i32 = r.iter().map(|x| x.total() + x.overkill).sum();
            cx.apply_power(ids::power::STRETCH_POWER, PLAYER, Dec::int(n as i64), PLAYER, card);
        });
        Flow::Done
    }
});

listener!(BowSmack {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let was_impaled = cx.is_impaled(p.target);
        hit(cx, p);
        if was_impaled {
            let e = cx.card_var(p.card, VarKind::Energy);
            cx.gain_energy(e);
        }
        Flow::Done
    }
});

listener!(DivineShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
});

listener!(EpicRetrieval {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        hit(cx, p);
        Flow::Done
    }
    fn after_arrow_dislodged(&self, cx: &mut Combat, me: Me, _target: Cid, _power_uid: u16, _arrow: u8, source: CardIdx) {
        if source != NO && source as u16 == me.idx {
            let e = cx.card_var(source, VarKind::Energy);
            cx.gain_energy(e);
        }
    }
});

listener!(ExplosiveArrows {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::EXPLOSIVE_ARROWS_POWER);
        apply_self(cx, ids::power::EXPLOSIVE_ARROWS_POWER, n, p);
        Flow::Done
    }
});

// `ModifyAimAmount` (x AimMultiplier) lives in `AimPower`.
listener!(HeavyShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
});

listener!(HobblingShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        if cx.shoot_arrow_attack(p.target, &a) {
            let n = Dec::int(cx.card_named_var(p.card, var_name::POWER) as i64);
            cx.apply_power(ids::power::WEAK_POWER, p.target, n, PLAYER, p.card);
            cx.apply_power(ids::power::VULNERABLE_POWER, p.target, n, PLAYER, p.card);
        }
        Flow::Done
    }
});

fn melodic_hits(cx: &Combat, card: CardIdx) -> i32 {
    calc_extra_with(cx, card, cx.melodies_this_turn()).trunc()
}

listener!(MelodicShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = melodic_hits(cx, p.card);
        let a = shot(cx, p).hits(hits);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(calc_extra_with(cx, card, cx.melodies_this_turn()))
    }
});

listener!(Nail {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.is_impaled(p.target) {
            hit(cx, p);
            let n = Dec::int(cx.card_named_var(p.card, var_name::POWER) as i64);
            cx.apply_power(ids::power::WEAK_POWER, p.target, n, PLAYER, p.card);
            cx.apply_power(ids::power::VULNERABLE_POWER, p.target, n, PLAYER, p.card);
        }
        Flow::Done
    }
});

listener!(Nock {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::AIM_POWER);
        apply_self(cx, ids::power::AIM_POWER, n, p);
        Flow::Done
    }
});

listener!(QuickShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
});

listener!(Quietus {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::QUIETUS_POWER);
        apply_self(cx, ids::power::QUIETUS_POWER, n, p);
        Flow::Done
    }
});

// Shoots first, then an `AttackContext` (BeforeAttack/AfterAttack around the impale) whose action damages the
// target (FromArrow), removes Aim and Quietus, and splashes the first result's total to the other hittable enemies
// (Move, Cap modifiers only).
listener!(RainShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cr(p.target).is_dead() {
            return Flow::Done;
        }
        let Some(arrow) = cx.try_shoot_arrow(p.card) else { return Flow::Done };
        let ctx_attack = Attack::from_card(PLAYER, p.card, 0, Targeting::AllOpponents);
        cx.dispatch_g(hookbit::before_attack, |cx, me, l| l.before_attack(cx, me, &ctx_attack));
        let mut hit_results: ArrayVec<DamageResult, 16> = ArrayVec::new();
        let mut hit_sizes: ArrayVec<u8, 2> = ArrayVec::new();
        let (card, target) = (p.card, p.target);
        let dmg = cx.card_var(card, VarKind::Damage);
        cx.impale_target(target, arrow, card, |cx| {
            let first = cx.damage(&[target], Dec::int(dmg as i64), ValueProp::MOVE.or(ValueProp::FROM_ARROW), PLAYER, card);
            for r in first.iter().take(16) {
                hit_results.push(*r);
            }
            hit_sizes.push(first.len() as u8);
            cx.remove_power_by_id(PLAYER, ids::power::AIM_POWER);
            cx.remove_power_by_id(PLAYER, ids::power::QUIETUS_POWER);
            if let Some(r) = first.first() {
                let mut others: ArrayVec<Cid, MAX_CREATURES> = ArrayVec::new();
                for &e in cx.enemies.iter() {
                    if e != target && cx.cr(e).is_alive() && cx.should_allow_hitting(e) {
                        others.push(e);
                    }
                }
                if !others.is_empty() {
                    let total = r.total() + r.overkill;
                    let mut rest = Results::new();
                    cx.damage_into_kinds(others.as_slice(), Dec::int(total as i64), ValueProp::MOVE, PLAYER, card, &mut rest, DMG_HOOK_CAP);
                    for r in rest.iter() {
                        if hit_results.len() < 16 {
                            hit_results.push(*r);
                        }
                    }
                    hit_sizes.push(rest.len() as u8);
                }
            }
        });
        cx.set_attack_results(hit_results.as_slice(), hit_sizes.as_slice());
        cx.dispatch_g(hookbit::after_attack, |cx, me, l| l.after_attack(cx, me, &ctx_attack));
        Flow::Done
    }
});

listener!(RapidShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        let a = shot(cx, p).hits(n);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
});

listener!(Recall {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                if !cx.is_impaled_by_player(p.target) {
                    return Flow::Done;
                }
                dislodge_all_from(cx, p.target, p.card);
                match cx.ask_pile(ids::card::RECALL, PileType::Discard, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.move_card(c, PileType::Draw, CardPilePosition::Top);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Draw, CardPilePosition::Top);
                }
                Flow::Done
            }
        }
    }
});

listener!(RepeatShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.x_value(p.card);
        if x > 0 {
            let a = shot(cx, p).hits(x);
            cx.shoot_arrow_attack(p.target, &a);
        }
        Flow::Done
    }
});

listener!(SerratedArrows {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::SERRATED_ARROWS_POWER);
        apply_self(cx, ids::power::SERRATED_ARROWS_POWER, n, p);
        Flow::Done
    }
});

listener!(SkyShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
});

listener!(TrickShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = shot(cx, p);
        cx.shoot_arrow_attack(p.target, &a);
        Flow::Done
    }
    fn get_result_location_for_card_play(&self, cx: &Combat, _me: Me, _card: CardIdx, base: CardLocation) -> CardLocation {
        if base.pile == PileType::Discard && cx.has_any_arrows() { CardLocation { pile: PileType::Hand, ..base } } else { base }
    }
});

listener!(Uproot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.is_impaled_by_player(p.target) {
            dislodge_all_from(cx, p.target, p.card);
            let n = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
            cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});

listener!(WhistlingArrows {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn after_arrow_shot(&self, cx: &mut Combat, me: Me, arrow: u8, _source: CardIdx) {
        let c = me.idx as CardIdx;
        if arrow != NO && cx.card_pile_type(c) == PileType::Discard {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

listener!(Wrest {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.is_impaled_by_player(p.target) {
            dislodge_all_from(cx, p.target, p.card);
            let n = cx.card_var(p.card, VarKind::Cards);
            cx.draw_cards(n, false);
        }
        Flow::Done
    }
});

// Multiplayer-only (block to the other players).
listener!(CoveringFire {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::COVERING_FIRE_POWER);
        apply_self(cx, ids::power::COVERING_FIRE_POWER, n, p);
        Flow::Done
    }
});

// Applies ArcheryPower by the card's AimPower var.
listener!(Archery {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::AIM_POWER);
        apply_self(cx, ids::power::ARCHERY_POWER, n, p);
        Flow::Done
    }
});

listener!(Yah {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::YAH_POWER, n, p);
        Flow::Done
    }
});

listener!(DislikeYou {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        hit(cx, p);
        let n = cx.card_power_var(p.card, ids::power::DISLIKE_YOU_POWER);
        cx.apply_power(ids::power::DISLIKE_YOU_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Honing {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::HONING_POWER);
        apply_self(cx, ids::power::HONING_POWER, n, p);
        Flow::Done
    }
});
