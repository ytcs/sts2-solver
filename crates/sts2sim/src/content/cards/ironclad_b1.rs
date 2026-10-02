//! Ironclad cards Infernal Blade .. Whirlwind (alphabetical slice [45, 90) of the pool), except the ones in `basic.rs`.
//!
//! Conventions used here:
//! * Cards whose Damage var grows permanently (Rampage, Thrash) keep the growth in `Card::dmg_bonus` (1/10000 units);
//!   read the current damage with `Combat::card_damage_dec`.
//! * `calculated_damage` is implemented by cards whose damage is a `CalculatedDamageVar` (Perfected Strike).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::content::gen_cards::var_name;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `DamageCmd.Attack(card damage var).FromCard(...)` with the exact decimal damage (includes permanent growth).
fn card_attack(cx: &Combat, p: &CardPlay, targeting: Targeting) -> Attack {
    let mut a = Attack::from_card(PLAYER, p.card, 0, targeting);
    a.damage = cx.card_damage_dec(p.card);
    a
}

/// `PowerCmd.Apply<T>(Owner.Creature, amount, Owner.Creature, card)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) -> Option<u16> {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card)
}

// ---- Infernal Blade: add a random Attack from the character's pool, free this turn, to hand ---------------------------------------
listener!(InfernalBlade {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, 1, |d| d.ctype == CardType::Attack);
        if let Some(c) = cards.first() {
            cx.set_to_free_this_turn(c);
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// ---- Inferno ---------------------------------------------------------------------------------------------------------------------
listener!(Inferno {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::INFERNO_POWER);
        if let Some(uid) = apply_self(cx, ids::power::INFERNO_POWER, v, p) {
            // InfernoPower.IncrementSelfDamage()
            if let Some(i) = cx.power_idx(PLAYER, uid) {
                cx.cr_mut(PLAYER).powers[i].aux += 1;
            }
        }
        Flow::Done
    }
});

listener!(Juggernaut {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::JUGGERNAUT_POWER);
        apply_self(cx, ids::power::JUGGERNAUT_POWER, v, p);
        Flow::Done
    }
});

// Upgrade (Innate) comes from the generated stat table.
listener!(Juggling {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::JUGGLING_POWER, 1, p);
        Flow::Done
    }
});

listener!(Mangle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let n = cx.card_named_var(p.card, var_name::STRENGTH_LOSS);
        cx.apply_power(ids::power::MANGLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Cost drops by 1 (this combat) for every card exhausted so far, including those exhausted before it entered combat.
listener!(Midnight {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        Flow::Done
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return;
        }
        let n = cx.hist.cards_exhausted as i32;
        cx.add_cost_modifier(card, -n, 0);
    }
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, _card: CardIdx, _by_ethereal: bool) {
        cx.add_cost_modifier(me.idx as CardIdx, -1, 0);
    }
});

listener!(MoltenFist {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let n = if cx.cr(p.target).is_alive() { cx.power_amount(p.target, ids::power::VULNERABLE_POWER) } else { 0 };
        if n > 0 {
            cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        }
        Flow::Done
    }
});

listener!(NotYet {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Heal);
        cx.heal(PLAYER, Dec::int(n as i64));
        Flow::Done
    }
});

listener!(Offering {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        cx.damage(&[PLAYER], Dec::int(loss as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE), PLAYER, p.card);
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(OneTwoPunch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::ATTACKS);
        apply_self(cx, ids::power::ONE_TWO_PUNCH_POWER, n, p);
        Flow::Done
    }
});

// Multiplayer-only in the real game; the clone goes to the discard pile.
listener!(Outrage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        if cx.in_progress {
            if let Some(c) = cx.clone_card(p.card) {
                cx.add_generated_card(c, PileType::Discard, CardPilePosition::Bottom);
            }
        }
        Flow::Done
    }
});

listener!(PactsEnd {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let need = cx.card_var(p.card, VarKind::Cards) as usize;
        if cx.player.exhaust.len() >= need {
            let a = card_attack(cx, p, Targeting::AllOpponents);
            cx.execute_attack(&a);
        }
        Flow::Done
    }
});

/// `PlayerCombatState.AllCards.Count(c => c.Tags.Contains(Strike))`.
fn strike_count(cx: &Combat) -> i32 {
    let pl = &cx.player;
    let mut n = 0;
    for pile in [&pl.hand, &pl.draw, &pl.discard, &pl.exhaust, &pl.play] {
        for &c in pile.iter() {
            if cx.card_def(c).tags & tag::STRIKE != 0 {
                n += 1;
            }
        }
    }
    n
}

listener!(PerfectedStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = self.calculated_damage(cx, p.card, p.target).unwrap_or(Dec::ZERO);
        let mut a = Attack::from_card(PLAYER, p.card, 0, Targeting::Single(p.target));
        a.damage = dmg;
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        let base = cx.card_var(card, VarKind::CalcBase);
        let extra = cx.card_var(card, VarKind::ExtraDamage);
        // `CombatManager.IsInProgress` gates the multiplier.
        let mult = if cx.in_progress { strike_count(cx) } else { 0 };
        Some(Dec::int(base as i64 + extra as i64 * mult as i64))
    }
});

listener!(Pillage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        // Draw until a non-Attack is drawn (or nothing could be drawn / the hand is full).
        loop {
            let drawn = cx.draw_one();
            match drawn {
                Some(c) if cx.card_def(c).ctype == CardType::Attack && cx.player.hand.len() < MAX_HAND => {}
                _ => break,
            }
        }
        Flow::Done
    }
});

// Token: transforms every Attack in hand into Giant Rock (upgraded when this card is).
listener!(GiantRock {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(PrimalForce {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut attacks: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in cx.player.hand.iter() {
            if cx.card_def(c).ctype == CardType::Attack {
                attacks.push(c);
            }
        }
        let up = cx.cards[p.card as usize].upgrade;
        for &c in attacks.iter() {
            if let Some(rock) = cx.new_card(ids::card::GIANT_ROCK, up.min(1)) {
                cx.transform_card(c, rock);
            }
        }
        Flow::Done
    }
});

listener!(Pyre {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::PYRE_POWER, n, p);
        Flow::Done
    }
});

listener!(Rage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::POWER);
        apply_self(cx, ids::power::RAGE_POWER, n, p);
        Flow::Done
    }
});

listener!(Rampage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let inc = cx.card_named_var(p.card, var_name::INCREASE);
        cx.add_card_damage(p.card, Dec::int(inc as i64));
        Flow::Done
    }
});

listener!(Rupture {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        apply_self(cx, ids::power::RUPTURE_POWER, n, p);
        Flow::Done
    }
});

// Exhaust every non-Attack card in hand, gaining block for each.
listener!(SecondWind {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut cards: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in cx.player.hand.iter() {
            if cx.card_def(c).ctype != CardType::Attack {
                cards.push(c);
            }
        }
        let block = cx.card_var(p.card, VarKind::Block);
        for &c in cards.iter() {
            cx.exhaust_card(c, false);
            cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        }
        Flow::Done
    }
});

listener!(SetupStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let n = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        apply_self(cx, ids::power::SETUP_STRIKE_POWER, n, p);
        Flow::Done
    }
});

listener!(Spite {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = if cx.hist.player_lost_hp_this_turn { cx.card_var(p.card, VarKind::Repeat) } else { 1 };
        let a = card_attack(cx, p, Targeting::Single(p.target)).hits(hits);
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(Stampede {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::POWER);
        apply_self(cx, ids::power::STAMPEDE_POWER, n, p);
        Flow::Done
    }
});

// Exhaust the whole hand, then add that many random cards (upgraded when this card is) to the hand.
listener!(Stoke {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        let count = hand.len();
        for &c in hand.iter() {
            cx.exhaust_card(c, false);
        }
        let pool = cx.character_pool();
        let cards = cx.get_for_combat(pool, count);
        if cx.cards[p.card as usize].upgrade > 0 {
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

// Costs 1 less this turn for every Attack played this turn (including those played before it was created).
listener!(Stomp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::AllOpponents);
        cx.execute_attack(&a);
        Flow::Done
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return;
        }
        let n = cx.hist.attacks_finished_this_turn as i32;
        cx.add_cost_modifier(card, -n, EXPIRE_END_OF_TURN);
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Attack {
            cx.add_cost_modifier(me.idx as CardIdx, -1, EXPIRE_END_OF_TURN);
        }
    }
});

listener!(StoneArmor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::PLATING_POWER);
        apply_self(cx, ids::power::PLATING_POWER, n, p);
        Flow::Done
    }
});

// Multiplayer-only.
listener!(Tank {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::TANK_POWER, 1, p);
        Flow::Done
    }
});

listener!(Taunt {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Hit count = 1 + the number of times the player has lost HP this combat.
listener!(TearAsunder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let base = cx.card_var(p.card, VarKind::CalcBase);
        let extra = cx.card_var(p.card, VarKind::CalcExtra);
        let hits = base + extra * (1 + cx.hist.player_hits_taken as i32);
        let a = card_attack(cx, p, Targeting::Single(p.target)).hits(hits);
        cx.execute_attack(&a);
        Flow::Done
    }
});

// Two hits; then exhaust a random Attack from hand and add its (modified) damage to this card permanently.
listener!(Thrash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target)).hits(2);
        cx.execute_attack(&a);
        let mut attacks: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in cx.player.hand.iter() {
            if cx.card_def(c).ctype == CardType::Attack {
                attacks.push(c);
            }
        }
        if attacks.is_empty() {
            return Flow::Done;
        }
        let k = cx.rng.combat_card_selection.next_int_range(0, attacks.len() as i32) as usize;
        let other = attacks[k];
        let me = Me { kind: Kind::Card, owner: PLAYER, idx: other as u16, id: cx.cards[other as usize].id, amount: 0 };
        let base = match crate::content::listener(&me).calculated_damage(cx, other, NO) {
            Some(d) => d,
            None => cx.card_damage_dec(other),
        };
        let (dmg, _) = cx.modify_damage(NO, PLAYER, base, ValueProp::MOVE, other);
        cx.add_card_damage(p.card, dmg);
        cx.exhaust_card(other, false);
        Flow::Done
    }
});

listener!(Tremble {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Unmovable {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::UNMOVABLE_POWER, 1, p);
        Flow::Done
    }
});

listener!(Unrelenting {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        apply_self(cx, ids::power::FREE_ATTACK_POWER, 1, p);
        Flow::Done
    }
});

listener!(Uppercut {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let n = cx.card_named_var(p.card, var_name::POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Vicious {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, ids::power::VICIOUS_POWER, n, p);
        Flow::Done
    }
});

// X cost: hits all enemies X times.
listener!(Whirlwind {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.resolve_energy_x(p.card);
        let a = card_attack(cx, p, Targeting::AllOpponents).hits(x);
        cx.execute_attack(&a);
        Flow::Done
    }
});
