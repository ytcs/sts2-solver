//! Powers owned or applied by the Act 1b (Underdocks) monsters. Stats come from `gen_powers.rs`.
//!
//! Per-power private state lives in `Power::aux` (see `aux`/`set_aux`).

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

// ArtifactPower: blocks visible debuffs, one charge per blocked application (spec 02 §6.3).
listener!(ArtifactPower {
    fn try_modify_power_amount_received(&self, _cx: &Combat, me: Me, power_id: u16, target: Cid, amt: Dec, _applier: Cid) -> Option<Dec> {
        if target != me.owner {
            return None;
        }
        if Combat::power_type_for_amount(power_id, amt.trunc()) != PowerType::Debuff {
            return None;
        }
        if !crate::content::power_def(power_id).visible {
            return None;
        }
        Some(Dec::ZERO)
    }
    fn after_modifying_power_amount_received(&self, cx: &mut Combat, me: Me, _power_id: u16) {
        cx.decrement_power(me.owner, me.idx);
    }
});

// RitualPower: +Amount Strength at the end of the owner's side turn; skipped once when an enemy just applied it.
listener!(RitualPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        if cx.cr(me.owner).side == Side::Enemy {
            set_aux(cx, &me, 1); // WasJustAppliedByEnemy
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        if aux(cx, &me) != 0 {
            set_aux(cx, &me, 0);
            return;
        }
        let a = amount(cx, &me);
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
    }
});

// PlatingPower (single player): enemies start with Amount block, regain it before their side ends, lose 1 per side turn start.
listener!(PlatingPower {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player || cx.cr(me.owner).is_player || cx.round > 1 {
            return;
        }
        let a = amount(cx, &me);
        cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
    }
    fn before_side_turn_end_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        let a = amount(cx, &me);
        cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        let owner = cx.cr(me.owner);
        if owner.side != side {
            return;
        }
        if owner.is_player {
            if cx.player.turn_number == 1 {
                return;
            }
            cx.decrement_power(me.owner, me.idx);
        } else {
            if cx.round == 1 {
                return;
            }
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-1), NO, NO);
        }
    }
});

// ThornsPower: powered attacks against the owner hurt the attacker.
listener!(ThornsPower {
    fn before_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _amount: Dec, props: ValueProp, dealer: Cid) {
        if target != me.owner || dealer == NO || !props.is_powered() {
            return;
        }
        let a = amount(cx, &me);
        cx.damage(&[dealer], Dec::int(a as i64), ValueProp::UNPOWERED.or(ValueProp::SKIP_HURT_ANIM), me.owner, NO);
    }
});

// SuckPower: +Amount Strength per hit of the owner's powered attack that dealt unblocked damage.
listener!(SuckPower {
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() {
            return;
        }
        let n = attack.results.iter().filter(|r| r.unblocked > 0).count() as i32;
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
        let hit = attack.results.iter().find(|r| r.receiver == me.owner);
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

// MinionPower: secondary enemy (PowerDef); keeps itself after the owner's death.
listener!(MinionPower {
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});

// IntangiblePower: all damage to the owner is capped at 1 (HP loss phase + preview cap); ticks down each enemy turn.
listener!(IntangiblePower {
    fn modify_hp_lost_after_osty(&self, cx: &Combat, me: Me, target: Cid, amt: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if !cx.in_progress || target != me.owner || amt < Dec::ONE {
            return amt;
        }
        Dec::ONE
    }
    fn modify_damage_cap(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner { Dec::MAX } else { Dec::ONE }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// VigorPower (spec 02 §6.6, verbatim quirk): `BeforeAttack` records the first powered attack of the owner as
// `commandToModify` (aux = card idx << 24 | attack id; 0 = none) and the amount at that moment (aux2); the bonus applies
// to the owner's powered damage; `AfterAttack` of that very command consumes the recorded amount. `commandToModify`
// is never cleared, so a Vigor gained later never fires again on other attacks.
fn vigor_pack(attack: &Attack) -> i32 {
    (((attack.card as u32) << 24) | (attack.id & 0xFF_FFFF)) as i32
}
listener!(VigorPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() || aux(cx, &me) != 0 {
            return;
        }
        let a = amount(cx, &me);
        set_aux(cx, &me, vigor_pack(attack));
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            cx.cr_mut(me.owner).powers[i].aux2 = a;
        }
    }
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() {
            return Dec::ZERO;
        }
        let a = aux(cx, &me);
        if a != 0 && q.card != NO && q.card != ((a as u32) >> 24) as u8 {
            return Dec::ZERO;
        }
        Dec::int(amount(cx, &me) as i64)
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        let a = aux(cx, &me);
        if a != 0 && a == vigor_pack(attack) {
            let start = cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].aux2);
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-(start as i64)), NO, NO);
        }
    }
});

// RavenousPower (CorpseSlug): when another creature on its side dies it is stunned (pending move delayed a turn) and
// gains Strength.
listener!(RavenousPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature == me.owner || cx.cr(creature).side != cx.cr(me.owner).side || cx.cr(me.owner).is_dead() {
            return;
        }
        cx.stun(me.owner, None);
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
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature != me.owner {
            return;
        }
        // CreateCreature(FatGremlin): HP draw #1, not yet in `enemies`.
        let Some(fat) = cx.create_enemy(ids::monster::FAT_GREMLIN, NO) else { return };
        let mut stolen_total = 0;
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
            stolen_total += stolen;
            cx.apply_power(ids::power::HEIST_POWER, fat, Dec::int(stolen as i64), me.owner, NO);
        }
        let _ = stolen_total; // `GremlinMercNormal.MarkGoldStolen` only feeds the gold reward proportion
        cx.spawn_enemy_live(ids::monster::SNEAKY_GREMLIN, NO, [0, 0]);
        cx.attach_enemy(fat);
        cx.after_enemy_added(fat);
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
});

// SmoggyPower (player debuff from LivingFog): after a Skill is played, every Skill card in the combat piles gets the Smog
// affliction (unplayable) until the end of the player's turn. Smog has no logic of its own.
pub const AFFLICTION_SMOG: u8 = 1;

fn smog_all_skills(cx: &mut Combat) {
    for i in 0..cx.n_cards as usize {
        let k = &cx.cards[i];
        if k.pile == 0 || k.pile > 5 || k.flags & cflag::REMOVED != 0 {
            continue;
        }
        if k.affliction == 0 && cx.card_def(i as CardIdx).ctype == CardType::Skill {
            cx.cards[i].affliction = AFFLICTION_SMOG;
            cx.cards[i].affliction_amount = 1;
        }
    }
}

listener!(SmoggyPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER || cx.card_def(play.card).ctype != CardType::Skill {
            return;
        }
        smog_all_skills(cx);
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if me.owner != PLAYER || cx.cards[card as usize].affliction != 0 || cx.card_def(card).ctype != CardType::Skill {
            return;
        }
        // CardPlaysStarted.Any(this turn, Skill): approximated by the per-turn skill counter.
        if cx.hist.skills_played_this_turn > 0 {
            cx.cards[card as usize].affliction = AFFLICTION_SMOG;
            cx.cards[card as usize].affliction_amount = 1;
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        for i in 0..cx.n_cards as usize {
            if cx.cards[i].affliction == AFFLICTION_SMOG {
                cx.cards[i].affliction = 0;
                cx.cards[i].affliction_amount = 0;
            }
        }
    }
    fn should_play(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.cards[card as usize].affliction != AFFLICTION_SMOG
    }
});

// ShriekPower (TerrorEel): below Amount HP the Eel is stunned into TERROR_MOVE, once.
listener!(ShriekPower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if target == me.owner && unblocked > 0 && cx.cr(target).hp <= amount(cx, &me) {
            cx.stun(me.owner, Some("TERROR_MOVE"));
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
        cx.stun(me.owner, Some("SLASH_MOVE"));
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
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature != me.owner {
            return;
        }
        cx.set_max_and_current_hp(me.owner, 999_999_999);
        let node = cx.node_by_id(me.owner, "ABOUT_TO_BLOW_MOVE");
        cx.set_move_immediate(me.owner, node, true);
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
    fn should_creature_be_removed_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});
