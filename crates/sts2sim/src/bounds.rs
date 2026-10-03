//! Provable bounds: fights a deck cannot win.
//!
//! `provably_unwinnable` proves that a fight is lost no matter how well it is played, for the cases where that can be shown with a
//! relaxation of the real game that can only favour the player:
//!
//! * **Player side.** Every card of the deck must be *pure*: playing it only spends its energy and deals a fixed amount of damage
//!   and / or gains a fixed amount of block (plus, optionally, applying Vulnerable / Weak to the enemy), and moves cards between
//!   piles. Purity is established by probing the card in the simulator (several different situations, played twice in a row,
//!   identical results required; no other state may change); unplayable junk (curses / statuses) cannot help. Relics must not hook
//!   anything that affects a fight (only heal-after-combat, gold, hand-size style hooks), there are no potions and no starting
//!   powers, no enchantments. Then in every turn the player can at most play a subset of its deck (every card once, total cost <= max
//!   energy; hand size and draw order are ignored, which only helps), i.e. choose a point of the damage / block frontier of that subset
//!   problem. Block does not carry over.
//! * **Enemy side.** One enemy at the start. Its attack damage in every turn is bounded from below by the cheapest damage over the
//!   states its move machine can be in that turn (every conditional / random branch counts as possible; scaling, healing, block,
//!   summons and phases only make the real fight harder and are ignored). Weakening is accounted for when the deck can apply Weak.
//!
//! The decision is then a small dynamic program over turns: the state is the HP lost so far, the value the most damage dealt so far.
//! The fight is unwinnable when no turn can reach the enemy's HP before the player is dead. Anything the analysis cannot cover
//! (other characters' resources, unknown cards / relics, more than one enemy, a horizon that is too long) returns `None`.
//! `tests/bounds.rs` checks the soundness empirically: no simulated policy ever wins a fight this module declares unwinnable.

use std::collections::HashMap;

use crate::content;
use crate::hooks::{hookbit, Mask};
use crate::ids;
use crate::scenario::{DeckCard, DeckExtra, Scenario, ScenarioExtras};
use crate::state::*;
use crate::types::*;
use crate::engine::Action;
use crate::dec::Dec;

/// What one play of a pure card does (per play, against a plain enemy; `damage_vuln` against a Vulnerable one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardProfile {
    pub cost: i32,
    pub damage: i32,
    pub damage_vuln: i32,
    pub block: i32,
    pub vuln: bool,
    pub weak: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardClass {
    Pure(CardProfile),
    /// Cannot be played and cannot help the player (curse / status / Ascender's Bane).
    Junk,
    Impure,
}

/// Hooks a (non-curse, non-status) pure card may override: its play, and being playable / unplayable.
fn pure_card_hooks() -> Mask {
    Mask::bit(hookbit::on_play)
        .or(Mask::bit(hookbit::is_playable))
        .or(Mask::bit(hookbit::should_play))
        .or(Mask::bit(hookbit::meta_props))
        .or(Mask::bit(hookbit::meta_display))
        .or(Mask::bit(hookbit::meta_initial))
}

/// Hooks a relic may override and still be unable to change what a fight is worth.
fn neutral_relic_hooks() -> Mask {
    Mask::bit(hookbit::after_combat_end)
        .or(Mask::bit(hookbit::after_combat_victory_early))
        .or(Mask::bit(hookbit::after_combat_victory))
        .or(Mask::bit(hookbit::after_room_entered))
        .or(Mask::bit(hookbit::modify_gold_gained))
        .or(Mask::bit(hookbit::after_gold_gained))
        .or(Mask::bit(hookbit::meta_props))
        .or(Mask::bit(hookbit::meta_display))
        .or(Mask::bit(hookbit::meta_initial))
        .or(Mask::bit(hookbit::modify_hand_draw))
        .or(Mask::bit(hookbit::modify_hand_draw_late))
        .or(Mask::bit(hookbit::after_modifying_hand_draw))
}

/// Hooks an enemy's power may use without being able to interrupt the enemy (stun it, put it to sleep, make it flee) in reaction to
/// what the player does. Powers that react to damage (Shriek, Asleep, Slumber, Thorns ...) are not in this list.
fn passive_power_hooks() -> Mask {
    use hookbit::*;
    [
        modify_damage_additive, modify_damage_multiplicative, modify_damage_cap, modify_hp_lost_before_osty, modify_hp_lost_before_osty_late,
        modify_hp_lost_after_osty, modify_hp_lost_after_osty_late, modify_block_additive, modify_block_multiplicative, modify_power_amount_given_additive,
        modify_power_amount_given_multiplicative, try_modify_power_amount_received, after_side_turn_start, after_side_turn_start_late, before_side_turn_start,
        before_side_turn_end_very_early, before_side_turn_end_early, before_side_turn_end, after_side_turn_end, after_side_turn_end_late, after_block_cleared,
        should_clear_block, meta_props, meta_display, meta_initial, before_applied, after_applied, after_removed, initial_power_aux, before_combat_start,
        before_combat_start_late, should_power_be_removed_on_death, should_power_be_removed_after_owner_death,
    ]
    .iter()
    .fold(Mask::EMPTY, |m, &b| m.or(Mask::bit(b)))
}

/// Can the enemy's behaviour be bounded from below whatever the player does? (No reaction to damage, no stun / sleep / flee.)
fn enemy_is_boundable(cx: &Combat, e: Cid) -> bool {
    use crate::defs::{Intent, MonsterNode};
    let ms = cx.cr(e).monster;
    if content::monster_mask(ms.id) != Mask::EMPTY {
        return false;
    }
    for node in content::monster_def(ms.id).nodes.iter() {
        if let MonsterNode::Move { intents, .. } = node {
            if intents.iter().any(|i| matches!(i, Intent::Escape | Intent::Sleep | Intent::Stun | Intent::Hidden)) {
                return false;
            }
        }
    }
    cx.cr(e).powers.iter().all(|p| subset(content::power_mask(p.id), passive_power_hooks()))
}

fn subset(m: Mask, of: Mask) -> bool {
    (0..4).all(|i| m.0[i] & !of.0[i] == 0)
}

pub fn relic_is_neutral(id: u16) -> bool {
    content::relic_implemented(id) && subset(content::relic_mask(id), neutral_relic_hooks())
}

// ---- probing a card ----------------------------------------------------------------------------------------------------------

fn strike(character: u8) -> u16 {
    match character {
        0 => ids::card::STRIKE_IRONCLAD,
        _ => ids::card::STRIKE_SILENT,
    }
}

/// One situation a card is played in.
#[derive(Clone, Copy, Debug)]
struct Ctx {
    /// Other cards in hand besides the probed card(s).
    fillers: usize,
    /// Enemy block / HP before the play, player block before the play, enemy Vulnerable (and Weak) before the play.
    enemy_block: i32,
    enemy_hp: i32,
    player_block: i32,
    enemy_vuln: bool,
    /// Cards moved to the exhaust pile / discard pile / draw pile emptied before the play.
    exhausted: usize,
    empty_draw: bool,
    /// Plays of the probed card in a row (all must behave identically).
    plays: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    cost: i32,
    damage: i32,
    block: i32,
    vuln: bool,
    weak: bool,
}

fn probe_scenario(character: u8, ascension: u8, max_energy: i32) -> Scenario {
    let mut deck = vec![];
    for _ in 0..28 {
        deck.push(DeckCard { id: strike(character), upgrade: 0 });
    }
    Scenario {
        run_seed: 7,
        total_floor: 1,
        character,
        ascension,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 1000,
        hp: 1000,
        max_energy,
        orb_slots: 0,
        potion_slots: 0,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(7),
    }
}

fn multiset(cx: &Combat) -> Vec<u16> {
    let mut v: Vec<u16> = (0..cx.n_cards as usize).map(|i| cx.cards[i].id).collect();
    v.sort_unstable();
    v
}

/// Everything about a card instance that can change what it (or another card) does later, except where it is.
fn card_sigs(cx: &Combat) -> Vec<String> {
    (0..cx.n_cards as usize)
        .map(|i| {
            let c = &cx.cards[i];
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}:{}",
                c.id, c.upgrade, c.kw_add, c.kw_remove, c.enchant, c.affliction, c.base_replay, c.cost_base, c.x_value, c.mods, c.counter, c.dmg_bonus
            )
        })
        .collect()
}

fn power_amounts(cx: &Combat, c: Cid) -> Vec<(u16, i32)> {
    let mut v: Vec<(u16, i32)> = cx.cr(c).powers.iter().map(|p| (p.id, p.amount)).collect();
    v.sort_unstable();
    v
}

fn probe_once(character: u8, ascension: u8, max_energy: i32, id: u16, upgrade: u8, ctx: Ctx) -> Option<Vec<Outcome>> {
    let mut cx = Combat::try_new(&probe_scenario(character, ascension, max_energy)).ok()?;
    if cx.enemies.len() != 1 || cx.stage != Stage::AwaitAction {
        return None;
    }
    let e = cx.enemies[0];
    // rebuild hand: probed card(s) + fillers
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    // (a second play replays the SAME card instance: growth per play is a property of the instance)
    let probed_card = cx.new_card(id, upgrade)?;
    cx.move_card(probed_card, PileType::Hand, CardPilePosition::Bottom);
    for _ in 0..ctx.fillers {
        let c = cx.new_card(strike(character), 0)?;
        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    }
    for _ in 0..ctx.exhausted {
        let c = cx.new_card(strike(character), 0)?;
        cx.move_card(c, PileType::Exhaust, CardPilePosition::Bottom);
    }
    if ctx.empty_draw {
        let draw = cx.player.draw;
        for &c in draw.iter() {
            cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
    }
    cx.cr_mut(e).hp = ctx.enemy_hp;
    cx.cr_mut(e).max_hp = ctx.enemy_hp.max(cx.cr(e).max_hp);
    cx.cr_mut(e).block = ctx.enemy_block;
    cx.cr_mut(PLAYER).block = ctx.player_block;
    cx.player.energy = 10;
    if ctx.enemy_vuln {
        cx.apply_power(ids::power::VULNERABLE_POWER, e, Dec::int(5), PLAYER, NO);
    }
    let before_cards = multiset(&cx);
    let sigs0 = card_sigs(&cx);
    let hp0 = cx.cr(PLAYER).hp;
    let n0 = cx.n_cards;
    let (enemy_hp0, enemy_blk0) = (cx.cr(e).hp, cx.cr(e).block);
    let pw_e0 = power_amounts(&cx, e);
    let pw_p0 = power_amounts(&cx, PLAYER);
    let stars0 = cx.player.stars;
    let orbs0 = cx.player.orbs.len();
    let creatures0 = (cx.enemies.len(), cx.osty().is_some());
    let mut outs: Vec<Outcome> = vec![];
    for k in 0..ctx.plays {
        let c = probed_card;
        if k > 0 {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        let pos = cx.player.hand.iter().position(|&h| h == c)?;
        if !cx.can_play(c) {
            return None;
        }
        let block_before = cx.cr(PLAYER).block;
        let energy_before = cx.player.energy;
        let (eh, eb) = (cx.cr(e).hp, cx.cr(e).block);
        let pw_before = power_amounts(&cx, e);
        let target = if cx.card_target_type(c) == TargetType::AnyEnemy { e } else { NO };
        if !cx.step(Action::PlayCard { hand_pos: pos as u8, target }) {
            return None;
        }
        let mut guard = 0;
        while cx.stage == Stage::AwaitChoice && guard < 8 {
            guard += 1;
            let mut acts = crate::engine::ActionBuf::new();
            cx.legal_actions(&mut acts);
            let a = acts.as_slice().iter().copied().find(|a| matches!(a, Action::Pick { .. })).or_else(|| acts.as_slice().iter().copied().find(|a| matches!(a, Action::Confirm)))?;
            if !cx.step(a) {
                return None;
            }
        }
        if cx.stage != Stage::AwaitAction || cx.missing.is_some() || cx.overflow != 0 {
            return None;
        }
        let spent = energy_before - cx.player.energy;
        if spent < 0 {
            return None;
        }
        // exactly this card was played: no other card was played by it (Havoc, Cascade ...)
        if cx.hist.cards_played_this_turn as usize != k + 1 {
            return None;
        }
        let damage = (eh - cx.cr(e).hp) + (eb - cx.cr(e).block);
        let block = cx.cr(PLAYER).block - block_before;
        let pw_after = power_amounts(&cx, e);
        let (mut vuln, mut weak) = (false, false);
        for &(pid, amt) in pw_after.iter() {
            let was = pw_before.iter().find(|(i, _)| *i == pid).map(|x| x.1).unwrap_or(0);
            if amt != was {
                if pid == ids::power::VULNERABLE_POWER && amt > was {
                    vuln = true;
                } else if pid == ids::power::WEAK_POWER && amt > was {
                    weak = true;
                } else {
                    return None; // any other change to the enemy's powers
                }
            }
        }
        for &(pid, was) in pw_before.iter() {
            if !pw_after.iter().any(|(i, _)| *i == pid) && was != 0 {
                return None;
            }
        }
        outs.push(Outcome { cost: spent, damage, block, vuln, weak });
    }
    // nothing else may have changed
    if cx.n_cards != n0 || multiset(&cx) != before_cards || cx.cr(PLAYER).hp > hp0 || cx.player.stars != stars0 || cx.player.orbs.len() != orbs0 || (cx.enemies.len(), cx.osty().is_some()) != creatures0 {
        return None;
    }
    if power_amounts(&cx, PLAYER) != pw_p0 {
        return None;
    }
    // no card (including the played one) was changed: no upgrades, cost changes, growth, new keywords
    if card_sigs(&cx) != sigs0 {
        return None;
    }
    let _ = (enemy_hp0, enemy_blk0, pw_e0);
    Some(outs)
}

/// Classifies a card by its definition and by probing.
pub fn classify_card(character: u8, ascension: u8, max_energy: i32, id: u16, upgrade: u8) -> CardClass {
    if character > 1 || !content::card_implemented(id) {
        return CardClass::Impure;
    }
    let d = content::card_def(id);
    if d.x_cost || d.star_cost >= 0 {
        return CardClass::Impure;
    }
    use crate::defs::VarKind;
    if d.vars.iter().any(|v| !matches!(v.kind, VarKind::Damage | VarKind::Block | VarKind::Cards | VarKind::Repeat | VarKind::HpLoss | VarKind::Power | VarKind::Named)) {
        return CardClass::Impure; // energy, stars, heal, summon, forge, gold, scaled damage ...
    }
    let junk_type = matches!(d.ctype, CardType::Curse | CardType::Status);
    if !junk_type && !subset(content::card_mask(id), pure_card_hooks()) {
        return CardClass::Impure;
    }
    if d.ctype == CardType::Power {
        return CardClass::Impure;
    }
    let base = Ctx { fillers: 0, enemy_block: 0, enemy_hp: 100_000, player_block: 0, enemy_vuln: false, exhausted: 0, empty_draw: false, plays: 1 };
    let ctxs = [
        base,
        Ctx { fillers: 4, exhausted: 3, ..base },
        Ctx { enemy_block: 7, enemy_hp: 40, player_block: 20, ..base },
        Ctx { plays: 2, ..base },
        Ctx { empty_draw: true, fillers: 2, ..base },
    ];
    let single = |ctx: Ctx| probe_once(character, ascension, max_energy, id, upgrade, ctx).and_then(|v| v.first().copied());
    let first = match single(ctxs[0]) {
        Some(o) => o,
        None => {
            // not playable at all (Ascender's Bane, curses): junk if its type says so, otherwise unknown
            return if junk_type && d.cost < 0 { CardClass::Junk } else { CardClass::Impure };
        }
    };
    let dbg = std::env::var("STS2_BOUNDS_DEBUG").is_ok();
    // against a Vulnerable enemy: only the damage may change
    let v = single(Ctx { enemy_vuln: true, ..base });
    let second = match v {
        Some(o) if o.cost == first.cost && o.block == first.block => Outcome { damage: o.damage, ..first },
        other => {
            if dbg {
                eprintln!("card {id}+{upgrade}: against a Vulnerable enemy {other:?}, plain {first:?}");
            }
            return CardClass::Impure;
        }
    };
    for c in ctxs.iter().skip(1) {
        let outs = probe_once(character, ascension, max_energy, id, upgrade, *c);
        let ok = match (&outs, c.plays) {
            (Some(v), 1) => v.len() == 1 && v[0] == first,
            // the same card played twice: the first play as before, the second as against an enemy the first play made Vulnerable
            (Some(v), 2) => v.len() == 2 && v[0] == first && v[1] == if first.vuln { second } else { first },
            _ => false,
        };
        if !ok {
            if dbg {
                eprintln!("card {id}+{upgrade}: context {c:?} gave {outs:?}, the first gave {first:?}");
            }
            return CardClass::Impure;
        }
    }
    CardClass::Pure(CardProfile { cost: first.cost, damage: first.damage, damage_vuln: second.damage, block: first.block, vuln: first.vuln, weak: first.weak })
}

// ---- the decision ------------------------------------------------------------------------------------------------------------

/// Why a fight cannot be won.
#[derive(Clone, Debug)]
pub struct Proof {
    pub enemy_hp: i32,
    pub player_hp: i32,
    /// Most damage the player can deal over its whole life (all turns it can survive).
    pub max_damage: i64,
    /// Turns the player can survive at most.
    pub max_turns: usize,
}

impl Proof {
    pub fn describe(&self) -> String {
        format!(
            "the enemy has {} HP but within the at most {} turns the player can survive it can deal at most {} damage",
            self.enemy_hp, self.max_turns, self.max_damage
        )
    }
}

const MAX_TURNS: usize = 100;
const MAX_STATES: usize = 2_000;

/// `Ok(Some(proof))` = provably lost, `Ok(None)` = not proven (the fight may or may not be winnable).
pub fn provably_unwinnable(sc: &Scenario, ex: &ScenarioExtras) -> Option<Proof> {
    if sc.character > 1 || !sc.potions.is_empty() {
        return None;
    }
    if ex.deck.iter().any(|d| d.enchant != 0 || d.props != DeckExtra::default().props) {
        return None;
    }
    if !sc.relics.iter().all(|r| relic_is_neutral(r.id)) {
        return None;
    }
    let cx = Combat::try_new_with(sc, ex).ok()?;
    if cx.enemies.len() != 1 || cx.stage != Stage::AwaitAction || !cx.cr(PLAYER).powers.is_empty() || cx.player.energy != sc.max_energy {
        return None;
    }
    let e = cx.enemies[0];
    let enemy_hp = cx.cr(e).hp;
    if enemy_hp <= 0 || cx.cr(e).monster.next_move == NO || !enemy_is_boundable(&cx, e) {
        return None;
    }
    // ---- the deck ----
    let mut cache: HashMap<(u16, u8), CardClass> = HashMap::new();
    let mut items: Vec<CardProfile> = vec![];
    for dc in sc.deck.iter() {
        let cl = *cache.entry((dc.id, dc.upgrade)).or_insert_with(|| classify_card(sc.character, sc.ascension, sc.max_energy, dc.id, dc.upgrade));
        match cl {
            CardClass::Pure(p) => items.push(p),
            CardClass::Junk => {}
            CardClass::Impure => return None,
        }
    }
    let can_vuln = items.iter().any(|p| p.vuln);
    let can_weak = items.iter().any(|p| p.weak);
    let dmg = |p: &CardProfile| if can_vuln { p.damage.max(p.damage_vuln) } else { p.damage } as i64;
    // ---- frontier: for every block value the most damage one turn can deal ----
    let energy = sc.max_energy.max(0) as usize;
    let bcap: usize = items.iter().map(|p| p.block.max(0) as usize).sum::<usize>().min(600);
    const NEG: i64 = i64::MIN / 4;
    let mut dp = vec![vec![NEG; bcap + 1]; energy + 1];
    dp[0][0] = 0;
    for p in items.iter() {
        let c = p.cost.max(0) as usize;
        let b = p.block.max(0) as usize;
        let d = dmg(p);
        for en in (0..=energy).rev() {
            if en < c {
                continue;
            }
            for bl in (0..=bcap).rev() {
                if bl < b {
                    continue;
                }
                let prev = dp[en - c][bl - b];
                if prev > NEG && prev + d > dp[en][bl] {
                    dp[en][bl] = prev + d;
                }
            }
        }
    }
    let mut frontier: Vec<(i64, usize)> = vec![]; // (damage, block)
    for bl in 0..=bcap {
        let d = (0..=energy).map(|en| dp[en][bl]).max().unwrap_or(NEG);
        if d > NEG {
            frontier.push((d, bl));
        }
    }
    let d_max = frontier.iter().map(|f| f.0).max().unwrap_or(0);
    // ---- enemy damage per turn (lower bound) ----
    let mut layer: Vec<MonsterState> = vec![cx.cr(e).monster];
    let mut incoming: Vec<i64> = vec![];
    for _ in 0..MAX_TURNS {
        let dmg_t = layer.iter().map(|ms| cx.bound_node_damage(e, ms.cur_state, can_weak)).min().unwrap_or(0);
        incoming.push(dmg_t);
        let mut next: Vec<MonsterState> = vec![];
        for ms in layer.iter() {
            let mut out = vec![];
            cx.bound_next_states(e, ms, &mut out);
            for s in out {
                if !next.iter().any(|m| Combat::bound_same_state(m, &s)) {
                    next.push(s);
                }
            }
            if next.len() > MAX_STATES {
                return None;
            }
        }
        if next.is_empty() {
            return None;
        }
        layer = next;
    }
    // ---- DP over turns: state = HP lost so far, value = most damage dealt so far ----
    let hp = sc.hp.max(1) as usize;
    let mut cur = vec![NEG; hp]; // loss in 0..hp-1 (the player is alive)
    cur[0] = 0;
    let mut best_total: i64 = 0;
    for t in 0..MAX_TURNS {
        // can the enemy die this turn (before it acts)?
        for l in 0..hp {
            if cur[l] > NEG && cur[l] + d_max >= enemy_hp as i64 {
                return None;
            }
        }
        let mut next = vec![NEG; hp];
        let inc = incoming[t];
        let mut alive = false;
        for l in 0..hp {
            if cur[l] == NEG {
                continue;
            }
            best_total = best_total.max(cur[l]);
            for &(d, bl) in frontier.iter() {
                let loss = (inc - bl as i64).max(0) as usize;
                let nl = l + loss;
                if nl >= hp {
                    continue;
                }
                let v = cur[l] + d;
                if v > next[nl] {
                    next[nl] = v;
                }
                alive = true;
            }
        }
        if !alive {
            return Some(Proof { enemy_hp, player_hp: sc.hp, max_damage: best_total + d_max, max_turns: t + 1 });
        }
        cur = next;
    }
    None // the player may survive beyond the horizon: nothing proven
}
