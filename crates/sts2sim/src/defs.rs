//! Static (immutable) definitions: what the game calls the *canonical* models. Per-instance mutable state lives in
//! `state.rs`. Card stat tables are meant to be machine-generated from the decompiled source; behaviour lives in
//! `content/`.

use crate::state::Combat;
use crate::types::*;

/// Kind of a card's dynamic variable (`DynamicVar` subclasses that gameplay reads).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum VarKind {
    Damage,
    Block,
    Cards,
    Energy,
    Stars,
    Repeat,
    HpLoss,
    Heal,
    Summon,
    Forge,
    OstyDamage,
    ExtraDamage,
    CalcBase,
    CalcExtra,
    Gold,
    MaxHp,
    /// `PowerVar<T>`; `arg` = power id.
    Power,
    /// Generic named var (`DynamicVar("Name", v)`); `arg` = `gen_cards::var_name::*`.
    Named,
    /// `CalculatedDamageVar` / `CalculatedBlockVar` (value computed by the card's own logic).
    CalcDamage,
    CalcBlock,
}

#[derive(Clone, Copy, Debug)]
pub struct VarDef {
    pub kind: VarKind,
    /// Power id for `Power`, slot for `Int`, 0 otherwise.
    pub arg: u16,
    pub base: i16,
    /// Delta applied per upgrade level (`UpgradeValueBy`).
    pub up: i16,
    /// `ValueProp` flags of damage/block vars.
    pub props: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct CardDef {
    pub id: u16,
    /// Canonical energy cost (-1 = no cost: curses/statuses).
    pub cost: i8,
    pub x_cost: bool,
    /// Canonical star cost (-1 = none).
    pub star_cost: i8,
    pub ctype: CardType,
    pub rarity: CardRarity,
    pub target: TargetType,
    pub keywords: u8,
    pub tags: u8,
    pub vars: &'static [VarDef],
    pub max_upgrade: u8,
    /// Energy cost delta per upgrade (`EnergyCost.UpgradeBy`).
    pub up_cost: i8,
    pub up_add_kw: u8,
    pub up_remove_kw: u8,
    /// `HasTurnEndInHandEffect`.
    pub turn_end_in_hand: bool,
    /// Star cost delta per upgrade.
    pub up_star_cost: i8,
    /// `OnUpgrade` does more than the mechanical edits above: the card's logic handles the rest.
    pub custom_upgrade: bool,
    /// `CanBeGeneratedInCombat` (false for cards excluded from random generation).
    pub can_be_generated_in_combat: bool,
    /// `MultiplayerConstraint == MultiplayerOnly` (excluded in single player).
    pub multiplayer_only: bool,
}

impl CardDef {
    pub const fn new(id: u16, cost: i8, ctype: CardType, rarity: CardRarity, target: TargetType) -> CardDef {
        CardDef {
            id,
            cost,
            x_cost: false,
            star_cost: -1,
            ctype,
            rarity,
            target,
            keywords: 0,
            tags: 0,
            vars: &[],
            max_upgrade: 1,
            up_cost: 0,
            up_add_kw: 0,
            up_remove_kw: 0,
            turn_end_in_hand: false,
            up_star_cost: 0,
            custom_upgrade: false,
            can_be_generated_in_combat: true,
            multiplayer_only: false,
        }
    }
    pub const fn x_cost(mut self) -> CardDef {
        self.x_cost = true;
        self
    }
    pub const fn star_cost(mut self, c: i8) -> CardDef {
        self.star_cost = c;
        self
    }
    pub const fn max_upgrade(mut self, n: u8) -> CardDef {
        self.max_upgrade = n;
        self
    }
    pub const fn up_cost(mut self, c: i8) -> CardDef {
        self.up_cost = c;
        self
    }
    pub const fn up_star_cost(mut self, c: i8) -> CardDef {
        self.up_star_cost = c;
        self
    }
    pub const fn up_add_kw(mut self, k: u8) -> CardDef {
        self.up_add_kw = k;
        self
    }
    pub const fn up_remove_kw(mut self, k: u8) -> CardDef {
        self.up_remove_kw = k;
        self
    }
    pub const fn turn_end_in_hand(mut self) -> CardDef {
        self.turn_end_in_hand = true;
        self
    }
    pub const fn not_generated_in_combat(mut self) -> CardDef {
        self.can_be_generated_in_combat = false;
        self
    }
    pub const fn multiplayer_only(mut self) -> CardDef {
        self.multiplayer_only = true;
        self
    }
    pub const fn custom_upgrade(mut self) -> CardDef {
        self.custom_upgrade = true;
        self
    }
    pub const fn vars(mut self, v: &'static [VarDef]) -> CardDef {
        self.vars = v;
        self
    }
    pub const fn kw(mut self, k: u8) -> CardDef {
        self.keywords = k;
        self
    }
    pub const fn tags(mut self, t: u8) -> CardDef {
        self.tags = t;
        self
    }
}

pub const fn var(kind: VarKind, base: i16, up: i16) -> VarDef {
    VarDef { kind, arg: 0, base, up, props: 8 }
}
pub const fn var_p(kind: VarKind, base: i16, up: i16, props: u8) -> VarDef {
    VarDef { kind, arg: 0, base, up, props }
}
pub const fn power_var(power: u16, base: i16, up: i16) -> VarDef {
    VarDef { kind: VarKind::Power, arg: power, base, up, props: 0 }
}
pub const fn named_var(name: u16, base: i16, up: i16) -> VarDef {
    VarDef { kind: VarKind::Named, arg: name, base, up, props: 0 }
}

#[derive(Clone, Copy, Debug)]
pub struct PowerDef {
    pub ptype: PowerType,
    pub instance: InstanceType,
    /// `AllowNegative` (Strength / Dexterity / Focus ...).
    pub allow_negative: bool,
    /// `OwnerIsSecondaryEnemy` (Minion, Illusion).
    pub secondary_enemy: bool,
    /// `StackType == Counter` (only affects `GetTypeForAmount`).
    pub counter: bool,
    /// `IsVisibleInternal` (Artifact only blocks visible debuffs).
    pub visible: bool,
}

impl PowerDef {
    pub const fn new(ptype: PowerType) -> PowerDef {
        PowerDef { ptype, instance: InstanceType::Single, allow_negative: false, secondary_enemy: false, counter: true, visible: true }
    }
    pub const fn allow_negative(mut self) -> PowerDef {
        self.allow_negative = true;
        self
    }
    pub const fn single(mut self) -> PowerDef {
        self.counter = false;
        self
    }
    pub const fn per_applier(mut self) -> PowerDef {
        self.instance = InstanceType::PerApplier;
        self
    }
    pub const fn secondary_enemy(mut self) -> PowerDef {
        self.secondary_enemy = true;
        self
    }
    pub const fn hidden(mut self) -> PowerDef {
        self.visible = false;
        self
    }
    pub const fn instanced(mut self) -> PowerDef {
        self.instance = InstanceType::Instanced;
        self
    }
}

/// `MonsterMoves` intents (what the player can observe; attack numbers are computed live).
#[derive(Clone, Copy)]
pub enum Intent {
    Attack { damage: fn(&Combat, u8) -> i32, hits: fn(&Combat, u8) -> i32 },
    Buff,
    Debuff,
    DebuffStrong,
    Defend,
    Escape,
    Heal,
    Hidden,
    Summon,
    Sleep,
    Stun,
    StatusCard,
    CardDebuff,
    DeathBlow,
}

/// Performs a monster move: `fn(cx, monster_creature)`.
pub type MoveFn = fn(&mut Combat, u8);
/// Evaluated at roll time (conditional branch predicate / weight lambda).
pub type CondFn = fn(&Combat, u8) -> bool;
pub type WeightFn = fn(&Combat, u8) -> f32;

#[derive(Clone, Copy)]
pub enum Repeat {
    CanRepeatForever,
    /// `CannotRepeat` ≡ `CanRepeatXTimes(1)`.
    CanRepeatXTimes(u8),
    UseOnlyOnce,
}

#[derive(Clone, Copy)]
pub struct Branch {
    pub target: u8,
    pub repeat: Repeat,
    /// Cooldown in moves (0 = none).
    pub cooldown: u8,
    pub weight: f32,
    pub weight_fn: Option<WeightFn>,
}

impl Branch {
    pub const fn new(target: u8) -> Branch {
        Branch { target, repeat: Repeat::CanRepeatForever, cooldown: 0, weight: 1.0, weight_fn: None }
    }
    pub const fn cannot_repeat(mut self) -> Branch {
        self.repeat = Repeat::CanRepeatXTimes(1);
        self
    }
    pub const fn max_repeats(mut self, n: u8) -> Branch {
        self.repeat = Repeat::CanRepeatXTimes(n);
        self
    }
    pub const fn once(mut self) -> Branch {
        self.repeat = Repeat::UseOnlyOnce;
        self
    }
    pub const fn cooldown(mut self, n: u8) -> Branch {
        self.cooldown = n;
        self
    }
    pub const fn weight(mut self, w: f32) -> Branch {
        self.weight = w;
        self
    }
}

/// One node of a monster's move state machine.
#[derive(Clone, Copy)]
pub enum MonsterNode {
    Move {
        id: &'static str,
        perform: MoveFn,
        intents: &'static [Intent],
        /// Unconditional successor (`FollowUpState`), `NO` if none.
        follow_up: u8,
        must_perform_once: bool,
    },
    Random { id: &'static str, branches: &'static [Branch] },
    Cond { id: &'static str, arms: &'static [(u8, CondFn)] },
}

#[derive(Clone, Copy)]
pub struct MonsterDef {
    pub id: u16,
    /// `(MinInitialHp, MaxInitialHp)` for the given ascension.
    pub hp: fn(u8) -> (i32, i32),
    pub nodes: &'static [MonsterNode],
    pub initial: u8,
    /// `AfterAddedToRoom` hook (spawn powers etc.).
    pub on_spawn: Option<fn(&mut Combat, u8)>,
}

/// `AscensionLevel` thresholds used by combat content (`AscensionHelper.GetValueIfAscension`).
pub mod asc {
    pub const TOUGH_ENEMIES: u8 = 8;
    pub const DEADLY_ENEMIES: u8 = 9;
    #[inline(always)]
    pub fn at(level: u8, ascension: u8) -> bool {
        ascension >= level
    }
    /// `GetValueIfAscension(level, ascended, base)`.
    #[inline(always)]
    pub fn val<T>(level: u8, ascension: u8, ascended: T, base: T) -> T {
        if ascension >= level { ascended } else { base }
    }
}

/// `PotionUsage`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PotionUsage {
    None = 0,
    CombatOnly = 1,
    AnyTime = 2,
    /// Triggers by itself (Fairy in a Bottle); can never be used manually.
    Automatic = 3,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum PotionRarity {
    None = 0,
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Event = 4,
    Token = 5,
}

#[derive(Clone, Copy, Debug)]
pub struct PotionDef {
    pub id: u16,
    pub rarity: PotionRarity,
    pub usage: PotionUsage,
    pub target: TargetType,
    pub vars: &'static [VarDef],
    pub can_be_generated_in_combat: bool,
}

impl PotionDef {
    pub const fn new(id: u16, rarity: PotionRarity, usage: PotionUsage, target: TargetType) -> PotionDef {
        PotionDef { id, rarity, usage, target, vars: &[], can_be_generated_in_combat: true }
    }
    pub const fn vars(mut self, v: &'static [VarDef]) -> PotionDef {
        self.vars = v;
        self
    }
    pub const fn not_generated_in_combat(mut self) -> PotionDef {
        self.can_be_generated_in_combat = false;
        self
    }
}

/// `MonsterNode::Move::follow_up` value meaning "the successor stored at runtime in `MonsterState::stun_follow_up`"
/// (`MoveState.FollowUpStateId` of the dynamically created STUNNED / REVIVE_MOVE states, see engine/lifecycle.rs).
pub const FOLLOW_STORED: u8 = 0xFD;
