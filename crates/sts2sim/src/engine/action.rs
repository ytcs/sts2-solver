//! The agent's action space and legal-action enumeration. Mirrors what a human can do at each moment.

use crate::defs::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

/// An agent action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Play the hand card at `hand_pos`; `target` is a creature id for single-target cards, `NO` otherwise.
    PlayCard { hand_pos: u8, target: Cid },
    /// Use the potion in `slot`; `target` is an enemy for enemy-targeted potions, `NO` otherwise (self/AoE).
    UsePotion { slot: u8, target: Cid },
    DiscardPotion { slot: u8 },
    EndTurn,
    /// Decision UI: click candidate `idx` (toggles; at `max` the latest selection is replaced).
    Pick { idx: u8 },
    /// Decision UI: confirm the selection (or skip when nothing is selected and skipping is allowed).
    Confirm,
}

const T: usize = MAX_CREATURES + 1; // target slots: creature id 0..MAX_CREATURES, plus "no target"
pub const MAX_PICK: usize = 64;
const OFF_PLAY: usize = 1;
const OFF_POTION: usize = OFF_PLAY + MAX_HAND * T;
const OFF_DISCARD: usize = OFF_POTION + MAX_POTIONS * T;
const OFF_PICK: usize = OFF_DISCARD + MAX_POTIONS;
const OFF_CONFIRM: usize = OFF_PICK + MAX_PICK;
/// Size of the dense, fixed action space: `EndTurn`, `PlayCard`, `UsePotion`, `DiscardPotion`, `Pick`, `Confirm`.
pub const ACTION_SPACE: usize = OFF_CONFIRM + 1;

pub type ActionBuf = ArrayVec<Action, 256>;

impl Action {
    /// Dense index in `0..ACTION_SPACE` (for policy heads / masks).
    pub fn index(self) -> usize {
        let t = |t: Cid| if t == NO { MAX_CREATURES } else { t as usize };
        match self {
            Action::EndTurn => 0,
            Action::PlayCard { hand_pos, target } => OFF_PLAY + hand_pos as usize * T + t(target),
            Action::UsePotion { slot, target } => OFF_POTION + slot as usize * T + t(target),
            Action::DiscardPotion { slot } => OFF_DISCARD + slot as usize,
            Action::Pick { idx } => OFF_PICK + idx as usize,
            Action::Confirm => OFF_CONFIRM,
        }
    }

    pub fn from_index(i: usize) -> Option<Action> {
        let t = |x: usize| if x == MAX_CREATURES { NO } else { x as Cid };
        if i == 0 {
            Some(Action::EndTurn)
        } else if i < OFF_POTION {
            let k = i - OFF_PLAY;
            Some(Action::PlayCard { hand_pos: (k / T) as u8, target: t(k % T) })
        } else if i < OFF_DISCARD {
            let k = i - OFF_POTION;
            Some(Action::UsePotion { slot: (k / T) as u8, target: t(k % T) })
        } else if i < OFF_PICK {
            Some(Action::DiscardPotion { slot: (i - OFF_DISCARD) as u8 })
        } else if i < OFF_CONFIRM {
            Some(Action::Pick { idx: (i - OFF_PICK) as u8 })
        } else if i == OFF_CONFIRM {
            Some(Action::Confirm)
        } else {
            None
        }
    }
}

impl Combat {
    /// Every action a human could take right now.
    pub fn legal_actions(&self, out: &mut ActionBuf) {
        let mut playable = 0;
        self.legal_actions_ex(out, &mut playable);
    }

    /// `legal_actions` that also reports which hand cards can be played: bit `k` of `hand_playable` = `can_play(hand[k])` (0 outside
    /// the play phase). Feed it to `observe_ex` so the observation does not evaluate `can_play` a second time.
    pub fn legal_actions_ex(&self, out: &mut ActionBuf, hand_playable: &mut u16) {
        *hand_playable = 0;
        out.clear();
        match self.stage {
            Stage::Over => {}
            Stage::AwaitChoice => {
                if let Some(d) = &self.decision {
                    if d.cands.len() > MAX_PICK {
                        // More candidates than the dense action space can address: the surplus cannot be picked.
                        crate::util::raise_overflow(crate::util::OV_CONTAINER);
                    }
                    for i in 0..d.cands.len().min(MAX_PICK) {
                        out.push(Action::Pick { idx: i as u8 });
                    }
                    let n = d.selected.len();
                    if (d.confirm_required && n >= d.min as usize && n <= d.max as usize) || (d.can_skip && n == 0) {
                        out.push(Action::Confirm);
                    }
                }
            }
            Stage::AwaitAction => {
                if self.player.phase != Phase::Play {
                    return;
                }
                for (pos, &c) in self.player.hand.iter().enumerate() {
                    if !self.can_play(c) {
                        continue;
                    }
                    *hand_playable |= 1 << pos;
                    if self.card_target_type(c) == TargetType::AnyEnemy {
                        for &e in self.enemies.iter() {
                            if self.is_valid_target(c, e) {
                                out.push(Action::PlayCard { hand_pos: pos as u8, target: e });
                            }
                        }
                    } else {
                        out.push(Action::PlayCard { hand_pos: pos as u8, target: NO });
                    }
                }
                for slot in 0..MAX_POTIONS {
                    let Some(p) = self.player.potions[slot] else { continue };
                    let d = crate::content::potion_def(p.id);
                    if d.usage != PotionUsage::Automatic && d.usage != PotionUsage::None {
                        if d.target == TargetType::AnyEnemy {
                            for &e in self.enemies.iter() {
                                if self.cr(e).is_alive() && self.cr(e).in_combat {
                                    out.push(Action::UsePotion { slot: slot as u8, target: e });
                                }
                            }
                        } else {
                            out.push(Action::UsePotion { slot: slot as u8, target: NO });
                        }
                    }
                    out.push(Action::DiscardPotion { slot: slot as u8 });
                }
                out.push(Action::EndTurn);
            }
        }
    }

    /// Fills a boolean mask over the dense action space (`mask.len() >= ACTION_SPACE`).
    pub fn action_mask(&self, mask: &mut [bool]) {
        for m in mask[..ACTION_SPACE].iter_mut() {
            *m = false;
        }
        let mut buf = ActionBuf::new();
        self.legal_actions(&mut buf);
        for a in buf.iter() {
            mask[a.index()] = true;
        }
    }
}
