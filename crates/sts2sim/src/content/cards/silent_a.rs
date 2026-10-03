//! Silent cards, first half of the `SILENT` pool array (ABRASIVE .. HIDDEN_DAGGERS) plus the Silent starter cards.
//! Ported from the decompiled `OnPlay` bodies; stats come from `gen_cards.rs`.

use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, RunResult, Targeting};
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

fn attack_all(cx: &mut Combat, p: &CardPlay) {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
}

fn apply_power_var(cx: &mut Combat, p: &CardPlay, power: u16, target: Cid) {
    let v = cx.card_power_var(p.card, power);
    cx.apply_power(power, target, Dec::int(v as i64), PLAYER, p.card);
}

/// `Rng.CombatTargets.NextItem(CombatState.HittableEnemies)`.
fn random_hittable_enemy(cx: &mut Combat) -> Option<Cid> {
    let h = cx.hittable_enemies();
    if h.is_empty() {
        return None;
    }
    let i = cx.rng.combat_targets.next_int_range(0, h.len() as i32) as usize;
    Some(h[i])
}

/// `CardCmd.Discard(cards)` then continue at `after` if a Sly auto-play suspended.
fn discard_then(cx: &mut Combat, cards: &[CardIdx], after: u8) -> Flow {
    if cx.discard_cards(cards, 0) == RunResult::Suspended {
        Flow::Suspend(after)
    } else {
        Flow::Done
    }
}

/// `FromHandForDiscard(prefs(.., n))` + `CardCmd.Discard`, the last effect of a card. Phase `1` handles the answer.
fn ask_discard_last(cx: &mut Combat, purpose: u16, n: u8) -> Flow {
    match cx.ask_hand(purpose, n, n, |_, _| true) {
        Ask::Resolved(cards) => discard_then(cx, cards.as_slice(), DONE),
        Ask::Pending => Flow::Suspend(1),
    }
}

fn answer_discard_last(cx: &mut Combat) -> Flow {
    let cards = cx.choice.cards;
    discard_then(cx, cards.as_slice(), DONE)
}

/// `CardPlaysFinished.Count(HappenedThisTurn && Type == Attack && Player == owner)`. (With the history-log port this is
/// `hist_count_this_turn(HKind::CardPlayFinished, |e| card_def(e.id).ctype == CardType::Attack)`.)
fn finished_attacks_this_turn(cx: &Combat) -> i32 {
    cx.hist.attacks_finished_this_turn as i32
}

// ---- starters ------------------------------------------------------------------------------------------------------------

listener!(StrikeSilent {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        Flow::Done
    }
});

listener!(DefendSilent {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        Flow::Done
    }
});

listener!(Neutralize {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::WEAK_POWER, p.target);
        Flow::Done
    }
});

listener!(Survivor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                block_from_var(cx, p);
                ask_discard_last(cx, ids::card::SURVIVOR, 1)
            }
            1 => answer_discard_last(cx),
            _ => Flow::Done,
        }
    }
});

// ---- A ---------------------------------------------------------------------------------------------------------------------

listener!(Abrasive {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::DEXTERITY_POWER, PLAYER);
        apply_power_var(cx, p, ids::power::THORNS_POWER, PLAYER);
        Flow::Done
    }
});

listener!(Accelerant {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::ACCELERANT);
        cx.apply_power(ids::power::ACCELERANT_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Accuracy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::ACCURACY_POWER, PLAYER);
        Flow::Done
    }
});

// Draw, then discard a card (Sly cards auto-play).
listener!(Acrobatics {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards(n, false);
                if cx.draw_pending() {
                    return Flow::Suspend(50); // a Stratagem prompt interrupted the draw
                }
                ask_discard_last(cx, ids::card::ACROBATICS, 1)
            }
            50 => ask_discard_last(cx, ids::card::ACROBATICS, 1),
            1 => answer_discard_last(cx),
            _ => Flow::Done,
        }
    }
});

listener!(Adrenaline {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Afterimage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::AFTERIMAGE_POWER, PLAYER);
        Flow::Done
    }
});

listener!(Anticipate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::DEXTERITY_POWER);
        cx.apply_power(ids::power::ANTICIPATE_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Assassinate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        apply_power_var(cx, p, ids::power::VULNERABLE_POWER, p.target);
        Flow::Done
    }
});

// ---- B ---------------------------------------------------------------------------------------------------------------------

listener!(Backflip {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Backstab {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p);
        Flow::Done
    }
});

// Creates Shivs (all at once) and enchants each with Inky (Weak on play).
listener!(BladeOfInk {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        let shivs = cx.create_shivs_in_hand(n);
        for &s in shivs.iter() {
            cx.enchant_card(s, ids::enchantment::INKY, 1);
        }
        Flow::Done
    }
});

listener!(BladeDance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

// Multiplayer only (AllAllies); single-player teammates = just the owner.
listener!(BladeSymphony {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

listener!(Blur {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        let v = cx.card_named_var(p.card, var_name::BLUR);
        cx.apply_power(ids::power::BLUR_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(BouncingFlask {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let reps = cx.card_var(p.card, VarKind::Repeat);
        for _ in 0..reps {
            let Some(e) = random_hittable_enemy(cx) else { continue };
            apply_power_var(cx, p, ids::power::POISON_POWER, e);
        }
        Flow::Done
    }
});

listener!(BubbleBubble {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.has_power(p.target, ids::power::POISON_POWER) {
            apply_power_var(cx, p, ids::power::POISON_POWER, p.target);
        }
        Flow::Done
    }
});

// Every non-X card in hand becomes free this turn; then NoDraw.
listener!(BulletTime {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        for &c in hand.iter() {
            if !cx.card_def(c).x_cost {
                cx.set_to_free_this_turn(c);
            }
        }
        cx.apply_power(ids::power::NO_DRAW_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(Burst {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::SKILLS);
        cx.apply_power(ids::power::BURST_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// ---- C ---------------------------------------------------------------------------------------------------------------------

// Discard the whole hand and draw that many cards (discard hooks / Sly wait until after the draw).
listener!(CalculatedGamble {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let hand = cx.player.hand;
                let n = hand.len() as i32;
                if cx.discard_cards(hand.as_slice(), n) == RunResult::Suspended {
                    Flow::Suspend(DONE)
                } else {
                    Flow::Done
                }
            }
            _ => Flow::Done,
        }
    }
});

listener!(CloakAndDagger {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

listener!(CorrosiveWave {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_named_var(p.card, var_name::CORROSIVE_WAVE);
        cx.apply_power(ids::power::CORROSIVE_WAVE_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer only (AnyAlly): can never be played in single player.
listener!(Concoct {});

// ---- D ---------------------------------------------------------------------------------------------------------------------

listener!(DaggerSpray {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents).hits(2));
        Flow::Done
    }
});

listener!(DaggerThrow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                attack_single(cx, p);
                cx.draw_cards(1, false);
                if cx.draw_pending() {
                    return Flow::Suspend(50);
                }
                ask_discard_last(cx, ids::card::DAGGER_THROW, 1)
            }
            50 => ask_discard_last(cx, ids::card::DAGGER_THROW, 1),
            1 => answer_discard_last(cx),
            _ => Flow::Done,
        }
    }
});

listener!(Dash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        attack_single(cx, p);
        Flow::Done
    }
});

listener!(DeadlyPoison {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::POISON_POWER, p.target);
        Flow::Done
    }
});

listener!(Deflect {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block_from_var(cx, p);
        Flow::Done
    }
});

listener!(DodgeAndRoll {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        // The power amount is the block actually gained (after Dexterity / Frail), not the raw var.
        let gained = block_from_var(cx, p);
        cx.apply_power(ids::power::BLOCK_NEXT_TURN_POWER, PLAYER, gained, PLAYER, p.card);
        Flow::Done
    }
});

// ---- E ---------------------------------------------------------------------------------------------------------------------

// `AttackCommand.CreateContextAsync` + raw `CreatureCmd.Damage` per round; each kill adds one more round.
listener!(EchoingSlash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let ctx = Attack::from_card(PLAYER, p.card, 0, Targeting::AllOpponents);
        cx.dispatch_g(hookbit::before_attack, |cx, me, l| l.before_attack(cx, me, &ctx));
        let mut rounds = 1;
        let mut hit = 0u8;
        let mut all = crate::engine::Results::new();
        while rounds > 0 {
            rounds -= 1;
            let targets = cx.hittable_enemies();
            let res = cx.damage(targets.as_slice(), Dec::int(dmg as i64), ValueProp::MOVE, PLAYER, p.card);
            rounds += res.iter().filter(|r| r.killed).count();
            for r in res.iter() {
                let mut r = *r;
                r.hit = hit; // `attackContext.AddHit(results)`: one result list per round
                all.push(r);
            }
            hit = hit.saturating_add(1);
        }
        // AttackContext disposal: `AfterAttack` sees every round's results
        cx.dispatch_after_attack(&ctx, &all);
        Flow::Done
    }
});

listener!(Envenom {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::ENVENOM_POWER, PLAYER);
        Flow::Done
    }
});

listener!(EscapePlan {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let drawn = if phase == 50 {
            cx.drawn_since_low(cx.cards[p.card as usize].counter[1])
        } else {
            let mark = cx.hist_mark();
            let d = cx.draw_cards_list(1, false);
            if cx.draw_pending() {
                cx.cards[p.card as usize].counter[1] = (mark & 0x7FFF) as i16;
                return Flow::Suspend(50); // a Stratagem prompt interrupted the draw
            }
            d
        };
        if let Some(c) = drawn.first() {
            if cx.card_def(c).ctype == CardType::Skill {
                block_from_var(cx, p);
            }
        }
        Flow::Done
    }
});

// Draw, and the drawn cards Retain this turn.
listener!(Expertise {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let drawn = if phase == 50 {
            cx.drawn_since_low(cx.cards[p.card as usize].counter[1])
        } else {
            let n = cx.card_var(p.card, VarKind::Cards);
            let mark = cx.hist_mark();
            let d = cx.draw_cards_list(n, false);
            if cx.draw_pending() {
                cx.cards[p.card as usize].counter[1] = (mark & 0x7FFF) as i16;
                return Flow::Suspend(50);
            }
            d
        };
        for &c in drawn.iter() {
            cx.apply_single_turn_retain(c);
        }
        Flow::Done
    }
});

listener!(Expose {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let amount = cx.card_named_var(p.card, var_name::POWER);
        let block = cx.cr(p.target).block;
        cx.lose_block(p.target, Dec::int(block as i64), PLAYER);
        cx.remove_power_by_id(p.target, ids::power::ARTIFACT_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(amount as i64), PLAYER, p.card);
        Flow::Done
    }
});

// ---- F ---------------------------------------------------------------------------------------------------------------------

// Multiplayer only (AnyAlly): can never be played in single player.
listener!(Fade {});

listener!(FanOfKnives {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::FAN_OF_KNIVES_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        // `CardsVar("Shivs", 4)`: a Named var (`DynamicVars["Shivs"]`), one `CreateInHand` call each.
        let n = cx.card_named_var(p.card, var_name::SHIVS);
        for _ in 0..n {
            cx.create_shivs_in_hand(1);
        }
        Flow::Done
    }
});

// Hits = number of Attack plays finished this turn (the current play has not finished yet).
listener!(Finisher {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * finished_attacks_this_turn(cx);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

// Multiplayer only constraint, but playable in single player (AnyEnemy): applies FlankingPower (x2 damage for every
// other player than the applier, i.e. no effect solo).
listener!(Flanking {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::FLANKING_POWER, p.target, Dec::int(2), PLAYER, p.card);
        Flow::Done
    }
});

// Hits = number of Skills in hand.
listener!(Flechettes {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let mut skills = 0;
        for &c in cx.player.hand.iter() {
            if cx.card_def(c).ctype == CardType::Skill {
                skills += 1;
            }
        }
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * skills;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

listener!(FlickFlack {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_all(cx, p);
        Flow::Done
    }
});

listener!(Sidestep {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.apply_power(ids::power::ENERGY_NEXT_TURN_POWER, PLAYER, Dec::int(e as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Footwork {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::DEXTERITY_POWER, PLAYER);
        Flow::Done
    }
});

// ---- G / H -----------------------------------------------------------------------------------------------------------------

listener!(GrandFinale {
    fn is_playable(&self, cx: &Combat, _card: CardIdx) -> bool {
        cx.player.draw.is_empty()
    }
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_all(cx, p);
        Flow::Done
    }
});

// Block, then make a non-Sly Skill in hand Sly this turn.
listener!(HandTrick {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                block_from_var(cx, p);
                match cx.ask_hand(ids::card::HAND_TRICK, 1, 1, |cx, c| cx.card_def(c).ctype == CardType::Skill && !cx.is_sly_this_turn(c)) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.apply_single_turn_sly(c);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.apply_single_turn_sly(c);
                }
                Flow::Done
            }
        }
    }
});

listener!(Haze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let poison = cx.card_power_var(p.card, ids::power::POISON_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::POISON_POWER, Dec::int(poison as i64), PLAYER, p.card);
        let weak = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(weak as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Discard 2 cards (Sly ones auto-play), then create Shivs (upgraded when this card is).
listener!(HiddenDaggers {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards) as u8;
                match cx.ask_hand(ids::card::HIDDEN_DAGGERS, n, n, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if cx.discard_cards(cards.as_slice(), 0) == RunResult::Suspended {
                            return Flow::Suspend(2);
                        }
                        make_shivs(cx, p)
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            1 => {
                let cards = cx.choice.cards;
                if cx.discard_cards(cards.as_slice(), 0) == RunResult::Suspended {
                    return Flow::Suspend(2);
                }
                make_shivs(cx, p)
            }
            _ => make_shivs(cx, p),
        }
    }
});

fn make_shivs(cx: &mut Combat, p: &CardPlay) -> Flow {
    let n = cx.card_named_var(p.card, var_name::SHIVS);
    let shivs = cx.create_shivs_in_hand(n);
    if cx.cards[p.card as usize].upgrade > 0 {
        for &s in shivs.iter() {
            cx.upgrade_in_combat(s);
        }
    }
    Flow::Done
}
