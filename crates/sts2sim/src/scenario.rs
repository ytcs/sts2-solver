//! Combat initial conditions (spec 05 §"input schema") and `Combat::new`.

use crate::content;
use crate::hooks::Mask;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// One deck card (deck order matters: it is the input to the initial shuffle).
#[derive(Clone, Copy, Debug)]
pub struct DeckCard {
    pub id: u16,
    pub upgrade: u8,
}

/// A relic at combat entry with its persistent state (`Relic::{counter, flags, aux}`; which property lives in which slot is
/// the relic's own `Listener::meta_props`). `RelicInit { id, ..Default::default() }` = a fresh instance.
#[derive(Clone, Copy, Debug, Default)]
pub struct RelicInit {
    pub id: u16,
    pub counter: i32,
    pub flags: u8,
    pub aux: i32,
}

/// Everything that can differ between combats.
#[derive(Clone, Debug)]
pub struct Scenario {
    /// Run seed and floor: seed the encounter-local RNG (`runSeed + totalFloor + hash(encounterId)`).
    pub run_seed: u64,
    pub total_floor: i32,
    /// 0 Ironclad, 1 Silent, 2 Defect, 3 Necrobinder, 4 Regent (selects the card pool used by generation effects).
    pub character: u8,
    pub ascension: u8,
    pub encounter: u16,
    pub max_hp: i32,
    pub hp: i32,
    pub max_energy: i32,
    pub orb_slots: u8,
    pub potion_slots: u8,
    pub deck: Vec<DeckCard>,
    pub relics: Vec<RelicInit>,
    pub potions: Vec<u16>,
    /// The run-level RNG streams at combat entry (state carries over between combats in the real game).
    pub rng: RngSet,
}

/// Optional per-card inputs of a saved deck card (`SerializableCard.enchantment` / `props`). Kept out of `DeckCard` so
/// existing `DeckCard { id, upgrade }` literals keep compiling; `deck[i]` extras are index-aligned with `Scenario::deck`
/// (missing entries = no enchantment, zero props).
#[derive(Clone, Copy, Debug, Default)]
pub struct DeckExtra {
    /// Enchantment id + 1 (`ids::enchantment::*` + 1); 0 = none.
    pub enchant: u8,
    pub enchant_amount: i16,
    /// The card's `[SavedProperty]` values (ints / bools in the order of the scenario JSON), stored in `Card::counter`.
    pub props: [i16; 2],
}

/// Everything optional about a combat's initial conditions (see `Combat::new_with`).
#[derive(Clone, Debug, Default)]
pub struct ScenarioExtras {
    pub deck: Vec<DeckExtra>,
    /// `Player.Gold` (read / changed by Debt, Thievery, Royalties ...).
    pub gold: i32,
    /// `RunState.CurrentActIndex`.
    pub act: u8,
}

#[derive(Debug)]
pub enum ScenarioError {
    UnimplementedCard(&'static str),
    UnimplementedRelic(&'static str),
    UnimplementedPotion(&'static str),
    UnimplementedEncounter(&'static str),
    /// More deck cards than `MAX_DECK` (the card arena also has to hold generated cards).
    DeckTooLarge,
    /// More relics than `MAX_RELICS`.
    TooManyRelics,
    /// More potions than potion slots (`MAX_POTIONS` at most).
    TooManyPotions,
    /// `orb_slots > MAX_ORBS`, `potion_slots > MAX_POTIONS` or a non-positive max HP.
    BadStats,
    /// The encounter's spawn list does not fit the creature slots.
    TooManyEnemies,
}

impl Scenario {
    pub fn validate(&self) -> Result<(), ScenarioError> {
        for c in &self.deck {
            if !content::card_implemented(c.id) {
                return Err(ScenarioError::UnimplementedCard(crate::ids::card::NAMES[c.id as usize]));
            }
        }
        for r in &self.relics {
            if !content::relic_implemented(r.id) {
                return Err(ScenarioError::UnimplementedRelic(crate::ids::relic::NAMES[r.id as usize]));
            }
        }
        for p in &self.potions {
            if !content::potion_implemented(*p) {
                return Err(ScenarioError::UnimplementedPotion(crate::ids::potion::NAMES[*p as usize]));
            }
        }
        if !content::encounter_implemented(self.encounter) {
            return Err(ScenarioError::UnimplementedEncounter(crate::ids::encounter::NAMES[self.encounter as usize]));
        }
        if self.deck.len() > MAX_DECK {
            return Err(ScenarioError::DeckTooLarge);
        }
        if self.relics.len() > MAX_RELICS {
            return Err(ScenarioError::TooManyRelics);
        }
        if self.potions.len() > MAX_POTIONS {
            return Err(ScenarioError::TooManyPotions);
        }
        if self.orb_slots as usize > MAX_ORBS || self.potion_slots as usize > MAX_POTIONS || self.max_hp <= 0 {
            return Err(ScenarioError::BadStats);
        }
        Ok(())
    }
}

/// `xxHash64(name)` of the nine run-level stream names (computed once; a reset only adds the seed).
fn stream_hashes() -> &'static [u64; 9] {
    static H: std::sync::LazyLock<[u64; 9]> = std::sync::LazyLock::new(|| {
        let h = crate::rng::deterministic_hash;
        [h("shuffle"), h("combat_card_generation"), h("combat_potion_generation"), h("combat_card_selection"), h("combat_energy_costs"), h("combat_targets"), h("monster_ai"), h("niche"), h("combat_orbs")]
    });
    &H
}

impl RngSet {
    /// Same as [`RngSet::from_run_seed`] with the stream-name hashes precomputed (the episode-reset path of the batch env).
    pub fn from_run_seed_fast(seed: u64) -> RngSet {
        let h = stream_hashes();
        let mk = |i: usize| crate::rng::Rng::new(seed.wrapping_add(h[i]));
        RngSet {
            shuffle: mk(0),
            combat_card_generation: mk(1),
            combat_potion_generation: mk(2),
            combat_card_selection: mk(3),
            combat_energy_costs: mk(4),
            combat_targets: mk(5),
            monster_ai: mk(6),
            niche: mk(7),
            combat_orbs: mk(8),
        }
    }
}

/// `xxHash64(encounter id)` per encounter id (the encounter-local RNG is seeded with `runSeed + floor + hash`).
fn encounter_hash(encounter: u16) -> u64 {
    static H: std::sync::LazyLock<Vec<u64>> =
        std::sync::LazyLock::new(|| crate::ids::encounter::NAMES.iter().map(|n| crate::rng::deterministic_hash(n)).collect());
    H[encounter as usize]
}

impl Combat {
    /// Builds the combat and runs it up to the first player decision (spec 01 §3-4). Panics on an invalid scenario
    /// (tests / tools); use [`Combat::try_new`] where a bad scenario must not take the process down.
    #[inline(always)]
    pub fn new(sc: &Scenario) -> Combat {
        Self::new_with(sc, &ScenarioExtras::default())
    }

    /// `new` plus the optional inputs (deck card enchantments / saved properties). Panics on an invalid scenario.
    pub fn new_with(sc: &Scenario, ex: &ScenarioExtras) -> Combat {
        // (not via `try_new_with`: wrapping the 20 KB state in a `Result` costs extra copies)
        let mut cx = Self::blank();
        cx.reset_with(sc, ex).expect("invalid scenario");
        cx
    }

    /// Non-panicking `new`: validates the scenario against the fixed capacities / implemented content.
    pub fn try_new(sc: &Scenario) -> Result<Combat, ScenarioError> {
        Self::try_new_with(sc, &ScenarioExtras::default())
    }

    pub fn try_new_with(sc: &Scenario, ex: &ScenarioExtras) -> Result<Combat, ScenarioError> {
        let mut cx = Self::blank();
        cx.reset_with(sc, ex)?;
        Ok(cx)
    }

    /// In-place [`Combat::new`]: re-initialises this combat to the start of `sc` (validating it first). Equivalent to
    /// `*self = Combat::new(sc)` but only touches live state (no 20 KB construct-and-copy): batch envs reset finished
    /// episodes through this.
    pub fn reset(&mut self, sc: &Scenario) -> Result<(), ScenarioError> {
        self.reset_with(sc, &ScenarioExtras::default())
    }

    pub fn reset_with(&mut self, sc: &Scenario, ex: &ScenarioExtras) -> Result<(), ScenarioError> {
        sc.validate()?;
        self.reset_validated(sc, ex, sc.run_seed, sc.rng)
    }

    /// `reset_with` for a scenario that already passed [`Scenario::validate`] (a batch env validates its scenario pool once):
    /// `run_seed` / `rng` replace the scenario's own, so the per-episode seeding needs no scenario clone. Never panics: an
    /// unusable scenario leaves the combat flagged (`overflow |= ov::SCENARIO`) and in a terminal state.
    pub fn reset_validated(&mut self, sc: &Scenario, ex: &ScenarioExtras, run_seed: u64, rng: RngSet) -> Result<(), ScenarioError> {
        crate::util::take_overflow();
        // Every field is named so that adding one to `Combat` without deciding how it resets is a compile error.
        let Combat {
            character,
            ascension,
            rng: rng_set,
            round,
            side,
            in_progress,
            is_starting,
            pending_loss,
            stage,
            outcome,
            creatures,
            allies,
            enemies,
            next_power_uid,
            listen,
            listen_cards,
            player,
            cards: _, // fully overwritten by `new_card_ex` when allocated; only `[..n_cards]` is ever read
            n_cards,
            hist,
            play_stack,
            potion_ctx,
            decision,
            choice,
            hook_ctx,
            draw_resume,
            drawing_hand,
            turn_cont,
            susp,
            draw_pass,
            enemy_cont,
            end_turn_resume,
            missing,
            overflow,
            work,
            work_limit,
            hook_depth,
            step_turns,
            player_hooks_active,
            escaped,
            extra_turn,
            dmg_card,
            dmg_result,
            attack_results,
            attack_hit_sizes,
            attack_unblocked_hits,
            attack_player_hits,
            draw_depth,
            draw_nosuspend,
            hook_shuffle,
            strat_possible,
            replay,
            hook_after,
            autoplay_stack,
            hist_log,
            decision_seq,
            deck_enchant_inc,
            deck_upgrade,
            deck_len,
            play_serial,
            gold,
            act,
            end_turn_requested,
            room_type,
            deck_upgradable,
            cur_power_card,
            auto_select,
        } = self;
        *character = sc.character;
        *ascension = sc.ascension;
        *rng_set = rng;
        *round = 1;
        *side = Side::Player;
        *in_progress = false;
        *is_starting = true;
        *pending_loss = false;
        *stage = Stage::AwaitAction;
        *outcome = Outcome::Ongoing;
        for c in creatures.iter_mut() {
            *c = Creature::default();
        }
        allies.clear();
        enemies.clear();
        *next_power_uid = 1;
        *listen = Mask::EMPTY;
        *listen_cards = Mask::EMPTY;
        *n_cards = 0;
        *hist = History::default();
        play_stack.clear();
        *potion_ctx = None;
        *decision = None;
        choice.cards.clear();
        *end_turn_resume = None;
        *hook_ctx = None;
        *draw_resume = None;
        *drawing_hand = false;
        *turn_cont = 0;
        susp.clear();
        *draw_pass = None;
        *enemy_cont = None;
        *missing = None;
        *overflow = 0;
        *work = 0;
        *work_limit = WORK_LIMIT;
        *hook_depth = 0;
        *step_turns = 0;
        *player_hooks_active = true;
        *escaped = 0;
        *extra_turn = false;
        *dmg_card = NO;
        *dmg_result = Default::default();
        attack_results.clear();
        attack_hit_sizes.clear();
        *draw_depth = 0;
        *draw_nosuspend = 0;
        *hook_shuffle = false;
        *strat_possible = false;
        *replay = None;
        *hook_after = None;
        *attack_unblocked_hits = 0;
        *attack_player_hits = 0;
        autoplay_stack.clear();
        hist_log.clear();
        *decision_seq = 0;
        *deck_enchant_inc = [0; MAX_DECK];
        *deck_upgrade = [0; MAX_DECK];
        *deck_len = 0;
        *play_serial = 0;
        *gold = ex.gold;
        *act = ex.act;
        *end_turn_requested = false;
        *room_type = room_type_of(sc.encounter);
        *cur_power_card = NO;
        *auto_select = false;
        *deck_upgradable = sc.deck.iter().enumerate().take(128).fold(0u128, |m, (i, d)| {
            if d.upgrade < content::card_def(d.id).max_upgrade { m | (1u128 << i) } else { m }
        });
        // PlayerCombatState
        let PlayerState {
            energy,
            max_energy,
            stars,
            turn_number,
            phase,
            hand,
            draw,
            discard,
            exhaust,
            play,
            relics,
            potions,
            potion_slots,
            orbs,
            orb_slots,
            next_orb_uid,
            effect_depth,
        } = player;
        *energy = 0;
        *max_energy = sc.max_energy;
        *stars = 0;
        *turn_number = 1;
        *phase = Phase::None;
        hand.clear();
        draw.clear();
        discard.clear();
        exhaust.clear();
        play.clear();
        relics.clear();
        *potions = [None; MAX_POTIONS];
        *potion_slots = sc.potion_slots;
        orbs.clear();
        *orb_slots = sc.orb_slots; // PlayerCombatState.ResetCombatState: OrbQueue.AddCapacity(BaseOrbSlotCount)
        *next_orb_uid = 1;
        *effect_depth = 0;

        let cx = self;
        // Player creature (CombatId 0).
        cx.creatures[PLAYER as usize] = Creature {
            active: true,
            in_combat: true,
            side: Side::Player,
            is_player: true,
            hp: sc.hp,
            max_hp: sc.max_hp,
            ..Default::default()
        };
        cx.allies.push(PLAYER);
        for r in &sc.relics {
            cx.player.relics.push(Relic { id: r.id, counter: r.counter, flags: r.flags, aux: r.aux });
            cx.listen |= content::relic_mask(r.id);
        }
        if sc.potions.len() > MAX_POTIONS {
            cx.overflow |= ov::SCENARIO; // (unvalidated scenario)
        }
        for (i, p) in sc.potions.iter().enumerate().take(MAX_POTIONS) {
            cx.player.potions[i] = Some(Potion { id: *p });
            cx.listen |= content::potion_mask(*p);
        }
        // Encounter monsters, in creation order (one niche draw each for HP).
        let mut erng = crate::rng::Rng::new(((run_seed as i64).wrapping_add(sc.total_floor as i64) as u64).wrapping_add(encounter_hash(sc.encounter)));
        let Some(spawns) = content::encounter_spawns(sc.encounter, &mut erng, sc.ascension) else {
            cx.overflow |= ov::SCENARIO;
            cx.stage = Stage::Over;
            return Err(ScenarioError::UnimplementedEncounter(crate::ids::encounter::NAMES[sc.encounter as usize]));
        };
        for sp in spawns.iter() {
            // (an unported monster is flagged in `Combat::missing` by `add_enemy` and skipped, so the fight reports UNIMPLEMENTED)
            let Some(c) = cx.add_enemy_v(sp.monster, sp.slot, sp.vars) else {
                if cx.missing.is_none() {
                    cx.overflow |= ov::CREATURES;
                    cx.stage = Stage::Over;
                    return Err(ScenarioError::TooManyEnemies);
                }
                continue;
            };
            cx.creatures[c as usize].monster.vars[0] = sp.vars[0];
            cx.creatures[c as usize].monster.vars[1] = sp.vars[1];
        }
        // PopulateCombatState: clone deck in order, then the initial (unsorted) shuffle.
        for (i, d) in sc.deck.iter().enumerate() {
            let x = ex.deck.get(i).copied().unwrap_or_default();
            let Some(c) = cx.new_card_ex(d.id, d.upgrade, x.enchant, x.enchant_amount) else {
                cx.stage = Stage::Over;
                return Err(ScenarioError::DeckTooLarge);
            };
            cx.cards[c as usize].counter = x.props;
            cx.cards[c as usize].deck_idx = i as u8;
            if i < MAX_DECK {
                cx.deck_upgrade[i] = d.upgrade;
                cx.deck_len = (i + 1) as u8;
            }
            cx.cards[c as usize].pile = PileType::Draw as u8;
            cx.player.draw.push(c);
        }
        cx.initial_shuffle();
        cx.start_combat();
        cx.sync_overflow();
        if cx.overflow & ov::LOOP != 0 {
            cx.stage = Stage::Over; // the combat-start hooks ran away (`Combat::trip_loop`)
        }
        Ok(())
    }

    /// An all-default combat (not a valid fight: `reset_with` makes it one).
    fn blank() -> Combat {
        let player_state = PlayerState {
            energy: 0,
            max_energy: 0,
            stars: 0,
            turn_number: 1,
            phase: Phase::None,
            hand: Pile::new(),
            draw: Pile::new(),
            discard: Pile::new(),
            exhaust: Pile::new(),
            play: Pile::new(),
            relics: ArrayVec::new(),
            potions: [None; MAX_POTIONS],
            potion_slots: 0,
            orbs: ArrayVec::new(),
            orb_slots: 0,
            next_orb_uid: 1,
            effect_depth: 0,
        };
        Combat {
            character: 0,
            ascension: 0,
            rng: RngSet::from_run_seed_fast(0),
            round: 1,
            side: Side::Player,
            in_progress: false,
            is_starting: true,
            pending_loss: false,
            stage: Stage::AwaitAction,
            outcome: Outcome::Ongoing,
            creatures: [Creature::default(); MAX_CREATURES],
            allies: ArrayVec::new(),
            enemies: ArrayVec::new(),
            next_power_uid: 1,
            listen: Mask::EMPTY,
            listen_cards: Mask::EMPTY,
            player: player_state,
            cards: [Card::default(); MAX_CARDS],
            n_cards: 0,
            hist: History::default(),
            play_stack: ArrayVec::new(),
            potion_ctx: None,
            decision: None,
            choice: Choice::default(),
            end_turn_resume: None,
            hook_ctx: None,
            draw_resume: None,
            drawing_hand: false,
            draw_depth: 0,
            draw_nosuspend: 0,
            hook_shuffle: false,
            strat_possible: false,
            replay: None,
            hook_after: None,
            turn_cont: 0,
            susp: ArrayVec::new(),
            draw_pass: None,
            enemy_cont: None,
            missing: None,
            overflow: 0,
            work: 0,
            work_limit: WORK_LIMIT,
            hook_depth: 0,
            step_turns: 0,
            player_hooks_active: true,
            escaped: 0,
            extra_turn: false,
            dmg_card: NO,
            dmg_result: Default::default(),
            attack_results: ArrayVec::new(),
            attack_hit_sizes: ArrayVec::new(),
            attack_unblocked_hits: 0,
            attack_player_hits: 0,
            autoplay_stack: ArrayVec::new(),
            hist_log: Default::default(),
            decision_seq: 0,
            deck_enchant_inc: [0; MAX_DECK],
            deck_upgrade: [0; MAX_DECK],
            deck_len: 0,
            play_serial: 0,
            gold: 0,
            act: 0,
            end_turn_requested: false,
            room_type: 0,
            cur_power_card: NO,
            auto_select: false,
            deck_upgradable: 0,
        }
    }
}

/// 0 monster, 1 elite, 2 boss: the encounter classes' `RoomType` (event encounters default to Monster) follows the id suffix.
fn room_type_of(encounter: u16) -> u8 {
    let n = crate::ids::encounter::NAMES[encounter as usize];
    if n.ends_with("_ELITE") {
        1
    } else if n.ends_with("_BOSS") {
        2
    } else {
        0
    }
}
