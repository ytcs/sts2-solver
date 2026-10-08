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

pub const HIST_CAP: usize = 160;

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

impl HKind {
    /// Kinds that `hist_any_last_player_turn` is asked about (`HappenedLastPlayerTurn`): their entries stay live for a second
    /// player turn. A new last-turn query of another kind must be added here (debug-asserted in `hist_any_last_player_turn`).
    #[inline(always)]
    pub const fn keeps_last_turn(self) -> bool {
        matches!(self, HKind::DamageReceived | HKind::CardPlayStarted)
    }

    /// Whether entries of this kind are stored in the per-turn ring. The others are only counted (`HistLog::total`): no
    /// content queries them per turn, and keeping them out of the ring leaves its capacity to the kinds that are queried
    /// (`hist_count_this_turn` / `hist_any_last_player_turn` / `HistLog::iter`). A new per-turn query of one of these kinds
    /// must add it here (debug builds assert).
    #[inline(always)]
    pub const fn in_ring(self) -> bool {
        !matches!(self, HKind::CardGenerated | HKind::CardPlayFinished | HKind::MonsterPerformedMove | HKind::OrbChanneled | HKind::PotionUsed | HKind::Summoned)
    }
}

/// `*c += 1` for a whole-combat `u16` counter; at the limit the counter stays and the combat is flagged (`ov::COUNTER`) instead
/// of wrapping silently.
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
    /// Whole-combat count of `DamageReceivedEntry`s with `Receiver == player && UnblockedDamage > 0` (Tear Asunder).
    pub player_hits_taken: u16,
    /// Whole-combat count of `CardGeneratedEntry`s whose creator is the player (Supermassive).
    pub generated_by_player: u16,
    /// Whole-combat count of Lightning orbs channeled (`OrbChanneledEntry` with `Orb is LightningOrb`, Voltaic).
    pub lightning_channeled: u16,
}

impl Default for HistLog {
    fn default() -> Self {
        HistLog { entries: [HistEntry::default(); HIST_CAP], n: 0, total: [0; HKIND_COUNT], ethereal_finished: 0, player_hits_taken: 0, generated_by_player: 0, lightning_channeled: 0 }
    }
}

impl HistLog {
    /// `*self = *src` with only the ring entries a query can read (`iter` reads `[n - min(n, CAP), n)`; `hist_push` reads an old entry only
    /// once the ring is full).
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

    /// `History.Clear()`: forgets everything. The ring is not zeroed: `iter()` only reads `[n - min(n, CAP), n)`.
    #[inline]
    pub fn clear(&mut self) {
        self.n = 0;
        self.total = [0; HKIND_COUNT];
        self.ethereal_finished = 0;
        self.player_hits_taken = 0;
        self.generated_by_player = 0;
        self.lightning_channeled = 0;
    }

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
        bump(&mut self.hist_log.total[kind as usize]);
        if !kind.in_ring() {
            return;
        }
        let n = self.hist_log.n as usize;
        let i = n % HIST_CAP;
        // The ring forgets its oldest entry. That is only harmless if no query can still ask for it. The queries are
        // "this turn" (same round / side / player turn) for every kind and "last player turn" (turn == current - 1, side ignored,
        // so it also covers what the enemies did in between) for the kinds in `HKind::keeps_last_turn`. Overwriting a live entry
        // loses data -> flag it.
        if n >= HIST_CAP {
            let old = &self.hist_log.entries[i];
            if self.hist_this_turn(old) || (old.kind.keeps_last_turn() && old.turn as i32 + 1 >= self.player.turn_number) {
                crate::util::raise_overflow(ov::HISTORY as u32);
            }
        }
        if self.round > u16::MAX as i32 || self.player.turn_number > u16::MAX as i32 {
            crate::util::raise_overflow(ov::COUNTER as u32); // entries store round / turn as u16
        }
        self.hist_log.entries[i] = e;
        self.hist_log.n = self.hist_log.n.saturating_add(1);
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
        debug_assert!(kind.in_ring(), "{kind:?} entries are counter-only (HKind::in_ring)");
        self.hist_log.iter().filter(|e| e.kind == kind && self.hist_this_turn(e) && f(e)).count()
    }

    /// Whole-combat count of entries of `kind`.
    pub fn hist_total(&self, kind: HKind) -> usize {
        self.hist_log.total[kind as usize] as usize
    }

    pub fn hist_any_last_player_turn(&self, kind: HKind, f: impl Fn(&HistEntry) -> bool) -> bool {
        debug_assert!(kind.in_ring() && kind.keeps_last_turn(), "{kind:?} is not kept for last-player-turn queries (HKind::keeps_last_turn)");
        self.hist_log.iter().any(|e| e.kind == kind && self.hist_last_player_turn(e) && f(e))
    }

    /// Cards played this turn (`CardPlayStartedEntry`s of this turn), optionally only those with `f`.
    pub fn plays_this_turn(&self, f: impl Fn(&HistEntry) -> bool) -> usize {
        self.hist_count_this_turn(HKind::CardPlayStarted, f)
    }

    /// `CardGeneratedEntry` (`flags & 1` = the creator is the player).
    pub(crate) fn hist_card_generated(&mut self, c: CardIdx, by_player: bool) {
        let id = self.cards[c as usize].id;
        if by_player && self.in_progress {
            bump(&mut self.hist_log.generated_by_player);
        }
        self.hist_push(HKind::CardGenerated, PLAYER, NO, id, c, 0, by_player as u8, 0, 0);
    }

    pub(crate) fn hist_card_play_started(&mut self, p: &CardPlay) {
        let id = self.cards[p.card as usize].id;
        // `aux` = `Resources.EnergyValue` (not the energy spent: an auto-play spends 0 but has the card's cost as its value);
        // `flags & 2` = `IsFirstInSeries` (the first iteration of a replayed card).
        self.hist_push(HKind::CardPlayStarted, PLAYER, p.target, id, p.card, 0, p.is_auto as u8 | (((p.play_index == 0) as u8) << 1), 0, p.energy_value.clamp(0, 255) as u8);
    }
}
