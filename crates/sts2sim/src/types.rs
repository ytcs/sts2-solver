//! Small shared enums / flag types mirroring the game's own (values match the C# enums where they are observable).

/// No-object sentinel for `u8` handles (card idx / creature idx / relic idx ...).
pub const NO: u8 = 0xFF;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum Side {
    #[default]
    Player = 0,
    Enemy = 1,
}

impl Side {
    #[inline(always)]
    pub fn opposite(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }
}

/// `PileType` (values match the game's enum).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum PileType {
    #[default]
    None = 0,
    Draw = 1,
    Hand = 2,
    Discard = 3,
    Exhaust = 4,
    Play = 5,
    Deck = 6,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CardPilePosition {
    Bottom = 1,
    Top = 2,
    Random = 3,
}

/// `PlayerTurnPhase`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum Phase {
    #[default]
    None = 0,
    Start = 1,
    AutoPrePlay = 2,
    Play = 3,
    AutoPostPlay = 4,
    End = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum CardType {
    #[default]
    None = 0,
    Attack = 1,
    Skill = 2,
    Power = 3,
    Status = 4,
    Curse = 5,
    Quest = 6,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(u8)]
pub enum CardRarity {
    #[default]
    None = 0,
    Basic = 1,
    Common = 2,
    Uncommon = 3,
    Rare = 4,
    Ancient = 5,
    Event = 6,
    Token = 7,
    Status = 8,
    Curse = 9,
    Quest = 10,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum TargetType {
    #[default]
    None = 0,
    Self_ = 1,
    AnyEnemy = 2,
    AllEnemies = 3,
    RandomEnemy = 4,
    AnyPlayer = 5,
    AnyAlly = 6,
    AllAllies = 7,
    TargetedNoCreature = 8,
    Osty = 9,
}

/// `CardKeyword` as a bitset (bit = enum value - 1).
pub mod kw {
    pub const EXHAUST: u8 = 1 << 0;
    pub const ETHEREAL: u8 = 1 << 1;
    pub const INNATE: u8 = 1 << 2;
    pub const UNPLAYABLE: u8 = 1 << 3;
    pub const RETAIN: u8 = 1 << 4;
    pub const SLY: u8 = 1 << 5;
    pub const ETERNAL: u8 = 1 << 6;
}

/// `CardTag` bitset.
pub mod tag {
    pub const STRIKE: u8 = 1 << 0;
    pub const DEFEND: u8 = 1 << 1;
    pub const MINION: u8 = 1 << 2;
    pub const OSTY_ATTACK: u8 = 1 << 3;
    pub const SHIV: u8 = 1 << 4;
}

/// `ValueProp` flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ValueProp(pub u8);

impl ValueProp {
    pub const NONE: ValueProp = ValueProp(0);
    pub const UNBLOCKABLE: ValueProp = ValueProp(2);
    pub const UNPOWERED: ValueProp = ValueProp(4);
    pub const MOVE: ValueProp = ValueProp(8);
    pub const SKIP_HURT_ANIM: ValueProp = ValueProp(0x10);

    #[inline(always)]
    pub const fn or(self, o: ValueProp) -> ValueProp {
        ValueProp(self.0 | o.0)
    }
    #[inline(always)]
    pub const fn has(self, o: ValueProp) -> bool {
        self.0 & o.0 == o.0
    }
    #[inline(always)]
    pub const fn unblockable(self) -> bool {
        self.0 & 2 != 0
    }
    /// `IsPoweredAttack()` / `IsPoweredCardOrMonsterMoveBlock()`: `Move && !Unpowered`.
    #[inline(always)]
    pub const fn is_powered(self) -> bool {
        self.0 & 8 != 0 && self.0 & 4 == 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Ongoing,
    Victory,
    Defeat,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum PowerType {
    #[default]
    Buff = 0,
    Debuff = 1,
    None = 2,
}

/// Power application stacking key (`PowerModel.InstanceType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum InstanceType {
    /// One instance per power id per creature (stacks amount).
    #[default]
    Single = 0,
    /// Always a new instance.
    Instanced = 1,
    /// Stacks only with an instance of the same applier.
    PerApplier = 2,
}

// ---- engine-core additions -------------------------------------------------------------------------------------

/// `AutoPlayType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum AutoPlayType {
    #[default]
    None = 0,
    Default = 1,
    SlyDiscard = 2,
}

/// `CardLocation` (`ModifyCardPlayResultLocation`): pile (+ position) a played card goes to. `PileType::None` =
/// removed from combat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CardLocation {
    pub pile: PileType,
    pub pos: CardPilePosition,
}

impl CardLocation {
    pub const fn new(pile: PileType, pos: CardPilePosition) -> CardLocation {
        CardLocation { pile, pos }
    }
}
