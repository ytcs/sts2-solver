use crate::hooks::Mask;
use crate::rng::Rng;
use crate::types::*;
use crate::util::{ArrayVec, SmallVec};

pub const MAX_CARDS: usize = 160;
pub const MAX_CREATURES: usize = 12;
pub const MAX_POWERS: usize = 16;
pub const MAX_RELICS: usize = 40;
pub const OBS_RELICS: usize = 24;
pub const MAX_POTIONS: usize = 8;
pub const MAX_HAND: usize = 10;
pub const MAX_ORBS: usize = 10;
pub const MAX_DECK: usize = 80;

pub mod ov {
    pub const CONTAINER: u16 = crate::util::OV_CONTAINER as u16;
    pub const CARDS: u16 = 1 << 1;
    pub const CREATURES: u16 = 1 << 2;
    pub const HISTORY: u16 = 1 << 3;
    pub const COUNTER: u16 = 1 << 4;
    pub const SCENARIO: u16 = 1 << 5;
    pub const LOOP: u16 = 1 << 6;
}

pub const WORK_LIMIT: u32 = 20_000;
pub const HOOK_DEPTH_LIMIT: u16 = 64;
pub const TURN_LIMIT: u16 = 20;

pub type Cid = u8;
pub const PLAYER: Cid = 0;

pub type CardIdx = u8;
pub type Pile = ArrayVec<CardIdx, MAX_CARDS>;

#[derive(Clone, Copy, Default, Debug)]
pub struct Power {
    pub id: u16,
    pub uid: u16,
    pub amount: i32,
    pub amount_on_turn_start: i32,
    pub aux: i32,
    pub applier: u8,
    pub skip_next_tick: bool,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CostMod {
    pub amount: i8,
    bits: u8,
}
impl CostMod {
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
    pub const REMOVED: u8 = 1 << 4;
    pub const X_CAPTURED: u8 = 1 << 5;
    pub const IS_CLONE: u8 = 1 << 6;
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Card {
    pub id: u16,
    pub pile: PileTypeBits,
    pub upgrade: u8,
    pub flags: u8,
    pub kw_add: u8,
    pub kw_remove: u8,
    pub enchant: u8,
    pub enchant_amount: i16,
    pub enchant_status: u8,
    pub enchant_aux: i16,
    pub affliction: u8,
    pub affliction_amount: i16,
    pub base_replay: u8,
    pub cost_base: i8,
    pub x_value: i16,
    pub mods: crate::engine::CostMods,
    pub star_mods: SmallVec<CostMod, 2>,
    pub counter: [i16; 2],
    pub dmg_bonus: i32,
    pub deck_idx: u8,
    pub dupe_of: u8,
    pub dampen_saved: u8,
}
pub type PileTypeBits = u8;

#[derive(Clone, Copy, Default, Debug)]
pub struct Orb {
    pub kind: u16,
    pub uid: u16,
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

#[derive(Clone, Copy, Debug)]
pub struct MonsterState {
    pub id: u16,
    pub cur_state: u8,
    pub next_move: u8,
    pub performed_first: bool,
    pub spawned_this_turn: bool,
    pub is_performing: bool,
    pub log: [u8; 8],
    pub log_len: u16,
    pub ever_logged: u64,
    pub performed_once: u64,
    pub performed: [u8; 4],
    pub stun_follow_up: u8,
    pub stunned: bool,
    pub stun_performed: bool,
    pub stun_move: Option<crate::defs::MoveFn>,
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
    pub is_pet: bool,
    pub in_combat: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub owner: Cid,
    pub slot: u8,
    pub powers: ArrayVec<Power, MAX_POWERS>,
    pub secondary: bool,
    pub monster: MonsterState,
    pub pristine: u8,
}

pub const PRISTINE_HP: u8 = 1;
pub const PRISTINE_BLOCK: u8 = 2;

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
            pristine: 0,
        }
    }
}

impl Creature {
    #[inline(always)]
    pub fn hp(&self) -> i32 {
        if self.pristine & PRISTINE_HP != 0 {
            crate::engine::look_dep();
        }
        self.hp
    }
    #[inline(always)]
    pub fn set_hp(&mut self, v: i32) {
        self.pristine &= !PRISTINE_HP;
        self.hp = v;
    }
    #[inline(always)]
    pub fn block(&self) -> i32 {
        if self.pristine & PRISTINE_BLOCK != 0 {
            crate::engine::look_dep();
        }
        self.block
    }
    #[inline(always)]
    pub fn set_block(&mut self, v: i32) {
        self.pristine &= !PRISTINE_BLOCK;
        self.block = v;
    }
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

#[derive(Clone, Copy)]
pub struct PlayerState {
    pub energy: i32,
    pub max_energy: i32,
    pub stars: i32,
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
    pub orbs: ArrayVec<Orb, MAX_ORBS>,
    pub orb_slots: u8,
    pub next_orb_uid: u16,
    pub effect_depth: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    AwaitAction,
    AwaitChoice,
    Over,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecisionSource {
    Hand,
    Pile(PileType),
    Options,
}

#[derive(Clone, Copy)]
pub struct Decision {
    pub source: DecisionSource,
    pub min: u8,
    pub max: u8,
    pub cands: ArrayVec<CardIdx, MAX_CARDS>,
    pub selected: ArrayVec<u8, 16>,
    pub confirm_required: bool,
    pub can_skip: bool,
    pub purpose: u16,
}

pub mod purpose {
    pub const POTION: u16 = 0x8000;
    pub const RELIC: u16 = 0x4000;
    pub const MONSTER: u16 = 0x2000;
    pub const ID: u16 = 0x1FFF;
    const _: () = assert!(crate::ids::card::COUNT <= ID as usize && crate::ids::potion::COUNT <= ID as usize);
    const _: () = assert!(crate::ids::relic::COUNT <= ID as usize && crate::ids::monster::COUNT <= ID as usize);
    pub const fn potion(id: u16) -> u16 {
        POTION | id
    }
    pub const fn relic(id: u16) -> u16 {
        RELIC | id
    }
    pub const fn monster(id: u16) -> u16 {
        MONSTER | id
    }
}

#[derive(Clone)]
pub struct Replay {
    pub s0: Combat,
    pub action: crate::engine::Action,
    pub answers: ArrayVec<ArrayVec<u8, 16>, 6>,
    pub pos: u8,
    pub at_prompt: bool,
    pub done: bool,
    pub capture: Option<Box<Combat>>,
}

#[derive(Clone, Copy, Default)]
pub struct Choice {
    pub cards: ArrayVec<CardIdx, 16>,
}

#[derive(Clone, Copy)]
pub struct SuspPass {
    pub bit: u32,
    pub me: crate::hooks::Me,
    pub pos: u8,
    pub full: bool,
    pub rest: ArrayVec<crate::hooks::Me, 8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayStep {
    Before,
    OnPlay(u8),
    After,
}

pub const AUTOPLAY_MAX: usize = 24;

#[derive(Clone, Copy)]
pub struct AutoQueue {
    pub cards: ArrayVec<CardIdx, AUTOPLAY_MAX>,
    pub force_exhaust: bool,
    pub sly: bool,
    pub plain: bool,
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
    pub result: CardLocation,
}

#[derive(Clone, Copy, Default)]
pub struct History {
    pub cards_played_this_turn: i16,
    pub attacks_played_this_turn: i16,
    pub skills_played_this_turn: i16,
    pub cards_exhausted_this_turn: i16,
    pub cards_finished_this_turn: i16,
    pub attacks_finished_this_turn: i16,
    pub skills_finished_this_turn: i16,
    pub shivs_finished_this_turn: i16,
    pub finished_cards: [u64; 3],
    pub play_amounts: ArrayVec<PlayAmount, 32>,
}

impl History {
    pub fn finished(&self, c: CardIdx) -> bool {
        self.finished_cards[(c / 64) as usize] >> (c % 64) & 1 != 0
    }
    pub fn set_finished(&mut self, c: CardIdx) {
        self.finished_cards[(c / 64) as usize] |= 1u64 << (c % 64);
    }
    pub fn remember_play(&mut self, uid: u16, card: CardIdx, amount: i32) {
        if self.play_amounts.len() < 32 {
            self.play_amounts.push(PlayAmount { uid, card, amount });
        } else {
            crate::util::raise_overflow(crate::util::OV_CONTAINER);
        }
    }
    pub fn take_play(&mut self, uid: u16, card: CardIdx) -> Option<i32> {
        let pos = self.play_amounts.as_slice().iter().rposition(|e| e.uid == uid && e.card == card)?;
        Some(self.play_amounts.remove(pos).amount)
    }
    pub fn play_entry(&mut self, uid: u16, card: CardIdx) -> Option<&mut i32> {
        self.play_amounts.as_mut_slice().iter_mut().rev().find(|e| e.uid == uid && e.card == card).map(|e| &mut e.amount)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct PlayAmount {
    pub uid: u16,
    pub card: CardIdx,
    pub amount: i32,
}

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
    pub listen: Mask,
    pub listen_cards: Mask,

    pub player: PlayerState,
    pub cards: [Card; MAX_CARDS],
    pub n_cards: u16,
    pub hist: History,

    pub play_stack: ArrayVec<PlayCtx, 6>,
    pub potion_ctx: Option<PotionCtx>,
    pub decision: Option<Decision>,
    pub choice: Choice,
    pub hook_ctx: Option<(crate::hooks::Me, u8)>,
    pub draw_resume: Option<(i32, bool)>,
    pub drawing_hand: bool,
    pub draw_depth: u8,
    pub draw_nosuspend: u8,
    pub hook_shuffle: bool,
    pub strat_possible: bool,
    pub replay: Option<Box<Replay>>,
    pub hook_after: Option<(crate::hooks::Me, u8)>,
    pub turn_cont: u8,
    pub susp: ArrayVec<SuspPass, 3>,
    pub draw_pass: Option<(CardIdx, u8)>,
    pub enemy_cont: Option<(ArrayVec<Cid, MAX_CREATURES>, u8, u8)>,
    pub end_turn_resume: Option<crate::hooks::Me>,
    pub missing: Option<(crate::hooks::Kind, u16)>,
    pub overflow: u16,
    pub work: u32,
    pub work_limit: u32,
    pub hook_depth: u16,
    pub step_turns: u16,

    pub player_hooks_active: bool,
    pub escaped: u8,
    pub extra_turn: bool,
    pub dmg_card: CardIdx,
    pub dmg_result: crate::engine::DamageResult,
    pub attack_results: ArrayVec<crate::engine::DamageResult, 16>,
    pub attack_hit_sizes: ArrayVec<u8, 16>,
    pub attack_unblocked_hits: u8,
    pub attack_player_hits: u8,
    pub autoplay_stack: ArrayVec<AutoQueue, 4>,
    pub hist_log: crate::engine::HistLog,
    pub decision_seq: u32,
    pub deck_enchant_inc: [u8; MAX_DECK],
    pub deck_upgrade: [u8; MAX_DECK],
    pub deck_len: u8,
    pub play_serial: u16,
    pub gold: i32,
    pub act: u8,
    pub end_turn_requested: bool,
    pub room_type: u8,
    pub deck_upgradable: u128,
    pub cur_power_card: CardIdx,
    pub auto_select: bool,
}

impl Creature {
    #[inline]
    pub fn copy_from(&mut self, src: &Creature) {
        let Creature { active, side, is_player, is_pet, in_combat, hp, max_hp, block, owner, slot, powers, secondary, monster, pristine } = self;
        *active = src.active;
        *side = src.side;
        *is_player = src.is_player;
        *is_pet = src.is_pet;
        *in_combat = src.in_combat;
        *hp = src.hp;
        *max_hp = src.max_hp;
        *block = src.block;
        *owner = src.owner;
        *slot = src.slot;
        powers.copy_from(&src.powers);
        *secondary = src.secondary;
        *monster = src.monster;
        *pristine = src.pristine;
    }
}

impl PlayerState {
    #[inline]
    pub fn copy_from(&mut self, src: &PlayerState) {
        let PlayerState { energy, max_energy, stars, turn_number, phase, hand, draw, discard, exhaust, play, relics, potions, potion_slots, orbs, orb_slots, next_orb_uid, effect_depth } = self;
        *energy = src.energy;
        *max_energy = src.max_energy;
        *stars = src.stars;
        *turn_number = src.turn_number;
        *phase = src.phase;
        hand.copy_from(&src.hand);
        draw.copy_from(&src.draw);
        discard.copy_from(&src.discard);
        exhaust.copy_from(&src.exhaust);
        play.copy_from(&src.play);
        relics.copy_from(&src.relics);
        *potions = src.potions;
        *potion_slots = src.potion_slots;
        orbs.copy_from(&src.orbs);
        *orb_slots = src.orb_slots;
        *next_orb_uid = src.next_orb_uid;
        *effect_depth = src.effect_depth;
    }
}

impl Clone for Combat {
    fn clone(&self) -> Combat {
        // SAFETY: `replay` is the only owning field; the bitwise copy shares its box, which is overwritten without being dropped.
        unsafe {
            let mut c = core::mem::MaybeUninit::<Combat>::uninit();
            core::ptr::copy_nonoverlapping(self as *const Combat, c.as_mut_ptr(), 1);
            core::ptr::write(core::ptr::addr_of_mut!((*c.as_mut_ptr()).replay), self.replay.clone());
            c.assume_init()
        }
    }

    // Copies only readable state (cards[..n_cards], live list entries, written history entries): stale slots are never read.
    fn clone_from(&mut self, src: &Combat) {
        let Combat {
            character,
            ascension,
            rng,
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
            cards,
            n_cards,
            hist,
            play_stack,
            potion_ctx,
            decision,
            choice,
            hook_ctx,
            draw_resume,
            drawing_hand,
            draw_depth,
            draw_nosuspend,
            hook_shuffle,
            strat_possible,
            replay,
            hook_after,
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
        *character = src.character;
        *ascension = src.ascension;
        *rng = src.rng;
        *round = src.round;
        *side = src.side;
        *in_progress = src.in_progress;
        *is_starting = src.is_starting;
        *pending_loss = src.pending_loss;
        *stage = src.stage;
        *outcome = src.outcome;
        for (d, s) in creatures.iter_mut().zip(src.creatures.iter()) {
            d.copy_from(s);
        }
        allies.copy_from(&src.allies);
        enemies.copy_from(&src.enemies);
        *next_power_uid = src.next_power_uid;
        *listen = src.listen;
        *listen_cards = src.listen_cards;
        player.copy_from(&src.player);
        let n = src.n_cards as usize;
        cards[..n].copy_from_slice(&src.cards[..n]);
        *n_cards = src.n_cards;
        *hist = src.hist;
        play_stack.copy_from(&src.play_stack);
        *potion_ctx = src.potion_ctx;
        match (decision.as_mut(), src.decision.as_ref()) {
            (Some(d), Some(s)) => {
                let Decision { source, min, max, cands, selected, confirm_required, can_skip, purpose } = d;
                *source = s.source;
                *min = s.min;
                *max = s.max;
                cands.copy_from(&s.cands);
                selected.copy_from(&s.selected);
                *confirm_required = s.confirm_required;
                *can_skip = s.can_skip;
                *purpose = s.purpose;
            }
            _ => *decision = src.decision,
        }
        choice.cards.copy_from(&src.choice.cards);
        *hook_ctx = src.hook_ctx;
        *draw_resume = src.draw_resume;
        *drawing_hand = src.drawing_hand;
        *draw_depth = src.draw_depth;
        *draw_nosuspend = src.draw_nosuspend;
        *hook_shuffle = src.hook_shuffle;
        *strat_possible = src.strat_possible;
        replay.clone_from(&src.replay);
        *hook_after = src.hook_after;
        *turn_cont = src.turn_cont;
        *susp = src.susp;
        *draw_pass = src.draw_pass;
        *enemy_cont = src.enemy_cont;
        *end_turn_resume = src.end_turn_resume;
        *missing = src.missing;
        *overflow = src.overflow;
        *work = src.work;
        *work_limit = src.work_limit;
        *hook_depth = src.hook_depth;
        *step_turns = src.step_turns;
        *player_hooks_active = src.player_hooks_active;
        *escaped = src.escaped;
        *extra_turn = src.extra_turn;
        *dmg_card = src.dmg_card;
        *dmg_result = src.dmg_result;
        attack_results.copy_from(&src.attack_results);
        attack_hit_sizes.copy_from(&src.attack_hit_sizes);
        *attack_unblocked_hits = src.attack_unblocked_hits;
        *attack_player_hits = src.attack_player_hits;
        autoplay_stack.copy_from(&src.autoplay_stack);
        hist_log.copy_from(&src.hist_log);
        *decision_seq = src.decision_seq;
        *deck_enchant_inc = src.deck_enchant_inc;
        *deck_upgrade = src.deck_upgrade;
        *deck_len = src.deck_len;
        *play_serial = src.play_serial;
        *gold = src.gold;
        *act = src.act;
        *end_turn_requested = src.end_turn_requested;
        *room_type = src.room_type;
        *deck_upgradable = src.deck_upgradable;
        *cur_power_card = src.cur_power_card;
        *auto_select = src.auto_select;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Counter,
    Aux,
    Flag(u8),
}

#[derive(Clone, Copy, Debug)]
pub struct PropDef {
    pub name: &'static str,
    pub slot: Slot,
    pub boolean: bool,
    pub skip_default: bool,
    pub lit: &'static str,
}

impl PropDef {
    pub const fn int(name: &'static str, slot: Slot) -> PropDef {
        PropDef { name, slot, boolean: false, skip_default: false, lit: "" }
    }
    pub const fn flag(name: &'static str, bit: u8) -> PropDef {
        PropDef { name, slot: Slot::Flag(bit), boolean: true, skip_default: false, lit: "" }
    }
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
    pub fn set_prop(&mut self, defs: &[PropDef], name: &str, v: i32) -> bool {
        let Some(d) = defs.iter().find(|d| d.name == name) else { return false };
        if d.lit.is_empty() {
            self.set(d.slot, v);
        }
        true
    }
}
