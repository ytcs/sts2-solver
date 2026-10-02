//! Necrobinder Doom cards (and the Doom-flavoured powers' cards).

use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `PowerCmd.Apply<T>(Owner.Creature, amount, Owner.Creature, card)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

fn apply_doom(cx: &mut Combat, target: Cid, amount: i32, p: &CardPlay) {
    cx.apply_power(ids::power::DOOM_POWER, target, Dec::int(amount as i64), PLAYER, p.card);
}

fn attack(cx: &mut Combat, p: &CardPlay, t: Targeting) -> crate::engine::Results {
    let d = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, t))
}

// Deal damage; apply Doom equal to the (blocked + unblocked) damage dealt.
listener!(BlightStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let results = attack(cx, p, Targeting::Single(p.target));
        let total: i32 = results.iter().map(|r| r.total()).sum();
        apply_doom(cx, p.target, total, p);
        Flow::Done
    }
});

// Doom then Weak on every hittable enemy.
listener!(Deathbringer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let doom = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::DOOM_POWER, Dec::int(doom as i64), PLAYER, p.card);
        let weak = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(weak as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Block; again (Repeat) if you applied Doom this turn.
listener!(DeathsDoor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut gains = 1;
        if cx.necro.doom_applied_this_turn {
            gains += cx.card_var(p.card, VarKind::Repeat);
        }
        let b = cx.card_var(p.card, VarKind::Block);
        for _ in 0..gains {
            cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        }
        Flow::Done
    }
});

// Doom on every hittable enemy, then kill every doomed hittable enemy.
listener!(EndOfDays {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let doom = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        let targets = cx.hittable_enemies();
        for &t in targets.iter() {
            apply_doom(cx, t, doom, p);
        }
        let mut doomed: crate::util::ArrayVec<Cid, MAX_CREATURES> = crate::util::ArrayVec::new();
        for &t in cx.hittable_enemies().iter() {
            if cx.is_doomed(t) {
                doomed.push(t);
            }
        }
        cx.doom_kill(doomed.as_slice());
        Flow::Done
    }
});

listener!(NegativePulse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        let doom = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        let targets = cx.hittable_enemies();
        for &t in targets.iter() {
            apply_doom(cx, t, doom, p);
        }
        Flow::Done
    }
});

// Doom = CalcBase + CalcExtra x floor(target's Doom / DoomThreshold).
listener!(NoEscape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let have = cx.power_amount(p.target, ids::power::DOOM_POWER);
        let threshold = cx.card_named_var(p.card, var_name::DOOM_THRESHOLD).max(1);
        let n = have / threshold;
        let amt = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * n;
        apply_doom(cx, p.target, amt, p);
        Flow::Done
    }
});

listener!(Oblivion {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        cx.apply_power(ids::power::OBLIVION_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Scourge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::DOOM_POWER);
        apply_doom(cx, p.target, n, p);
        let c = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(c, false);
        Flow::Done
    }
});

// Damage = target's Doom amount.
listener!(TimesUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let doom = cx.power_amount(p.target, ids::power::DOOM_POWER);
        let d = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * doom;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(Countdown {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::COUNTDOWN_POWER);
        apply_self(cx, ids::power::COUNTDOWN_POWER, n, p);
        Flow::Done
    }
});

// Gain energy, draw, then gain the Neurosurge debuff (Doom on yourself each turn).
listener!(Neurosurge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let c = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(c, false);
        let n = cx.card_power_var(p.card, ids::power::NEUROSURGE_POWER);
        apply_self(cx, ids::power::NEUROSURGE_POWER, n, p);
        Flow::Done
    }
});

listener!(ReaperForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::REAPER_FORM_POWER, 1, p);
        Flow::Done
    }
});

listener!(Shroud {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Block);
        apply_self(cx, ids::power::SHROUD_POWER, n, p);
        Flow::Done
    }
});

listener!(Debilitate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let n = cx.card_power_var(p.card, ids::power::DEBILITATE_POWER);
        cx.apply_power(ids::power::DEBILITATE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Powers whose card applies a single self-power.
listener!(Calcify {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::CALCIFY_POWER);
        apply_self(cx, ids::power::CALCIFY_POWER, n, p);
        Flow::Done
    }
});

listener!(DanseMacabre {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::DANSE_MACABRE_POWER);
        apply_self(cx, ids::power::DANSE_MACABRE_POWER, n, p);
        Flow::Done
    }
});

listener!(Lethality {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::LETHALITY_POWER);
        apply_self(cx, ids::power::LETHALITY_POWER, n, p);
        Flow::Done
    }
});

listener!(SleightOfFlesh {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::SLEIGHT_OF_FLESH_POWER);
        apply_self(cx, ids::power::SLEIGHT_OF_FLESH_POWER, n, p);
        Flow::Done
    }
});

listener!(SentryMode {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::SENTRY_MODE_POWER);
        apply_self(cx, ids::power::SENTRY_MODE_POWER, n, p);
        Flow::Done
    }
});

listener!(DevourLife {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_power_var(p.card, ids::power::DEVOUR_LIFE_POWER);
        apply_self(cx, ids::power::DEVOUR_LIFE_POWER, n, p);
        Flow::Done
    }
});

listener!(Haunt {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::HpLoss);
        apply_self(cx, ids::power::HAUNT_POWER, n, p);
        Flow::Done
    }
});

listener!(Pagestorm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, ids::power::PAGESTORM_POWER, n, p);
        Flow::Done
    }
});

listener!(CallOfTheVoid {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, ids::power::CALL_OF_THE_VOID_POWER, n, p);
        Flow::Done
    }
});

listener!(Demesne {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, ids::power::DEMESNE_POWER, n, p);
        Flow::Done
    }
});

listener!(SpiritOfAsh {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::BLOCK_ON_EXHAUST);
        apply_self(cx, ids::power::SPIRIT_OF_ASH_POWER, n, p);
        Flow::Done
    }
});

listener!(ForbiddenGrimoire {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::FORBIDDEN_GRIMOIRE_POWER, 1, p);
        Flow::Done
    }
});

// Lose Strength, gain max energy.
listener!(Friendship {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        apply_self(cx, ids::power::STRENGTH_POWER, -s, p);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::FRIENDSHIP_POWER, e, p);
        Flow::Done
    }
});
