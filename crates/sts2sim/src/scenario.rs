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
    DeckTooLarge,
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
        if self.deck.len() > MAX_CARDS / 2 {
            return Err(ScenarioError::DeckTooLarge);
        }
        Ok(())
    }
}

impl Combat {
    /// Builds the combat and runs it up to the first player decision (spec 01 §3-4).
    #[inline(always)]
    pub fn new(sc: &Scenario) -> Combat {
        Self::new_with(sc, &ScenarioExtras::default())
    }

    /// `new` plus the optional inputs (deck card enchantments / saved properties).
    pub fn new_with(sc: &Scenario, ex: &ScenarioExtras) -> Combat {
        sc.validate().expect("invalid scenario");
        let player_state = PlayerState {
            energy: 0,
            max_energy: sc.max_energy,
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
            potion_slots: sc.potion_slots,
            orbs: ArrayVec::new(),
            orb_slots: sc.orb_slots, // PlayerCombatState.ResetCombatState: OrbQueue.AddCapacity(BaseOrbSlotCount)
            next_orb_uid: 1,
            effect_depth: 0,
        };
        let mut cx = Combat {
            character: sc.character,
            ascension: sc.ascension,
            rng: sc.rng,
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
            turn_cont: 0,
            susp_after: None,
            enemy_cont: None,
            missing: None,
            player_hooks_active: true,
            escaped: 0,
            extra_turn: false,
            dmg_card: NO,
            dmg_result: Default::default(),
            attack_results: ArrayVec::new(),
            attack_unblocked_hits: 0,
            attack_player_hits: 0,
            autoplay_stack: ArrayVec::new(),
            hist_log: Default::default(),
            decision_seq: 0,
            deck_enchant_inc: [0; 80],
            deck_upgrade: [0; 80],
            deck_len: 0,
            play_serial: 0,
            gold: ex.gold,
            act: ex.act,
            end_turn_requested: false,
            room_type: room_type_of(sc.encounter),
            cur_power_card: NO,
            auto_select: false,
            deck_upgradable: sc.deck.iter().enumerate().take(128).fold(0u128, |m, (i, d)| {
                if d.upgrade < content::card_def(d.id).max_upgrade { m | (1u128 << i) } else { m }
            }),
        };
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
        for (i, p) in sc.potions.iter().enumerate().take(MAX_POTIONS) {
            cx.player.potions[i] = Some(Potion { id: *p });
            cx.listen |= content::potion_mask(*p);
        }
        // Encounter monsters, in creation order (one niche draw each for HP).
        let mut erng = crate::rng::Rng::named((sc.run_seed as i64).wrapping_add(sc.total_floor as i64) as u64, crate::ids::encounter::NAMES[sc.encounter as usize]);
        let spawns = content::encounter_spawns(sc.encounter, &mut erng, sc.ascension).unwrap();
        for sp in spawns.iter() {
            // (an unported monster is flagged in `Combat::missing` by `add_enemy` and skipped, so the fight reports UNIMPLEMENTED)
            let Some(c) = cx.add_enemy_v(sp.monster, sp.slot, sp.vars) else {
                assert!(cx.missing.is_some(), "too many enemies");
                continue;
            };
            cx.creatures[c as usize].monster.vars[0] = sp.vars[0];
            cx.creatures[c as usize].monster.vars[1] = sp.vars[1];
        }
        // PopulateCombatState: clone deck in order, then the initial (unsorted) shuffle.
        for (i, d) in sc.deck.iter().enumerate() {
            let x = ex.deck.get(i).copied().unwrap_or_default();
            let c = cx.new_card_ex(d.id, d.upgrade, x.enchant, x.enchant_amount).expect("card arena");
            cx.cards[c as usize].counter = x.props;
            cx.cards[c as usize].deck_idx = i as u8;
            if i < 80 {
                cx.deck_upgrade[i] = d.upgrade;
                cx.deck_len = (i + 1) as u8;
            }
            cx.cards[c as usize].pile = PileType::Draw as u8;
            cx.player.draw.push(c);
        }
        cx.initial_shuffle();
        cx.start_combat();
        cx
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
