use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    #[inline(always)]
    pub fn rel(&self, me: Me) -> &Relic {
        &self.player.relics[me.idx as usize]
    }
    #[inline(always)]
    pub fn rel_mut(&mut self, me: Me) -> &mut Relic {
        &mut self.player.relics[me.idx as usize]
    }

    pub fn has_relic(&self, id: u16) -> bool {
        self.player.relics.iter().any(|r| r.id == id)
    }

    #[inline(always)]
    pub fn turn_number(&self) -> i32 {
        self.player.turn_number
    }

    pub fn is_owner_or_osty(&self, dealer: Cid) -> bool {
        if dealer == PLAYER {
            return true;
        }
        dealer != NO && {
            let d = self.cr(dealer);
            d.is_pet && d.owner == PLAYER && d.monster.id == crate::ids::monster::OSTY
        }
    }

    pub fn alive_enemies(&self) -> ArrayVec<Cid, MAX_CREATURES> {
        let mut o = ArrayVec::new();
        for &e in self.enemies.iter() {
            if self.cr(e).is_alive() {
                o.push(e);
            }
        }
        o
    }

    pub fn damage_hittable_enemies(&mut self, amount: i32, props: ValueProp) {
        let targets = self.hittable_enemies();
        if !targets.is_empty() {
            self.damage(targets.as_slice(), Dec::int(amount as i64), props, PLAYER, NO);
        }
    }

    pub fn damage_random_hittable_enemy(&mut self, amount: i32, props: ValueProp) {
        let targets = self.hittable_enemies();
        if targets.is_empty() {
            return;
        }
        let k = self.rng.combat_targets.next_int_range(0, targets.len() as i32) as usize;
        let t = targets[k];
        self.damage(&[t], Dec::int(amount as i64), props, PLAYER, NO);
    }

    pub fn has_potions(&self) -> bool {
        self.player.potions.iter().any(|p| p.is_some())
    }

    pub fn costs_energy_or_stars(&self, c: CardIdx) -> bool {
        let d = self.card_def(c);
        if !d.x_cost && self.card_cost(c, true) > 0 {
            return true;
        }
        self.card_star_cost(c) > 0
    }

    pub fn stable_shuffle_selection(&mut self, list: &mut [CardIdx]) {
        let cards = &self.cards;
        crate::sort::intro_sort(list, |a, b| {
            let (ca, cb) = (&cards[*a as usize], &cards[*b as usize]);
            if ca.id != cb.id {
                return if ca.id < cb.id { -1 } else { 1 };
            }
            (ca.upgrade as i32 - cb.upgrade as i32).signum()
        });
        self.rng.combat_card_selection.shuffle(list);
    }

    pub fn select_item(&mut self, items: &[CardIdx]) -> Option<CardIdx> {
        if items.is_empty() {
            return None;
        }
        let i = self.rng.combat_card_selection.next_int_range(0, items.len() as i32) as usize;
        Some(items[i])
    }

    pub fn all_cards(&self) -> ArrayVec<CardIdx, MAX_CARDS> {
        let mut o = ArrayVec::new();
        let p = &self.player;
        for pile in [&p.hand, &p.draw, &p.discard, &p.exhaust, &p.play] {
            for &c in pile.iter() {
                o.push(c);
            }
        }
        o
    }
}
