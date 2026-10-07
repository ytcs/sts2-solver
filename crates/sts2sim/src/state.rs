//! The complete, plain-data combat state. Everything here is `Copy`-able: cloning a fight is a memcpy and the
//! hot path never allocates.

use crate::hooks::Mask;
use crate::rng::Rng;
use crate::types::*;
use crate::util::{ArrayVec, SmallVec};

pub const MAX_CARDS: usize = 160;
pub const MAX_CREATURES: usize = 12;
pub const MAX_POWERS: usize = 16;
/// Relics a combat holds (late runs carry 25-30); the observation shows at most `OBS_RELICS` of the combat ones (`relic_mask`).
pub const MAX_RELICS: usize = 40;
pub const OBS_RELICS: usize = 24;
/// Belt slots (A10 has 2; belt relics add more). 8 since M3 (`docs/rl_redesign.md`).
pub const MAX_POTIONS: usize = 8;
pub const MAX_HAND: usize = 10;
pub const MAX_ORBS: usize = 10;
/// Largest deck a combat accepts: deck card `i` indexes the `deck_*` side tables (`[_; MAX_DECK]`) and the rest of the card
/// arena is kept for generated cards.
pub const MAX_DECK: usize = 80;

/// Bits of [`Combat::overflow`]: a fixed capacity was exceeded and data was dropped, so the fight can no longer be
/// guaranteed faithful. Env wrappers must abort / truncate such an episode (like `missing`).
pub mod ov {
    /// A fixed-capacity `ArrayVec` (power list, decision candidates, snapshot, results, piles ...) was full on a push.
    pub const CONTAINER: u16 = crate::util::OV_CONTAINER as u16;
    /// The card arena (`MAX_CARDS`) was full when a card had to be created.
    pub const CARDS: u16 = 1 << 1;
    /// No free creature slot (`MAX_CREATURES`) for a spawned enemy / pet.
    pub const CREATURES: u16 = 1 << 2;
    /// The history ring overwrote an entry that a this-turn / last-turn query could still need.
    pub const HISTORY: u16 = 1 << 3;
    /// A saturating whole-combat counter hit its limit.
    pub const COUNTER: u16 = 1 << 4;
    /// The scenario does not fit the fixed capacities (deck / relics / potions / ...).
    pub const SCENARIO: u16 = 1 << 5;
    /// Runaway-work safeguard: one `step` (or one look-ahead turn) exceeded [`WORK_LIMIT`], [`HOOK_DEPTH_LIMIT`] or
    /// [`TURN_LIMIT`] (an infinite trigger chain). The step was cut short: the combat is only safe to drop (see `Combat::trip_loop`).
    pub const LOOP: u16 = 1 << 6;
}

// Limits of the runaway-work safeguard (`engine/budget.rs`). Measured (2026-10) over ~95k fights / 5.5M steps: the training scenario
// sets (`data/train/eval.json`, `mid.json`, `data/corpus/fights_*.json`), 13k fuzz scenarios (`tools/fuzz_gen.py gen`, incl. `--relic-mode
// many`; `tools/fuzz_gen_mix.py --gen-only`, all five characters, incl. `--mode deep`), the oracle templates and frozen regressions,
// each played with 4 random-policy seeds and a greedy play-everything policy for up to 600 (deep: 3000) steps, observing every step.
// Maxima: 174 work units in one step (a Glory boss template), hook depth 12 (fuzz) / 10 (`eidolon_long_exhaust_queue`), 2 player turns
// started by one step, 59 work units in one look-ahead turn.

/// Most work units one `Combat::step` (or one look-ahead turn) may spend before it is cut short with `ov::LOOP`. A unit is a hook pass
/// that has listeners (`dispatch_slow`, `dispatch_resumable`, `dispatch_modifiers_slow`, the post-play pass), a card play (each
/// `PlayStep::Before`, i.e. every replay of a card), an attack hit or a monster state-machine transition. ~115x the measured maximum
/// (174); a tripped step costs a few milliseconds at most.
pub const WORK_LIMIT: u32 = 20_000;
/// Most hook passes nested inside each other (a hook whose effect fires a hook whose effect ...). The chain is Rust recursion: a
/// looping one overflows the stack long before it exhausts the work budget, so the depth has its own, stack-safe limit (~5x the
/// measured maximum of 12; 64 levels fit a 2 MB debug-build test thread).
pub const HOOK_DEPTH_LIMIT: u16 = 64;
/// Most player turns one step may start (10x the measured maximum of 2): an end of turn requested at every turn start (Void Form
/// auto-played by Mayhem) recurses turn after turn inside one step, outside any hook pass.
pub const TURN_LIMIT: u16 = 20;

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

/// A local cost modifier (`CardEnergyCost` modifier / temporary star cost), packed into two bytes.
#[derive(Clone, Copy, Default, Debug)]
pub struct CostMod {
    pub amount: i8,
    /// bit0 = relative (add to the running cost), bit1 = reduce_only, bits 2.. = `expire` (see below).
    bits: u8,
}
impl CostMod {
    /// `expire`: 0 = lasts the combat, `EXPIRE_END_OF_TURN` (2) and/or `EXPIRE_WHEN_PLAYED` (4).
    #[inline(always)]
    pub const fn new(amount: i8, relative: bool, reduce_only: bool, expire: u8) -> CostMod {
        CostMod { amount, bits: relative as u8 | (reduce_only as u8) << 1 | expire << 2 }
    }
    #[inline(always)]
    pub const fn relative(self) -> bool {
        self.bits & 1 != 0
    }
    #[inline(always)]
    pub const fn reduce_only(self) -> bool {
        self.bits & 2 != 0
    }
    /// bit1 (2) = expires end of turn, bit2 (4) = expires when played. 0 = lasts the combat.
    #[inline(always)]
    pub const fn expire(self) -> u8 {
        self.bits >> 2
    }
}
pub const EXPIRE_END_OF_TURN: u8 = 2;
pub const EXPIRE_WHEN_PLAYED: u8 = 4;

pub mod cflag {
    pub const EXHAUST_ON_NEXT_PLAY: u8 = 1 << 0;
    pub const SINGLE_TURN_RETAIN: u8 = 1 << 1;
    pub const SINGLE_TURN_SLY: u8 = 1 << 2;
    pub const IS_DUPE: u8 = 1 << 3;
    /// Card left the combat for good (`HasBeenRemovedFromState`).
    pub const REMOVED: u8 = 1 << 4;
    pub const X_CAPTURED: u8 = 1 << 5;
    /// Created by `CardModel.CreateClone` (`IsClone`): see `Combat::clone_card`.
    pub const IS_CLONE: u8 = 1 << 6;
}

/// One card instance in the combat arena.
#[derive(Clone, Copy, Default, Debug)]
pub struct Card {
    pub id: u16,
    pub pile: PileTypeBits,
    pub upgrade: u8,
    pub flags: u8,
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
    pub star_mods: SmallVec<CostMod, 2>,
    /// Per-card persistent counters (Rampage damage, Regret, ...), meaning defined by the card.
    pub counter: [i16; 2],
    /// Permanent bonus to the card's Damage var in units of 1/10000 (Rampage, Thrash: `DynamicVars.Damage.BaseValue += x`).
    pub dmg_bonus: i32,
    /// Deck index this combat card was cloned from (`DeckVersion`), `NO` if none.
    pub deck_idx: u8,
    /// The card this dupe / clone was created from (`DupeOf`), `NO` if none.
    pub dupe_of: u8,
    /// Upgrade level this card had when `DampenPower` downgraded it (`downgradedCardsToOldUpgradeLevels`), 0 = none.
    pub dampen_saved: u8,
}
pub type PileTypeBits = u8;

/// One orb in the `OrbQueue`. `kind` is an `ids::orb::*` id.
#[derive(Clone, Copy, Default, Debug)]
pub struct Orb {
    pub kind: u16,
    /// Unique per combat (object identity).
    pub uid: u16,
    /// Dark: accumulated `_evokeVal` (starts at 6); Glass: base `_passiveVal` (starts at 4); 0 otherwise.
    pub val: i32,
}

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
    /// Cache: some power of this creature has `PowerDef::secondary_enemy` (`OwnerIsSecondaryEnemy`: Minion, Illusion), which
    /// makes it not count for `is_ending`. Maintained by `Combat::sync_secondary` at every power-list mutation.
    pub secondary: bool,
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
            secondary: false,
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
    /// `OrbQueue.Capacity`: the number of orb slots (starts at `Player.BaseOrbSlotCount`; cards / potions / relics change it).
    /// `PlayerCombatState.OrbQueue`: orbs front (next to evoke) first, and the slot capacity.
    pub orbs: ArrayVec<Orb, MAX_ORBS>,
    pub orb_slots: u8,
    /// Next `Orb::uid` (orbs are objects in the game; tests such as `orb == Orbs[0]` and `Remove(orb)` are by reference).
    pub next_orb_uid: u16,
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
    /// Candidates in the order the GAME presents them (what the oracle's selector indexes). For pile screens this can
    /// reveal the pile order, so the agent never sees this order: it sees `Combat::decision_view` (a canonical order).
    pub cands: ArrayVec<CardIdx, MAX_CARDS>,
    /// Candidate indices (into `cands`) selected so far, in click order.
    pub selected: ArrayVec<u8, 16>,
    /// `RequireManualConfirmation` (`min != max`).
    pub confirm_required: bool,
    /// May finish with nothing selected (skippable choose-a-card screens).
    pub can_skip: bool,
    /// Content-defined purpose tag (which card/relic/potion asked).
    pub purpose: u16,
}

/// A step re-run from its starting state so a decision raised somewhere the engine cannot suspend (a draw's reshuffle in the
/// middle of a card effect, an auto-played card ...) can be answered by the agent: see `engine/replay.rs`.
#[derive(Clone)]
pub struct Replay {
    /// The state the step started from.
    pub s0: Combat,
    pub action: crate::engine::Action,
    /// The agent's answers so far (candidate indices in game order, click order), consumed in order by the re-run.
    pub answers: ArrayVec<ArrayVec<u8, 16>, 6>,
    pub pos: u8,
    /// True when this is the state the agent is shown (the prompt with the effect's partial results), not a step in flight.
    pub at_prompt: bool,
    /// The prompt's selection was completed (the owning `step` then re-runs the action).
    pub done: bool,
    /// The state at the first unanswered prompt of the running step.
    pub capture: Option<Box<Combat>>,
}

/// The finished selection handed back to the resumed effect.
#[derive(Clone, Copy, Default)]
pub struct Choice {
    /// The chosen cards (`Decision::cands[pick]`), in click order.
    pub cards: ArrayVec<CardIdx, 16>,
}

/// A suspended resumable hook pass: the listener that raised the decision and the listeners that were still to run (the game
/// iterates a list built at the start of the pass; models that moved meanwhile, e.g. an auto-played card, keep their place in
/// it). `full` = nothing was cut off at the capacity (otherwise the pass is rebuilt from a fresh snapshot, best effort).
#[derive(Clone, Copy)]
pub struct SuspPass {
    pub bit: u32,
    pub me: crate::hooks::Me,
    pub pos: u8,
    pub full: bool,
    pub rest: ArrayVec<crate::hooks::Me, 8>,
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
/// Most cards one `AutoPlayFromDrawPile` call can queue (Cascade with X = energy; Ice Cream can bank a lot of energy).
pub const AUTOPLAY_MAX: usize = 24;

#[derive(Clone, Copy)]
pub struct AutoQueue {
    pub cards: ArrayVec<CardIdx, AUTOPLAY_MAX>,
    /// `AutoPlayFromDrawPile(forceExhaust)`.
    pub force_exhaust: bool,
    /// `AutoPlayType.SlyDiscard` queue (else `Default`).
    pub sly: bool,
    /// A plain list of `CardCmd.AutoPlay` calls (Eidolon): the cards' own exhaust-on-next-play flags are left alone.
    pub plain: bool,
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
    /// `CardPlayFinishedEntry`s of any card this turn (Pale Blue Dot: nested auto-plays finish before their parent).
    pub cards_finished_this_turn: i16,
    /// `CardPlayFinishedEntry`s of Attack cards this turn.
    pub attacks_finished_this_turn: i16,
    /// `CardPlayFinishedEntry`s of Skill cards / Shiv-tagged cards this turn (Silent: Finesse-likes).
    pub skills_finished_this_turn: i16,
    pub shivs_finished_this_turn: i16,
    /// Bitset over card arena indices: cards with a `CardPlayFinishedEntry` this turn (Necrobinder).
    pub finished_cards: [u64; 3],
    /// Per-play scratch used by Serpent Form / Strangle: the power amount when `BeforeCardPlayed` ran for a card.
    pub play_amounts: ArrayVec<PlayAmount, 32>,
}

impl History {
    /// Whether card `c` has a `CardPlayFinishedEntry` this turn.
    pub fn finished(&self, c: CardIdx) -> bool {
        self.finished_cards[(c / 64) as usize] >> (c % 64) & 1 != 0
    }
    pub fn set_finished(&mut self, c: CardIdx) {
        self.finished_cards[(c / 64) as usize] |= 1u64 << (c % 64);
    }
    /// `amountsForPlayedCards.Add(card, amount)` of power instance `uid` (entries nest: auto-plays start plays inside plays).
    pub fn remember_play(&mut self, uid: u16, card: CardIdx, amount: i32) {
        if self.play_amounts.len() < 32 {
            self.play_amounts.push(PlayAmount { uid, card, amount });
        } else {
            crate::util::raise_overflow(crate::util::OV_CONTAINER);
        }
    }
    /// `amountsForPlayedCards.Remove(card, out amount)`.
    pub fn take_play(&mut self, uid: u16, card: CardIdx) -> Option<i32> {
        let pos = self.play_amounts.as_slice().iter().rposition(|e| e.uid == uid && e.card == card)?;
        Some(self.play_amounts.remove(pos).amount)
    }
    /// Mutable access to the entry (`playedCards[card] += ...`).
    pub fn play_entry(&mut self, uid: u16, card: CardIdx) -> Option<&mut i32> {
        self.play_amounts.as_mut_slice().iter_mut().rev().find(|e| e.uid == uid && e.card == card).map(|e| &mut e.amount)
    }
}

/// `Dictionary<CardModel, int> amountsForPlayedCards` entry of a power instance (keyed by power uid + card).
#[derive(Clone, Copy, Default, Debug)]
pub struct PlayAmount {
    pub uid: u16,
    pub card: CardIdx,
    pub amount: i32,
}

#[derive(Clone)]
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
    pub play_stack: ArrayVec<PlayCtx, 6>,
    pub potion_ctx: Option<PotionCtx>,
    pub decision: Option<Decision>,
    pub choice: Choice,
    /// A hook that raised a decision, resumed through `Listener::resume_hook` once the choice is in `choice`.
    pub hook_ctx: Option<(crate::hooks::Me, u8)>,
    /// A turn-start hand draw interrupted by a decision raised in `AfterShuffle` (Stratagem): (cards still to draw,
    /// from_hand_draw). `turn_cont == 4` resumes it.
    pub draw_resume: Option<(i32, bool)>,
    /// True while the turn-start hand draw runs (the only draw whose `AfterShuffle` decisions can be resumed).
    pub drawing_hand: bool,
    /// Nesting depth of `draw_cards_list` (a draw started by an `AfterCardDrawn` hook of another draw is depth 2).
    pub draw_depth: u8,
    /// >0 while a draw whose caller reads the drawn cards / asks right afterwards runs: its shuffle decisions cannot be paused.
    pub draw_nosuspend: u8,
    /// True while a hook (Foregone Conclusion's `BeforeHandDraw`) shuffles by itself: its `AfterShuffle` decision (Stratagem) can be
    /// paused (`hook_after`).
    pub hook_shuffle: bool,
    /// True once a Stratagem card exists in this combat: every `step` then runs under replay (see `engine/replay.rs`) so the card's
    /// reshuffle prompt works in draw contexts that cannot be suspended.
    pub strat_possible: bool,
    /// Replay state of the step that is running / of the prompt the agent is looking at (None almost always).
    pub replay: Option<Box<Replay>>,
    /// A hook whose own effect waits for the nested `AfterShuffle` decision pass: `resume_hook(phase)` runs once that pass is done.
    pub hook_after: Option<(crate::hooks::Me, u8)>,
    /// Where a turn start suspended by a hook decision resumes (0 = not suspended): 1 = in `BeforeHandDraw`,
    /// 2 = in `BeforeHandDrawLate`, 3 = in `AfterPlayerTurnStart`, 4 = interrupted opening hand draw, 5 / 6 / 7 = in the early /
    /// normal / late `AfterAutoPrePlayPhaseEntered` pass.
    pub turn_cont: u8,
    /// Resumable notification passes (`Combat::dispatch_resumable`) suspended by a decision, innermost last (a pass can be
    /// suspended inside another suspended pass: the turn-start hook of Mayhem auto-plays a card whose draw reshuffles).
    pub susp: ArrayVec<SuspPass, 3>,
    /// The `AfterShuffle` / `AfterCardDrawn(Early)` pass (drawn card, 0 = early / 1 = normal / 2 = after-shuffle) that a decision
    /// interrupted during the turn-start hand draw (`turn_cont` 4 finishes it, then the rest of the draw).
    pub draw_pass: Option<(CardIdx, u8)>,
    /// An enemy turn suspended inside a monster move that raised a decision (Knowledge Demon's Curse of Knowledge):
    /// the `Enemies` snapshot taken at the start of the turn and the index of the suspended mover.
    pub enemy_cont: Option<(ArrayVec<Cid, MAX_CREATURES>, u8, u8)>,
    /// The `AfterAutoPostPlayPhaseEntered` listener that suspended (auto-played card raised a decision) while the
    /// player's turn was ending; the turn end resumes from it once the decision is made.
    pub end_turn_resume: Option<crate::hooks::Me>,
    /// First piece of content used in this combat that has no Rust implementation yet (kind, id). A fight with this
    /// set is NOT faithful; env wrappers must treat it as an error.
    pub missing: Option<(crate::hooks::Kind, u16)>,
    /// Bitset of `ov::*`: a fixed capacity was exceeded and data was dropped (never silently: see `util::raise_overflow`).
    /// Non-zero = the fight is NOT faithful; env wrappers must abort / truncate the episode (like `missing`).
    pub overflow: u16,
    /// Runaway-work safeguard (`ov::LOOP`): work units spent by the running step (reset by `step`, `look_turn`, `reset`).
    pub work: u32,
    /// The step's budget for `work`: [`WORK_LIMIT`] (tests may lower it); 0 once the safeguard tripped (every further unit fails).
    pub work_limit: u32,
    /// Hook passes running now, nested inside each other (limit [`HOOK_DEPTH_LIMIT`]).
    pub hook_depth: u16,
    /// Player turns started by the running step (limit [`TURN_LIMIT`]).
    pub step_turns: u16,

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
    /// `AttackCommand.Results` (first 16 per-hit results) of the attack whose `after_attack` hooks are being dispatched
    /// (only filled when some listener has `after_attack`): Suck, Skittish.
    pub attack_results: ArrayVec<crate::engine::DamageResult, 16>,
    /// Sizes of the per-hit groups of `attack_results` (C# `command.Results` is a list of per-hit result lists).
    pub attack_hit_sizes: ArrayVec<u8, 16>,
    /// Side channel for `AfterAttack` (C# `command.Results`): set by `execute_attack` right before the hook pass.
    /// `attack_unblocked_hits` = results with unblocked damage > 0 (any receiver); `attack_player_hits` = those whose
    /// receiver is the player creature.
    pub attack_unblocked_hits: u8,
    pub attack_player_hits: u8,
    /// Auto-play queues still waiting to be drained (one per in-progress `AutoPlayFromDrawPile` / Sly discard call; they
    /// nest like the C# locals: a card auto-played from a queue may itself start another one).
    pub autoplay_stack: ArrayVec<AutoQueue, 4>,
    /// Combat history log (`engine/history.rs`).
    pub hist_log: crate::engine::HistLog,
    /// Number of decisions raised so far (lets a driver tell "the same decision" from "the next one").
    pub decision_seq: u32,
    /// `DeckVersion` write-backs of enchantment amounts (Goopy): increments per deck index (outputs of the combat).
    pub deck_enchant_inc: [u8; MAX_DECK],
    /// Upgrade level of each run-deck card (`DeckVersion.CurrentUpgradeLevel`; index = deck index) and the deck size. Combat copies
    /// upgrade independently; only deck-level upgrades (Improvement power at combat end) change these.
    pub deck_upgrade: [u8; MAX_DECK],
    pub deck_len: u8,
    /// Identity of the card play iteration in flight (`CardPlay` object): bumped before each `BeforeCardPlayed`.
    pub play_serial: u16,
    /// Run inputs combat reads (spec 05 §2.1): the player's gold and the act index (0-based).
    pub gold: i32,
    pub act: u8,
    /// `PlayerCmd.EndTurn` was requested (Void Form ...): the end-turn signal is consumed when the effect / turn start
    /// that raised it returns.
    pub end_turn_requested: bool,
    /// Room kind of the encounter (0 monster, 1 elite, 2 boss), for relics gated on `CurrentRoom.RoomType`.
    pub room_type: u8,
    /// Bit i set when deck card i (scenario deck order) is upgradable (FishingRod / WarHammer item counts).
    pub deck_upgradable: u128,
    /// `cardSource` of the power application being dispatched (`BeforePowerAmountChanged` has no card parameter).
    pub cur_power_card: CardIdx,
    /// An automated card selector is active (Whispering Earring pushes `VakuuCardSelector`): card-selection screens
    /// resolve to the first `max` candidates instead of raising a decision.
    pub auto_select: bool,
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
