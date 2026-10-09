// Contract: exactly what a human player sees; hidden state (pile order, RNG streams) never reaches the observation (tests/observe.rs).
use crate::content;
use crate::dec::Dec;
use crate::defs::*;
use crate::hooks::{Kind, Me};
use crate::state::*;
use crate::engine::{LOOK_H, LOOK_NODES};
use crate::types::*;

pub const OBS_MAX_ENEMIES: usize = 8;
pub const OBS_MAX_PILE: usize = 64;
pub const OBS_MAX_CANDS: usize = 64;
pub const OBS_POWERS: usize = 16;
pub const OBS_INTENTS: usize = 3;

pub const CARD_F: usize = 15;
pub const POWER_F: usize = 3;
pub const ENEMY_F: usize = 8 + OBS_POWERS * POWER_F + OBS_INTENTS * 3 + 4;
const GLOBAL_F: usize = 10;
const PLAYER_F: usize = 8 + OBS_POWERS * POWER_F;
const RELIC_F: usize = OBS_RELICS * 2;
const POTION_F: usize = MAX_POTIONS * 2;
const REGENT_F: usize = MAX_HAND + OBS_MAX_CANDS;
const DECISION_F: usize = 8 + OBS_MAX_CANDS * (CARD_F + 1);
pub const OSTY_F: usize = 4 + OBS_POWERS * POWER_F + MAX_HAND;
pub const ORB_F: usize = 3;
pub const ORBS_F: usize = MAX_ORBS * ORB_F + 1;
pub const LOOK_F: usize = OBS_MAX_ENEMIES * LOOK_H * (LOOK_NODES + 1);
pub const MOVE_STATE_F: usize = 2;
pub const ENEMY_MOVES_F: usize = OBS_MAX_ENEMIES * MOVE_STATE_F;
pub const DEC_SOURCE_F: usize = 2;
pub const PLAYED_F: usize = CARD_F + 2;

pub const OBS_SIZE: usize = GLOBAL_F
    + PLAYER_F
    + RELIC_F
    + POTION_F
    + MAX_HAND * CARD_F
    + OBS_MAX_PILE * 2 * 3
    + 3
    + OBS_MAX_ENEMIES * ENEMY_F
    + DECISION_F
    + REGENT_F
    + OSTY_F
    + ORBS_F
    + LOOK_F
    + ENEMY_MOVES_F
    + DEC_SOURCE_F
    + PLAYED_F;

const _: () = assert!(OBS_MAX_CANDS == crate::engine::MAX_PICK);

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
    #[inline(always)]
    fn zeros(&mut self, n: usize) {
        self.i += n;
    }
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardPreview {
    pub damage: i32,
    pub block: i32,
    pub count: i32,
    pub osty_damage: Option<i32>,
}

pub mod source_kind {
    pub const CARD: i32 = 1;
    pub const POTION: i32 = 2;
    pub const RELIC: i32 = 3;
    pub const MONSTER: i32 = 4;
}

impl Combat {
    pub fn intent_damage(&self, monster: Cid, base: i32) -> i32 {
        self.modify_damage_value(PLAYER, monster, Dec::int(base as i64), ValueProp::MOVE, NO).trunc().max(0)
    }

    fn obs_star_cost(&self, c: CardIdx) -> i32 {
        if self.card_has_star_cost_x(c) {
            -2
        } else if self.card_current_star_cost(c) < 0 {
            -1
        } else {
            self.card_star_cost(c)
        }
    }

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

    fn write_card(&self, w: &mut W, c: CardIdx, playable: Option<bool>, pre: Option<CardPreview>) {
        let card = &self.cards[c as usize];
        let d = content::card_def(card.id);
        let playable = match playable {
            Some(p) => p as i32,
            None => (self.stage == Stage::AwaitAction && self.player.phase == Phase::Play && self.card_pile_type(c) == PileType::Hand && self.can_play(c)) as i32,
        };
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
    }

    #[inline(always)]
    fn write_powers(&self, w: &mut W, cr: &Creature) {
        let n = cr.powers.len().min(OBS_POWERS);
        for p in &cr.powers.as_slice()[..n] {
            w.n(p.id as i32 + 1);
            w.n(p.amount);
            w.n(self.power_display(p));
        }
        w.zeros((OBS_POWERS - n) * POWER_F);
    }

    // Sorted over the whole pile, then truncated: a big pile's order stays hidden.
    fn write_pile_list(&self, w: &mut W, pile: &[CardIdx]) {
        let n = pile.len().min(OBS_MAX_PILE);
        let m = pile.len().min(MAX_CARDS);
        let mut keys = [0u32; MAX_CARDS];
        for (k, &c) in pile.iter().take(m).enumerate() {
            let card = &self.cards[c as usize];
            keys[k] = (content::card_def(card.id).rarity as u32) << 24 | (card.id as u32) << 8 | card.upgrade as u32;
        }
        keys[..m].sort_unstable();
        for &key in &keys[..n] {
            w.n(((key >> 8) & 0xFFFF) as i32 + 1);
            w.n((key & 0xFF) as i32);
        }
        w.zeros((OBS_MAX_PILE - n) * 2);
    }

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

    pub fn observe(&self, out: &mut [f32]) -> usize {
        self.observe_ex(out, None)
    }

    pub fn observe_ex(&self, out: &mut [f32], hand_playable: Option<u16>) -> usize {
        let size = OBS_SIZE;
        let out = &mut out[..size];
        // SAFETY: `out` has exactly `size` f32s; all-zero bytes are +0.0.
        unsafe { core::ptr::write_bytes(out.as_mut_ptr(), 0, size) };
        let mut w = W { out, i: 0 };
        let hand_ok = self.stage == Stage::AwaitAction && self.player.phase == Phase::Play;
        let me = self.cr(PLAYER);
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
        prof!(5, t5, {
        w.n(me.hp);
        w.n(me.max_hp);
        w.n(me.block);
        w.n(self.player.energy);
        w.n(self.max_energy());
        w.n(self.player.stars);
        w.n(self.player.orb_slots as i32);
        w.n(self.player.potion_slots as i32);
        self.write_powers(&mut w, me);
        let mut n_relics = 0;
        for r in self.player.relics.as_slice().iter().filter(|r| crate::relic_mask::OBSERVED.get(r.id as usize).copied().unwrap_or(true)).take(OBS_RELICS) {
            n_relics += 1;
            w.n(r.id as i32 + 1);
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
        let mut hand_pre = [CardPreview::default(); MAX_HAND];
        prof!(1, t1, {
        for (k, c) in self.player.hand.iter().enumerate().take(MAX_HAND) {
            hand_pre[k] = self.card_preview(*c);
        }
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => {
                    let pre = hand_playable.map(|m| hand_ok && m >> k & 1 != 0);
                    self.write_card(&mut w, c, pre, Some(hand_pre[k]))
                }
                None => w.zeros(CARD_F),
            }
        }
        });
        prof!(2, t2, {
        self.write_pile_list(&mut w, self.player.draw.as_slice());
        self.write_pile_list(&mut w, self.player.discard.as_slice());
        self.write_pile_list(&mut w, self.player.exhaust.as_slice());
        });
        w.n(self.player.draw.len() as i32);
        w.n(self.player.discard.len() as i32);
        w.n(self.player.exhaust.len() as i32);
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
            self.write_powers(&mut w, cr);
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
        let star_view = self.decision.as_ref().map(|d| self.decision_view(d));
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
                let view = star_view.as_ref().expect("a view of the pending decision");
                for k in 0..OBS_MAX_CANDS {
                    match view.get(k).map(|vi| (vi, d.cands[vi as usize])) {
                        Some((vi, c)) => {
                            self.write_card(&mut w, c, None, None);
                            w.n(d.selected.contains(vi) as i32);
                        }
                        None => w.zeros(CARD_F + 1),
                    }
                }
            }
            None => w.zeros(DECISION_F),
        }
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(c) => w.n(self.obs_star_cost(c)),
                None => w.f(0.0),
            }
        }
        for k in 0..OBS_MAX_CANDS {
            match self.decision.as_ref().and_then(|d| star_view.as_ref().and_then(|v| v.get(k)).map(|vi| d.cands[vi as usize])) {
                Some(c) => w.n(self.obs_star_cost(c)),
                None => w.f(0.0),
            }
        }
        match self.osty() {
            Some(o) => {
                let cr = self.cr(o);
                w.n(1);
                w.n(cr.is_alive() as i32);
                w.n(cr.hp);
                w.n(cr.max_hp);
                self.write_powers(&mut w, cr);
            }
            None => w.zeros(4 + OBS_POWERS * POWER_F),
        }
        for k in 0..MAX_HAND {
            match self.player.hand.get(k) {
                Some(_) => w.n(hand_pre[k].osty_damage.unwrap_or(0)),
                None => w.f(0.0),
            }
        }
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
        prof!(4, t4, {
        let mut dig = crate::engine::LookDigests::default();
        for k in 0..OBS_MAX_ENEMIES {
            match self.enemies.get(k) {
                Some(e) if self.cr(e).is_alive() => {
                    for row in self.lookahead_shared(e, &mut dig).iter() {
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
                self.write_card(&mut w, c, Some(false), Some(p));
                w.n(self.obs_star_cost(c));
                w.n(p.osty_damage.unwrap_or(0));
            }
            None => w.zeros(PLAYED_F),
        }
        debug_assert_eq!(w.i, size);
        size
    }
}

pub fn layout() -> Vec<(&'static str, usize, usize)> {
    let sizes = [
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
        ("dec_source", DEC_SOURCE_F),
        ("played", PLAYED_F),
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

pub fn layout_consts() -> Vec<(&'static str, usize)> {
    use crate::engine::{ACTION_SPACE, MAX_PICK};
    vec![
        ("OBS_SIZE", OBS_SIZE),
        ("ACTION_SPACE", ACTION_SPACE),
        ("CARD_F", CARD_F),
        ("POWER_F", POWER_F),
        ("ENEMY_F", ENEMY_F),
        ("GLOBAL_F", GLOBAL_F),
        ("PLAYER_F", PLAYER_F),
        ("DECISION_F", DECISION_F),
        ("PLAYED_F", PLAYED_F),
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
