use crate::defs::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayCard { hand_pos: u8, target: Cid },
    UsePotion { slot: u8, target: Cid },
    DiscardPotion { slot: u8 },
    EndTurn,
    Pick { idx: u8 },
    Confirm,
}

const T: usize = MAX_CREATURES + 1;
pub const MAX_PICK: usize = 64;
const OFF_PLAY: usize = 1;
const OFF_POTION: usize = OFF_PLAY + MAX_HAND * T;
const OFF_DISCARD: usize = OFF_POTION + MAX_POTIONS * T;
const OFF_PICK: usize = OFF_DISCARD + MAX_POTIONS;
const OFF_CONFIRM: usize = OFF_PICK + MAX_PICK;
pub const ACTION_SPACE: usize = OFF_CONFIRM + 1;

pub type ActionBuf = ArrayVec<Action, 256>;

impl Action {
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
    pub fn legal_actions(&self, out: &mut ActionBuf) {
        let mut playable = 0;
        self.legal_actions_ex(out, &mut playable);
    }

    pub fn legal_actions_ex(&self, out: &mut ActionBuf, hand_playable: &mut u16) {
        *hand_playable = 0;
        out.clear();
        match self.stage {
            Stage::Over => {}
            Stage::AwaitChoice => {
                if let Some(d) = &self.decision {
                    if d.cands.len() > MAX_PICK {
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

    pub fn interchangeable(&self, a: Action, b: Action) -> bool {
        let pot = |s: u8| self.player.potions.get(s as usize).copied().flatten().map(|p| p.id);
        match (a, b) {
            _ if a == b => true,
            (Action::PlayCard { hand_pos: x, target: s }, Action::PlayCard { hand_pos: y, target: t }) => {
                s == t && matches!((self.player.hand.get(x as usize), self.player.hand.get(y as usize)), (Some(c), Some(d)) if self.same_card(c, d))
            }
            (Action::UsePotion { slot: x, target: s }, Action::UsePotion { slot: y, target: t }) => s == t && pot(x).is_some() && pot(x) == pot(y),
            (Action::DiscardPotion { slot: x }, Action::DiscardPotion { slot: y }) => pot(x).is_some() && pot(x) == pot(y),
            (Action::Pick { idx: x }, Action::Pick { idx: y }) => {
                let Some(d) = &self.decision else { return false };
                let v = self.decision_view(d);
                match (v.get(x as usize), v.get(y as usize)) {
                    (Some(i), Some(j)) => d.selected.contains(i) == d.selected.contains(j) && self.same_card(d.cands[i as usize], d.cands[j as usize]),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    // deck_idx only matters as deck card vs generated card inside a fight; play_amounts and finished bits are per-card state outside Card.
    pub fn same_card(&self, x: CardIdx, y: CardIdx) -> bool {
        if x == y {
            return true;
        }
        let b = &self.cards[y as usize];
        let Card { id, pile, upgrade, flags, kw_add, kw_remove, enchant, enchant_amount, enchant_status, enchant_aux, affliction, affliction_amount, base_replay, cost_base, x_value, mods, star_mods, counter, dmg_bonus, deck_idx, dupe_of, dampen_saved } =
            &self.cards[x as usize];
        (*id, *pile, *upgrade, *flags, *kw_add, *kw_remove, *enchant, *enchant_amount, *enchant_status, *enchant_aux) == (b.id, b.pile, b.upgrade, b.flags, b.kw_add, b.kw_remove, b.enchant, b.enchant_amount, b.enchant_status, b.enchant_aux)
            && (*affliction, *affliction_amount, *base_replay, *cost_base, *x_value, *counter, *dmg_bonus, *dupe_of, *dampen_saved) == (b.affliction, b.affliction_amount, b.base_replay, b.cost_base, b.x_value, b.counter, b.dmg_bonus, b.dupe_of, b.dampen_saved)
            && mods.as_slice() == b.mods.as_slice()
            && star_mods.as_slice() == b.star_mods.as_slice()
            && (*deck_idx == NO) == (b.deck_idx == NO)
            && self.hist.finished(x) == self.hist.finished(y)
            && !self.hist.play_amounts.iter().any(|e| e.card == x || e.card == y)
    }

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
