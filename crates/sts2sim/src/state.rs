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
    /// Created by `CardModel.CreateClone` (`IsClone`): see `Combat::clone_card`.
    pub const IS_CLONE: u16 = 1 << 6;
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
    /// Enchantment id + 1 (0 = none).
    pub enchant: u8,
    pub enchant_amount: i16,
    /// `EnchantmentModel.Status`: 0 = Normal, 1 = Disabled.
    pub enchant_status: u8,
    /// Enchantment-private state (Glam used / Momentum extra damage ...).
    pub enchant_aux: i16,
    /// Affliction id + 1 (0 = none).
    pub affliction: u8,
    pub affliction_amount: i16,
    pub base_replay: u8,
    /// Base energy cost after upgrades (`CardEnergyCost._base`); -1 = no cost.
    pub cost_base: i8,
    pub x_value: i16,
    pub mods: crate::engine::CostMods,
    /// Temporary star costs (`_temporaryStarCosts`); the LAST entry wins. `amount` = cost, `expire` as for `mods`.
    pub star_mods: ArrayVec<CostMod, 2>,
    /// Per-card persistent counters (Rampage damage, Regret, ...), meaning defined by the card.
    pub counter: [i16; 2],
    /// Permanent bonus to the card's Damage var in units of 1/10000 (Rampage, Thrash: `DynamicVars.Damage.BaseValue += x`).
    pub dmg_bonus: i32,
    /// Deck index this combat card was cloned from (`DeckVersion`), `NO` if none.
    pub deck_idx: u8,
    /// The card this dupe / clone was created from (`DupeOf`), `NO` if none.
    pub dupe_of: u8,
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
    /// The synthetic `STUNNED` state has been performed (`_performedAtLeastOnce`).
    pub stun_performed: bool,
    /// Side effect of the stunned turn (`CreatureCmd.Stun(creature, stunMove, ..)`).
    pub stun_move: Option<crate::defs::MoveFn>,
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
            stun_performed: false,
            stun_move: None,
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

/// The nine run-level streams combat consumes (names as `RunRngType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RngStream {
    Shuffle,
    CombatCardGeneration,
    CombatPotionGeneration,
    CombatCardSelection,
    CombatEnergyCosts,
    CombatTargets,
    MonsterAi,
    Niche,
    CombatOrbs,
}

impl Combat {
    pub fn rng_stream_mut(&mut self, s: RngStream) -> &mut Rng {
        match s {
            RngStream::Shuffle => &mut self.rng.shuffle,
            RngStream::CombatCardGeneration => &mut self.rng.combat_card_generation,
            RngStream::CombatPotionGeneration => &mut self.rng.combat_potion_generation,
            RngStream::CombatCardSelection => &mut self.rng.combat_card_selection,
            RngStream::CombatEnergyCosts => &mut self.rng.combat_energy_costs,
            RngStream::CombatTargets => &mut self.rng.combat_targets,
            RngStream::MonsterAi => &mut self.rng.monster_ai,
            RngStream::Niche => &mut self.rng.niche,
            RngStream::CombatOrbs => &mut self.rng.combat_orbs,
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
/// Cards of one `AutoPlayFromDrawPile` / `DiscardAndDraw` call still waiting to be auto-played (front = next).
#[derive(Clone, Copy)]
pub struct AutoQueue {
    pub cards: ArrayVec<CardIdx, 10>,
    /// `AutoPlayFromDrawPile(forceExhaust)`.
    pub force_exhaust: bool,
    /// `AutoPlayType.SlyDiscard` queue (else `Default`).
    pub sly: bool,
    /// Index in `play_stack` of the card play whose effect started the call (-1: none). The queue continues when the
    /// play above it finishes.
    pub owner: i8,
}

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
    /// `resultLocation` of `OnPlayWrapper` (`PileType::None` = removed from combat).
    pub result: CardLocation,
}

/// Counters the game's combat history exposes to gameplay code (cards played this turn etc.).
#[derive(Clone, Copy, Default)]
pub struct History {
    pub cards_played_this_turn: i16,
    pub attacks_played_this_turn: i16,
    pub skills_played_this_turn: i16,
    /// `CardExhaustedEntry`s of the current round/side.
    pub cards_exhausted_this_turn: i16,
    /// `CardPlayFinishedEntry`s of Attack cards this turn.
    pub attacks_finished_this_turn: i16,
    /// `CardPlayFinishedEntry`s of Skill cards / Shiv-tagged cards this turn (Silent: Finesse-likes).
    pub skills_finished_this_turn: i16,
    pub shivs_finished_this_turn: i16,
    /// Bitset over card arena indices: cards with a `CardPlayFinishedEntry` this turn (Necrobinder).
    pub finished_cards: [u64; 3],
    /// Per-play scratch used by Serpent Form / Strangle: the power amount when `BeforeCardPlayed` ran for a card.
    pub play_amounts: ArrayVec<PlayAmount, 16>,
}

impl History {
    /// Whether card `c` has a `CardPlayFinishedEntry` this turn.
    pub fn finished(&self, c: CardIdx) -> bool {
        self.finished_cards[(c / 64) as usize] >> (c % 64) & 1 != 0
    }
    pub fn set_finished(&mut self, c: CardIdx) {
        self.finished_cards[(c / 64) as usize] |= 1u64 << (c % 64);
    }
}

/// `Dictionary<CardModel, int> amountsForPlayedCards` entry of a power instance (keyed by power uid + card).
#[derive(Clone, Copy, Default, Debug)]
pub struct PlayAmount {
    pub uid: u16,
    pub card: CardIdx,
    pub amount: i32,
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
    /// The part of `listen` contributed by card instances / their enchantments / afflictions: a snapshot skips the pile
    /// scan entirely when none of them listens to the queried hooks.
    pub listen_cards: Mask,

    pub player: PlayerState,
    pub cards: [Card; MAX_CARDS],
    pub n_cards: u16,
    pub hist: History,

    /// In-flight card plays, innermost last (an auto-play started from inside `on_play` pushes a nested play);
    /// suspended while a decision is pending.
    pub play_stack: ArrayVec<PlayCtx, 4>,
    pub potion_ctx: Option<PotionCtx>,
    pub decision: Option<Decision>,
    pub choice: Choice,
    /// A hook that raised a decision, resumed through `Listener::resume_hook` once the choice is in `choice`.
    pub hook_ctx: Option<(crate::hooks::Me, u8)>,
    /// Where a turn start suspended by a hook decision resumes (0 = not suspended).
    pub turn_cont: u8,
    /// The `AfterAutoPostPlayPhaseEntered` listener that suspended (auto-played card raised a decision) while the
    /// player's turn was ending; the turn end resumes from it once the decision is made.
    pub end_turn_resume: Option<crate::hooks::Me>,
    /// First piece of content used in this combat that has no Rust implementation yet (kind, id). A fight with this
    /// set is NOT faithful; env wrappers must treat it as an error.
    pub missing: Option<(crate::hooks::Kind, u16)>,

    // ---- engine-core additions ----
    /// `Player.IsActiveForHooks`: false from the end of the player's death sequence (`DeactivateHooks`) until revived.
    /// Relics / potions / orbs / cards and the player's powers stop listening while false.
    pub player_hooks_active: bool,
    /// Enemies that escaped (`CombatState.EscapedCreatures`).
    pub escaped: u8,
    /// `PlayersTakingExtraTurn` is non-empty (single player).
    pub extra_turn: bool,
    /// Side channel for the post-damage hooks whose C# signature has more parameters than the Rust hook: the card
    /// source and the full `DamageResult` of the result being dispatched.
    pub dmg_card: CardIdx,
    pub dmg_result: crate::engine::DamageResult,
    /// Auto-play queues still waiting to be drained (one per in-progress `AutoPlayFromDrawPile` / Sly discard call; they
    /// nest like the C# locals: a card auto-played from a queue may itself start another one).
    pub autoplay_stack: ArrayVec<AutoQueue, 4>,
    /// Combat history log (`engine/history.rs`).
    pub hist_log: crate::engine::HistLog,
    /// Number of decisions raised so far (lets a driver tell "the same decision" from "the next one").
    pub decision_seq: u32,
    /// `DeckVersion` write-backs of enchantment amounts (Goopy): increments per deck index (outputs of the combat).
    pub deck_enchant_inc: [u8; 80],
    /// Upgrade level of each run-deck card (`DeckVersion.CurrentUpgradeLevel`; index = deck index) and the deck size. Combat copies
    /// upgrade independently; only deck-level upgrades (Improvement power at combat end) change these.
    pub deck_upgrade: [u8; 80],
    pub deck_len: u8,
    /// Identity of the card play iteration in flight (`CardPlay` object): bumped before each `BeforeCardPlayed`.
    pub play_serial: u16,
    /// Run inputs combat reads (spec 05 §2.1): the player's gold and the act index (0-based).
    pub gold: i32,
    pub act: u8,
    /// `PlayerCmd.EndTurn` was requested (Void Form ...): the end-turn signal is consumed when the effect / turn start
    /// that raised it returns.
    pub end_turn_requested: bool,
}
