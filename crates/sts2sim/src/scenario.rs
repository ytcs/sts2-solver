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

#[derive(Clone, Copy, Debug)]
pub struct RelicInit {
    pub id: u16,
    pub counter: i32,
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
    pub fn new(sc: &Scenario) -> Combat {
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
            orb_slots: sc.orb_slots,
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
            player: player_state,
            cards: [Card::default(); MAX_CARDS],
            n_cards: 0,
            hist: History::default(),
            play_ctx: None,
            play_stack: Default::default(),
            play_base: 0,
            potion_ctx: None,
            decision: None,
            choice: Choice::default(),
            missing: None,
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
            cx.player.relics.push(Relic { id: r.id, counter: r.counter, ..Default::default() });
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
            let c = cx.add_enemy(sp.monster, sp.slot).expect("too many enemies");
            cx.creatures[c as usize].monster.vars[0] = sp.vars[0];
            cx.creatures[c as usize].monster.vars[1] = sp.vars[1];
        }
        // PopulateCombatState: clone deck in order, then the initial (unsorted) shuffle.
        for (i, d) in sc.deck.iter().enumerate() {
            let c = cx.new_card(d.id, d.upgrade).expect("card arena");
            cx.cards[c as usize].deck_idx = i as u8;
            cx.cards[c as usize].pile = PileType::Draw as u8;
            cx.player.draw.push(c);
        }
        cx.initial_shuffle();
        cx.start_combat();
        cx
    }
}
