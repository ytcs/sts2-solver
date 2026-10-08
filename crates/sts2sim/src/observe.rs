//! The agent's observation: exactly the information a human player has — no more, no less.
//!
//! Visible: HP/block/energy/powers, relics + counters, potions, the hand in order, the draw pile as an *unordered*
//! multiset (the game's own pile screen sorts it by rarity then id), discard and exhaust in pile order, every enemy's
//! HP/block/powers/current intent (type, per-hit damage as the UI computes it, hit count) and the moves it has already
//! performed, and any pending decision's candidates.
//! Hidden: draw-pile order, every RNG stream, enemies' future moves beyond the displayed intent.
//! `tests/observe.rs` perturbs the hidden state and asserts the observation is unchanged.
//!
//! Two versions of the vector exist (`OBS_VERSION`, `observe_v`). Version 1 is what every network before v2 was trained on and stays
//! bit-identical (`tests/rl/test_obs_version.py`). Version 2 adds the visible information v1 leaves out:
//! * per card (`CARD_F_V2` = 15): the numbers the card text shows for calculated damage / block (Body Slam, Perfected Strike, Unleash,
//!   Expect a Fight ...) in the existing damage / block fields, a `count` field for the other calculated numbers (Finisher's hits, No Escape's
//!   Doom, Normality's cards left ...), the affliction amount and the replay count; each damage / block var previewed with its own `ValueProp`
//!   (status damage gets no Strength); single-turn Retain / Sly set the keyword bits. Every preview is `Calculate(target = none)`: the number the
//!   card shows at rest (target-dependent cards such as Time's Up show their base).
//! * per power (`POWER_F_V2` = 3): the number the power icon displays when it is not the amount (`PowerModel.DisplayAmount`; Surrounded's facing).
//! * the pending selection: up to 64 candidates (all pickable ones), what asked for it (`dec_source`: card / potion / relic / monster id) and
//!   the card being played (`played`).
//! * piles: sorted before the 64-entry cap is applied (v1 truncates first, which leaks a little of the hidden order of a big pile).

use crate::content;
use crate::dec::Dec;
use crate::defs::*;
use crate::hooks::{Kind, Me};
use crate::state::*;
use crate::engine::{LOOK_H, LOOK_NODES};
use crate::types::*;
use std::sync::atomic::{AtomicU8, Ordering};

pub const OBS_MAX_ENEMIES: usize = 8;
pub const OBS_MAX_PILE: usize = 64;
/// Selection candidates shown, v1 (v2: `OBS_MAX_CANDS_V2`).
pub const OBS_MAX_CANDS: usize = 16;
/// v2: every candidate that can be picked (`engine::MAX_PICK`).
pub const OBS_MAX_CANDS_V2: usize = 64;
pub const OBS_POWERS: usize = 16;
pub const OBS_INTENTS: usize = 3;

/// Per-card features (`CARD_F` floats), v1.
pub const CARD_F: usize = 12;
/// v2: v1's twelve, then the calculated count, the affliction amount and the replay count.
pub const CARD_F_V2: usize = 15;
/// Floats per power slot: (id + 1, amount), v2 also the displayed number.
pub const POWER_F: usize = 2;
pub const POWER_F_V2: usize = 3;
/// Per-enemy features (`ENEMY_F` floats), v1.
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
/// then per hand slot the damage preview of an Osty attack card (what the card text shows), 0 otherwise. (v1 sizes.)
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
/// v2, appended: what asked for the pending selection, `(kind, id + 1)`; kind 1 card, 2 potion, 3 relic, 4 monster (`purpose`), zeros
/// without a selection.
pub const DEC_SOURCE_F: usize = 2;
/// v2, appended: the card being played (the innermost play in flight: the card whose effect asked for the selection), a card entry
/// (`CARD_F_V2`) then its star cost and Osty damage preview; zeros when no card is being played.
pub const PLAYED_F: usize = CARD_F_V2 + 2;

/// The sizes of one version's vector.
#[derive(Clone, Copy, Debug)]
pub struct Dims {
    pub version: u8,
    pub card_f: usize,
    pub cands: usize,
    pub power_f: usize,
    pub player_f: usize,
    pub enemy_f: usize,
    pub decision_f: usize,
    pub regent_f: usize,
    pub osty_f: usize,
    pub dec_source_f: usize,
    pub played_f: usize,
    pub size: usize,
}

/// The sizes of observation version `version` (1 or 2).
pub const fn dims(version: u8) -> Dims {
    let v2 = version >= 2;
    let card_f = if v2 { CARD_F_V2 } else { CARD_F };
    let cands = if v2 { OBS_MAX_CANDS_V2 } else { OBS_MAX_CANDS };
    let power_f = if v2 { POWER_F_V2 } else { POWER_F };
    let player_f = 8 + OBS_POWERS * power_f;
    let enemy_f = 8 + OBS_POWERS * power_f + OBS_INTENTS * 3 + 4;
    let decision_f = 8 + cands * (card_f + 1);
    let regent_f = MAX_HAND + cands;
    let osty_f = 4 + OBS_POWERS * power_f + MAX_HAND;
    let dec_source_f = if v2 { DEC_SOURCE_F } else { 0 };
    let played_f = if v2 { PLAYED_F } else { 0 };
    let size = GLOBAL_F
        + player_f
        + RELIC_F
        + POTION_F
        + MAX_HAND * card_f
        + OBS_MAX_PILE * 2 * 3 // draw multiset, discard, exhaust: (id+1, upgrade) per slot
        + 3 // pile sizes
        + OBS_MAX_ENEMIES * enemy_f
        + decision_f
        + regent_f
        + osty_f
        + ORBS_F
        + LOOK_F
        + ENEMY_MOVES_F
        + dec_source_f
        + played_f;
    Dims { version, card_f, cands, power_f, player_f, enemy_f, decision_f, regent_f, osty_f, dec_source_f, played_f, size }
}

/// Total length of the flat observation vector, version 1.
pub const OBS_SIZE: usize = dims(1).size;
/// Total length, version 2.
pub const OBS_SIZE_V2: usize = dims(2).size;
/// The longest vector of any version (buffers that must hold every version).
pub const OBS_SIZE_MAX: usize = OBS_SIZE_V2;
/// The newest observation version.
pub const OBS_VERSION_MAX: u8 = 2;

const _: () = assert!(dims(1).player_f == PLAYER_F && dims(1).enemy_f == ENEMY_F && dims(1).decision_f == DECISION_F);
const _: () = assert!(dims(1).regent_f == REGENT_F && dims(1).osty_f == OSTY_F && OBS_MAX_CANDS_V2 == crate::engine::MAX_PICK);

/// The process-wide observation version: what `observe` / `observe_ex` write and what an environment, search engine or replay created
/// without an explicit version uses. 1 by default (every network trained before v2).
pub static OBS_VERSION: AtomicU8 = AtomicU8::new(1);

/// The process-wide observation version.
pub fn obs_version() -> u8 {
    OBS_VERSION.load(Ordering::Relaxed)
}

/// Sets the process-wide observation version (1 or 2); returns the previous one, `None` (unchanged) for an unknown version.
pub fn set_obs_version(version: u8) -> Option<u8> {
    if !(1..=OBS_VERSION_MAX).contains(&version) {
        return None;
    }
    Some(OBS_VERSION.swap(version, Ordering::Relaxed))
}

/// Length of the observation vector of `version` (0 for an unknown version).
pub const fn obs_size(version: u8) -> usize {
    match version {
        1 | 2 => dims(version).size,
        _ => 0,
    }
}

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
pub static mut OBS_PROF: [u64; 32] = [0; 32];
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

/// The numbers a card's text shows (v2 observation): see [`Combat::card_preview`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardPreview {
    /// Damage dealt by the player: a `DamageVar` or a `CalculatedDamageVar` not from Osty, through `Hook.ModifyDamage`; 0 when none.
    pub damage: i32,
    /// Block: a `BlockVar` or a `CalculatedBlockVar`, through `Hook.ModifyBlock`; 0 when none.
    pub block: i32,
    /// Another calculated number (`CalculatedHits`, `CalculatedCards`, `CalculatedDoom` ...); 0 when none.
    pub count: i32,
    /// Damage dealt by Osty (`OstyDamageVar` / a `CalculatedDamageVar.FromOsty`); `None` when the card has none or Osty was never summoned.
    pub osty_damage: Option<i32>,
}

/// Kind codes of `dec_source` (v2).
pub mod source_kind {
    pub const CARD: i32 = 1;
    pub const POTION: i32 = 2;
    pub const RELIC: i32 = 3;
    pub const MONSTER: i32 = 4;
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

    /// The numbers the card shows with no target selected (`DynamicVar.UpdateCardPreview(target = null, runGlobalHooks)`): each damage /
    /// block var through the damage / block hooks with its own `ValueProp`; calculated vars from the card's `calculated_damage` /
    /// `calculated_value` hooks (`CalculatedVar.Calculate(null)`; damage floored at 0). A card from Osty (`OSTY_ATTACK` tag: Unleash, Squeeze,
    /// Protector, Rattle ...) reports its damage as `osty_damage` with Osty as the dealer.
    pub fn card_preview(&self, c: CardIdx) -> CardPreview {
        let d = self.card_def(c);
        let mut p = CardPreview::default();
        let osty = self.osty();
        let me = Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 };
        let li = content::listener(&me);
        for v in d.vars.iter() {
            let props = ValueProp(v.props);
            match v.kind {
                VarKind::Damage => p.damage = self.modify_damage_value(NO, PLAYER, Dec::int(self.card_base_damage(c) as i64), props, c).trunc(),
                VarKind::Block => p.block = self.modify_block(PLAYER, Dec::int(self.card_var(c, VarKind::Block) as i64), props, c).trunc(),
                VarKind::OstyDamage => {
                    if let Some(o) = osty {
                        p.osty_damage = Some(self.modify_damage_value(NO, o, Dec::int(self.card_var(c, VarKind::OstyDamage) as i64), props, c).trunc());
                    }
                }
                VarKind::CalcDamage => {
                    let Some(base) = li.calculated_damage(self, c, NO) else { continue };
                    if d.tags & tag::OSTY_ATTACK != 0 {
                        if let Some(o) = osty {
                            p.osty_damage = Some(self.modify_damage_value(NO, o, base, props, c).trunc().max(0));
                        }
                    } else {
                        p.damage = self.modify_damage_value(NO, PLAYER, base, props, c).trunc().max(0);
                    }
                }
                VarKind::CalcBlock => {
                    if let Some(base) = li.calculated_value(self, c, NO) {
                        p.block = self.modify_block(PLAYER, base, props, c).trunc();
                    }
                }
                _ => {}
            }
        }
        if !d.vars.iter().any(|v| v.kind == VarKind::CalcBlock) {
            if let Some(n) = li.calculated_value(self, c, NO) {
                p.count = n.trunc();
            }
        }
        p
    }

    /// The number a power's icon shows when it is not the amount (`PowerModel.DisplayAmount` overrides, shown for `Counter` powers), and
    /// Surrounded's facing (1 right, 2 left; drawn as the player's orientation). 0 for every other power.
    pub fn power_display(&self, p: &Power) -> i32 {
        use crate::ids::power as pw;
        let aux = p.aux;
        match p.id {
            pw::AUTOMATION_POWER => 10 - aux,
            pw::CACOPHONY_POWER => 33 - aux,
            pw::FERAL_POWER | pw::HARDENED_SHELL_POWER => (p.amount - aux).max(0),
            pw::MONOLOGUE_POWER => aux & 0xFFFF,
            pw::ORBIT_POWER => 4 - aux.rem_euclid(4),
            pw::PALE_BLUE_DOT_POWER => (5 - self.hist.cards_finished_this_turn as i32).max(0),
            pw::PANACHE_POWER => 5 - (aux & 0xFF),
            pw::SLOTH_POWER | pw::TENDER_POWER | pw::WITHERING_PRESENCE_POWER => aux,
            pw::SLOW_POWER => aux * 10,
            pw::TAG_TEAM_POWER => 1,
            pw::SURROUNDED_POWER => aux + 1,
            _ => 0,
        }
    }

    /// `playable`: the already computed `can_play` of a hand card (see `observe_ex`), `None` = compute it here. `pre` (v2): the card's
    /// `card_preview` when the caller already has it.
    fn write_card(&self, w: &mut W, c: CardIdx, playable: Option<bool>, v2: bool, pre: Option<CardPreview>) {
        let card = &self.cards[c as usize];
        let d = content::card_def(card.id);
        let playable = match playable {
            Some(p) => p as i32,
            None => (self.stage == Stage::AwaitAction && self.player.phase == Phase::Play && self.card_pile_type(c) == PileType::Hand && self.can_play(c)) as i32,
        };
        if v2 {
            let p = pre.unwrap_or_else(|| self.card_preview(c));
            let mut kw = self.card_keywords(c);
            if card.flags & cflag::SINGLE_TURN_RETAIN != 0 {
                kw |= kw::RETAIN;
            }
            if card.flags & cflag::SINGLE_TURN_SLY != 0 {
                kw |= kw::SLY;
            }
            w.n(card.id as i32 + 1);
            w.n(card.upgrade as i32);
            w.n(if d.x_cost { -1 } else { self.card_cost(c, true).max(0) });
            w.n(playable);
            w.n(kw as i32);
            w.n(card.enchant as i32);
            w.n(p.damage);
            w.n(p.block);
            w.n(card.counter[0] as i32);
            w.n(card.counter[1] as i32);
            w.n(card.enchant_amount as i32);
            w.n(card.affliction as i32);
            w.n(p.count);
            w.n(if card.affliction != 0 { card.affliction_amount as i32 } else { 0 });
            w.n(self.enchanted_replay_count(c));
            return;
        }
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

    /// `(id + 1, amount)` of the first `OBS_POWERS` powers (v2: also the displayed number), zeros for the rest.
    #[inline(always)]
    fn write_powers(&self, w: &mut W, cr: &Creature, v2: bool) {
        let n = cr.powers.len().min(OBS_POWERS);
        for p in &cr.powers.as_slice()[..n] {
            w.n(p.id as i32 + 1);
            w.n(p.amount);
            if v2 {
                w.n(self.power_display(p));
            }
        }
        w.zeros((OBS_POWERS - n) * if v2 { POWER_F_V2 } else { POWER_F });
    }

    /// v1 caps the pile at `OBS_MAX_PILE` cards in pile order and then sorts (the cut follows the hidden order); v2 sorts the whole pile
    /// and shows its first `OBS_MAX_PILE` entries.
    fn write_pile_list(&self, w: &mut W, pile: &[CardIdx], sorted_multiset: bool, v2: bool) {
        let n = pile.len().min(OBS_MAX_PILE);
        if sorted_multiset {
            // order-free view: sort by (rarity, id, upgrade) — what the pile screen shows. The sort key is packed into one u32
            // (equal keys are identical entries, so an unstable sort of keys gives the same vector as a stable sort of cards).
            let mut keys = [0u32; MAX_CARDS];
            let m = if v2 { pile.len().min(MAX_CARDS) } else { n };
            for (k, &c) in pile.iter().take(m).enumerate() {
                let card = &self.cards[c as usize];
                keys[k] = (content::card_def(card.id).rarity as u32) << 24 | (card.id as u32) << 8 | card.upgrade as u32;
            }
            keys[..m].sort_unstable();
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

    /// What asked for a decision: `(kind, id)` from its `purpose` (`purpose::*` flags; a plain id is a card).
    pub fn decision_source(d: &Decision) -> (i32, i32) {
        let id = (d.purpose & purpose::ID) as i32;
        if d.purpose & purpose::POTION != 0 {
            (source_kind::POTION, (d.purpose & !purpose::POTION) as i32)
        } else if d.purpose & purpose::RELIC != 0 {
            (source_kind::RELIC, id)
        } else if d.purpose & purpose::MONSTER != 0 {
            (source_kind::MONSTER, id)
        } else {
            (source_kind::CARD, id)
        }
    }

    /// Writes the flat observation of the process-wide version (`OBS_VERSION`) into `out` (`out.len() >= obs_size(version)`). Returns its length.
    pub fn observe(&self, out: &mut [f32]) -> usize {
        self.observe_ex(out, None)
    }

    /// `observe` with the playability of the hand cards precomputed: bit `k` of `hand_playable` = `can_play(hand[k])` (as
    /// produced by `legal_actions_ex`), so a batch env that also needs the legal actions evaluates each hand card once.
    pub fn observe_ex(&self, out: &mut [f32], hand_playable: Option<u16>) -> usize {
        self.observe_v(out, hand_playable, obs_version())
    }

    /// `observe_ex` of an explicit version (1 or 2; anything else is taken as 1). Returns `obs_size(version)`.
    pub fn observe_v(&self, out: &mut [f32], hand_playable: Option<u16>, version: u8) -> usize {
        let v2 = version == 2;
        let dm = dims(if v2 { 2 } else { 1 });
        let size = dm.size;
        let out = &mut out[..size];
        // SAFETY: `out` has exactly `size` f32s; all-zero bytes are +0.0. (`fill(0.0)` compiled to a store loop.)
        unsafe { core::ptr::write_bytes(out.as_mut_ptr(), 0, size) };
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
        self.write_powers(&mut w, me, v2);
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
        // v2: each hand card's preview once (the hand entry and the Osty section both read it)
        let mut hand_pre = [CardPreview::default(); MAX_HAND];
        prof!(1, t1, {
        if v2 {
            for (k, c) in self.player.hand.iter().enumerate().take(MAX_HAND) {
                hand_pre[k] = self.card_preview(*c);
            }
        }
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => {
                    let pre = hand_playable.map(|m| hand_ok && m >> k & 1 != 0);
                    self.write_card(&mut w, c, pre, v2, if v2 { Some(hand_pre[k]) } else { None })
                }
                None => w.zeros(dm.card_f),
            }
        }
        });
        // ---- piles ----
        prof!(2, t2, {
        self.write_pile_list(&mut w, self.player.draw.as_slice(), true, v2);
        self.write_pile_list(&mut w, self.player.discard.as_slice(), true, v2);
        self.write_pile_list(&mut w, self.player.exhaust.as_slice(), true, v2);
        });
        w.n(self.player.draw.len() as i32);
        w.n(self.player.discard.len() as i32);
        w.n(self.player.exhaust.len() as i32);
        // ---- enemies (list order) ----
        prof!(3, t3, {
        for k in 0..OBS_MAX_ENEMIES {
            let Some(e) = self.enemies.get(k) else {
                w.zeros(dm.enemy_f);
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
            self.write_powers(&mut w, cr, v2);
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
                for k in 0..dm.cands {
                    // displayed order (`view`), never the game's pile order
                    match view.get(k).map(|vi| (vi, d.cands[vi as usize])) {
                        Some((vi, c)) => {
                            self.write_card(&mut w, c, None, v2, None);
                            w.n(d.selected.contains(vi) as i32);
                        }
                        None => w.zeros(dm.card_f + 1),
                    }
                }
            }
            None => w.zeros(dm.decision_f),
        }
        // ---- Regent: star costs (appended) ----
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => w.n(self.obs_star_cost(c)),
                None => w.f(0.0),
            }
        }
        let star_view = self.decision.as_ref().map(|d| self.decision_view(d));
        for k in 0..dm.cands {
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
                self.write_powers(&mut w, cr, v2);
            }
            None => w.zeros(4 + OBS_POWERS * dm.power_f),
        }
        for k in 0..MAX_HAND {
            if v2 {
                // every Osty damage number the card shows: `OstyDamageVar` and calculated damage from Osty (Unleash, Squeeze, Protector)
                match self.player.hand.get(k) {
                    Some(_) => w.n(hand_pre[k].osty_damage.unwrap_or(0)),
                    None => w.f(0.0),
                }
                continue;
            }
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
        if v2 {
            // ---- v2: what asked for the selection, and the card being played (appended) ----
            match &self.decision {
                Some(d) => {
                    let (kind, id) = Self::decision_source(d);
                    w.n(kind);
                    w.n(id + 1);
                }
                None => w.zeros(DEC_SOURCE_F),
            }
            match self.play_stack.last().map(|p| p.play.card).filter(|&c| c != NO) {
                Some(c) => {
                    let p = self.card_preview(c);
                    self.write_card(&mut w, c, Some(false), true, Some(p));
                    w.n(self.obs_star_cost(c));
                    w.n(p.osty_damage.unwrap_or(0));
                }
                None => w.zeros(PLAYED_F),
            }
        }
        debug_assert_eq!(w.i, size);
        size
    }
}

/// The sections of the process-wide version's vector (`layout_v`).
pub fn layout() -> Vec<(&'static str, usize, usize)> {
    layout_v(obs_version())
}

/// The sections of the observation vector of `version` in order: `(name, offset, size)`. The sizes sum to `obs_size(version)` (tested).
/// v2 has the same sections (resized: cards, powers, candidates) plus `dec_source` and `played` before `end`.
pub fn layout_v(version: u8) -> Vec<(&'static str, usize, usize)> {
    let dm = dims(if version == 2 { 2 } else { 1 });
    let mut sizes: Vec<(&'static str, usize)> = vec![
        ("global", GLOBAL_F),
        ("player", dm.player_f),
        ("relics", RELIC_F),
        ("potions", POTION_F),
        ("hand", MAX_HAND * dm.card_f),
        ("draw", OBS_MAX_PILE * 2),
        ("discard", OBS_MAX_PILE * 2),
        ("exhaust", OBS_MAX_PILE * 2),
        ("pile_sizes", 3),
        ("enemies", OBS_MAX_ENEMIES * dm.enemy_f),
        ("decision", dm.decision_f),
        ("regent", dm.regent_f),
        ("osty", dm.osty_f),
        ("orbs", ORBS_F),
        ("look", LOOK_F),
        ("enemy_moves", ENEMY_MOVES_F),
    ];
    if dm.version >= 2 {
        sizes.push(("dec_source", dm.dec_source_f));
        sizes.push(("played", dm.played_f));
    }
    sizes.push(("end", 0));
    let mut out = vec![];
    let mut off = 0;
    for (n, sz) in sizes {
        out.push((n, off, sz));
        off += sz;
    }
    out
}

/// Constants of the process-wide version (`layout_consts_v`).
pub fn layout_consts() -> Vec<(&'static str, usize)> {
    layout_consts_v(obs_version())
}

/// Constants a consumer of the observation / action space needs (strides, capacities, vocabulary sizes, action offsets) for `version`.
pub fn layout_consts_v(version: u8) -> Vec<(&'static str, usize)> {
    use crate::engine::{ACTION_SPACE, MAX_PICK};
    let dm = dims(if version == 2 { 2 } else { 1 });
    vec![
        ("OBS_VERSION", dm.version as usize),
        ("OBS_SIZE", dm.size),
        ("ACTION_SPACE", ACTION_SPACE),
        ("CARD_F", dm.card_f),
        ("POWER_F", dm.power_f),
        ("ENEMY_F", dm.enemy_f),
        ("GLOBAL_F", GLOBAL_F),
        ("PLAYER_F", dm.player_f),
        ("DECISION_F", dm.decision_f),
        ("PLAYED_F", dm.played_f),
        ("OBS_MAX_ENEMIES", OBS_MAX_ENEMIES),
        ("OBS_MAX_PILE", OBS_MAX_PILE),
        ("OBS_MAX_CANDS", dm.cands),
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
