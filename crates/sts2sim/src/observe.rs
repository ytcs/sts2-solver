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
use crate::types::*;

pub const OBS_MAX_ENEMIES: usize = 8;
pub const OBS_MAX_PILE: usize = 64;
pub const OBS_MAX_CANDS: usize = 16;
pub const OBS_POWERS: usize = 16;
pub const OBS_INTENTS: usize = 3;

/// Per-card features (`CARD_F` floats).
pub const CARD_F: usize = 10;
/// Per-enemy features (`ENEMY_F` floats).
pub const ENEMY_F: usize = 8 + OBS_POWERS * 2 + OBS_INTENTS * 3 + 4;
const GLOBAL_F: usize = 10;
const PLAYER_F: usize = 8 + OBS_POWERS * 2;
const RELIC_F: usize = MAX_RELICS * 2;
const POTION_F: usize = MAX_POTIONS * 2;
const DECISION_F: usize = 8 + OBS_MAX_CANDS * (CARD_F + 1);
/// Total length of the flat observation vector.
pub const OBS_SIZE: usize = GLOBAL_F
    + PLAYER_F
    + RELIC_F
    + POTION_F
    + MAX_HAND * CARD_F
    + OBS_MAX_PILE * 2 * 3 // draw multiset, discard, exhaust: (id+1, upgrade) per slot
    + 3 // pile sizes
    + OBS_MAX_ENEMIES * ENEMY_F
    + DECISION_F;

struct W<'a> {
    out: &'a mut [f32],
    i: usize,
}
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
    fn zeros(&mut self, n: usize) {
        for _ in 0..n {
            self.f(0.0);
        }
    }
}

impl Combat {
    /// Intent damage as the UI computes it: `Hook.ModifyDamage(dealer = monster, target = player, Move)` floored at 0.
    pub fn intent_damage(&self, monster: Cid, base: i32) -> i32 {
        self.modify_damage(PLAYER, monster, Dec::int(base as i64), ValueProp::MOVE, NO).0.trunc().max(0)
    }

    fn write_card(&self, w: &mut W, c: CardIdx) {
        let card = &self.cards[c as usize];
        let d = content::card_def(card.id);
        let playable = (self.stage == Stage::AwaitAction && self.player.phase == Phase::Play && self.card_pile_type(c) == PileType::Hand && self.can_play(c)) as i32;
        let dmg = if d.vars.iter().any(|v| v.kind == VarKind::Damage) {
            self.modify_damage(NO, PLAYER, Dec::int(self.card_var(c, VarKind::Damage) as i64), ValueProp::MOVE, c).0.trunc()
        } else {
            0
        };
        let blk = if d.vars.iter().any(|v| v.kind == VarKind::Block) {
            self.modify_block(PLAYER, Dec::int(self.card_var(c, VarKind::Block) as i64), ValueProp::MOVE, c).trunc()
        } else {
            0
        };
        w.n(card.id as i32 + 1);
        w.n(card.upgrade as i32);
        w.n(if d.x_cost { -1 } else { self.card_cost(c, true).max(0) });
        w.n(playable);
        w.n(self.card_keywords(c) as i32);
        w.n(card.enchant as i32);
        w.n(dmg);
        w.n(blk);
        w.n(card.counter[0] as i32);
        w.n(card.counter[1] as i32);
    }

    fn write_pile_list(&self, w: &mut W, pile: &[CardIdx], sorted_multiset: bool) {
        let mut ents: [(u16, u8); OBS_MAX_PILE] = [(0, 0); OBS_MAX_PILE];
        let n = pile.len().min(OBS_MAX_PILE);
        for (k, &c) in pile.iter().take(n).enumerate() {
            ents[k] = (self.cards[c as usize].id, self.cards[c as usize].upgrade);
        }
        if sorted_multiset {
            // order-free view: sort by (rarity, id, upgrade) — what the pile screen shows
            ents[..n].sort_by_key(|&(id, up)| (content::card_def(id).rarity, id, up));
        }
        for k in 0..OBS_MAX_PILE {
            if k < n {
                w.n(ents[k].0 as i32 + 1);
                w.n(ents[k].1 as i32);
            } else {
                w.f(0.0);
                w.f(0.0);
            }
        }
    }

    /// Writes the flat observation into `out` (`out.len() >= OBS_SIZE`). Returns `OBS_SIZE`.
    pub fn observe(&self, out: &mut [f32]) -> usize {
        let mut w = W { out, i: 0 };
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
        w.n(me.hp);
        w.n(me.max_hp);
        w.n(me.block);
        w.n(self.player.energy);
        w.n(self.max_energy());
        w.n(self.player.stars);
        w.n(self.player.orb_slots as i32);
        w.n(self.player.potion_slots as i32);
        for k in 0..OBS_POWERS {
            match me.powers.get(k) {
                Some(p) => {
                    w.n(p.id as i32 + 1);
                    w.n(p.amount);
                }
                None => w.zeros(2),
            }
        }
        for k in 0..MAX_RELICS {
            match self.player.relics.get(k) {
                Some(r) => {
                    w.n(r.id as i32 + 1);
                    w.n(r.counter);
                }
                None => w.zeros(2),
            }
        }
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
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => self.write_card(&mut w, c),
                None => w.zeros(CARD_F),
            }
        }
        // ---- piles ----
        self.write_pile_list(&mut w, self.player.draw.as_slice(), true);
        self.write_pile_list(&mut w, self.player.discard.as_slice(), false);
        self.write_pile_list(&mut w, self.player.exhaust.as_slice(), false);
        w.n(self.player.draw.len() as i32);
        w.n(self.player.discard.len() as i32);
        w.n(self.player.exhaust.len() as i32);
        // ---- enemies (list order) ----
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
            w.n(ms.stunned as i32);
            for j in 0..OBS_POWERS {
                match cr.powers.get(j) {
                    Some(p) => {
                        w.n(p.id as i32 + 1);
                        w.n(p.amount);
                    }
                    None => w.zeros(2),
                }
            }
            // current intent(s)
            let def = content::monster_def(ms.id);
            let mut n_int = 0;
            if ms.next_move == crate::engine::STUN_NODE {
                w.n(11); // Intent::Stun
                w.n(0);
                w.n(0);
                n_int += 1;
            } else if ms.next_move != NO {
                if let MonsterNode::Move { intents, .. } = &def.nodes[ms.next_move as usize] {
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
                for k in 0..OBS_MAX_CANDS {
                    match d.cands.get(k) {
                        Some(c) => {
                            self.write_card(&mut w, c);
                            w.n(d.selected.contains(k as u8) as i32);
                        }
                        None => w.zeros(CARD_F + 1),
                    }
                }
            }
            None => w.zeros(DECISION_F),
        }
        debug_assert_eq!(w.i, OBS_SIZE);
        OBS_SIZE
    }
}
