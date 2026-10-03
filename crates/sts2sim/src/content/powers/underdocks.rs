//! Powers owned or applied by the Act 1b (Underdocks) monsters. Stats come from `gen_powers.rs`.
//!
//! Per-power private state lives in `Power::aux` (see `aux`/`set_aux`).

use crate::content;
use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].aux)
}
fn set_aux(cx: &mut Combat, me: &Me, v: i32) {
    if let Some(i) = cx.power_idx(me.owner, me.idx) {
        cx.cr_mut(me.owner).powers[i].aux = v;
    }
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount)
}

// SuckPower: +Amount Strength per hit of the owner's powered attack that dealt unblocked damage.
listener!(SuckPower {
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() {
            return;
        }
        // C# groups the results per hit; in a hit that damaged a pet (Osty took the damage) the owner's own result is dropped
        // (`RemoveAll(r => r.Receiver == petHit.Receiver.PetOwner.Creature)`), and the hit counts when any remaining
        // result has unblocked damage.
        let mut n = 0;
        let mut at = 0usize;
        for &sz in cx.attack_hit_sizes.iter() {
            let end = (at + sz as usize).min(cx.attack_results.len());
            let group = &cx.attack_results.as_slice()[at..end];
            at += sz as usize;
            let pet_hit = group.iter().any(|r| cx.cr(r.receiver).is_pet);
            if group.iter().any(|r| r.unblocked > 0 && !(pet_hit && r.receiver == PLAYER)) {
                n += 1;
            }
        }
        if n > 0 {
            let a = amount(cx, &me);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int((a * n) as i64), me.owner, NO);
        }
    }
});

// HardenedShellPower: the owner can lose at most Amount HP per turn (aux = HP lost so far this turn).
listener!(HardenedShellPower {
    fn modify_hp_lost_before_osty_late(&self, cx: &Combat, me: Me, target: Cid, amt: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if target != me.owner || amt.is_zero() {
            return amt;
        }
        amt.min(Dec::int((amount(cx, &me) - aux(cx, &me)) as i64))
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target != me.owner {
            return;
        }
        let v = aux(cx, &me) + unblocked;
        set_aux(cx, &me, v);
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, _side: Side) {
        set_aux(cx, &me, 0);
    }
});

// SkittishPower: the first powered card attack that damages the owner each turn gives it Amount block.
listener!(SkittishPower {
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if aux(cx, &me) != 0 || !attack.props.has(ValueProp::MOVE) || attack.card == NO {
            return;
        }
        let hit = cx.attack_results.iter().find(|r| r.receiver == me.owner);
        if let Some(r) = hit {
            if r.unblocked != 0 {
                set_aux(cx, &me, 1);
                let a = amount(cx, &me);
                cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
            }
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            set_aux(cx, &me, 0);
        }
    }
});

// RavenousPower (CorpseSlug): when another creature on its side dies it is stunned (pending move delayed a turn) and
// gains Strength.
listener!(RavenousPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature == me.owner || cx.cr(creature).side != cx.cr(me.owner).side || cx.cr(me.owner).is_dead() {
            return;
        }
        cx.stun(me.owner, None, None);
        let a = amount(cx, &me);
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
    }
});

// ThieveryPower (GremlinMerc): `aux` = gold stolen so far (`DynamicVars.Gold`). Stealing is done by the monster's moves.
listener!(ThieveryPower {});
// HeistPower: only returns stolen gold as a reward when its owner dies (run-level, not modelled).
listener!(HeistPower {});

// SurprisePower (GremlinMerc): on its owner's death spawn Sneaky + Fat Gremlins (spec 04 §3.2) and keep combat open.
listener!(SurprisePower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        // CreateCreature(FatGremlin): HP draw #1, not yet in `enemies`.
        let Some(fat) = cx.create_enemy(ids::monster::FAT_GREMLIN, NO) else { return };
        let thieves: crate::util::ArrayVec<u16, MAX_POWERS> = {
            let mut v = crate::util::ArrayVec::new();
            for p in cx.cr(me.owner).powers.iter() {
                if p.id == ids::power::THIEVERY_POWER {
                    v.push(p.uid);
                }
            }
            v
        };
        for &uid in thieves.iter() {
            let stolen = cx.power_idx(me.owner, uid).map_or(0, |i| cx.cr(me.owner).powers[i].aux);
            // `MarkGoldStolen` only feeds the gold reward proportion (run-level).
            cx.apply_power(ids::power::HEIST_POWER, fat, Dec::int(stolen as i64), me.owner, NO);
        }
        cx.summon_enemy(ids::monster::SNEAKY_GREMLIN, NO, [0, 0]);
        cx.attach_enemy(fat);
        cx.after_enemy_added(fat);
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
});

// SmoggyPower (player debuff from LivingFog): after a Skill is played, every Skill card in the combat piles gets the Smog
// affliction (unplayable) until the end of the player's turn. Smog has no logic of its own.
fn smog_card(cx: &mut Combat, i: usize) {
    if cx.cards[i].affliction == 0 && cx.card_def(i as CardIdx).ctype == CardType::Skill {
        cx.afflict_card(i as CardIdx, ids::affliction::SMOG, 1);
    }
}

listener!(SmoggyPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER || cx.card_def(play.card).ctype != CardType::Skill {
            return;
        }
        for i in 0..cx.n_cards as usize {
            let k = &cx.cards[i];
            if k.pile == 0 || k.pile > 5 || k.flags & cflag::REMOVED != 0 {
                continue;
            }
            smog_card(cx, i);
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if me.owner != PLAYER || cx.cards[card as usize].affliction != 0 || cx.card_def(card).ctype != CardType::Skill {
            return;
        }
        // CardPlaysStarted.Any(this turn, Skill, player)
        let skill_played = cx.plays_this_turn(|e| cx.card_def(e.card).ctype == CardType::Skill) > 0;
        if skill_played {
            smog_card(cx, card as usize);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        for i in 0..cx.n_cards as usize {
            if cx.cards[i].affliction == ids::affliction::SMOG as u8 + 1 {
                cx.clear_affliction(i as CardIdx);
            }
        }
    }
    fn should_play(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.cards[card as usize].affliction != ids::affliction::SMOG as u8 + 1
    }
});

// ShriekPower (TerrorEel): below Amount HP the Eel is stunned into TERROR_MOVE, once.
listener!(ShriekPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target == me.owner && unblocked > 0 && cx.cr(target).hp <= amount(cx, &me) {
            cx.stun(me.owner, None, Some(crate::content::monsters::underdocks_b::eel::TERROR));
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// AsleepPower (LagavulinMatriarch): damage wakes it (stun into SLASH); otherwise it sleeps Amount enemy turns.
listener!(AsleepPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target != me.owner || unblocked == 0 {
            return;
        }
        if let Some(p) = cx.cr(me.owner).power(ids::power::PLATING_POWER).map(|p| p.uid) {
            cx.remove_power(me.owner, p);
        }
        cx.stun(me.owner, None, Some(crate::content::monsters::underdocks_b::matriarch::SLASH));
        cx.remove_power(me.owner, me.idx);
    }
    fn before_side_turn_end_very_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side || amount(cx, &me) > 1 {
            return;
        }
        if let Some(p) = cx.cr(me.owner).power(ids::power::PLATING_POWER).map(|p| p.uid) {
            cx.remove_power(me.owner, p);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// SteamEruptionPower (WaterfallGiant): when the Giant "dies" it is revived at 999999999 HP into ABOUT_TO_BLOW.
listener!(SteamEruptionPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        // TriggerAboutToBlowState: SetMaxAndCurrentHp(999999999), SetMoveImmediate(ABOUT_TO_BLOW, force)
        cx.set_max_hp(me.owner, Dec::int(999_999_999));
        cx.set_current_hp(me.owner, Dec::int(999_999_999));
        cx.set_move_immediate(me.owner, crate::content::monsters::underdocks_b::giant::ABOUT_TO_BLOW, true);
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
    fn should_creature_be_removed_from_combat_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});
