//! Powers applied by colorless cards (Automation, Panache, The Bomb, Dark Shackles, ...).
//!
//! NOTE for the merge: `VigorPower` is also written (monster-only variant) in the underdocks branch; this one is the full
//! port (BeforeAttack command tracking). `RetainHandPower` / `BlockNextTurnPower` live in `potion_support.rs`.

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `participants.Contains(Owner)` for a side-turn hook: the owner belongs to the side whose turn it is.
#[inline]
fn on_side(cx: &Combat, me: &Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

// RetainHandPower: defined in potion_support.rs.

// ---- NoBlockPower: the owner's card-sourced block is zeroed; ticks after the enemy turn --------------------------
listener!(NoBlockPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn modify_block_multiplicative(&self, _cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if q.target != me.owner || q.props.has(ValueProp::UNPOWERED) || q.card == NO {
            return Dec::ONE;
        }
        Dec::ZERO
    }
});

// BlockNextTurnPower: defined in potion_support.rs.

// ---- FastenPower: +Amount block on powered Defend-tagged card block --------------------------------------------
listener!(FastenPower {
    fn modify_block_additive(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if me.owner != q.target || !q.props.is_powered() {
            return Dec::ZERO;
        }
        if q.card != NO && cx.card_def(q.card).tags & tag::DEFEND == 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// ---- VigorPower: the next powered attack deals +Amount, then the stacks present when it started are consumed ------
// `aux` mirrors `Data.commandToModify` (low 16 bits: Amount when the attack started; high bits: source card + 1);
// like the C# it is never cleared, so a stale value only matters if Vigor survives its first attack.
fn vigor_card(aux: i32) -> CardIdx {
    ((aux >> 16) - 1) as CardIdx
}
listener!(VigorPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, a: &crate::engine::Attack) {
        if a.dealer != me.owner || !a.props.is_powered() || a.card == NO {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let p = &mut cx.cr_mut(me.owner).powers[i];
        if p.aux != 0 {
            return;
        }
        p.aux = ((a.card as i32 + 1) << 16) | (p.amount & 0xFFFF);
    }
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() {
            return Dec::ZERO;
        }
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx) else { return Dec::ZERO };
        if p.aux != 0 && q.card != NO && q.card != vigor_card(p.aux) {
            return Dec::ZERO;
        }
        Dec::int(p.amount as i64)
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, a: &crate::engine::Attack) {
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.aux != 0 && a.dealer == me.owner && a.card == vigor_card(p.aux) {
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-((p.aux & 0xFFFF) as i64)), NO, NO);
        }
    }
});

// ---- PrepTimePower: Vigor at the start of each of the owner's turns ----------------------------------------------
listener!(PrepTimePower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            let a = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::VIGOR_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
        }
    }
});

// ---- AutomationPower (instanced): every 10th card drawn gives `Amount` energy; `aux` = cards counted so far -----
listener!(AutomationPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, _from_hand_draw: bool) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let (amount, n) = {
            let p = &mut cx.cr_mut(me.owner).powers[i];
            p.aux += 1;
            (p.amount, p.aux)
        };
        if n >= 10 {
            cx.gain_energy(amount);
            cx.cr_mut(me.owner).powers[i].aux = 0;
        }
    }
});

// ---- PanachePower (instanced): after 5 cards played in a turn deal `Amount` unpowered damage to all enemies -------
// `aux`: low byte = cards counted this turn (the C# `CardsLeft` = 5 - count), bit 8 = `alreadyApplied`.
listener!(PanachePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let aux = cx.cr(me.owner).powers[i].aux;
        let mut n = aux & 0xFF;
        if aux & 0x100 != 0 {
            n += 1;
            if n >= 5 {
                let amount = cx.cr(me.owner).powers[i].amount;
                let enemies = cx.hittable_enemies();
                cx.damage(enemies.as_slice(), Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
                n = 0;
            }
        }
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux = 0x100 | n;
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux &= !0xFF;
            }
        }
    }
});

/// `RollingBoulderPower.CanonicalVars = DamageVar(5m, Unpowered)` (the per-turn increase; not upgraded).
const ROLLING_BOULDER_INCREMENT: i32 = 5;

// ---- RollingBoulderPower (instanced): damages all enemies at the start of each turn, then grows by 5 ---------------
listener!(RollingBoulderPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let Some(amount) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map(|p| p.amount) else { return };
        let enemies = cx.hittable_enemies();
        cx.damage(enemies.as_slice(), Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
        // `SetAmount(Amount + DynamicVars.Damage.IntValue)`: no hooks.
        cx.set_power_amount(me.owner, me.idx, amount + ROLLING_BOULDER_INCREMENT);
    }
});

// ---- TheBombPower (instanced): `Amount` = turns left, `aux` = damage ---------------------------------------------
listener!(TheBombPower {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !on_side(cx, &me, side) {
            return;
        }
        let Some(p) = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).copied() else { return };
        if p.amount > 1 {
            cx.decrement_power(me.owner, me.idx);
            return;
        }
        let enemies = cx.hittable_enemies();
        cx.damage(enemies.as_slice(), Dec::int(p.aux as i64), ValueProp::UNPOWERED, me.owner, NO);
        cx.remove_power(me.owner, me.idx);
    }
});

// ---- TheGambitPower: the owner dies when it takes unblocked powered attack damage ----------------------------------
listener!(TheGambitPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, _dealer: Cid) {
        if target == me.owner && props.is_powered() && unblocked > 0 {
            cx.remove_power(me.owner, me.idx);
            cx.kill(&[me.owner]);
        }
    }
});

// ---- Temporary Strength powers (`TemporaryStrengthPower`): Coordinate (+), Dark Shackles (-) --------------------------
// The shared mechanics are `Combat::temp_*` (engine/potion_gen.rs).
listener!(DarkShacklesPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.temp_before_applied(ids::power::STRENGTH_POWER, -1, target, amount, applier, card);
    }
    fn after_power_amount_changed_full(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        cx.temp_after_amount_changed(me, ids::power::STRENGTH_POWER, -1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        cx.temp_after_side_turn_end(me, ids::power::STRENGTH_POWER, -1, side);
    }
});
listener!(CoordinatePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.temp_before_applied(ids::power::STRENGTH_POWER, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed_full(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        cx.temp_after_amount_changed(me, ids::power::STRENGTH_POWER, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        cx.temp_after_side_turn_end(me, ids::power::STRENGTH_POWER, 1, side);
    }
});

// ---- multiplayer-only powers (no teammates in single player) --------------------------------------------------------
// Beacon of Hope only shares block with *other* players.
listener!(BeaconOfHopePower {});

// Knockdown: the owner takes `Amount`x damage from powered attacks by anyone but the applier; removed after the owner's turn.
listener!(KnockdownPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let applier = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(NO, |p| p.applier);
        if q.dealer == applier {
            return Dec::ONE;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if on_side(cx, &me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Tag Team: the next Attack by anyone but the applier is played `Amount` extra times, then the power is removed.
listener!(TagTeamPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, target: Cid, count: i32) -> i32 {
        if cx.card_def(card).ctype != CardType::Attack {
            return count;
        }
        let applier = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(NO, |p| p.applier);
        if applier == PLAYER {
            return count; // `card.Owner.Creature == Applier`
        }
        let tt = cx.card_def(card).target;
        if tt == TargetType::AnyEnemy && target != me.owner {
            return count;
        }
        if !matches!(tt, TargetType::AnyEnemy | TargetType::AllEnemies) {
            return count;
        }
        count + cx.power_amount(me.owner, me.id)
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.remove_power(me.owner, me.idx);
    }
});

// ---- CalamityPower: after each Attack you play, add `Amount` random Attacks of your character's pool to the hand --------
// The per-card amount recorded at `BeforeCardPlayed` (the C# dictionary) lives in `card.counter[1]` (0 = none, else amount + 1).
listener!(CalamityPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype != CardType::Attack {
            return;
        }
        let a = cx.power_amount(me.owner, me.id);
        cx.cards[play.card as usize].counter[1] = (a + 1) as i16;
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let rec = cx.cards[play.card as usize].counter[1];
        if rec == 0 {
            return;
        }
        cx.cards[play.card as usize].counter[1] = 0;
        let amount = (rec - 1) as usize;
        let pool = cx.character_pool();
        let cards = cx.get_for_combat_where(pool, amount, |d| d.ctype == CardType::Attack);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        let _ = me;
    }
});

// ---- NostalgiaPower: the first `Amount` Attacks/Skills played each turn go back on top of the draw pile ---------------
// (`History.CardPlaysStarted` this turn, Attack/Skill only; the card being played is not logged yet at this point).
listener!(NostalgiaPower {
    fn modify_card_play_result_location_ex(&self, cx: &Combat, me: Me, card: CardIdx, _is_auto: bool, pile: PileType, pos: CardPilePosition) -> (PileType, CardPilePosition) {
        if me.owner != PLAYER || !matches!(cx.card_def(card).ctype, CardType::Attack | CardType::Skill) || pile != PileType::Discard {
            return (pile, pos);
        }
        let started = (cx.hist.attacks_played_this_turn + cx.hist.skills_played_this_turn) as i32;
        if started >= cx.power_amount(me.owner, me.id) {
            return (pile, pos);
        }
        (PileType::Draw, CardPilePosition::Top)
    }
});

// ---- MayhemPower: auto-play `Amount` cards from the top of the draw pile at the start of each turn ------------------
listener!(MayhemPower {
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if me.owner == PLAYER {
            let a = cx.power_amount(me.owner, me.id);
            cx.auto_play_from_draw_pile_hook(a, CardPilePosition::Top, false);
        }
    }
});

// ---- EntropyPower: at the start of the turn transform `Amount` chosen hand cards into random cards ----------------
// The choice is raised from inside the hook: see `Combat::hook_decision` (the step is rolled back and replayed).
listener!(EntropyPower {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let n = cx.power_amount(me.owner, me.id).clamp(0, 16) as u8;
        let ask = cx.ask_hand(ids::card::ENTROPY, n, n, |_, _| true);
        let crate::engine::Ask::Resolved(cards) = cx.hook_decision(ask, Kind::Power, ids::power::ENTROPY_POWER) else { return };
        for &c in cards.iter() {
            cx.transform_to_random(c, crate::engine::Stream::CardSelection);
        }
    }
});

// ---- StratagemPower: after a reshuffle choose `Amount` cards of the draw pile to put into the hand ---------------------
listener!(StratagemPower {
    fn after_shuffle(&self, cx: &mut Combat, me: Me) {
        if me.owner != PLAYER {
            return;
        }
        let n = cx.power_amount(me.owner, me.id).clamp(0, 16) as u8;
        let ask = cx.ask_pile(ids::card::STRATAGEM, PileType::Draw, n, n, |_, _| true);
        let crate::engine::Ask::Resolved(cards) = cx.hook_decision(ask, Kind::Power, ids::power::STRATAGEM_POWER) else { return };
        for &c in cards.iter() {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});
