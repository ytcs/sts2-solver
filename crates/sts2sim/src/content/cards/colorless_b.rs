//! COLORLESS pool cards, part B: cards that need the engine extensions in `engine/ext.rs` (nested auto-play, potion
//! generation, hook-raised decisions via their powers).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::RunResult;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Procure a random potion (`PotionFactory.CreateRandomPotionInCombat` + `PotionCmd.TryToProcure`).
listener!(Alchemize {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        if let Some(pot) = cx.create_random_potion(true) {
            cx.try_procure_potion(pot);
        }
        Flow::Done
    }
});

// Auto-play `Cards` random Attacks from the discard pile (`StableShuffle` with the SHUFFLE stream, then take).
// The picked cards are parked in the card's two counters (4 x u8, stored as index + 1, 0 = none); the resume phase is
// `index + 1` of the pick whose auto-play suspended on a decision.
fn beat_down_run(cx: &mut Combat, p: &CardPlay, start: usize) -> Flow {
    let (c0, c1) = (cx.cards[p.card as usize].counter[0] as u16, cx.cards[p.card as usize].counter[1] as u16);
    let slots = [(c0 & 0xFF) as u8, (c0 >> 8) as u8, (c1 & 0xFF) as u8, (c1 >> 8) as u8];
    for i in start..4 {
        if slots[i] == 0 || cx.is_over_or_ending() {
            break;
        }
        let item = slots[i] - 1;
        // The game picks the target itself (`NextItem(HittableEnemies)`, one `CombatTargets` draw) and then calls
        // `AutoPlay(item, target)`: the draw happens even when `AutoPlay` then refuses the card (Normality ...).
        let mut target = NO;
        if cx.card_target_type(item) == TargetType::AnyEnemy {
            let h = cx.hittable_enemies();
            if !h.is_empty() {
                target = h[cx.rng.combat_targets.next_int_range(0, h.len() as i32) as usize];
            }
        }
        if cx.auto_play(item, target, AutoPlayType::Default, false) == RunResult::Suspended {
            return Flow::Suspend((i + 1) as u8);
        }
    }
    Flow::Done
}

listener!(BeatDown {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        if phase == 0 {
            let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
            for &c in cx.player.discard.iter() {
                if cx.card_def(c).ctype == CardType::Attack && cx.card_keywords(c) & kw::UNPLAYABLE == 0 {
                    list.push(c);
                }
            }
            cx.stable_shuffle_cards(list.as_mut_slice(), RngStream::Shuffle);
            let n = cx.card_var(p.card, VarKind::Cards) as usize;
            let mut packed = [0u8; 4];
            for (i, &c) in list.iter().take(n.min(4)).enumerate() {
                packed[i] = c + 1;
            }
            let card = &mut cx.cards[p.card as usize];
            card.counter[0] = (packed[0] as u16 | (packed[1] as u16) << 8) as i16;
            card.counter[1] = (packed[2] as u16 | (packed[3] as u16) << 8) as i16;
        }
        beat_down_run(cx, p, phase as usize)
    }
});

// `Cards` times: auto-play a random playable card of the draw pile (any card if none is playable); each pick is a
// `StableShuffle` of the pile with the SHUFFLE stream. The resume phase is `iteration + 1`.
listener!(Catastrophe {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards) as usize;
        for i in (phase as usize)..n {
            let mut playable: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
            for &c in cx.player.draw.iter() {
                if cx.card_keywords(c) & kw::UNPLAYABLE == 0 {
                    playable.push(c);
                }
            }
            cx.stable_shuffle_cards(playable.as_mut_slice(), RngStream::Shuffle);
            let mut chosen = playable.first();
            if chosen.is_none() {
                let mut all: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
                for &c in cx.player.draw.iter() {
                    all.push(c);
                }
                cx.stable_shuffle_cards(all.as_mut_slice(), RngStream::Shuffle);
                chosen = all.first();
            }
            if let Some(c) = chosen {
                if cx.auto_play(c, NO, AutoPlayType::Default, false) == RunResult::Suspended {
                    return Flow::Suspend((i + 1) as u8);
                }
            }
        }
        Flow::Done
    }
});

listener!(Entropy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.apply_power(ids::power::ENTROPY_POWER, PLAYER, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Mayhem {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::MAYHEM_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});
