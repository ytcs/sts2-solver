//! Necrobinder cards that summon / use Osty.
//!
//! Osty attacks: `DamageCmd.Attack(x).FromOsty(Owner.Osty, card, play)` — the dealer is Osty's creature, so the player's
//! Strength / Weak do not apply (they key on `dealer == Owner`). Every Osty attack is guarded by
//! `Osty.CheckMissingWithAnim(Owner)` = "Osty is dead or missing": the whole effect is skipped (the card is still paid).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `PowerCmd.Apply<T>(Owner.Creature, amount, Owner.Creature, card)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

// Summon (Summon var).
listener!(Afterlife {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        Flow::Done
    }
});

listener!(Reanimate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        Flow::Done
    }
});

// Summon, then exhaust a card of your choice from the draw pile.
listener!(Cleanse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Summon);
                cx.summon(n);
                match cx.ask_pile(ids::card::CLEANSE, PileType::Draw, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.exhaust_card(c, false);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.exhaust_card(c, false);
                }
                Flow::Done
            }
        }
    }
});

// X cost: Summon X times, add X Souls (upgraded if this card is) to the draw pile at random positions.
listener!(Dirge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.x_value(p.card);
        let n = cx.card_var(p.card, VarKind::Summon);
        for _ in 0..x {
            cx.summon(n);
        }
        let up = cx.cards[p.card as usize].upgrade > 0;
        cx.add_souls_to_draw_pile(x, up);
        Flow::Done
    }
});

// Next turn: summon + energy.
listener!(Invoke {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_var(p.card, VarKind::Summon);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::SUMMON_NEXT_TURN_POWER, s, p);
        apply_self(cx, ids::power::ENERGY_NEXT_TURN_POWER, e, p);
        Flow::Done
    }
});

listener!(PullAggro {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(NecroMastery {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        apply_self(cx, ids::power::NECRO_MASTERY_POWER, 1, p);
        Flow::Done
    }
});

// Summon, then heal Osty.
listener!(Spur {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        if let Some(o) = cx.osty() {
            let h = cx.card_var(p.card, VarKind::Heal);
            cx.heal(o, Dec::int(h as i64));
        }
        Flow::Done
    }
});

// ---- Osty attacks -----------------------------------------------------------------------------------------------------

listener!(Poke {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let d = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        }
        Flow::Done
    }
});

// Osty attacks, then choose a card in hand without Retain to give it Retain.
listener!(Snap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let osty = cx.living_osty();
                if osty != NO {
                    let d = cx.card_var(p.card, VarKind::OstyDamage);
                    cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
                }
                match cx.ask_hand(ids::card::SNAP, 1, 1, |cx, c| cx.card_keywords(c) & kw::RETAIN == 0) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.apply_keyword(c, kw::RETAIN);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.apply_keyword(c, kw::RETAIN);
                }
                Flow::Done
            }
        }
    }
});

// Costs 0 this turn once Osty has attacked this turn.
fn flatten_reduce_cost(cx: &mut Combat, c: CardIdx) {
    let card = &mut cx.cards[c as usize];
    if let Some(last) = card.mods.last() {
        if !last.relative() && !last.reduce_only() && last.amount == 0 && last.expire() == EXPIRE_END_OF_TURN {
            return;
        }
    }
    card.mods.push(CostMod::new(0, false, false, EXPIRE_END_OF_TURN));
}

listener!(Flatten {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let d = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        }
        Flow::Done
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.osty_attacks_this_turn() == 0 {
            return;
        }
        flatten_reduce_cost(cx, card);
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, a: &Attack) {
        if a.dealer == NO || Some(a.dealer) != cx.osty() {
            return;
        }
        flatten_reduce_cost(cx, me.idx as CardIdx);
    }
});

// Osty attacks; if this card has not been played yet this turn, draw.
listener!(Fetch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let d = cx.card_var(p.card, VarKind::OstyDamage);
        cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        if !cx.hist.finished(p.card) {
            let n = cx.card_var(p.card, VarKind::Cards);
            cx.draw_cards(n, false);
        }
        Flow::Done
    }
});

// Osty attacks 1 + (Osty attacks this turn) times.
listener!(Rattle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let d = cx.card_var(p.card, VarKind::OstyDamage);
            let hits = 1 + cx.osty_attacks_this_turn() as i32;
            cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)).hits(hits));
        }
        Flow::Done
    }
});

// Osty attacks, then apply Sic 'Em (Osty hits on this enemy summon).
listener!(SicEm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let d = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        }
        let n = cx.card_power_var(p.card, ids::power::SIC_EM_POWER);
        cx.apply_power(ids::power::SIC_EM_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Unplayable without Osty. Osty attacks all enemies, then Vulnerable to all.
listener!(HighFive {
    fn is_playable(&self, cx: &Combat, _card: CardIdx) -> bool {
        !cx.is_osty_missing()
    }
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let d = cx.card_var(p.card, VarKind::OstyDamage);
        cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::AllOpponents));
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::VULNERABLE_POWER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Osty attacks all enemies, you gain Block, then Osty dies.
listener!(BoneShards {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let d = cx.card_var(p.card, VarKind::OstyDamage);
        cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::AllOpponents));
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        if cx.is_osty_alive() {
            let o = cx.osty().unwrap();
            cx.kill(&[o]);
        }
        Flow::Done
    }
});

// Osty attacks; when you play a card that spent >= 2 energy this returns from the discard pile to your hand.
listener!(RightHandHand {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let d = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        }
        Flow::Done
    }
    fn after_card_played_late(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let c = me.idx as CardIdx;
        let need = cx.card_var(c, VarKind::Energy);
        if play.energy_spent >= need && cx.card_pile_type(c) == PileType::Discard {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

// 25 (+5) + 5 x (other Osty Attack cards in all your piles).
listener!(Squeeze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let mut n = 0;
        for pile in [PileType::Hand, PileType::Draw, PileType::Discard, PileType::Exhaust, PileType::Play] {
            for &c in cx.pile(pile).iter() {
                if c != p.card && cx.card_def(c).tags & tag::OSTY_ATTACK != 0 {
                    n += 1;
                }
            }
        }
        let d = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * n;
        cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
});

// 10 (+5) + 1 x Osty's max HP.
listener!(Protector {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let d = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * cx.cr(osty).max_hp;
        cx.execute_attack(&Attack::from_card(osty, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
});

// Kill Osty; gain Block equal to 3 x its max HP.
listener!(Sacrifice {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        // CalculatedBlockVar: base 0 + extra 1 x (Osty alive ? MaxHp * 3 : 0), computed before Osty dies.
        let block = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * (cx.cr(osty).max_hp * 3);
        cx.kill(&[osty]);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});
