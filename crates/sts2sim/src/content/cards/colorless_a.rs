//! COLORLESS pool cards, part A: attacks, blocks, draw/energy, simple power cards. (Cards needing deeper engine support
//! live in `colorless_b.rs`.)

use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::{CardDef, VarKind};
use crate::engine::{Ask, Attack, HKind, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- helpers ----------------------------------------------------------------------------------------------------------

/// `DamageCmd.Attack(card damage var).FromCard(card, play)`.
fn atk(cx: &Combat, p: &CardPlay, t: Targeting) -> Attack {
    Attack::from_card(PLAYER, p.card, cx.card_var(p.card, VarKind::Damage), t)
}

fn single(cx: &mut Combat, p: &CardPlay) -> crate::engine::Results {
    let a = atk(cx, p, Targeting::Single(p.target));
    cx.execute_attack(&a)
}

/// `CreatureCmd.GainBlock(Owner.Creature, DynamicVars.Block, cardPlay)`.
fn block(cx: &mut Combat, p: &CardPlay) {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
}

/// `PowerCmd.Apply<T>(ctx, Owner.Creature, amount, Owner.Creature, this)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) -> Option<u16> {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card)
}

fn draw(cx: &mut Combat, p: &CardPlay) {
    let n = cx.card_var(p.card, VarKind::Cards);
    cx.draw_cards(n, false);
}

// ---- Anointed: pull Rare cards from the draw pile into the hand ----------------------------------------------------------
listener!(Anointed {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let room = MAX_HAND as i32 - cx.player.hand.len() as i32;
        let mut rares: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.draw.iter() {
            if cx.card_def(c).rarity == CardRarity::Rare {
                rares.push(c);
            }
        }
        // `TakeRandom(count, CombatCardSelection)` = UnstableShuffle, then Take.
        cx.unstable_shuffle_cards(rares.as_mut_slice(), RngStream::CombatCardSelection);
        let n = room.max(0) as usize;
        let picked: crate::util::ArrayVec<CardIdx, MAX_CARDS> = {
            let mut v = crate::util::ArrayVec::new();
            for &c in rares.iter().take(n) {
                v.push(c);
            }
            v
        };
        cx.add_cards_to_pile(picked.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        Flow::Done
    }
});

listener!(Automation {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::AUTOMATION_POWER, e, p);
        Flow::Done
    }
});

// Multiplayer only: shares block with the other players (none in single player).
listener!(BeaconOfHope {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::BEACON_OF_HOPE_POWER, 1, p);
        Flow::Done
    }
});

// Multiplayer only (AnyAlly): gives the target player energy.
listener!(BelieveInYou {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

// Bolas returns to the hand at the start of the next turn if it was played last player turn
// (`CardPlaysFinished.Any(e => e.HappenedLastPlayerTurn && e.CardPlay.Card == this)`; the canonical history logs the
// play start, which only differs for a play that never finished).
fn return_if_played_last_turn(cx: &mut Combat, me: Me) {
    let c = me.idx as CardIdx;
    if cx.hist_any_last_player_turn(HKind::CardPlayStarted, |e| e.card == c) && cx.card_pile_type(c) != PileType::Hand {
        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    }
}

listener!(Bolas {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        return_if_played_last_turn(cx, me);
    }
});

listener!(Calamity {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::CALAMITY_POWER, 1, p);
        Flow::Done
    }
});

// Multiplayer only (AnyAlly).
listener!(Coordinate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        let t = if p.target == NO { PLAYER } else { p.target };
        cx.apply_power(ids::power::COORDINATE_POWER, t, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(DarkShackles {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::STRENGTH_LOSS);
        cx.apply_power(ids::power::DARK_SHACKLES_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(DramaticEntrance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = atk(cx, p, Targeting::AllOpponents);
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(Equilibrium {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let n = cx.card_named_var(p.card, var_name::EQUILIBRIUM);
        apply_self(cx, ids::power::RETAIN_HAND_POWER, n, p);
        Flow::Done
    }
});

listener!(EternalArmor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::PLATING_POWER);
        apply_self(cx, ids::power::PLATING_POWER, n, p);
        Flow::Done
    }
});

listener!(Fasten {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::EXTRA_BLOCK);
        apply_self(cx, ids::power::FASTEN_POWER, n, p);
        Flow::Done
    }
});

listener!(Finesse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        draw(cx, p);
        Flow::Done
    }
});

// Block equal to the damage dealt (blocked + unblocked + overkill).
listener!(Fisticuffs {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let results = single(cx, p);
        let total: i32 = results.iter().map(|r| r.blocked + r.unblocked + r.overkill).sum();
        cx.gain_block(PLAYER, Dec::int(total as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(FlashOfSteel {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        draw(cx, p);
        Flow::Done
    }
});

// Multiplayer only (AllAllies): block for every living player (just the user in single player).
listener!(Rally {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

// Multiplayer only. Calculated damage: 5 + 5 per hit the target took this turn from another ally's powered attack
// (no allies in single player -> 0; damage history from pets is not tracked).
fn calc_gang_up(cx: &Combat, card: CardIdx, _target: Cid) -> Dec {
    let base = cx.card_var(card, VarKind::CalcBase);
    Dec::int(base as i64)
}

listener!(GangUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = calc_gang_up(cx, p.card, p.target);
        let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target));
        a.damage = d;
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_gang_up(cx, card, target))
    }
});

// 0 + 1 * (card plays finished so far this combat); Retain when upgraded (stat table).
fn calc_gold_axe(cx: &Combat, card: CardIdx, _target: Cid) -> Dec {
    let base = cx.card_var(card, VarKind::CalcBase) as i64;
    let extra = cx.card_var(card, VarKind::ExtraDamage) as i64;
    let mult = if cx.in_progress { cx.hist_total(HKind::CardPlayFinished) as i64 } else { 0 };
    Dec::int(base + extra * mult)
}

listener!(GoldAxe {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = calc_gold_axe(cx, p.card, p.target);
        let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target));
        a.damage = d;
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_gold_axe(cx, card, target))
    }
});

// Gold when the attack kills (unless a Minion/Reattach-style power says the death is not "fatal").
listener!(HandOfGreed {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let fatal_ok = cx.all_powers_trigger_fatal(p.target);
        let results = single(cx, p);
        if fatal_ok && results.iter().any(|r| r.killed) {
            let g = cx.card_named_var(p.card, var_name::GOLD);
            cx.gain_gold(g);
        }
        Flow::Done
    }
});

// Pick a random draw-pile card (preferring Attack/Skill/Power) that has no replay yet and give it +Replay plays.
listener!(HiddenGem {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.player.draw.is_empty() {
            return Flow::Done;
        }
        let mut all: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        let mut core: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.draw.iter() {
            let t = cx.card_def(c).ctype;
            let ok = cx.card_keywords(c) & kw::UNPLAYABLE == 0 && !matches!(t, CardType::Curse | CardType::Quest) && cx.enchanted_replay_count(c) < 1;
            if ok {
                all.push(c);
                if matches!(t, CardType::Attack | CardType::Skill | CardType::Power) {
                    core.push(c);
                }
            }
        }
        let items = if core.is_empty() { &all } else { &core };
        if items.is_empty() {
            return Flow::Done;
        }
        let i = cx.rng.combat_card_selection.next_int_range(0, items.len() as i32) as usize;
        let c = items[i];
        let r = cx.card_named_var(p.card, var_name::REPLAY);
        let card = &mut cx.cards[c as usize];
        card.base_replay = card.base_replay.saturating_add(r as u8);
        Flow::Done
    }
});

// Multiplayer only (AllAllies): each living player draws.
listener!(HuddleUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        draw(cx, p);
        Flow::Done
    }
});

listener!(Impatience {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let has_attack = cx.player.hand.iter().any(|&c| cx.card_def(c).ctype == CardType::Attack);
        if !has_attack {
            draw(cx, p);
        }
        Flow::Done
    }
});

// Multiplayer only (AnyAlly): block for the user + Covered on the ally.
listener!(Intercept {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        if p.target != NO {
            cx.apply_power(ids::power::COVERED_POWER, p.target, Dec::ONE, PLAYER, p.card);
        }
        Flow::Done
    }
});

// Generate `Cards` distinct colorless cards (never another Jack of All Trades) into the hand.
listener!(JackOfAllTrades {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards) as usize;
        let cards = cx.get_distinct_for_combat(&crate::content::gen_pools::COLORLESS, n, |d| d.id != ids::card::JACK_OF_ALL_TRADES);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// Attack, then generate `Cards` cost-0 non-X cards of the character's pool (with replacement), upgraded if this card is.
listener!(Jackpot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards) as usize;
        let pool = cx.character_pool();
        let cards = cx.get_for_combat_where(pool, n, |d: &CardDef| d.cost == 0 && !d.x_cost);
        let upgraded = cx.cards[p.card as usize].upgrade > 0;
        for &c in cards.iter() {
            if upgraded {
                cx.upgrade_in_combat(c);
            }
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// Multiplayer only.
listener!(Knockdown {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        let n = cx.card_power_var(p.card, ids::power::KNOCKDOWN_POWER);
        cx.apply_power(ids::power::KNOCKDOWN_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer only (AnyAlly): block for the target player.
listener!(Lift {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        let t = if p.target == NO { PLAYER } else { p.target };
        cx.gain_block(t, Dec::int(b as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(MasterOfStrategy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        draw(cx, p);
        Flow::Done
    }
});

// Multiplayer only (AnyAlly): block equal to the target's block.
listener!(Mimic {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let t = if p.target == NO { PLAYER } else { p.target };
        let base = cx.card_var(p.card, VarKind::CalcBase) as i64;
        let extra = cx.card_var(p.card, VarKind::CalcExtra) as i64;
        let b = base + extra * cx.cr(t).block as i64;
        cx.gain_block(PLAYER, Dec::int(b), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

// Calculated damage: 0 + 1 per card in the draw pile.
fn calc_mind_blast(cx: &Combat, card: CardIdx, _target: Cid) -> Dec {
    let base = cx.card_var(card, VarKind::CalcBase) as i64;
    let extra = cx.card_var(card, VarKind::ExtraDamage) as i64;
    let mult = if cx.in_progress { cx.player.draw.len() as i64 } else { 0 };
    Dec::int(base + extra * mult)
}

listener!(MindBlast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = calc_mind_blast(cx, p.card, p.target);
        let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target));
        a.damage = d;
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_mind_blast(cx, card, target))
    }
});

listener!(Nostalgia {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::NOSTALGIA_POWER, 1, p);
        Flow::Done
    }
});

// One AttackContext (BeforeAttack/AfterAttack once) with two hits: the target, then every other hittable enemy for the
// total damage dealt to the target (`TotalDamage + OverkillDamage`, unpowered).
listener!(Omnislice {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let ctx_attack = Attack::from_card(PLAYER, p.card, 0, Targeting::AllOpponents);
        cx.dispatch_g(hookbit::before_attack, |cx, me, l| l.before_attack(cx, me, &ctx_attack));
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let first = cx.damage(&[p.target], Dec::int(dmg as i64), ValueProp::MOVE, PLAYER, p.card);
        let mut hit_results: crate::util::ArrayVec<crate::engine::DamageResult, 16> = crate::util::ArrayVec::new();
        let mut hit_sizes: crate::util::ArrayVec<u8, 2> = crate::util::ArrayVec::new();
        for r in first.iter().take(16) {
            hit_results.push(*r);
        }
        hit_sizes.push(first.len() as u8);
        if let Some(r) = first.first() {
            let mut others: crate::util::ArrayVec<Cid, MAX_CREATURES> = crate::util::ArrayVec::new();
            for &e in cx.enemies.iter() {
                if e != p.target && cx.cr(e).is_alive() && cx.should_allow_hitting(e) {
                    others.push(e);
                }
            }
            if !others.is_empty() {
                let total = r.blocked + r.unblocked + r.overkill;
                let rest = cx.damage(others.as_slice(), Dec::int(total as i64), ValueProp::UNPOWERED.or(ValueProp::MOVE), PLAYER, p.card);
                for r in rest.iter() {
                    if hit_results.len() < 16 {
                        hit_results.push(*r);
                    }
                }
                hit_sizes.push(rest.len() as u8);
            }
        }
        cx.set_attack_results(hit_results.as_slice(), hit_sizes.as_slice());
        cx.dispatch_g(hookbit::after_attack, |cx, me, l| l.after_attack(cx, me, &ctx_attack));
        Flow::Done
    }
});

listener!(Panache {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::PANACHE_DAMAGE);
        apply_self(cx, ids::power::PANACHE_POWER, n, p);
        Flow::Done
    }
});

listener!(PanicButton {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let n = cx.card_named_var(p.card, var_name::TURNS);
        apply_self(cx, ids::power::NO_BLOCK_POWER, n, p);
        Flow::Done
    }
});

listener!(PrepTime {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::PREP_TIME_POWER);
        apply_self(cx, ids::power::PREP_TIME_POWER, n, p);
        Flow::Done
    }
});

listener!(Production {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

// Block equal to the current block as Block Next Turn.
listener!(Prolong {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.cr(PLAYER).block;
        apply_self(cx, ids::power::BLOCK_NEXT_TURN_POWER, b, p);
        Flow::Done
    }
});

listener!(Prowess {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        apply_self(cx, ids::power::STRENGTH_POWER, s, p);
        let d = cx.card_power_var(p.card, ids::power::DEXTERITY_POWER);
        apply_self(cx, ids::power::DEXTERITY_POWER, d, p);
        Flow::Done
    }
});

// Exhaust up to `Cards` cards from the hand.
listener!(Purity {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards) as u8;
                match cx.ask_hand(ids::card::PURITY, 0, n, |_, _| true) {
                    Ask::Resolved(cards) => {
                        for &c in cards.iter() {
                            cx.exhaust_card(c, false);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let cards = cx.choice.cards;
                for &c in cards.iter() {
                    cx.exhaust_card(c, false);
                }
                Flow::Done
            }
        }
    }
});

// Powers that are always-damage-all-enemies etc. are in the power file.
fn is_temporary_power(id: u16) -> bool {
    use ids::power as p;
    matches!(
        id,
        p::ANTICIPATE_POWER
            | p::FEEDING_FRENZY_POWER
            | p::DYING_STAR_POWER
            | p::COORDINATE_POWER
            | p::CRUSH_UNDER_POWER
            | p::DARK_SHACKLES_POWER
            | p::FLEX_POTION_POWER
            | p::ENFEEBLING_TOUCH_POWER
            | p::FOCUSED_STRIKE_POWER
            | p::FADE_POWER
            | p::HYPERBEAM_FOCUS_DOWN_POWER
            | p::HELICAL_DART_POWER
            | p::HOTFIX_POWER
            | p::MONARCHS_GAZE_STRENGTH_DOWN_POWER
            | p::PIERCING_WAIL_POWER
            | p::REPTILE_TRINKET_POWER
            | p::SETUP_STRIKE_POWER
            | p::SHACKLING_POTION_POWER
            | p::SPEED_POTION_POWER
            | p::SYNCHRONIZE_POWER
            | p::MANGLE_POWER
            | p::SLEIGHT_OF_FLESH_POWER
            | p::ILLUSION_POWER
    )
}

// Calculated damage: 10 + 5 per debuff on the target (temporary-power debuffs do not count).
fn calc_rend(cx: &Combat, card: CardIdx, target: Cid) -> Dec {
    let base = cx.card_var(card, VarKind::CalcBase) as i64;
    let extra = cx.card_var(card, VarKind::ExtraDamage) as i64;
    let mult = if cx.in_progress && target != NO {
        cx.cr(target)
            .powers
            .iter()
            .filter(|pw| Combat::power_type_for_amount(pw.id, pw.amount) == PowerType::Debuff && !is_temporary_power(pw.id))
            .count() as i64
    } else {
        0
    };
    Dec::int(base + extra * mult)
}

listener!(Rend {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = calc_rend(cx, p.card, p.target);
        let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target));
        a.damage = d;
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_rend(cx, card, target))
    }
});

// Only does something if it is the only card in the hand: draw one card at a time, then gain energy.
listener!(Restlessness {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let only = cx.player.hand.iter().all(|&c| c == p.card);
        if only {
            let n = cx.card_var(p.card, VarKind::Cards);
            for _ in 0..n {
                cx.draw_cards_nosuspend(1, false);
            }
            let e = cx.card_var(p.card, VarKind::Energy);
            cx.gain_energy(e);
        }
        Flow::Done
    }
});

listener!(RollingBoulder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::ROLLING_BOULDER_POWER);
        apply_self(cx, ids::power::ROLLING_BOULDER_POWER, n, p);
        Flow::Done
    }
});

listener!(Salvo {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        apply_self(cx, ids::power::RETAIN_HAND_POWER, 1, p);
        Flow::Done
    }
});

// Draw as many cards as fit in the hand.
listener!(Scrawl {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let n = MAX_HAND as i32 - cx.player.hand.len() as i32;
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Choose a Skill from the draw pile and put it in the hand.
listener!(SecretTechnique {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_pile(ids::card::SECRET_TECHNIQUE, PileType::Draw, 1, 1, |cx, c| cx.card_def(c).ctype == CardType::Skill) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

listener!(SecretWeapon {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_pile(ids::card::SECRET_WEAPON, PileType::Draw, 1, 1, |cx, c| cx.card_def(c).ctype == CardType::Attack) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

// Attack, then choose 1 of 3 random draw-pile cards to put in the hand.
listener!(SeekerStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                single(cx, p);
                let n = cx.card_var(p.card, VarKind::Cards) as usize;
                let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
                for &c in cx.player.draw.iter() {
                    list.push(c);
                }
                cx.stable_shuffle_cards(list.as_mut_slice(), RngStream::CombatCardSelection);
                let mut options: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
                for &c in list.iter().take(n) {
                    options.push(c);
                }
                match cx.ask_pile(ids::card::SEEKER_STRIKE, PileType::Draw, 1, 1, |_, c| options.contains(c)) {
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

// Weak then Vulnerable on each hittable enemy, enemy by enemy.
listener!(Shockwave {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::POWER);
        let enemies = cx.hittable_enemies();
        for &e in enemies.iter() {
            cx.apply_power(ids::power::WEAK_POWER, e, Dec::int(n as i64), PLAYER, p.card);
            cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(n as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});

// Choose 1 of 3 attacks from the OTHER characters' pools (upgraded if Splash is), free this turn.
listener!(Splash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                // `UnlockState.CharacterCardPools` order: Ironclad, Silent, Regent, Necrobinder, Defect; minus the own pool.
                use crate::content::gen_pools as gp;
                let pools: [(u8, &[u16]); 5] = [(0, &gp::IRONCLAD), (1, &gp::SILENT), (4, &gp::REGENT), (3, &gp::NECROBINDER), (2, &gp::DEFECT)];
                let mut all: crate::util::ArrayVec<u16, 512> = crate::util::ArrayVec::new();
                for (ch, pool) in pools {
                    if ch != cx.character {
                        for &id in pool {
                            all.push(id);
                        }
                    }
                }
                let cards = cx.get_distinct_for_combat(all.as_slice(), 3, |d: &CardDef| d.ctype == CardType::Attack);
                if cx.cards[p.card as usize].upgrade > 0 {
                    for &c in cards.iter() {
                        cx.upgrade_in_combat(c);
                    }
                }
                match cx.ask_options(ids::card::SPLASH, cards.as_slice(), true) {
                    Ask::Resolved(cards) => {
                        // synchronous answer (Whispering Earring's selector, empty option list): same continuation as the resumed phase
                        cx.choice.cards = cards;
                        self.on_play(cx, p, 1)
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.set_to_free_this_turn(c);
                    cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

listener!(Stratagem {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::STRATAGEM_POWER, 1, p);
        Flow::Done
    }
});

// Multiplayer only.
listener!(TagTeam {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        cx.apply_power(ids::power::TAG_TEAM_POWER, p.target, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer only: grows by `Increase` every play (`ExtraDamageFromPlays` lives in `counter[0]`).
listener!(TheBall {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage) + cx.cards[p.card as usize].counter[0] as i32;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        let inc = cx.card_named_var(p.card, var_name::INCREASE);
        cx.cards[p.card as usize].counter[0] += inc as i16;
        Flow::Done
    }
});

listener!(TheBomb {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let turns = cx.card_named_var(p.card, var_name::TURNS);
        let dmg = cx.card_named_var(p.card, var_name::BOMB_DAMAGE);
        if let Some(uid) = apply_self(cx, ids::power::THE_BOMB_POWER, turns, p) {
            // `SetDamage`: the instance's damage lives in `aux` (Amount counts the turns left).
            if let Some(i) = cx.power_idx(PLAYER, uid) {
                cx.cr_mut(PLAYER).powers[i].aux = dmg;
            }
        }
        Flow::Done
    }
});

listener!(TheGambit {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        apply_self(cx, ids::power::THE_GAMBIT_POWER, 1, p);
        Flow::Done
    }
});

// Draw, then put a hand card back on top of the draw pile.
listener!(ThinkingAhead {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards_nosuspend(n, false); // a decision follows: a Stratagem pick cannot be paused here
                match cx.ask_hand(ids::card::THINKING_AHEAD, 1, 1, |_, _| true) {
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

// Returns to the hand at the start of the next turn if it was played this one (see Bolas).
listener!(ThrummingHatchet {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        return_if_played_last_turn(cx, me);
    }
});

listener!(UltimateDefend {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

listener!(UltimateStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        single(cx, p);
        Flow::Done
    }
});

// X-cost: hits `X` times at random enemies.
listener!(Volley {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.x_value(p.card);
        let a = atk(cx, p, Targeting::Random).hits(x);
        cx.execute_attack(&a);
        Flow::Done
    }
});
