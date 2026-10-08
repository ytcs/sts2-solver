use crate::hooks::CardPlay;
use crate::state::*;
use crate::types::*;

pub const HIST_CAP: usize = 160;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum HKind {
    #[default]
    BlockGained = 0,
    CardAfflicted,
    CardDiscarded,
    CardDrawn,
    CardExhausted,
    CardGenerated,
    CardPlayStarted,
    CardPlayFinished,
    CreatureAttacked,
    DamageReceived,
    EnergySpent,
    MonsterPerformedMove,
    OrbChanneled,
    PotionUsed,
    PowerReceived,
    StarsModified,
    Summoned,
}
pub const HKIND_COUNT: usize = 17;

impl HKind {
    #[inline(always)]
    // Every kind hist_any_last_player_turn is asked about must be listed here (debug-asserted).
    pub const fn keeps_last_turn(self) -> bool {
        matches!(self, HKind::DamageReceived | HKind::CardPlayStarted)
    }

    #[inline(always)]
    pub const fn in_ring(self) -> bool {
        !matches!(self, HKind::CardGenerated | HKind::CardPlayFinished | HKind::MonsterPerformedMove | HKind::OrbChanneled | HKind::PotionUsed | HKind::Summoned)
    }
}

#[inline(always)]
pub(crate) fn bump(c: &mut u16) {
    if *c == u16::MAX {
        crate::util::raise_overflow(ov::COUNTER as u32);
    } else {
        *c += 1;
    }
}
#[derive(Clone, Copy, Default, Debug)]
pub struct HistEntry {
    pub kind: HKind,
    pub side: Side,
    pub flags: u8,
    pub props: u8,
    pub round: u16,
    pub turn: u16,
    pub actor: Cid,
    pub other: Cid,
    pub card: CardIdx,
    pub aux: u8,
    pub id: u16,
    pub val: i16,
}

#[derive(Clone, Copy)]
pub struct HistLog {
    pub entries: [HistEntry; HIST_CAP],
    pub n: u32,
    pub total: [u16; HKIND_COUNT],
    pub ethereal_finished: u16,
    pub player_hits_taken: u16,
    pub generated_by_player: u16,
    pub lightning_channeled: u16,
}

impl Default for HistLog {
    fn default() -> Self {
        HistLog { entries: [HistEntry::default(); HIST_CAP], n: 0, total: [0; HKIND_COUNT], ethereal_finished: 0, player_hits_taken: 0, generated_by_player: 0, lightning_channeled: 0 }
    }
}

impl HistLog {
    #[inline]
    pub fn copy_from(&mut self, src: &HistLog) {
        let HistLog { entries, n, total, ethereal_finished, player_hits_taken, generated_by_player, lightning_channeled } = self;
        let m = (src.n as usize).min(HIST_CAP);
        entries[..m].copy_from_slice(&src.entries[..m]);
        *n = src.n;
        *total = src.total;
        *ethereal_finished = src.ethereal_finished;
        *player_hits_taken = src.player_hits_taken;
        *generated_by_player = src.generated_by_player;
        *lightning_channeled = src.lightning_channeled;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.n = 0;
        self.total = [0; HKIND_COUNT];
        self.ethereal_finished = 0;
        self.player_hits_taken = 0;
        self.generated_by_player = 0;
        self.lightning_channeled = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = &HistEntry> {
        let n = self.n as usize;
        let len = n.min(HIST_CAP);
        let start = n - len;
        (start..n).map(move |i| &self.entries[i % HIST_CAP])
    }
}

impl Combat {
    pub fn hist_push(&mut self, kind: HKind, actor: Cid, other: Cid, id: u16, card: CardIdx, val: i32, flags: u8, props: u8, aux: u8) {
        if !self.in_progress && !self.is_starting {
            return;
        }
        let e = HistEntry {
            kind,
            side: self.side,
            flags,
            props,
            round: self.round as u16,
            turn: self.player.turn_number as u16,
            actor,
            other,
            card,
            aux,
            id,
            val: val.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        };
        bump(&mut self.hist_log.total[kind as usize]);
        if !kind.in_ring() {
            return;
        }
        let n = self.hist_log.n as usize;
        let i = n % HIST_CAP;
        if n >= HIST_CAP {
            let old = &self.hist_log.entries[i];
            if self.hist_this_turn(old) || (old.kind.keeps_last_turn() && old.turn as i32 + 1 >= self.player.turn_number) {
                crate::util::raise_overflow(ov::HISTORY as u32);
            }
        }
        if self.round > u16::MAX as i32 || self.player.turn_number > u16::MAX as i32 {
            crate::util::raise_overflow(ov::COUNTER as u32);
        }
        self.hist_log.entries[i] = e;
        self.hist_log.n = self.hist_log.n.saturating_add(1);
    }

    #[inline]
    pub fn hist_this_turn(&self, e: &HistEntry) -> bool {
        e.round == self.round as u16 && e.side == self.side && e.turn == self.player.turn_number as u16
    }

    #[inline]
    pub fn hist_last_player_turn(&self, e: &HistEntry) -> bool {
        e.turn as i32 == self.player.turn_number - 1
    }

    pub fn hist_count_this_turn(&self, kind: HKind, f: impl Fn(&HistEntry) -> bool) -> usize {
        debug_assert!(kind.in_ring(), "{kind:?} entries are counter-only (HKind::in_ring)");
        self.hist_log.iter().filter(|e| e.kind == kind && self.hist_this_turn(e) && f(e)).count()
    }

    pub fn hist_total(&self, kind: HKind) -> usize {
        self.hist_log.total[kind as usize] as usize
    }

    pub fn hist_any_last_player_turn(&self, kind: HKind, f: impl Fn(&HistEntry) -> bool) -> bool {
        debug_assert!(kind.in_ring() && kind.keeps_last_turn(), "{kind:?} is not kept for last-player-turn queries (HKind::keeps_last_turn)");
        self.hist_log.iter().any(|e| e.kind == kind && self.hist_last_player_turn(e) && f(e))
    }

    pub fn plays_this_turn(&self, f: impl Fn(&HistEntry) -> bool) -> usize {
        self.hist_count_this_turn(HKind::CardPlayStarted, f)
    }

    pub(crate) fn hist_card_generated(&mut self, c: CardIdx, by_player: bool) {
        let id = self.cards[c as usize].id;
        if by_player && self.in_progress {
            bump(&mut self.hist_log.generated_by_player);
        }
        self.hist_push(HKind::CardGenerated, PLAYER, NO, id, c, 0, by_player as u8, 0, 0);
    }

    pub(crate) fn hist_card_play_started(&mut self, p: &CardPlay) {
        let id = self.cards[p.card as usize].id;
        self.hist_push(HKind::CardPlayStarted, PLAYER, p.target, id, p.card, 0, p.is_auto as u8 | (((p.play_index == 0) as u8) << 1), 0, p.energy_value.clamp(0, 255) as u8);
    }
}
