//! The complete, plain-data combat state. Everything here is `Copy`-able: cloning a fight is a memcpy and the
//! hot path never allocates.

use crate::hooks::Mask;
use crate::rng::Rng;
use crate::types::*;
use crate::util::ArrayVec;

pub const MAX_CARDS: usize = 160;
pub const MAX_CREATURES: usize = 16;
pub const MAX_POWERS: usize = 16;
pub const MAX_RELICS: usize = 24;
pub const MAX_POTIONS: usize = 4;
pub const MAX_HAND: usize = 10;
pub const MAX_ORBS: usize = 10;

/// Creature handles: `0` is always the player. Pets (Osty) and enemies take later slots; slots of removed
/// enemies are recycled. Enemy/ally *order* (which matters for hooks) lives in `Combat::allies/enemies`.
pub type Cid = u8;
pub const PLAYER: Cid = 0;

pub type CardIdx = u8;
pub type Pile = ArrayVec<CardIdx, MAX_CARDS>;

/// A power instance on a creature. `uid` is unique per combat so a hook snapshot can find the live instance
/// again even after sibling powers were removed (list indices shift).
#[derive(Clone, Copy, Default, Debug)]
pub struct Power {
    pub id: u16,
    pub uid: u16,
    pub amount: i32,
    pub amount_on_turn_start: i32,
    /// Per-power private state (counters / flags), meaning defined by the power implementation.
    pub aux: i32,
    pub applier: u8,
    pub skip_next_tick: bool,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CostMod {
    pub amount: i8,
    pub relative: bool,
    pub reduce_only: bool,
    /// bit1 (2) = expires end of turn, bit2 (4) = expires when played. 0 = lasts the combat.
    pub expire: u8,
}
pub const EXPIRE_END_OF_TURN: u8 = 2;
pub const EXPIRE_WHEN_PLAYED: u8 = 4;

pub mod cflag {
    pub const EXHAUST_ON_NEXT_PLAY: u16 = 1 << 0;
    pub const SINGLE_TURN_RETAIN: u16 = 1 << 1;
    pub const SINGLE_TURN_SLY: u16 = 1 << 2;
    pub const IS_DUPE: u16 = 1 << 3;
    /// Card left the combat for good (`HasBeenRemovedFromState`).
    pub const REMOVED: u16 = 1 << 4;
    pub const X_CAPTURED: u16 = 1 << 5;
}

/// One card instance in the combat arena.
#[derive(Clone, Copy, Default, Debug)]
pub struct Card {
    pub id: u16,
    pub pile: PileTypeBits,
    pub upgrade: u8,
    pub flags: u16,
    /// Local keyword delta vs canonical (`AddKeyword` / `RemoveKeyword`).
    pub kw_add: u8,
    pub kw_remove: u8,
    pub enchant: u8,
    pub enchant_amount: i16,
    pub affliction: u8,
    pub affliction_amount: i16,
    pub base_replay: u8,
    /// Base energy cost after upgrades (`CardEnergyCost._base`); -1 = no cost.
    pub cost_base: i8,
    pub x_value: i16,
    pub mods: ArrayVec<CostMod, 3>,
    /// Per-card persistent counters (Rampage damage, Regret, ...), meaning defined by the card.
    pub counter: [i16; 2],
    /// Deck index this combat card was cloned from (`DeckVersion`), `NO` if none.
    pub deck_idx: u8,
}
pub type PileTypeBits = u8;

#[derive(Clone, Copy, Default, Debug)]
pub struct Relic {
    pub id: u16,
    pub counter: i32,
    pub flags: u8,
    pub aux: i32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Potion {
    pub id: u16,
}

/// Per-monster AI + private state (see `monster.rs`).
#[derive(Clone, Copy, Debug)]
pub struct MonsterState {
    pub id: u16,
    /// Current state-machine node (index into the monster def's state table).
    pub cur_state: u8,
    /// Pending move node (`NO` = `UNSET_MOVE`).
    pub next_move: u8,
    pub performed_first: bool,
    pub spawned_this_turn: bool,
    pub is_performing: bool,
    /// Ring of the most recent logged move nodes (newest at `log[(log_len-1)&7]`).
    pub log: [u8; 8],
    pub log_len: u16,
    /// Bitset of nodes that appear anywhere in the log (for `UseOnlyOnce`).
    pub ever_logged: u64,
    /// `_performedAtLeastOnce` per node.
    pub performed_once: u64,
    /// Follow-up node id for a STUNNED move (`NO` = none).
    /// Last performed move nodes (newest last) — the pattern history a player has actually seen.
    pub performed: [u8; 4],
    pub stun_follow_up: u8,
    pub stunned: bool,
    /// Monster-private integers (IsFront, counters, ...), meaning defined by the monster implementation.
    pub vars: [i32; 6],
}

impl Default for MonsterState {
    fn default() -> Self {
        MonsterState {
            id: 0,
            cur_state: 0,
            next_move: NO,
            performed_first: false,
            spawned_this_turn: false,
            is_performing: false,
            log: [NO; 8],
            log_len: 0,
            ever_logged: 0,
            performed_once: 0,
            performed: [NO; 4],
            stun_follow_up: NO,
            stunned: false,
            vars: [0; 6],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Creature {
    pub active: bool,
    pub side: Side,
    pub is_player: bool,
    /// Ally-side monster (Osty).
    pub is_pet: bool,
    /// Attached to the combat (false after removal / escape).
    pub in_combat: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    /// Pet owner (`PetOwner`), `NO` otherwise.
    pub owner: Cid,
    /// Encounter slot index (`NO` = none).
    pub slot: u8,
    pub powers: ArrayVec<Power, MAX_POWERS>,
    pub monster: MonsterState,
}

impl Default for Creature {
    fn default() -> Self {
        Creature {
            active: false,
            side: Side::Enemy,
            is_player: false,
            is_pet: false,
            in_combat: false,
            hp: 0,
            max_hp: 0,
            block: 0,
            owner: NO,
            slot: NO,
            powers: ArrayVec::new(),
            monster: MonsterState::default(),
        }
    }
}

impl Creature {
    #[inline(always)]
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }
    #[inline(always)]
    pub fn is_dead(&self) -> bool {
        self.hp <= 0
    }
    pub fn power(&self, id: u16) -> Option<&Power> {
        self.powers.iter().find(|p| p.id == id)
    }
    pub fn power_amount(&self, id: u16) -> i32 {
        self.power(id).map_or(0, |p| p.amount)
    }
}

/// The run-level RNG streams that combat consumes. Each persists across combats in the real game, so a
/// scenario supplies either a seed or the full saved state of each stream.
#[derive(Clone, Copy, Debug)]
pub struct RngSet {
    pub shuffle: Rng,
    pub combat_card_generation: Rng,
    pub combat_potion_generation: Rng,
    pub combat_card_selection: Rng,
    pub combat_energy_costs: Rng,
    pub combat_targets: Rng,
    pub monster_ai: Rng,
    pub niche: Rng,
    pub combat_orbs: Rng,
}

impl RngSet {
    /// `RunRngSet(seed)`: each stream is `new Rng(seed, snake_case(name))`.
    pub fn from_run_seed(seed: u64) -> Self {
        RngSet {
            shuffle: Rng::named(seed, "shuffle"),
            combat_card_generation: Rng::named(seed, "combat_card_generation"),
            combat_potion_generation: Rng::named(seed, "combat_potion_generation"),
            combat_card_selection: Rng::named(seed, "combat_card_selection"),
            combat_energy_costs: Rng::named(seed, "combat_energy_costs"),
            combat_targets: Rng::named(seed, "combat_targets"),
            monster_ai: Rng::named(seed, "monster_ai"),
            niche: Rng::named(seed, "niche"),
            combat_orbs: Rng::named(seed, "combat_orbs"),
        }
    }
}

/// Player-side combat state (`PlayerCombatState` + the run-level bits combat reads).
#[derive(Clone, Copy)]
pub struct PlayerState {
    pub energy: i32,
    /// `Player.MaxEnergy` (character base + permanent changes); hooks modify it at read time.
    pub max_energy: i32,
    pub stars: i32,
    /// Per-player turn counter (1-based).
    pub turn_number: i32,
    pub phase: Phase,
    pub hand: Pile,
    pub draw: Pile,
    pub discard: Pile,
    pub exhaust: Pile,
    pub play: Pile,
    pub relics: ArrayVec<Relic, MAX_RELICS>,
    pub potions: [Option<Potion>; MAX_POTIONS],
    pub potion_slots: u8,
    pub orb_slots: u8,
    /// `BeginCardOrPotionEffect` depth.
    pub effect_depth: u8,
}

/// What the agent must do next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Player turn, play phase: awaiting `PlayCard` / `UsePotion` / `EndTurn`.
    AwaitAction,
    /// A card/potion/relic effect asked for a choice (see `Combat::decision`).
    AwaitChoice,
    Over,
}

/// Where a decision's candidates come from (what the player's screen shows).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecisionSource {
    Hand,
    /// A combat pile (draw pile candidates are presented sorted by (rarity, id) to hide the order).
    Pile(PileType),
    /// "Choose a card" screen over freshly generated cards (Discovery-style).
    Options,
}

/// A pending decision: select `min..=max` of `cands`, exactly like the game's UI. The agent clicks candidates
/// (`Action::Pick`, toggling; at `max` the most recent selection is replaced) and finishes with `Action::Confirm`;
/// when `confirm_required` is false the decision completes by itself once `max` cards are selected.
#[derive(Clone, Copy)]
pub struct Decision {
    pub source: DecisionSource,
    pub min: u8,
    pub max: u8,
    pub cands: ArrayVec<CardIdx, 64>,
    /// Candidate positions selected so far, in click order.
    pub selected: ArrayVec<u8, 16>,
    /// `RequireManualConfirmation` (`min != max`).
    pub confirm_required: bool,
    /// May finish with nothing selected (skippable choose-a-card screens).
    pub can_skip: bool,
    /// Content-defined purpose tag (which card/relic/potion asked).
    pub purpose: u16,
}

/// The finished selection handed back to the resumed effect.
#[derive(Clone, Copy, Default)]
pub struct Choice {
    /// The chosen cards (`Decision::cands[pick]`), in click order.
    pub cards: ArrayVec<CardIdx, 16>,
}

/// Step of the card-play state machine (`CardModel.OnPlayWrapper`), resumable across decisions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayStep {
    Before,
    OnPlay(u8),
    After,
}

/// In-flight potion use (suspended while a decision is pending).
#[derive(Clone, Copy)]
pub struct PotionCtx {
    pub potion: u16,
    pub target: Cid,
    pub phase: u8,
}

#[derive(Clone, Copy)]
pub struct PlayCtx {
    pub play: crate::hooks::CardPlay,
    pub step: PlayStep,
    pub count: u8,
    pub result: PileType,
    /// Cards pulled by `AutoPlayFromDrawPile` that this card's effect still has to auto-play (in order).
    pub queue: ArrayVec<CardIdx, 16>,
    /// `ExhaustOnNextPlay` value assigned to each queued card right before its auto-play.
    pub queue_exhaust: bool,
}

/// Counters the game's combat history exposes to gameplay code (cards played this turn etc.).
#[derive(Clone, Copy, Default)]
pub struct History {
    pub cards_played_this_turn: i16,
    pub attacks_played_this_turn: i16,
    pub skills_played_this_turn: i16,
    /// `CardExhaustedEntry`s of the current round/side.
    pub cards_exhausted_this_turn: i16,
}

#[derive(Clone, Copy)]
pub struct Combat {
    pub character: u8,
    pub ascension: u8,
    pub rng: RngSet,

    pub round: i32,
    pub side: Side,
    pub in_progress: bool,
    pub is_starting: bool,
    pub pending_loss: bool,
    pub stage: Stage,
    pub outcome: Outcome,

    pub creatures: [Creature; MAX_CREATURES],
    pub allies: ArrayVec<Cid, 4>,
    pub enemies: ArrayVec<Cid, MAX_CREATURES>,
    pub next_power_uid: u16,
    /// Union of the hook masks of every model that has ever been present in this combat (conservative: never
    /// cleared). A hook whose bit is clear has no listener, so dispatching it is a single bit test.
    pub listen: Mask,

    pub player: PlayerState,
    pub cards: [Card; MAX_CARDS],
    pub n_cards: u16,
    pub hist: History,

    /// In-flight card play (suspended while a decision is pending).
    pub play_ctx: Option<PlayCtx>,
    /// Outer card plays suspended while a nested auto-play waits for a decision (innermost last).
    pub play_stack: ArrayVec<PlayCtx, 4>,
    /// `run_play` never pops below this stack depth (it belongs to callers further out).
    pub play_base: u8,
    pub potion_ctx: Option<PotionCtx>,
    pub decision: Option<Decision>,
    pub choice: Choice,
    /// First piece of content used in this combat that has no Rust implementation yet (kind, id). A fight with this
    /// set is NOT faithful; env wrappers must treat it as an error.
    pub missing: Option<(crate::hooks::Kind, u16)>,
    /// `Player.IsActiveForHooks`: true from combat start until `DeactivateHooks()` (after the player's death sequence).
    /// The player's relics / potions / cards / powers only receive hooks while this is set.
    pub player_active: bool,
    /// Whether the card being added by `add_generated_card` was created by the player (`creator != null`, read by
    /// Regalite via `AfterCardGeneratedForCombat`). Monster-applied status cards must clear it around the call.
    pub gen_by_player: bool,
    /// Room kind of the encounter (0 monster, 1 elite, 2 boss), for relics gated on `CurrentRoom.RoomType`.
    pub room_type: u8,
    /// Bit i set when deck card i (scenario deck order) is upgradable: `Deck.Cards.Where(IsUpgradable)` as read by the
    /// post-combat deck relics (FishingRod / WarHammer), which only need to draw from the right-sized item list.
    pub deck_upgradable: u128,
    /// `Player.Gold` (scenarios start with the character default, 99).
    pub gold: i32,
    /// `cardSource` of the power application being dispatched (`BeforePowerAmountChanged` has no card parameter here).
    pub cur_power_card: CardIdx,
    /// A hook that raised a decision (or started a card play that may suspend) and wants `hook_resume` called when it is
    /// done. A suspendable dispatch stops after the listener that set it.
    pub pending_hook: Option<PendingHook>,
    /// An automated card selector is active (`CardSelectCmd.PushSelector(VakuuCardSelector)` during Whispering Earring's
    /// auto-play): card-selection screens resolve to the first `max` candidates instead of raising a decision.
    pub auto_select: bool,
    /// Where the interrupted player-turn start resumes (see `Combat::run_turn_start`).
    pub turn_cont: Option<TurnCont>,
}

/// See `Combat::suspend_hook`.
#[derive(Clone, Copy, Debug)]
pub struct PendingHook {
    pub me: crate::hooks::Me,
    pub phase: u8,
}

/// Resume point of an interrupted player-turn start: the step and how many listeners of its dispatch already ran.
#[derive(Clone, Copy, Debug)]
pub struct TurnCont {
    pub step: u8,
    pub done: u8,
}

// ---- relic persistent state description (see `Listener::meta_*` and content/relics) ------------------------------------

/// Which field of [`Relic`] stores a relic property.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Counter,
    Aux,
    /// Bit `n` of `Relic::flags` (booleans).
    Flag(u8),
}

/// A `[SavedProperty]` of the real relic class: how the oracle names it (C# property name), where the Rust relic keeps it,
/// and whether the game omits it when it has the type default (`SerializationCondition.SaveIfNotTypeDefault`).
#[derive(Clone, Copy, Debug)]
pub struct PropDef {
    pub name: &'static str,
    pub slot: Slot,
    pub boolean: bool,
    pub skip_default: bool,
    /// Non-empty: a property the relic always saves with this fixed JSON value (empty arrays, ...); no state slot.
    pub lit: &'static str,
}

impl PropDef {
    pub const fn int(name: &'static str, slot: Slot) -> PropDef {
        PropDef { name, slot, boolean: false, skip_default: false, lit: "" }
    }
    pub const fn flag(name: &'static str, bit: u8) -> PropDef {
        PropDef { name, slot: Slot::Flag(bit), boolean: true, skip_default: false, lit: "" }
    }
    /// A saved property with a fixed JSON literal value (not stored in the relic).
    pub const fn constant(name: &'static str, lit: &'static str) -> PropDef {
        PropDef { name, slot: Slot::Counter, boolean: false, skip_default: false, lit }
    }
    pub const fn skip_default(mut self) -> PropDef {
        self.skip_default = true;
        self
    }
}

impl Relic {
    #[inline(always)]
    pub fn flag(&self, bit: u8) -> bool {
        self.flags & (1 << bit) != 0
    }
    #[inline(always)]
    pub fn set_flag(&mut self, bit: u8, v: bool) {
        if v {
            self.flags |= 1 << bit;
        } else {
            self.flags &= !(1 << bit);
        }
    }
    /// Reads a property through its slot (bools as 0/1).
    pub fn get(&self, s: Slot) -> i32 {
        match s {
            Slot::Counter => self.counter,
            Slot::Aux => self.aux,
            Slot::Flag(b) => self.flag(b) as i32,
        }
    }
    pub fn set(&mut self, s: Slot, v: i32) {
        match s {
            Slot::Counter => self.counter = v,
            Slot::Aux => self.aux = v,
            Slot::Flag(b) => self.set_flag(b, v != 0),
        }
    }
}
