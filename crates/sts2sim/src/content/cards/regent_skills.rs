//! Regent skill and power cards (bodies follow the decompiled `Models/Cards/<Class>.cs` `OnPlay`).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, RunResult, Targeting};
use crate::content::gen_cards::var_name;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn block(cx: &mut Combat, p: &CardPlay) {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
}
/// `PowerCmd.Apply<T>(owner, <card power var T>)`.
fn self_power(cx: &mut Combat, p: &CardPlay, power: u16) {
    let n = cx.card_power_var(p.card, power);
    cx.apply_power(power, PLAYER, Dec::int(n as i64), PLAYER, p.card);
}
fn self_power_n(cx: &mut Combat, p: &CardPlay, power: u16, n: i32) {
    cx.apply_power(power, PLAYER, Dec::int(n as i64), PLAYER, p.card);
}
fn upgraded(cx: &Combat, p: &CardPlay) -> u8 {
    (cx.cards[p.card as usize].upgrade > 0) as u8
}

// ---- starter / star generators -----------------------------------------------------------------------------------

listener!(DefendRegent {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

listener!(Venerate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(n);
        Flow::Done
    }
});

listener!(GatherLight {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let n = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(n);
        Flow::Done
    }
});

listener!(Glow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(s);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        self_power_n(cx, p, ids::power::DRAW_CARDS_NEXT_TURN_POWER, n);
        Flow::Done
    }
});

listener!(Alignment {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(CloakOfStars {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

listener!(HiddenCache {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(s);
        self_power(cx, p, ids::power::STAR_NEXT_TURN_POWER);
        Flow::Done
    }
});

listener!(RoyalGamble {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(s);
        Flow::Done
    }
});

listener!(Convergence {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::RETAIN_HAND_POWER, 1);
        let e = cx.card_var(p.card, VarKind::Energy);
        self_power_n(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        let s = cx.card_var(p.card, VarKind::Stars);
        self_power_n(cx, p, ids::power::STAR_NEXT_TURN_POWER, s);
        Flow::Done
    }
});

listener!(BigBang {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        let s = cx.card_var(p.card, VarKind::Stars);
        cx.gain_stars(s);
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});

// ---- block / buff skills ------------------------------------------------------------------------------------------

listener!(Reflect {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        self_power_n(cx, p, ids::power::REFLECT_POWER, 1);
        Flow::Done
    }
});

// After a play the card returns to the hand instead of the discard pile.
listener!(ParticleWall {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
    fn get_result_location_for_card_play(&self, _cx: &Combat, _me: Me, _card: CardIdx, base: CardLocation) -> CardLocation {
        if base.pile == PileType::Discard { CardLocation::new(PileType::Hand, base.pos) } else { base }
    }
});

listener!(Patter {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        self_power(cx, p, ids::power::VIGOR_POWER);
        Flow::Done
    }
});

listener!(Terraforming {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::VIGOR_POWER);
        Flow::Done
    }
});

// +Strength for the player, -1 Strength on every enemy.
listener!(Resonance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::STRENGTH_POWER);
        let enemies = cx.hittable_enemies();
        for &e in enemies.iter() {
            cx.apply_power(ids::power::STRENGTH_POWER, e, Dec::int(-1), PLAYER, p.card);
        }
        Flow::Done
    }
});

listener!(KnowThyPlace {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let w = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(w as i64), PLAYER, p.card);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Block now and `BlockVar("BlockNextTurn", 5)` (modified by block hooks now) next turn.
listener!(Glitterstream {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let next = cx.card_named_var(p.card, var_name::BLOCK_NEXT_TURN) as i64;
        let modified = cx.modify_block(PLAYER, Dec::int(next), ValueProp::MOVE, p.card);
        block(cx, p);
        cx.apply_power(ids::power::BLOCK_NEXT_TURN_POWER, PLAYER, modified, PLAYER, p.card);
        Flow::Done
    }
});

listener!(IAmInvincible {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
    // At the end of the turn, if this card is on top of the draw pile it plays itself.
    fn after_auto_post_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if cx.player.draw.first() == Some(me.idx as CardIdx) {
            let _ = cx.auto_play_from_draw_pile(1, CardPilePosition::Top, false);
        }
    }
});

listener!(Prophesize {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// ---- card generation / selection ---------------------------------------------------------------------------------------

fn colorless(cx: &mut Combat, n: usize) -> crate::util::ArrayVec<CardIdx, 16> {
    let pool = &crate::content::gen_pools::COLORLESS;
    cx.get_distinct_for_combat(pool, n, |_| true)
}

listener!(ManifestAuthority {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let cards = colorless(cx, 1);
        if let Some(c) = cards.first() {
            if upgraded(cx, p) != 0 {
                cx.upgrade_in_combat(c);
            }
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(BundleOfJoy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        let cards = colorless(cx, n.max(0) as usize);
        for &c in cards.iter() {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// Three colorless cards (upgraded when this is), choose one (or none) for the hand.
listener!(Quasar {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let cards = colorless(cx, 3);
                if upgraded(cx, p) != 0 {
                    for &c in cards.iter() {
                        cx.upgrade_in_combat(c);
                    }
                }
                match cx.ask_options(ids::card::QUASAR, cards.as_slice(), true) {
                    Ask::Resolved(cards) => {
                        // synchronous answer (Whispering Earring's selector, empty option list): same continuation as the resumed phase
                        cx.choice.cards = cards;
                        self.on_play(cx, p, 1)
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

// Draw, then put Cards-var... cards from the hand back on top of the draw pile.
fn put_back_on_top(cx: &mut Combat, cards: crate::util::ArrayVec<CardIdx, 16>) {
    for &c in cards.iter() {
        cx.move_card(c, PileType::Draw, CardPilePosition::Top);
    }
}

listener!(Glimmer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards_nosuspend(n, false);
                let k = cx.card_named_var(p.card, var_name::PUT_BACK).clamp(0, 10) as u8;
                match cx.ask_hand(ids::card::GLIMMER, k, k, |_, _| true) {
                    Ask::Resolved(cards) => {
                        put_back_on_top(cx, cards);
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let cards = cx.choice.cards;
                put_back_on_top(cx, cards);
                Flow::Done
            }
        }
    }
});

listener!(PhotonCut {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let d = cx.card_base_damage(p.card);
                cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards_nosuspend(n, false);
                let k = cx.card_named_var(p.card, var_name::PUT_BACK).clamp(0, 10) as u8;
                match cx.ask_hand(ids::card::PHOTON_CUT, k, k, |_, _| true) {
                    Ask::Resolved(cards) => {
                        put_back_on_top(cx, cards);
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let cards = cx.choice.cards;
                put_back_on_top(cx, cards);
                Flow::Done
            }
        }
    }
});

// Block, then move a discard-pile card to the top of the draw pile.
listener!(CosmicIndifference {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                block(cx, p);
                match cx.ask_pile(ids::card::COSMIC_INDIFFERENCE, PileType::Discard, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        top_if_in_draw_or_discard(cx, cards.first());
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let c = cx.choice.cards.first();
                top_if_in_draw_or_discard(cx, c);
                Flow::Done
            }
        }
    }
});

fn top_if_in_draw_or_discard(cx: &mut Combat, c: Option<CardIdx>) {
    if let Some(c) = c {
        if matches!(cx.card_pile_type(c), PileType::Draw | PileType::Discard) {
            cx.move_card(c, PileType::Draw, CardPilePosition::Top);
        }
    }
}

// Choose a hand card to transform into Minion Strike.
listener!(Begone {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let up = upgraded(cx, p);
        match phase {
            0 => match cx.ask_hand(ids::card::BEGONE, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.transform_cards(&[c], &[Some((ids::card::MINION_STRIKE, up))]);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.transform_cards(&[c], &[Some((ids::card::MINION_STRIKE, up))]);
                }
                Flow::Done
            }
        }
    }
});

// Choose Cards cards from the draw pile and transform them into Minion Dive Bomb.
listener!(Charge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let up = upgraded(cx, p);
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards).clamp(0, 255) as u8;
                match cx.ask_pile(ids::card::CHARGE, PileType::Draw, n, n, |_, _| true) {
                    Ask::Resolved(cards) => {
                        for &c in cards.iter() {
                            cx.transform_cards(&[c], &[Some((ids::card::MINION_DIVE_BOMB, up))]);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let cards = cx.choice.cards;
                for &c in cards.iter() {
                    cx.transform_cards(&[c], &[Some((ids::card::MINION_DIVE_BOMB, up))]);
                }
                Flow::Done
            }
        }
    }
});

// Choose any number of hand cards and transform each into Minion Sacrifice.
listener!(Guards {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let up = upgraded(cx, p);
        match phase {
            0 => match cx.ask_hand(ids::card::GUARDS, 0, MAX_HAND as u8, |_, _| true) {
                Ask::Resolved(cards) => {
                    for &c in cards.iter() {
                        cx.transform_cards(&[c], &[Some((ids::card::MINION_SACRIFICE, up))]);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                let cards = cx.choice.cards;
                for &c in cards.iter() {
                    cx.transform_cards(&[c], &[Some((ids::card::MINION_SACRIFICE, up))]);
                }
                Flow::Done
            }
        }
    }
});

// Draw, choose a playable Skill in hand and auto-play it Repeat times. The chosen card is kept in `counter[0]` (+1) so the
// repeat loop can resume after a nested decision; phase 10 + k = "k plays done".
fn decisions_filter(cx: &Combat, c: CardIdx) -> bool {
    cx.card_def(c).ctype == CardType::Skill && cx.card_keywords(c) & kw::UNPLAYABLE == 0
}
fn decisions_loop(cx: &mut Combat, p: &CardPlay, from: u8) -> Flow {
    let sel = cx.cards[p.card as usize].counter[0];
    if sel <= 0 {
        return Flow::Done;
    }
    let n = cx.card_var(p.card, VarKind::Repeat);
    for k in from as i32..n {
        if cx.auto_play((sel - 1) as CardIdx, NO, AutoPlayType::Default, false) == RunResult::Suspended {
            return Flow::Suspend(10 + (k + 1) as u8);
        }
    }
    Flow::Done
}
listener!(DecisionsDecisions {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards_nosuspend(n, false);
                match cx.ask_hand(ids::card::DECISIONS_DECISIONS, 1, 1, decisions_filter) {
                    Ask::Resolved(cards) => {
                        cx.cards[p.card as usize].counter[0] = cards.first().map_or(0, |c| c as i16 + 1);
                        decisions_loop(cx, p, 0)
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            1 => {
                cx.cards[p.card as usize].counter[0] = cx.choice.cards.first().map_or(0, |c| c as i16 + 1);
                decisions_loop(cx, p, 0)
            }
            k => decisions_loop(cx, p, k - 10),
        }
    }
});

// A card that replays itself into the hand every Cards-th Skill played in a turn (while outside the hand).
listener!(MakeItSo {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = cx.card_base_damage(p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
    fn after_card_played_late(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let c = me.idx as CardIdx;
        if cx.card_def(play.card).ctype == CardType::Skill && cx.card_pile_type(c) != PileType::Hand {
            let n = cx.hist.skills_finished_this_turn as i32;
            let k = cx.card_var(c, VarKind::Cards);
            if k > 0 && n % k == 0 {
                cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
        }
    }
});

// ---- Forge skills ------------------------------------------------------------------------------------------------------

listener!(Bulwark {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});

listener!(Conqueror {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        cx.apply_power(ids::power::CONQUEROR_POWER, p.target, Dec::int(1), PLAYER, p.card);
        Flow::Done
    }
});

listener!(RefineBlade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        let e = cx.card_var(p.card, VarKind::Energy);
        self_power_n(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        Flow::Done
    }
});

listener!(SpoilsOfBattle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(TheSmith {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});

// Every Sovereign Blade outside the hand returns to it, then Forge.
listener!(SummonForth {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let all = cx.player_combat_cards();
        for &c in all.iter() {
            if cx.cards[c as usize].id == ids::card::SOVEREIGN_BLADE && cx.card_pile_type(c) != PileType::Hand {
                cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            }
        }
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});

// ---- powers -------------------------------------------------------------------------------------------------------------

listener!(Arsenal {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::ARSENAL_POWER);
        Flow::Done
    }
});
listener!(BlackHole {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::BLACK_HOLE_POWER);
        Flow::Done
    }
});
listener!(ChildOfTheStars {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::BLOCK_FOR_STARS);
        self_power_n(cx, p, ids::power::CHILD_OF_THE_STARS_POWER, n);
        Flow::Done
    }
});
listener!(Furnace {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_var(p.card, VarKind::Forge);
        self_power_n(cx, p, ids::power::FURNACE_POWER, f);
        Flow::Done
    }
});
listener!(Genesis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::STARS_PER_TURN);
        self_power_n(cx, p, ids::power::GENESIS_POWER, n);
        Flow::Done
    }
});
listener!(MonarchsGaze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::STRENGTH_LOSS);
        self_power_n(cx, p, ids::power::MONARCHS_GAZE_POWER, n);
        Flow::Done
    }
});
// The card's `Power` var (1) becomes the power's Strength var; the power itself is applied with amount 1.
listener!(Monologue {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::MONOLOGUE_POWER, 1);
        Flow::Done
    }
});
listener!(NeutronAegis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::PLATING_POWER);
        Flow::Done
    }
});
listener!(Orbit {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        self_power_n(cx, p, ids::power::ORBIT_POWER, e);
        Flow::Done
    }
});
listener!(PaleBlueDot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        self_power_n(cx, p, ids::power::PALE_BLUE_DOT_POWER, n);
        Flow::Done
    }
});
listener!(Parry {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::PARRY_POWER);
        Flow::Done
    }
});
listener!(PillarOfCreation {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        self_power_n(cx, p, ids::power::PILLAR_OF_CREATION_POWER, b);
        Flow::Done
    }
});
listener!(Royalties {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let g = cx.card_var(p.card, VarKind::Gold);
        self_power_n(cx, p, ids::power::ROYALTIES_POWER, g);
        Flow::Done
    }
});
listener!(SeekingEdge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::SEEKING_EDGE_POWER, 1);
        let f = cx.card_var(p.card, VarKind::Forge);
        cx.forge(f);
        Flow::Done
    }
});
listener!(SpectrumShift {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        self_power_n(cx, p, ids::power::SPECTRUM_SHIFT_POWER, n);
        Flow::Done
    }
});
listener!(SwordSage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::SWORD_SAGE_POWER);
        Flow::Done
    }
});
listener!(TheSealedThrone {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::THE_SEALED_THRONE_POWER, 1);
        Flow::Done
    }
});
listener!(Tyranny {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::TYRANNY_POWER, 1);
        Flow::Done
    }
});
listener!(ForegoneConclusion {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        self_power_n(cx, p, ids::power::FOREGONE_CONCLUSION_POWER, n);
        Flow::Done
    }
});
// Applies the power, then ends the turn once this action has finished (`PlayerCmd.EndTurn`).
listener!(VoidForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power(cx, p, ids::power::VOID_FORM_POWER);
        cx.request_end_turn();
        Flow::Done
    }
});
// MultiplayerOnly cards: single player has no allies, so the AnyAlly ones are never playable (spec 03 §4).
listener!(HammerTime {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        self_power_n(cx, p, ids::power::HAMMER_TIME_POWER, 1);
        Flow::Done
    }
});
listener!(Constellation {});
listener!(Largesse {});
listener!(Tutor {});
listener!(Plot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        self_power_n(cx, p, ids::power::DRAW_CARDS_NEXT_TURN_POWER, n);
        Flow::Done
    }
});
