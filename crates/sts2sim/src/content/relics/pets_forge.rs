//! Relics that need the pet / Forge / extra-turn systems: Bone Flute, Byrdpip, Fencing Manual, Pael's Eye, Pael's Legion.
//!
//! Relic state conventions: flags = booleans, counter/aux = ints (see `Listener::meta_*`).

use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::ids;
use crate::engine::HKind;
use crate::{listener, relic_props};
use crate::state::*;
use crate::types::*;

// ---- Bone Flute: every attack by the owner's Osty gives `Block` (unpowered) ------------------------------------------------------

listener!(BoneFlute {
    fn after_attack(&self, cx: &mut Combat, _me: Me, attack: &Attack) {
        if attack.dealer == NO {
            return;
        }
        let d = cx.cr(attack.dealer);
        if d.is_pet && d.owner == PLAYER && d.monster.id == ids::monster::OSTY {
            cx.gain_block(PLAYER, Dec::int(g::bone_flute::BLOCK as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

// ---- Byrdpip: a (do-nothing) pet joins every combat -------------------------------------------------------------------------------

listener!(Byrdpip {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        cx.add_pet(ids::monster::BYRDPIP, 9999);
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::constant("Skin", "\"version1\"")]
    }
});

// ---- Fencing Manual: Forge on the first turn --------------------------------------------------------------------------------------

listener!(FencingManual {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.forge(g::fencing_manual::FORGE);
        }
    }
});

// ---- Pael's Eye: an empty turn is skipped for an extra turn (once per combat) --------------------------------------------------------

// flag 0 = `UsedThisCombat`, flag 1 = `WasOwnerPartOfLastPlayerTurn` (starts true); neither is saved.
fn paels_eye_ready(cx: &Combat, me: Me) -> bool {
    let r = cx.rel(me);
    if r.flag(0) || !r.flag(1) {
        return false;
    }
    // `AnyCardsPlayedThisTurn`: Whispering Earring's turn-1 auto-plays count; otherwise any manual play this turn.
    if cx.turn_number() == 1 && cx.has_relic(ids::relic::WHISPERING_EARRING) {
        return false;
    }
    cx.hist_count_this_turn(HKind::CardPlayStarted, |e| e.flags & 1 == 0) == 0
}
listener!(PaelsEye {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(1, true);
        }
    }
    fn should_take_extra_turn(&self, cx: &Combat, me: Me) -> bool {
        paels_eye_ready(cx, me)
    }
    fn before_side_turn_end_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player || !paels_eye_ready(cx, me) {
            return;
        }
        let hand = cx.player.hand;
        for &c in hand.iter() {
            cx.exhaust_card(c, false);
        }
    }
    fn after_taking_extra_turn(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, true);
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (0, 0b10, 0)
    }
});

// ---- Pael's Legion: card block is doubled, then goes on cooldown for `Turns` turns ---------------------------------------------------

// counter = `Cooldown` (decremented every turn start, may go negative), flag 0 = `TriggeredBlockLastTurn`,
// aux = `AffectedCardPlay` identity (card + 1, play index; 0 = none). Only `Skin` is saved.
fn play_ident(card: CardIdx, play_index: u8) -> i32 {
    ((card as i32 + 1) << 8) | play_index as i32
}
listener!(PaelsLegion {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        cx.add_pet(ids::monster::PAELS_LEGION, 9999);
    }
    fn modify_block_multiplicative(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if !q.props.has(ValueProp::MOVE) || q.card == NO || cx.rel(me).counter > 0 {
            return Dec::ONE;
        }
        Dec::int(2)
    }
    fn after_modifying_block_amount(&self, cx: &mut Combat, me: Me, modified: Dec, card: CardIdx) {
        if modified <= Dec::ZERO || card == NO {
            return;
        }
        // The `CardPlay` the block came from: the innermost play of that card on the play stack.
        let Some(ctx) = cx.play_stack.iter().rev().find(|c| c.play.card == card) else { return };
        let id = play_ident(card, ctx.play.play_index);
        let r = cx.rel_mut(me);
        if r.aux != 0 && r.aux != id {
            return;
        }
        r.aux = id;
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.rel(me).aux != 0 && cx.rel(me).aux == play_ident(play.card, play.play_index) {
            let r = cx.rel_mut(me);
            r.aux = 0;
            r.counter = g::paels_legion::TURNS;
            r.set_flag(0, true);
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let r = cx.rel_mut(me);
            r.counter -= 1;
            r.set_flag(0, false);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.counter = 0;
        r.aux = 0;
        r.set_flag(0, false);
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::constant("Skin", "\"eyes\"")]
    }
    // `DisplayAmount`: -1 when no combat is in progress or the cooldown is spent.
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        if cx.in_progress && r.counter > 0 { Some(r.counter) } else { None }
    }
});
