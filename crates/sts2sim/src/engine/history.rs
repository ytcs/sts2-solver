//! Combat history (`Combat/History/*`, spec 01 §15): the log gameplay code queries ("cards drawn this turn", "damage
//! received last turn", ...).
//!
//! The game keeps one unbounded list of entries cleared at combat end. Queries only ever ask for the current turn
//! (`HappenedThisTurn`), the previous player turn (`HappenedLastPlayerTurn`) or the whole combat, so this port keeps
//!  * a ring of the most recent `HIST_CAP` entries (enough for the current + previous turn of any realistic fight), and
//!  * whole-combat counters per entry kind (`Combat::hist_total`) that never overflow.
//!
//! Every entry snapshots `(round, side, player turn number)` like `CombatHistoryEntry`:
//!  `HappenedThisTurn` = same round && same side && same turn number;
//!  `HappenedLastPlayerTurn` = `turn == current turn - 1` (the side is NOT checked).

use crate::hooks::CardPlay;
use crate::state::*;
use crate::types::*;

pub const HIST_CAP: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum HKind {
    #[default]
    BlockGained = 0,
    CardAfflicted,
    CardDiscarded,
    /// `flags & 1` = `fromHandDraw`.
    CardDrawn,
    CardExhausted,
    CardGenerated,
    /// `CardPlay` started: `id` = card id, `aux` = energy value of the play (`Resources.EnergyValue`), `flags & 1` = auto.
    CardPlayStarted,
    /// Counter-only (see `HistLog::ethereal_finished`).
    CardPlayFinished,
    CreatureAttacked,
    /// `actor` = receiver, `other` = dealer, `val` = unblocked damage, `flags & 1` = `WasFullyBlocked`,
    /// `flags & 2` = `WasBlockBroken`, `props` = `ValueProp`.
    DamageReceived,
    /// `val` = energy spent.
    EnergySpent,
    MonsterPerformedMove,
    OrbChanneled,
    PotionUsed,
    /// `id` = power id, `actor` = receiver, `other` = applier, `val` = amount.
    PowerReceived,
    /// `val` = delta.
    StarsModified,
    Summoned,
}
pub const HKIND_COUNT: usize = 17;

#[derive(Clone, Copy, Default, Debug)]
pub struct HistEntry {
    pub kind: HKind,
    pub side: Side,
    pub flags: u8,
    pub props: u8,
    pub round: u16,
    /// The (single) player's `TurnNumber` when the entry was logged.
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
    /// Total entries ever pushed (ring index = `n % HIST_CAP`).
    pub n: u32,
    /// Whole-combat counters per kind.
    pub total: [u16; HKIND_COUNT],
    /// Completed plays of Ethereal cards (`CardPlayFinishedEntry.WasEthereal`) — `CardPlayFinished` is counter-only (no ring
    /// entry) to keep the ring for the entries that need per-turn filtering.
    pub ethereal_finished: u16,
}

impl Default for HistLog {
    fn default() -> Self {
        HistLog { entries: [HistEntry::default(); HIST_CAP], n: 0, total: [0; HKIND_COUNT], ethereal_finished: 0 }
    }
}

impl HistLog {
    /// Entries still in the ring, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &HistEntry> {
        let n = self.n as usize;
        let len = n.min(HIST_CAP);
        let start = n - len;
        (start..n).map(move |i| &self.entries[i % HIST_CAP])
    }
}

impl Combat {
    /// Appends an entry (`Combat.History.*`): logged immediately after the event, before its After-hook.
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
        let i = self.hist_log.n as usize % HIST_CAP;
        self.hist_log.entries[i] = e;
        self.hist_log.n += 1;
        self.hist_log.total[kind as usize] = self.hist_log.total[kind as usize].saturating_add(1);
    }

    /// `entry.HappenedThisTurn(state)`.
    #[inline]
    pub fn hist_this_turn(&self, e: &HistEntry) -> bool {
        e.round == self.round as u16 && e.side == self.side && e.turn == self.player.turn_number as u16
    }

    /// `entry.HappenedLastPlayerTurn(player)`: the side is not checked.
    #[inline]
    pub fn hist_last_player_turn(&self, e: &HistEntry) -> bool {
        e.turn as i32 == self.player.turn_number - 1
    }

    /// Number of entries of `kind` that satisfy `f` and happened this turn.
    pub fn hist_count_this_turn(&self, kind: HKind, f: impl Fn(&HistEntry) -> bool) -> usize {
        self.hist_log.iter().filter(|e| e.kind == kind && self.hist_this_turn(e) && f(e)).count()
    }

    /// Whole-combat count of entries of `kind`.
    pub fn hist_total(&self, kind: HKind) -> usize {
        self.hist_log.total[kind as usize] as usize
    }

    pub fn hist_any_last_player_turn(&self, kind: HKind, f: impl Fn(&HistEntry) -> bool) -> bool {
        self.hist_log.iter().any(|e| e.kind == kind && self.hist_last_player_turn(e) && f(e))
    }

    /// Cards played this turn (`CardPlayStartedEntry`s of this turn), optionally only those with `f`.
    pub fn plays_this_turn(&self, f: impl Fn(&HistEntry) -> bool) -> usize {
        self.hist_count_this_turn(HKind::CardPlayStarted, f)
    }

    pub(crate) fn hist_card_play_started(&mut self, p: &CardPlay) {
        let id = self.cards[p.card as usize].id;
        self.hist_push(HKind::CardPlayStarted, PLAYER, p.target, id, p.card, 0, p.is_auto as u8, 0, p.energy_spent.max(0) as u8);
    }
}
