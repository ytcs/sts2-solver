//! The agent's observation: exactly the information a human player has — no more, no less.
//!
//! Visible: HP/block/energy/powers, relics + counters, potions, the hand in order, the draw pile as an *unordered*
//! multiset (the game's own pile screen sorts it by rarity then id), discard and exhaust in pile order, every enemy's
//! HP/block/powers/current intent (type, per-hit damage as the UI computes it, hit count) and the moves it has already
//! performed, and any pending decision's candidates.
//! Hidden: draw-pile order, every RNG stream, enemies' future moves beyond the displayed intent.
//! `tests/observe.rs` perturbs the hidden state and asserts the observation is unchanged.

use crate::content;
use crate::dec::Dec;
use crate::defs::*;
use crate::state::*;
use crate::engine::{LOOK_H, LOOK_NODES};
use crate::types::*;

pub const OBS_MAX_ENEMIES: usize = 8;
pub const OBS_MAX_PILE: usize = 64;
pub const OBS_MAX_CANDS: usize = 16;
pub const OBS_POWERS: usize = 16;
pub const OBS_INTENTS: usize = 3;

/// Per-card features (`CARD_F` floats).
pub const CARD_F: usize = 12;
/// Per-enemy features (`ENEMY_F` floats).
pub const ENEMY_F: usize = 8 + OBS_POWERS * 2 + OBS_INTENTS * 3 + 4;
const GLOBAL_F: usize = 10;
const PLAYER_F: usize = 8 + OBS_POWERS * 2;
const RELIC_F: usize = OBS_RELICS * 2;
const POTION_F: usize = MAX_POTIONS * 2;
/// Regent block (appended at the END of the vector): current star cost of each hand card (-1 = none, X = all stars is
/// reported as -2) and of each decision candidate. Stars themselves are in the player block.
const REGENT_F: usize = MAX_HAND + OBS_MAX_CANDS;
const DECISION_F: usize = 8 + OBS_MAX_CANDS * (CARD_F + 1);
/// Osty block (appended at the END of the vector): present, alive, hp, max_hp, powers (id+1, amount) x `OBS_POWERS`,
/// then per hand slot the damage preview of an Osty attack card (what the card text shows), 0 otherwise.
pub const OSTY_F: usize = 4 + OBS_POWERS * 2 + MAX_HAND;
/// Per-orb features: (kind + 1, passive value, evoke value).
pub const ORB_F: usize = 3;
/// Orb block (appended at the end of the vector): `MAX_ORBS` orb entries front first (the slot count is the
/// `orb_slots` field of the player block), then the number of Lightning orbs channeled this combat (Voltaic's text).
pub const ORBS_F: usize = MAX_ORBS * ORB_F + 1;
/// Expert pattern knowledge (appended at the END of the vector): per enemy slot and per future turn (`LOOK_H`), the probability
/// of each move node and the expected total attack damage (see `Combat::lookahead`). The enemy's identity and node indices
/// are in the enemy block, so the agent learns what each node means exactly as a player learns a monster.
pub const LOOK_F: usize = OBS_MAX_ENEMIES * LOOK_H * (LOOK_NODES + 1);
/// Where each enemy's move pattern stands (appended at the END of the vector, S1): per enemy slot, the pending move node + 1 (the
/// intent block shows only its intent types; 255 = stunned) and the node it resumes after a stun / a stored follow-up + 1 (0 when
/// none is pending), in the encoding of the performed-move history.
pub const MOVE_STATE_F: usize = 2;
pub const ENEMY_MOVES_F: usize = OBS_MAX_ENEMIES * MOVE_STATE_F;
/// Total length of the flat observation vector.
pub const OBS_SIZE: usize = GLOBAL_F
    + PLAYER_F
    + RELIC_F
    + POTION_F
    + MAX_HAND * CARD_F
    + OBS_MAX_PILE * 2 * 3 // draw multiset, discard, exhaust: (id+1, upgrade) per slot
    + 3 // pile sizes
    + OBS_MAX_ENEMIES * ENEMY_F
    + DECISION_F
    + REGENT_F
    + OSTY_F
    + ORBS_F
    + LOOK_F
    + ENEMY_MOVES_F;

struct W<'a> {
    out: &'a mut [f32],
    i: usize,
}
/// Observation leaves out relics without a combat effect (`relic_mask::OBSERVED`). On by default; off reproduces observations from before M2
/// (networks trained before the mask, A/B tests).
pub static MASK_RELICS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

impl W<'_> {
    #[inline(always)]
    fn f(&mut self, v: f32) {
        self.out[self.i] = v;
        self.i += 1;
    }
    #[inline(always)]
    fn n(&mut self, v: i32) {
        self.f(v as f32)
    }
    /// `n` zero floats. The whole vector is zero-filled once up front (one memset) so skipping is enough.
    #[inline(always)]
    fn zeros(&mut self, n: usize) {
        self.i += n;
    }
}

/// Cycle counters per section of `observe_ex` (feature `obs_prof`; not thread safe, diagnostics only).
#[cfg(feature = "obs_prof")]
pub static mut OBS_PROF: [u64; 16] = [0; 16];
#[cfg(feature = "obs_prof")]
#[inline(always)]
fn tsc() -> u64 {
    // SAFETY: rdtsc has no preconditions.
    unsafe { core::arch::x86_64::_rdtsc() }
}
macro_rules! prof {
    ($k:expr, $t:ident, $body:block) => {{
        #[cfg(feature = "obs_prof")]
        let $t = tsc();
        $body
        #[cfg(feature = "obs_prof")]
        unsafe {
            OBS_PROF[$k] += tsc() - $t;
        }
    }};
}

impl Combat {
    /// Intent damage as the UI computes it: `Hook.ModifyDamage(dealer = monster, target = player, Move)` floored at 0.
    pub fn intent_damage(&self, monster: Cid, base: i32) -> i32 {
        self.modify_damage_value(PLAYER, monster, Dec::int(base as i64), ValueProp::MOVE, NO).trunc().max(0)
    }

    /// Star cost as shown on the card: -1 none, -2 X (all stars), else the current cost with modifiers.
    fn obs_star_cost(&self, c: CardIdx) -> i32 {
        if self.card_has_star_cost_x(c) {
            -2
        } else if self.card_current_star_cost(c) < 0 {
            -1
        } else {
            self.card_star_cost(c)
        }
    }

    /// `playable`: the already computed `can_play` of a hand card (see `observe_ex`), `None` = compute it here.
    fn write_card(&self, w: &mut W, c: CardIdx, playable: Option<bool>) {
        let card = &self.cards[c as usize];
        let d = content::card_def(card.id);
        let playable = match playable {
            Some(p) => p as i32,
            None => (self.stage == Stage::AwaitAction && self.player.phase == Phase::Play && self.card_pile_type(c) == PileType::Hand && self.can_play(c)) as i32,
        };
        #[cfg(feature = "obs_prof")]
        let t6 = tsc();
        let dmg = if d.vars.iter().any(|v| v.kind == VarKind::Damage) {
            self.modify_damage_value(NO, PLAYER, Dec::int(self.card_base_damage(c) as i64), ValueProp::MOVE, c).trunc()
        } else {
            0
        };
        #[cfg(feature = "obs_prof")]
        unsafe {
            OBS_PROF[6] += tsc() - t6;
        }
        #[cfg(feature = "obs_prof")]
        let t7 = tsc();
        let blk = if d.vars.iter().any(|v| v.kind == VarKind::Block) {
            self.modify_block(PLAYER, Dec::int(self.card_var(c, VarKind::Block) as i64), ValueProp::MOVE, c).trunc()
        } else {
            0
        };
        #[cfg(feature = "obs_prof")]
        unsafe {
            OBS_PROF[7] += tsc() - t7;
        }
        w.n(card.id as i32 + 1);
        w.n(card.upgrade as i32);
        #[cfg(feature = "obs_prof")]
        let t8 = tsc();
        w.n(if d.x_cost { -1 } else { self.card_cost(c, true).max(0) });
        #[cfg(feature = "obs_prof")]
        unsafe {
            OBS_PROF[8] += tsc() - t8;
        }
        w.n(playable);
        #[cfg(feature = "obs_prof")]
        let t9 = tsc();
        w.n(self.card_keywords(c) as i32);
        #[cfg(feature = "obs_prof")]
        unsafe {
            OBS_PROF[9] += tsc() - t9;
        }
        w.n(card.enchant as i32);
        w.n(dmg);
        w.n(blk);
        w.n(card.counter[0] as i32);
        w.n(card.counter[1] as i32);
        w.n(card.enchant_amount as i32);
        w.n(card.affliction as i32);
    }

    /// `(id + 1, amount)` of the first `OBS_POWERS` powers, zeros for the rest.
    #[inline(always)]
    fn write_powers(w: &mut W, cr: &Creature) {
        let n = cr.powers.len().min(OBS_POWERS);
        for p in &cr.powers.as_slice()[..n] {
            w.n(p.id as i32 + 1);
            w.n(p.amount);
        }
        w.zeros((OBS_POWERS - n) * 2);
    }

    fn write_pile_list(&self, w: &mut W, pile: &[CardIdx], sorted_multiset: bool) {
        let n = pile.len().min(OBS_MAX_PILE);
        if sorted_multiset {
            // order-free view: sort by (rarity, id, upgrade) — what the pile screen shows. The sort key is packed into one u32
            // (equal keys are identical entries, so an unstable sort of keys gives the same vector as a stable sort of cards).
            let mut keys = [0u32; OBS_MAX_PILE];
            for (k, &c) in pile.iter().take(n).enumerate() {
                let card = &self.cards[c as usize];
                keys[k] = (content::card_def(card.id).rarity as u32) << 24 | (card.id as u32) << 8 | card.upgrade as u32;
            }
            keys[..n].sort_unstable();
            for &key in &keys[..n] {
                w.n(((key >> 8) & 0xFFFF) as i32 + 1);
                w.n((key & 0xFF) as i32);
            }
        } else {
            for &c in pile.iter().take(n) {
                w.n(self.cards[c as usize].id as i32 + 1);
                w.n(self.cards[c as usize].upgrade as i32);
            }
        }
        w.zeros((OBS_MAX_PILE - n) * 2);
    }

    /// Writes the flat observation into `out` (`out.len() >= OBS_SIZE`). Returns `OBS_SIZE`.
    pub fn observe(&self, out: &mut [f32]) -> usize {
        self.observe_ex(out, None)
    }

    /// `observe` with the playability of the hand cards precomputed: bit `k` of `hand_playable` = `can_play(hand[k])` (as
    /// produced by `legal_actions_ex`), so a batch env that also needs the legal actions evaluates each hand card once.
    pub fn observe_ex(&self, out: &mut [f32], hand_playable: Option<u16>) -> usize {
        let out = &mut out[..OBS_SIZE];
        // SAFETY: `out` has exactly OBS_SIZE f32s; all-zero bytes are +0.0. (`fill(0.0)` compiled to a store loop.)
        unsafe { core::ptr::write_bytes(out.as_mut_ptr(), 0, OBS_SIZE) };
        let mut w = W { out, i: 0 };
        let hand_ok = self.stage == Stage::AwaitAction && self.player.phase == Phase::Play;
        let me = self.cr(PLAYER);
        // ---- global ----
        w.n(self.round);
        w.n(self.player.turn_number);
        w.n((self.stage == Stage::AwaitAction) as i32);
        w.n((self.stage == Stage::AwaitChoice) as i32);
        w.n((self.stage == Stage::Over) as i32);
        w.n(matches!(self.outcome, Outcome::Victory) as i32);
        w.n(self.hist.cards_played_this_turn as i32);
        w.n(self.hist.attacks_played_this_turn as i32);
        w.n(self.hist.skills_played_this_turn as i32);
        w.n(self.ascension as i32);
        // ---- player ----
        prof!(5, t5, {
        w.n(me.hp);
        w.n(me.max_hp);
        w.n(me.block);
        w.n(self.player.energy);
        w.n(self.max_energy());
        w.n(self.player.stars);
        w.n(self.player.orb_slots as i32);
        w.n(self.player.potion_slots as i32);
        Self::write_powers(&mut w, me);
        // relics with no combat effect are not shown (`relic_mask`, `docs/rl_redesign.md` M2) unless the mask is switched off
        let mask = MASK_RELICS.load(std::sync::atomic::Ordering::Relaxed);
        let mut n_relics = 0;
        for r in self.player.relics.as_slice().iter().filter(|r| !mask || crate::relic_mask::OBSERVED.get(r.id as usize).copied().unwrap_or(true)).take(OBS_RELICS) {
            n_relics += 1;
            w.n(r.id as i32 + 1);
            // The counter a player can see on the relic (`ShowCounter ? DisplayAmount`), not the raw state slot.
            w.n(crate::content::relic_listener(r.id).meta_display(self, r).unwrap_or(0));
        }
        w.zeros((OBS_RELICS - n_relics) * 2);
        });
        for k in 0..MAX_POTIONS {
            match self.player.potions[k] {
                Some(p) => {
                    w.n(p.id as i32 + 1);
                    w.n(1);
                }
                None => w.zeros(2),
            }
        }
        // ---- hand (ordered) ----
        prof!(1, t1, {
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => {
                    let pre = hand_playable.map(|m| hand_ok && m >> k & 1 != 0);
                    self.write_card(&mut w, c, pre)
                }
                None => w.zeros(CARD_F),
            }
        }
        });
        // ---- piles ----
        prof!(2, t2, {
        self.write_pile_list(&mut w, self.player.draw.as_slice(), true);
        self.write_pile_list(&mut w, self.player.discard.as_slice(), true);
        self.write_pile_list(&mut w, self.player.exhaust.as_slice(), true);
        });
        w.n(self.player.draw.len() as i32);
        w.n(self.player.discard.len() as i32);
        w.n(self.player.exhaust.len() as i32);
        // ---- enemies (list order) ----
        prof!(3, t3, {
        for k in 0..OBS_MAX_ENEMIES {
            let Some(e) = self.enemies.get(k) else {
                w.zeros(ENEMY_F);
                continue;
            };
            let cr = self.cr(e);
            let ms = &cr.monster;
            w.n(1);
            w.n(e as i32);
            w.n(ms.id as i32 + 1);
            w.n(cr.hp);
            w.n(cr.max_hp);
            w.n(cr.block);
            w.n(cr.is_alive() as i32);
            w.n(self.is_stunned(e) as i32);
            Self::write_powers(&mut w, cr);
            // current intent(s)
            let mut n_int = 0;
            if ms.next_move != NO {
                if let Some((_, intents)) = self.move_view(e) {
                    for it in intents.iter().take(OBS_INTENTS) {
                        let (kind, dmg, hits) = match it {
                            Intent::Attack { damage, hits } => (1, self.intent_damage(e, damage(self, e)), hits(self, e)),
                            Intent::Buff => (2, 0, 0),
                            Intent::Debuff => (3, 0, 0),
                            Intent::DebuffStrong => (4, 0, 0),
                            Intent::Defend => (5, 0, 0),
                            Intent::Escape => (6, 0, 0),
                            Intent::Heal => (7, 0, 0),
                            Intent::Hidden => (8, 0, 0),
                            Intent::Summon => (9, 0, 0),
                            Intent::Sleep => (10, 0, 0),
                            Intent::Stun => (11, 0, 0),
                            Intent::StatusCard => (12, 0, 0),
                            Intent::CardDebuff => (13, 0, 0),
                            Intent::DeathBlow => (14, 0, 0),
                            Intent::DeathBlowAttack { damage } => (14, self.intent_damage(e, damage(self, e)), 1),
                        };
                        w.n(kind);
                        w.n(dmg);
                        w.n(hits);
                        n_int += 1;
                    }
                }
            }
            w.zeros((OBS_INTENTS - n_int) * 3);
            for j in 0..4 {
                w.n(if ms.performed[j] == NO { 0 } else { ms.performed[j] as i32 + 1 });
            }
        }
        });
        // ---- pending decision ----
        match &self.decision {
            Some(d) => {
                w.n(1);
                w.n(match d.source {
                    DecisionSource::Hand => 1,
                    DecisionSource::Pile(p) => 1 + p as i32,
                    DecisionSource::Options => 8,
                });
                w.n(d.min as i32);
                w.n(d.max as i32);
                w.n(d.selected.len() as i32);
                w.n(d.confirm_required as i32);
                w.n(d.can_skip as i32);
                w.n(d.cands.len() as i32);
                let view = self.decision_view(d);
                for k in 0..OBS_MAX_CANDS {
                    // displayed order (`view`), never the game's pile order
                    match view.get(k).map(|vi| (vi, d.cands[vi as usize])) {
                        Some((vi, c)) => {
                            self.write_card(&mut w, c, None);
                            w.n(d.selected.contains(vi) as i32);
                        }
                        None => w.zeros(CARD_F + 1),
                    }
                }
            }
            None => w.zeros(DECISION_F),
        }
        // ---- Regent: star costs (appended) ----
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => w.n(self.obs_star_cost(c)),
                None => w.f(0.0),
            }
        }
        let star_view = self.decision.as_ref().map(|d| self.decision_view(d));
        for k in 0..OBS_MAX_CANDS {
            match self.decision.as_ref().and_then(|d| star_view.as_ref().and_then(|v| v.get(k)).map(|vi| d.cands[vi as usize])) {
                Some(c) => w.n(self.obs_star_cost(c)),
                None => w.f(0.0),
            }
        }
        // ---- Osty (visible to the player: portrait, HP bar, powers; block is the owner's) ----
        match self.osty() {
            Some(o) => {
                let cr = self.cr(o);
                w.n(1);
                w.n(cr.is_alive() as i32);
                w.n(cr.hp);
                w.n(cr.max_hp);
                Self::write_powers(&mut w, cr);
            }
            None => w.zeros(4 + OBS_POWERS * 2),
        }
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) if self.card_def(c).vars.iter().any(|v| v.kind == VarKind::OstyDamage) && self.osty().is_some() => {
                    let o = self.osty().unwrap();
                    let base = Dec::int(self.card_var(c, VarKind::OstyDamage) as i64);
                    w.n(self.modify_damage_value(NO, o, base, ValueProp::MOVE, c).trunc());
                }
                _ => w.f(0.0),
            }
        }
        // ---- Defect: orbs (appended; visible to the player: each orb with its current passive / evoke value) ----
        for k in 0..MAX_ORBS {
            match self.player.orbs.get(k) {
                Some(o) => {
                    w.n(o.kind as i32 + 1);
                    w.n(self.orb_passive_val(&o).trunc());
                    w.n(self.orb_evoke_val(&o).trunc());
                }
                None => w.zeros(ORB_F),
            }
        }
        w.n(self.hist_log.lightning_channeled as i32);
        // ---- expert pattern knowledge about upcoming enemy turns (appended) ----
        prof!(4, t4, {
        for k in 0..OBS_MAX_ENEMIES {
            match self.enemies.get(k) {
                Some(e) if self.cr(e).is_alive() => {
                    for row in self.lookahead(e).iter() {
                        for &p in row.prob.iter() {
                            w.f(p);
                        }
                        w.f(row.exp_damage);
                    }
                }
                _ => w.zeros(LOOK_H * (LOOK_NODES + 1)),
            }
        }
        });
        // ---- where each enemy's pattern stands (appended) ----
        for k in 0..OBS_MAX_ENEMIES {
            match self.enemies.get(k) {
                Some(e) if self.cr(e).monster.next_move != NO => {
                    let ms = &self.cr(e).monster;
                    w.n(ms.next_move as i32 + 1);
                    let stored = match content::monster_def(ms.id).nodes.get(ms.next_move as usize) {
                        _ if ms.next_move == crate::engine::STUN_NODE => true,
                        Some(MonsterNode::Move { follow_up, .. }) => *follow_up == FOLLOW_STORED,
                        _ => false,
                    };
                    w.n(if stored && ms.stun_follow_up != NO { ms.stun_follow_up as i32 + 1 } else { 0 });
                }
                _ => w.zeros(MOVE_STATE_F),
            }
        }
        debug_assert_eq!(w.i, OBS_SIZE);
        OBS_SIZE
    }
}

/// The sections of the observation vector in order: `(name, offset, size)`. The sizes sum to `OBS_SIZE` (tested).
pub fn layout() -> Vec<(&'static str, usize, usize)> {
    let sizes: [(&'static str, usize); 17] = [
        ("global", GLOBAL_F),
        ("player", PLAYER_F),
        ("relics", RELIC_F),
        ("potions", POTION_F),
        ("hand", MAX_HAND * CARD_F),
        ("draw", OBS_MAX_PILE * 2),
        ("discard", OBS_MAX_PILE * 2),
        ("exhaust", OBS_MAX_PILE * 2),
        ("pile_sizes", 3),
        ("enemies", OBS_MAX_ENEMIES * ENEMY_F),
        ("decision", DECISION_F),
        ("regent", REGENT_F),
        ("osty", OSTY_F),
        ("orbs", ORBS_F),
        ("look", LOOK_F),
        ("enemy_moves", ENEMY_MOVES_F),
        ("end", 0),
    ];
    let mut out = vec![];
    let mut off = 0;
    for (n, sz) in sizes {
        out.push((n, off, sz));
        off += sz;
    }
    out
}

/// Constants a consumer of the observation / action space needs (strides, capacities, vocabulary sizes, action offsets).
pub fn layout_consts() -> Vec<(&'static str, usize)> {
    use crate::engine::{ACTION_SPACE, MAX_PICK};
    vec![
        ("OBS_SIZE", OBS_SIZE),
        ("ACTION_SPACE", ACTION_SPACE),
        ("CARD_F", CARD_F),
        ("ENEMY_F", ENEMY_F),
        ("GLOBAL_F", GLOBAL_F),
        ("PLAYER_F", PLAYER_F),
        ("DECISION_F", DECISION_F),
        ("OBS_MAX_ENEMIES", OBS_MAX_ENEMIES),
        ("OBS_MAX_PILE", OBS_MAX_PILE),
        ("OBS_MAX_CANDS", OBS_MAX_CANDS),
        ("OBS_POWERS", OBS_POWERS),
        ("OBS_INTENTS", OBS_INTENTS),
        ("MAX_HAND", MAX_HAND),
        ("MAX_POTIONS", MAX_POTIONS),
        ("MAX_RELICS", OBS_RELICS),
        ("MAX_ORBS", MAX_ORBS),
        ("MAX_CREATURES", MAX_CREATURES),
        ("MAX_PICK", MAX_PICK),
        ("LOOK_H", LOOK_H),
        ("LOOK_NODES", LOOK_NODES),
        ("MOVE_STATE_F", MOVE_STATE_F),
        ("N_CARDS", crate::ids::card::COUNT),
        ("N_POWERS", crate::ids::power::COUNT),
        ("N_RELICS", crate::ids::relic::COUNT),
        ("N_POTIONS", crate::ids::potion::COUNT),
        ("N_MONSTERS", crate::ids::monster::COUNT),
        ("N_ENCHANTMENTS", crate::ids::enchantment::COUNT),
        ("N_AFFLICTIONS", crate::ids::affliction::COUNT),
        ("N_ORBS", crate::ids::orb::COUNT),
        ("OFF_PLAY", 1),
        ("OFF_POTION", 1 + MAX_HAND * (MAX_CREATURES + 1)),
        ("OFF_DISCARD", 1 + MAX_HAND * (MAX_CREATURES + 1) + MAX_POTIONS * (MAX_CREATURES + 1)),
        ("OFF_PICK", 1 + MAX_HAND * (MAX_CREATURES + 1) + MAX_POTIONS * (MAX_CREATURES + 1) + MAX_POTIONS),
        ("OFF_CONFIRM", 1 + MAX_HAND * (MAX_CREATURES + 1) + MAX_POTIONS * (MAX_CREATURES + 1) + MAX_POTIONS + MAX_PICK),
    ]
}
