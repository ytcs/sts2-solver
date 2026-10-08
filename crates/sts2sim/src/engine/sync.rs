use crate::state::*;
use crate::dec::Dec;
use crate::util::ArrayVec;
use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct ObsCard {
    pub id: u16,
    pub upgrade: u8,
    pub cost: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HandSync {
    pub from_draw: u16,
    pub from_discard: u16,
    pub from_exhaust: u16,
    pub created: u16,
    pub returned: u16,
    pub cost_fixes: u16,
}

impl Combat {
    pub fn sync_hand(&mut self, obs: &[ObsCard]) -> HandSync {
        let mut rep = HandSync::default();
        let old: Vec<CardIdx> = self.player.hand.iter().copied().collect();
        let mut used = vec![false; old.len()];
        let mut slots: Vec<Option<CardIdx>> = vec![None; obs.len()];
        let mut missing: Vec<usize> = vec![];
        for (i, o) in obs.iter().enumerate() {
            let hit = old.iter().enumerate().position(|(j, &c)| !used[j] && self.cards[c as usize].id == o.id && self.cards[c as usize].upgrade == o.upgrade);
            match hit {
                Some(j) => {
                    used[j] = true;
                    slots[i] = Some(old[j]);
                }
                None => missing.push(i),
            }
        }
        for (j, &c) in old.iter().enumerate() {
            if !used[j] {
                self.player.hand.remove_value(c);
                let n = self.player.draw.len();
                let at = self.rng.shuffle.next_int((n + 1) as i32) as usize;
                self.player.draw.insert(at, c);
                self.cards[c as usize].pile = PileType::Draw as u8;
                rep.returned += 1;
            }
        }
        for i in missing {
            let o = obs[i];
            let mut found: Option<CardIdx> = None;
            for (pile, counter) in [(PileType::Draw, 0u8), (PileType::Discard, 1), (PileType::Exhaust, 2)] {
                let p = self.pile(pile);
                let cand = p.iter().copied().find(|&c| self.cards[c as usize].id == o.id && self.cards[c as usize].upgrade == o.upgrade);
                if let Some(c) = cand {
                    self.pile_mut(pile).remove_value(c);
                    match counter {
                        0 => rep.from_draw += 1,
                        1 => rep.from_discard += 1,
                        _ => rep.from_exhaust += 1,
                    }
                    found = Some(c);
                    break;
                }
            }
            if found.is_none() {
                found = self.new_card(o.id, o.upgrade);
                if found.is_some() {
                    rep.created += 1;
                }
            }
            slots[i] = found;
        }
        self.player.hand.clear();
        for c in slots.into_iter().flatten() {
            self.player.hand.push(c);
            self.cards[c as usize].pile = PileType::Hand as u8;
        }
        let hand: Vec<CardIdx> = self.player.hand.iter().copied().collect();
        for (c, o) in hand.iter().zip(obs.iter()) {
            if let Some(want) = o.cost {
                if want < 0 || self.card_def(*c).x_cost {
                    continue;
                }
                let have = self.card_cost(*c, true);
                if have != want {
                    let local = self.card_cost(*c, false);
                    let target = (local + want - have).max(0);
                    self.set_cost_this_combat(*c, target, false);
                    rep.cost_fixes += 1;
                }
            }
        }
        rep
    }

    pub fn sync_pile(&mut self, pile: PileType, obs: &[ObsCard]) -> HandSync {
        use std::collections::HashMap;
        let mut rep = HandSync::default();
        let mut want: HashMap<(u16, u8), i32> = HashMap::new();
        for o in obs {
            *want.entry((o.id, o.upgrade)).or_insert(0) += 1;
        }
        let current: Vec<CardIdx> = self.pile(pile).iter().copied().collect();
        for c in current {
            let key = (self.cards[c as usize].id, self.cards[c as usize].upgrade);
            match want.get_mut(&key) {
                Some(n) if *n > 0 => *n -= 1,
                _ => {
                    self.pile_mut(pile).remove_value(c);
                    let n = self.player.draw.len();
                let at = self.rng.shuffle.next_int((n + 1) as i32) as usize;
                self.player.draw.insert(at, c);
                self.cards[c as usize].pile = PileType::Draw as u8;
                    rep.returned += 1;
                }
            }
        }
        let other = if pile == PileType::Discard { PileType::Exhaust } else { PileType::Discard };
        for ((id, up), n) in want {
            for _ in 0..n.max(0) {
                let mut found: Option<CardIdx> = None;
                for src in [PileType::Draw, other] {
                    let cand = self.pile(src).iter().copied().find(|&c| self.cards[c as usize].id == id && self.cards[c as usize].upgrade == up);
                    if let Some(c) = cand {
                        self.pile_mut(src).remove_value(c);
                        if src == PileType::Draw { rep.from_draw += 1 } else if src == PileType::Discard { rep.from_discard += 1 } else { rep.from_exhaust += 1 }
                        found = Some(c);
                        break;
                    }
                }
                if found.is_none() {
                    found = self.new_card(id, up);
                    if found.is_some() {
                        rep.created += 1;
                    }
                }
                if let Some(c) = found {
                    self.pile_mut(pile).push(c);
                    self.cards[c as usize].pile = pile as u8;
                }
            }
        }
        rep
    }

    // Call after the hand and the visible piles are synced: the draw pile is what is left.
    pub fn sync_draw(&mut self, obs: &[ObsCard]) -> HandSync {
        use std::collections::HashMap;
        let mut rep = HandSync::default();
        let mut want: HashMap<(u16, u8), i32> = HashMap::new();
        for o in obs {
            *want.entry((o.id, o.upgrade)).or_insert(0) += 1;
        }
        let current: Vec<CardIdx> = self.player.draw.iter().copied().collect();
        let mut surplus: Vec<CardIdx> = vec![];
        for c in current {
            let key = (self.cards[c as usize].id, self.cards[c as usize].upgrade);
            match want.get_mut(&key) {
                Some(n) if *n > 0 => *n -= 1,
                _ => surplus.push(c),
            }
        }
        for ((id, up), n) in want {
            for _ in 0..n.max(0) {
                if let Some(k) = surplus.iter().position(|&c| self.cards[c as usize].id == id) {
                    let c = surplus.remove(k);
                    self.player.draw.remove_value(c);
                    self.cards[c as usize].pile = PileType::None as u8;
                    self.cards[c as usize].flags |= cflag::REMOVED;
                }
                if let Some(c) = self.new_card(id, up) {
                    let len = self.player.draw.len();
                    let at = self.rng.shuffle.next_int((len + 1) as i32) as usize;
                    self.player.draw.insert(at, c);
                    self.cards[c as usize].pile = PileType::Draw as u8;
                    rep.created += 1;
                }
            }
        }
        for c in surplus {
            self.player.draw.remove_value(c);
            self.cards[c as usize].pile = PileType::None as u8;
            self.cards[c as usize].flags |= cflag::REMOVED;
            rep.returned += 1;
        }
        rep
    }

    pub fn sync_powers(&mut self, cid: Cid, obs: &[(u16, i32)]) -> u16 {
        let mut changed = 0u16;
        let mut left: Vec<Option<(u16, i32)>> = obs.iter().map(|&o| Some(o)).collect();
        let current: Vec<(u16, u16, i32)> = self.cr(cid).powers.iter().map(|p| (p.uid, p.id, p.amount)).collect();
        for (uid, id, amount) in current {
            let hit = left.iter().position(|o| matches!(o, Some((oid, _)) if *oid == id));
            match hit {
                Some(k) => {
                    let (_, want) = left[k].take().unwrap();
                    if want != amount {
                        if let Some(i) = self.power_idx(cid, uid) {
                            self.cr_mut(cid).powers[i].amount = want;
                            changed += 1;
                        }
                    }
                }
                None => {
                    if let Some(i) = self.power_idx(cid, uid) {
                        self.cr_mut(cid).powers.remove(i);
                        self.sync_secondary(cid);
                        changed += 1;
                    }
                }
            }
        }
        for (id, amount) in left.into_iter().flatten() {
            if amount != 0 && self.apply_power(id, cid, Dec::int(amount as i64), cid, NO).is_some() {
                changed += 1;
            }
        }
        changed
    }

    pub fn sync_turn_start(&mut self, cid: Cid, obs: &[(u16, i32)]) {
        let mut left: Vec<Option<(u16, i32)>> = obs.iter().map(|&o| Some(o)).collect();
        let ids: Vec<(u16, u16)> = self.cr(cid).powers.iter().map(|p| (p.uid, p.id)).collect();
        for (uid, id) in ids {
            if let Some(k) = left.iter().position(|o| matches!(o, Some((oid, _)) if *oid == id)) {
                let (_, v) = left[k].take().unwrap();
                if let Some(i) = self.power_idx(cid, uid) {
                    self.cr_mut(cid).powers[i].amount_on_turn_start = v;
                }
            }
        }
    }

    pub fn sync_creature(&mut self, cid: Cid, hp: i32, max_hp: i32, block: i32) {
        let c = self.cr_mut(cid);
        c.hp = hp;
        c.max_hp = max_hp;
        c.block = block;
    }

    pub fn sync_energy(&mut self, energy: i32, stars: i32) {
        self.player.energy = energy;
        self.player.stars = stars;
    }

    pub fn sync_enemies(&mut self, obs: &[ObsEnemy]) -> EnemySync {
        let mut rep = EnemySync::default();
        let mut pool: Vec<Cid> = self.enemies.iter().copied().collect();
        for c in 1..MAX_CREATURES as Cid {
            let cr = self.cr(c);
            if cr.active && cr.side == Side::Enemy && !cr.is_player && !cr.is_pet && !pool.contains(&c) {
                pool.push(c);
            }
        }
        let mut used = vec![false; pool.len()];
        for o in obs {
            let listed = |c: Cid| self.enemies.contains(c);
            let key = |j: usize| {
                let c = pool[j];
                let alive_now = listed(c) && self.cr(c).is_alive();
                ((alive_now != o.alive) as u8, !listed(c) as u8, (self.cr(c).hp - o.hp).abs(), j)
            };
            let pick = (0..pool.len()).filter(|&j| !used[j] && self.cr(pool[j]).monster.id == o.monster).min_by_key(|&j| key(j));
            if let Some(j) = pick {
                used[j] = true;
            }
            rep.pairs.push(pick.map(|j| pool[j]));
        }
        let n_listed = self.enemies.len();
        for (k, p) in rep.pairs.iter_mut().enumerate() {
            if p.is_none() {
                rep.missing += 1;
                if let Some(j) = (k..n_listed).chain(0..k.min(n_listed)).find(|&j| !used[j]) {
                    used[j] = true;
                    *p = Some(pool[j]);
                }
            }
        }
        for (j, &c) in pool.iter().enumerate() {
            if !used[j] && self.enemies.contains(c) {
                self.detach_creature(c);
                if self.cr(c).is_alive() {
                    rep.removed += 1;
                }
            }
        }
        self.enemies.clear();
        for (o, c) in obs.iter().zip(rep.pairs.iter()) {
            let Some(c) = *c else { continue };
            let cr = self.cr_mut(c);
            if o.alive && (!cr.in_combat || cr.hp <= 0) {
                rep.revived += 1;
            }
            cr.in_combat |= o.alive;
            cr.hp = o.hp;
            cr.max_hp = o.max_hp;
            cr.block = o.block;
            self.enemies.push(c);
        }
        rep
    }

    pub fn sync_options(&mut self, want: &[(u16, u8)]) -> bool {
        if !matches!(self.decision, Some(Decision { source: DecisionSource::Options, .. })) || self.replay.is_some() {
            return false;
        }
        let mut cands: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &(id, up) in want {
            match self.new_card(id, up) {
                Some(c) => cands.push(c),
                None => return false,
            }
        }
        let d = self.decision.as_mut().expect("pending decision");
        d.cands = cands;
        d.selected.clear();
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObsEnemy {
    pub monster: u16,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub alive: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EnemySync {
    pub pairs: Vec<Option<Cid>>,
    pub revived: u16,
    pub removed: u16,
    pub missing: u16,
}

#[derive(Clone, Debug, Default)]
pub struct ObsRelic {
    pub id: u16,
    pub props: Vec<(String, i32)>,
    pub counter: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct RelicSync {
    pub changed: u16,
    pub unpaired: u16,
    pub unknown_props: Vec<String>,
    pub counter_mismatch: Vec<u16>,
}

impl Combat {
    pub fn sync_relics(&mut self, obs: &[ObsRelic]) -> RelicSync {
        let mut rep = RelicSync::default();
        let mut used = [false; MAX_RELICS];
        for o in obs {
            let Some(i) = (0..self.player.relics.len()).find(|&i| !used[i] && self.player.relics[i].id == o.id) else {
                rep.unpaired += 1;
                continue;
            };
            used[i] = true;
            let l = crate::content::relic_listener(o.id);
            let defs = l.meta_props();
            let before = self.player.relics[i];
            let mut r = before;
            if defs.iter().any(|d| d.lit.is_empty()) {
                for (name, v) in &o.props {
                    if !r.set_prop(defs, name, *v) {
                        rep.unknown_props.push(format!("{}.{name}", crate::ids::relic::NAMES[o.id as usize]));
                    }
                }
                for d in defs.iter().filter(|d| d.lit.is_empty() && d.skip_default) {
                    if !o.props.iter().any(|(n, _)| n == d.name) {
                        r.set(d.slot, 0);
                    }
                }
            } else if let Some(c) = o.counter {
                if l.meta_display(self, &r) != Some(c) {
                    r.counter = c;
                    if l.meta_display(self, &r) != Some(c) {
                        r = before;
                        rep.counter_mismatch.push(o.id);
                    }
                }
            }
            if (r.counter, r.flags, r.aux) != (before.counter, before.flags, before.aux) {
                self.player.relics[i] = r;
                rep.changed += 1;
            }
        }
        rep
    }
}
