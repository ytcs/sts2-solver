use crate::content;
use crate::defs::*;
use crate::hooks::{hookbit, Kind, Me};
use crate::state::*;
use crate::types::*;

pub const STUN_NODE: u8 = 0xFE;

pub static STUN_INTENTS: [Intent; 1] = [Intent::Stun];

impl Combat {
    fn alloc_slot(&self) -> Option<Cid> {
        (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).active).or_else(|| (1..MAX_CREATURES as u8).find(|&i| !self.cr(i).in_combat))
    }

    pub fn add_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        self.add_enemy_v(monster_id, slot, [0, 0])
    }

    pub fn add_enemy_v(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let cid = self.create_enemy_v(monster_id, slot, vars)?;
        self.attach_enemy(cid);
        Some(cid)
    }

    pub fn create_enemy(&mut self, monster_id: u16, slot: u8) -> Option<Cid> {
        self.create_enemy_v(monster_id, slot, [0, 0])
    }

    pub fn create_enemy_v(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        if !content::monster_implemented(monster_id) {
            self.flag_missing(Kind::Monster, monster_id);
            return None;
        }
        let Some(cid) = self.alloc_slot() else {
            self.overflow |= ov::CREATURES;
            return None;
        };
        let def = content::monster_def(monster_id);
        let (mut lo, mut hi) = (def.hp)(self.ascension);
        let bonus = content::monster_hp_bonus(monster_id, vars);
        lo += bonus;
        hi += bonus;
        let mut cands: crate::util::ArrayVec<i32, 64> = crate::util::ArrayVec::new();
        for hp in lo..=hi {
            if !self.enemies.iter().any(|&e| self.cr(e).max_hp == hp) {
                cands.push(hp);
            }
        }
        let hp = if cands.is_empty() {
            self.rng.niche.next_int_range(lo, hi + 1)
        } else {
            cands[self.rng.niche.next_int_range(0, cands.len() as i32) as usize]
        };
        self.listen |= content::monster_mask(monster_id);
        let mut ms = MonsterState { id: monster_id, cur_state: def.initial, spawned_this_turn: true, ..Default::default() };
        ms.vars[0] = vars[0];
        ms.vars[1] = vars[1];
        let mut cr = Creature::default();
        cr.active = true;
        cr.in_combat = true;
        cr.side = Side::Enemy;
        cr.set_hp(hp);
        cr.max_hp = hp;
        cr.slot = slot;
        cr.monster = ms;
        self.creatures[cid as usize] = cr;
        Some(cid)
    }

    pub fn attach_enemy(&mut self, cid: Cid) {
        let def = content::monster_def(self.cr(cid).monster.id);
        let slot = self.cr(cid).slot;
        self.enemies.push(cid);
        if matches!(def.nodes[def.initial as usize], MonsterNode::Move { .. }) {
            self.log_move(cid, def.initial);
        }
        if slot != NO {
            self.sort_enemies_by_slot();
        }
    }

    pub fn summon_enemy(&mut self, monster_id: u16, slot: u8, vars: [i32; 2]) -> Option<Cid> {
        let c = self.create_enemy_v(monster_id, slot, vars)?;
        self.attach_enemy(c);
        self.after_enemy_added(c);
        Some(c)
    }

    pub fn after_enemy_added(&mut self, c: Cid) {
        let def = content::monster_def(self.cr(c).monster.id);
        if let Some(f) = def.on_spawn {
            f(self, c);
        }
        if self.side == Side::Player {
            self.roll_move(c);
        }
        self.dispatch_u(hookbit::after_creature_added_to_combat, |cx, me, l| l.after_creature_added_to_combat(cx, me, c));
    }

    pub fn next_free_slot(&self, n_slots: u8) -> u8 {
        (0..n_slots).find(|&s| !self.enemies.iter().any(|&e| self.cr(e).slot == s)).unwrap_or(NO)
    }

    pub fn last_free_slot(&self, n_slots: u8) -> u8 {
        (0..n_slots).rev().find(|&s| !self.enemies.iter().any(|&e| self.cr(e).slot == s)).unwrap_or(NO)
    }

    pub fn sort_enemies_by_slot(&mut self) {
        let n = self.enemies.len();
        for i in 1..n {
            let x = self.enemies[i];
            let kx = self.slot_key(x);
            let mut j = i;
            while j > 0 && self.slot_key(self.enemies[j - 1]) > kx {
                self.enemies[j] = self.enemies[j - 1];
                j -= 1;
            }
            self.enemies[j] = x;
        }
    }
    fn slot_key(&self, c: Cid) -> i32 {
        let s = self.cr(c).slot;
        if s == NO { -1 } else { s as i32 }
    }

    fn log_move(&mut self, c: Cid, node: u8) {
        let ms = &mut self.creatures[c as usize].monster;
        ms.log[(ms.log_len & 7) as usize] = node;
        ms.log_len += 1;
        ms.ever_logged |= 1u64 << node;
    }

    pub fn last_logged_move(&self, c: Cid) -> u8 {
        let ms = &self.cr(c).monster;
        if ms.log_len == 0 { NO } else { ms.log[((ms.log_len - 1) & 7) as usize] }
    }

    #[inline]
    fn node(&self, c: Cid, n: u8) -> &'static MonsterNode {
        &content::monster_def(self.cr(c).monster.id).nodes[n as usize]
    }

    fn node_is_move(&self, c: Cid, n: u8) -> bool {
        n == STUN_NODE || matches!(self.node(c, n), MonsterNode::Move { .. })
    }

    pub fn can_transition_away(&self, c: Cid, n: u8) -> bool {
        if n == STUN_NODE {
            return self.cr(c).monster.stun_performed;
        }
        match self.node(c, n) {
            MonsterNode::Move { must_perform_once, .. } => !*must_perform_once || self.cr(c).monster.performed_once >> n & 1 != 0,
            _ => true,
        }
    }

    pub fn move_view(&self, c: Cid) -> Option<(&'static str, &'static [Intent])> {
        let nm = self.cr(c).monster.next_move;
        if nm == NO {
            return None;
        }
        if nm == STUN_NODE {
            return Some(("STUNNED", &STUN_INTENTS));
        }
        match self.node(c, nm) {
            MonsterNode::Move { id, intents, .. } => Some((id, intents)),
            _ => None,
        }
    }

    pub fn is_stunned(&self, c: Cid) -> bool {
        self.cr(c).monster.next_move == STUN_NODE
    }

    fn branch_weight(&self, c: Cid, b: &Branch) -> f32 {
        self.branch_weight_ms(c, &self.cr(c).monster, b)
    }

    fn branch_weight_ms(&self, c: Cid, ms: &MonsterState, b: &Branch) -> f32 {
        let n = ms.log_len as usize;
        let mult: f32 = match b.repeat {
            Repeat::CanRepeatForever => 1.0,
            Repeat::UseOnlyOnce => {
                if ms.ever_logged >> b.target & 1 != 0 { 0.0 } else { 1.0 }
            }
            Repeat::CanRepeatXTimes(k) => {
                let k = k as usize;
                if n < k {
                    1.0
                } else {
                    let mut any_diff = false;
                    for i in 0..k {
                        if ms.log[(n - 1 - i) & 7] != b.target {
                            any_diff = true;
                            break;
                        }
                    }
                    if any_diff { 1.0 } else { 0.0 }
                }
            }
        };
        if b.cooldown > 0 {
            let mut hit = false;
            for i in 0..(b.cooldown as usize).min(n) {
                if ms.log[(n - 1 - i) & 7] == b.target {
                    hit = true;
                    break;
                }
            }
            if hit {
                return 0.0;
            }
        }
        let w = match b.weight_fn {
            Some(f) => f(self, c),
            None => b.weight,
        };
        mult * w
    }

    fn next_state(&mut self, c: Cid, cur: u8) -> u8 {
        let def = content::monster_def(self.cr(c).monster.id);
        if cur == STUN_NODE {
            let f = self.cr(c).monster.stun_follow_up;
            return if f == NO { def.initial } else { f };
        }
        match &def.nodes[cur as usize] {
            MonsterNode::Move { follow_up, .. } => {
                if *follow_up == NO {
                    def.initial
                } else if *follow_up == crate::defs::FOLLOW_STORED {
                    self.cr(c).monster.stun_follow_up
                } else {
                    *follow_up
                }
            }
            MonsterNode::Random { branches, .. } => {
                let mut ws = [0f32; 12];
                let mut sum = 0f64;
                for (i, b) in branches.iter().enumerate() {
                    ws[i] = self.branch_weight(c, b);
                    sum += ws[i] as f64;
                }
                let max = sum as f32;
                let mut r = self.rng.monster_ai.next_float_max(max);
                for (i, b) in branches.iter().enumerate() {
                    r -= ws[i];
                    if r <= 0.0 {
                        return b.target;
                    }
                }
                // Deviation: the game throws on this f32 remainder; it goes to the last branch with positive weight.
                let last = (0..branches.len()).rev().find(|&i| ws[i] > 0.0).unwrap_or(branches.len() - 1);
                branches[last].target
            }
            MonsterNode::Cond { arms, .. } => {
                for (target, pred) in arms.iter() {
                    if pred(self, c) {
                        return *target;
                    }
                }
                panic!("ConditionalBranchState: no arm true");
            }
        }
    }

    fn on_exit_state(&mut self, c: Cid, n: u8) {
        let ms = &mut self.creatures[c as usize].monster;
        if n == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << n);
        }
    }

    pub fn prepare_for_next_turn(&mut self, c: Cid) {
        if self.cr(c).in_combat {
            self.roll_move(c);
        }
    }

    pub fn roll_move(&mut self, c: Cid) {
        let cur = self.cr(c).monster.cur_state;
        let performed_first = self.cr(c).monster.performed_first;
        if !self.can_transition_away(c, cur) || (!performed_first && self.node_is_move(c, cur)) {
            self.creatures[c as usize].monster.next_move = cur;
            return;
        }
        let mut cur = cur;
        let mut first_logged = NO;
        loop {
            if !self.tick() {
                return;
            }
            let nxt = self.next_state(c, cur);
            self.on_exit_state(c, cur);
            cur = nxt;
            self.creatures[c as usize].monster.cur_state = cur;
            if first_logged == NO && self.node_is_move(c, cur) {
                first_logged = cur;
            }
            if self.node_is_move(c, cur) {
                break;
            }
        }
        self.log_move(c, first_logged);
        self.creatures[c as usize].monster.next_move = cur;
    }

    pub fn set_move_immediate(&mut self, c: Cid, node: u8, force: bool) {
        let nm = self.cr(c).monster.next_move;
        let can = nm == NO || self.can_transition_away(c, nm);
        if can || force {
            let cur = self.cr(c).monster.cur_state;
            self.on_exit_state(c, cur);
            let ms = &mut self.creatures[c as usize].monster;
            ms.next_move = node;
            ms.cur_state = node;
        }
    }

    pub fn stun(&mut self, c: Cid, stun_move: Option<MoveFn>, next_move: Option<u8>) {
        if self.cr(c).is_player || !self.cr(c).in_combat || self.cr(c).is_dead() {
            return;
        }
        let follow = match next_move {
            Some(n) => n,
            None => {
                let l = self.last_logged_move(c);
                assert!(l != NO, "StateLog is empty");
                l
            }
        };
        let nm = self.cr(c).monster.next_move;
        if nm == NO || self.can_transition_away(c, nm) {
            let ms = &mut self.creatures[c as usize].monster;
            ms.stun_follow_up = follow;
            ms.stun_move = stun_move;
            ms.stun_performed = false;
            self.set_move_immediate(c, STUN_NODE, false);
        }
    }

    pub fn perform_move(&mut self, c: Cid) -> Option<u8> {
        let def = content::monster_def(self.cr(c).monster.id);
        let nm = self.cr(c).monster.next_move;
        assert!(nm != NO, "monster performing UNSET_MOVE");
        self.creatures[c as usize].monster.is_performing = true;
        if nm == STUN_NODE {
            let f = self.cr(c).monster.stun_move;
            self.creatures[c as usize].monster.stun_performed = true;
            if let Some(f) = f {
                f(self, c);
            }
        } else if let MonsterNode::Move { perform, .. } = &def.nodes[nm as usize] {
            self.creatures[c as usize].monster.performed_once |= 1u64 << nm;
            perform(self, c);
        }
        if self.stage == Stage::AwaitChoice {
            return Some(nm);
        }
        self.finish_move(c, nm);
        None
    }

    pub(crate) fn finish_move(&mut self, c: Cid, nm: u8) {
        {
            let ms = &mut self.creatures[c as usize].monster;
            ms.performed.copy_within(1..4, 0);
            ms.performed[3] = nm;
            ms.performed_first = true;
        }
        self.hist_push(crate::engine::HKind::MonsterPerformedMove, c, NO, nm as u16, NO, 0, 0, 0, 0);
        self.creatures[c as usize].monster.is_performing = false;
        if self.cr(c).is_dead() && self.cr(c).in_combat && self.should_creature_be_removed_after_death(c) {
            self.detach_creature(c);
        }
    }

    pub fn monster_me(&self, c: Cid) -> Me {
        Me { kind: Kind::Monster, owner: c, idx: 0, id: self.cr(c).monster.id, amount: 0 }
    }
}

pub const LOOK_H: usize = 4;
pub const LOOK_NODES: usize = 16;
const LOOK_PATHS: usize = 8;
const LOOK_JOINT: &[u16] = &[crate::ids::monster::TWO_TAILED_RAT];
// Projections run on LOOK_SEED streams: the real random state is never read.
const LOOK_SEED: u64 = 0x10_0CA4_EAD;

#[derive(Clone, Copy)]
pub struct LookRow {
    pub prob: [f32; LOOK_NODES],
    pub exp_damage: f32,
}

const EMPTY_ROW: LookRow = LookRow { prob: [0.0; LOOK_NODES], exp_damage: 0.0 };

type Outcomes = crate::util::ArrayVec<(MonsterState, f32), 48>;
type LookList = crate::util::ArrayVec<(u8, f32, f32), 20>;

macro_rules! lprof {
    ($k:expr, $body:expr) => {{
        #[cfg(feature = "obs_prof")]
        let t = unsafe { core::arch::x86_64::_rdtsc() };
        let r = $body;
        #[cfg(feature = "obs_prof")]
        unsafe {
            crate::observe::OBS_PROF[$k] += core::arch::x86_64::_rdtsc() - t;
        }
        r
    }};
}
macro_rules! lcount {
    ($k:expr, $n:expr) => {{
        #[cfg(feature = "obs_prof")]
        unsafe {
            crate::observe::OBS_PROF[$k] += $n as u64;
        }
    }};
}

impl Combat {
    fn look_enter(&self, c: Cid, mut ms: MonsterState, left: u8, to: u8, first: u8, p: f32, out: &mut Outcomes) {
        if left == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << left);
        }
        ms.cur_state = to;
        let is_move = self.node_is_move(c, to);
        let first = if first == NO && is_move { to } else { first };
        if is_move {
            ms.log[(ms.log_len & 7) as usize] = first;
            ms.log_len += 1;
            if first < 64 {
                ms.ever_logged |= 1u64 << first;
            }
            ms.next_move = to;
            if out.len() < 48 {
                out.push((ms, p));
            }
            return;
        }
        let def = content::monster_def(ms.id);
        match &def.nodes[to as usize] {
            MonsterNode::Cond { arms, .. } => {
                for (target, pred) in arms.iter() {
                    if pred(self, c) {
                        self.look_enter(c, ms, to, *target, first, p, out);
                        return;
                    }
                }
            }
            MonsterNode::Random { branches, .. } => {
                let mut ws = [0f32; 12];
                let mut sum = 0f64;
                for (i, b) in branches.iter().enumerate().take(12) {
                    ws[i] = self.branch_weight_ms(c, &ms, b);
                    sum += ws[i] as f64;
                }
                if sum <= 0.0 {
                    if let Some(b) = branches.first() {
                        self.look_enter(c, ms, to, b.target, first, p, out);
                    }
                    return;
                }
                for (i, b) in branches.iter().enumerate().take(12) {
                    if ws[i] > 0.0 {
                        self.look_enter(c, ms, to, b.target, first, p * (ws[i] as f64 / sum) as f32, out);
                    }
                }
            }
            MonsterNode::Move { .. } => {}
        }
    }

    fn look_outcomes(&self, c: Cid, out: &mut Outcomes) {
        let ms = self.cr(c).monster;
        let cur = ms.cur_state;
        if !self.can_transition_away(c, cur) || (!ms.performed_first && self.node_is_move(c, cur)) {
            out.push((MonsterState { next_move: cur, ..ms }, 1.0));
            return;
        }
        let def = content::monster_def(ms.id);
        let nxt = if cur == STUN_NODE {
            if ms.stun_follow_up == NO { def.initial } else { ms.stun_follow_up }
        } else {
            match &def.nodes[cur as usize] {
                MonsterNode::Move { follow_up, .. } if *follow_up == NO => def.initial,
                MonsterNode::Move { follow_up, .. } if *follow_up == crate::defs::FOLLOW_STORED => ms.stun_follow_up,
                MonsterNode::Move { follow_up, .. } => *follow_up,
                _ => cur,
            }
        };
        self.look_enter(c, ms, cur, nxt, NO, 1.0, out);
    }

    fn look_roll_one(mut self: Box<Self>, i: usize, fork: &impl Fn(&Combat, Cid) -> bool, p: f32, out: &mut Vec<(Box<Combat>, f32)>, random: &mut u16) {
        let Some(e) = self.enemies.get(i) else { return out.push((self, p)) };
        let mut outs = Outcomes::new();
        if self.cr(e).in_combat {
            self.look_outcomes(e, &mut outs);
        }
        if outs.len() > 1 {
            *random |= 1 << e;
        }
        if outs.len() <= 1 || !fork(&self, e) {
            if let Some(best) = outs.iter().fold(None, |b: Option<(MonsterState, f32)>, &o| if b.is_some_and(|b| b.1 >= o.1) { b } else { Some(o) }) {
                self.creatures[e as usize].monster = best.0;
            }
            return out.push((self, p));
        }
        let n = outs.len();
        lcount!(20, n - 1);
        for &(m, q) in outs.iter().take(n - 1) {
            let mut cx = look_box(&self);
            cx.creatures[e as usize].monster = m;
            out.push((cx, p * q));
        }
        self.creatures[e as usize].monster = outs[n - 1].0;
        out.push((self, p * outs[n - 1].1));
    }

    fn look_project(&self, who: &[Cid], fork: impl Fn(&Combat, Cid) -> bool) -> (crate::util::ArrayVec<[LookList; LOOK_H], MAX_CREATURES>, u16) {
        let mut random = 0u16;
        let mut lists = crate::util::ArrayVec::<[LookList; LOOK_H], MAX_CREATURES>::new();
        let mut ids = [0u16; MAX_CREATURES];
        for (k, &c) in who.iter().enumerate() {
            lists.push(core::array::from_fn(|_| LookList::new()));
            ids[k] = self.cr(c).monster.id;
        }
        lcount!(19, 1);
        crate::util::quiet(|| {
            let mut base = lprof!(14, look_box(self));
            base.rng = *look_rng();
            base.auto_select = true;
            base.replay = None;
            base.player_hooks_active = false;
            let pl = &mut base.creatures[PLAYER as usize];
            pl.set_hp(1 << 24);
            pl.max_hp = 1 << 24;
            pl.set_block(0);
            pl.powers.clear();
            for cr in base.creatures.iter_mut().filter(|cr| cr.active && !cr.is_player) {
                cr.pristine = PRISTINE_HP | PRISTINE_BLOCK;
            }
            let [mut paths, mut next, mut rolled] = LOOK_VECS.with(|v| core::mem::take(&mut *v.borrow_mut()));
            paths.push((base, 1.0));
            for h in 0..LOOK_H {
                lcount!(21, paths.len());
                lprof!(15, for (mut cx, p) in paths.drain(..) {
                    if cx.look_turn() {
                        next.push((cx, p));
                    } else {
                        look_free(cx);
                    }
                });
                let n_roll = next.iter().map(|(cx, _)| cx.enemies.len()).max().unwrap_or(0);
                for i in 0..n_roll {
                    lprof!(16, for (cx, p) in next.drain(..) {
                        cx.look_roll_one(i, &fork, p, &mut rolled, &mut random);
                    });
                    core::mem::swap(&mut next, &mut rolled);
                    if next.len() > LOOK_PATHS || (i + 1 == n_roll && next.len() > 1) {
                        lprof!(17, look_merge(&mut next));
                    }
                }
                lprof!(18, for (cx, p) in next.iter_mut() {
                    let mut inert = crate::util::ArrayVec::new();
                    inert.copy_from(&cx.creatures[PLAYER as usize].powers);
                    cx.creatures[PLAYER as usize].powers.copy_from(&self.cr(PLAYER).powers);
                    cx.player_hooks_active = self.player_hooks_active;
                    for (k, &f) in who.iter().enumerate() {
                        let cr = cx.cr(f);
                        let node = cr.monster.next_move;
                        if !cr.is_alive() || !cr.in_combat || cr.monster.id != ids[k] || node == NO {
                            continue;
                        }
                        let pd = *p * cx.node_attack_damage(f, node);
                        let list = &mut lists[k][h];
                        match list.as_mut_slice().iter_mut().find(|e| e.0 == node) {
                            Some(e) => {
                                e.1 += *p;
                                e.2 += pd;
                            }
                            None => list.push((node, *p, pd)),
                        }
                    }
                    cx.creatures[PLAYER as usize].powers.copy_from(&inert);
                    cx.player_hooks_active = false;
                });
                core::mem::swap(&mut paths, &mut next);
                if paths.is_empty() {
                    break;
                }
            }
            for (cx, _) in paths.drain(..) {
                look_free(cx);
            }
            for (cx, _) in next.drain(..).chain(rolled.drain(..)) {
                look_free(cx);
            }
            LOOK_VECS.with(|v| *v.borrow_mut() = [paths, next, rolled]);
        });
        (lists, random)
    }

    fn look_project_one(&self, f: Cid) -> [LookList; LOOK_H] {
        self.look_project(&[f], look_fork(self, f)).0[0]
    }

    pub fn intent_plan(&self, c: Cid) -> Vec<Vec<(String, f32, String)>> {
        let cr = self.cr(c);
        if !cr.is_alive() || !cr.in_combat || cr.monster.next_move == NO || cr.is_player {
            return Vec::new();
        }
        let def = content::monster_def(cr.monster.id);
        self.look_project_one(c)
            .iter()
            .map(|list| {
                let mut acc: Vec<(u8, f32, f32)> = list.iter().copied().collect();
                acc.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
                acc.into_iter()
                    .map(|(node, p, pd)| {
                        if node == STUN_NODE {
                            return ("STUNNED".to_string(), p, "stunned".to_string());
                        }
                        let MonsterNode::Move { id, intents, .. } = &def.nodes[node as usize] else { return ("?".to_string(), p, String::new()) };
                        let parts: Vec<String> = intents
                            .iter()
                            .map(|it| match it {
                                Intent::Attack { damage, hits } => {
                                    let (d, h) = (self.intent_damage(c, damage(self, c)), hits(self, c));
                                    if h > 1 { format!("{d}x{h}") } else { format!("{d}") }
                                }
                                Intent::DeathBlowAttack { damage } => format!("{} (dies)", self.intent_damage(c, damage(self, c))),
                                Intent::StatusCard => "status".into(),
                                Intent::CardDebuff => "card debuff".into(),
                                Intent::Buff => "buff".into(),
                                Intent::Debuff => "debuff".into(),
                                Intent::DebuffStrong => "strong debuff".into(),
                                Intent::Defend => "block".into(),
                                Intent::Escape => "escape".into(),
                                Intent::Heal => "heal".into(),
                                Intent::Hidden => "hidden".into(),
                                Intent::Summon => "summon".into(),
                                Intent::Sleep => "sleep".into(),
                                Intent::Stun => "stun".into(),
                                Intent::DeathBlow => "death blow".into(),
                            })
                            .collect();
                        let mut text = parts.join(" + ");
                        let projected = if p > 0.0 { pd / p } else { 0.0 };
                        if (projected - self.node_attack_damage(c, node)).abs() >= 0.5 {
                            text = format!("{text} (projected {projected:.0})");
                        }
                        let fx = self.move_effects(c, node);
                        if !fx.is_empty() {
                            text = format!("{text} [{fx}]");
                        }
                        (id.trim_end_matches("_MOVE").to_string(), p, text)
                    })
                    .collect()
            })
            .collect()
    }

    pub fn move_effects(&self, c: Cid, node: u8) -> String {
        use std::collections::BTreeMap;
        let mut cx = self.clone();
        cx.budget_reset();
        cx.rng = *look_rng();
        cx.creatures[PLAYER as usize].set_block(1 << 20);
        cx.creatures[c as usize].monster.next_move = node;
        let powers = |cx: &Combat, who: Cid| -> BTreeMap<u16, i32> {
            let mut m = BTreeMap::new();
            for p in cx.cr(who).powers.iter() {
                *m.entry(p.id).or_insert(0) += p.amount;
            }
            m
        };
        let piles = |cx: &Combat| -> [BTreeMap<u16, i32>; 4] {
            let mut out: [BTreeMap<u16, i32>; 4] = Default::default();
            for (k, pile) in [&cx.player.hand, &cx.player.draw, &cx.player.discard, &cx.player.exhaust].iter().enumerate() {
                for &ci in pile.iter() {
                    *out[k].entry(cx.cards[ci as usize].id).or_insert(0) += 1;
                }
            }
            out
        };
        let (me0, self0, piles0, blk0) = (powers(&cx, PLAYER), powers(&cx, c), piles(&cx), cx.cr(c).block());
        let alive0 = cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).count();
        let _ = cx.perform_move(c);
        let (me1, self1, piles1, blk1) = (powers(&cx, PLAYER), powers(&cx, c), piles(&cx), cx.cr(c).block());
        let alive1 = cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).count();
        let pdiff = |a: &BTreeMap<u16, i32>, b: &BTreeMap<u16, i32>| -> Vec<String> {
            let keys: std::collections::BTreeSet<u16> = a.keys().chain(b.keys()).copied().collect();
            keys.into_iter()
                .filter_map(|k| {
                    let d = b.get(&k).copied().unwrap_or(0) - a.get(&k).copied().unwrap_or(0);
                    (d != 0).then(|| format!("{} {:+}", crate::ids::power::NAMES[k as usize].trim_end_matches("_POWER"), d))
                })
                .collect()
        };
        let mut out = Vec::new();
        let me = pdiff(&me0, &me1);
        if !me.is_empty() {
            out.push(format!("me: {}", me.join(", ")));
        }
        for (k, name) in ["hand", "draw", "discard", "exhaust"].iter().enumerate() {
            let added: Vec<String> = piles1[k]
                .iter()
                .filter_map(|(&id, &n)| {
                    let d = n - piles0[k].get(&id).copied().unwrap_or(0);
                    (d > 0).then(|| format!("+{d} {}", crate::ids::card::NAMES[id as usize]))
                })
                .collect();
            if !added.is_empty() {
                out.push(format!("{name}: {}", added.join(", ")));
            }
        }
        let mut me_self = pdiff(&self0, &self1);
        if blk1 > blk0 {
            me_self.push(format!("block +{}", blk1 - blk0));
        }
        if !me_self.is_empty() {
            out.push(format!("self: {}", me_self.join(", ")));
        }
        if alive1 > alive0 {
            out.push(format!("summons {}", alive1 - alive0));
        }
        out.join("; ")
    }

    fn node_attack_damage(&self, c: Cid, node: u8) -> f32 {
        if node == STUN_NODE {
            return 0.0;
        }
        let def = content::monster_def(self.cr(c).monster.id);
        let MonsterNode::Move { intents, .. } = &def.nodes[node as usize] else { return 0.0 };
        let mut total = 0i64;
        for it in intents.iter() {
            match it {
                Intent::Attack { damage, hits } => total += self.intent_damage(c, damage(self, c)) as i64 * hits(self, c) as i64,
                Intent::DeathBlowAttack { damage } => total += self.intent_damage(c, damage(self, c)) as i64,
                _ => {}
            }
        }
        total as f32
    }

    // The key must cover everything a projection reads: the digest, the turn position and `c`.
    fn look_key_of(&self, c: Cid, digest: u64) -> u64 {
        let mut h = digest;
        let v = c as u64
            | (self.round as u32 as u64) << 8
            | (self.player.turn_number as u32 as u64) << 24
            | (self.side as u64) << 40
            | (self.stage as u64) << 44
            | (self.enemy_cont.is_some() as u64) << 50;
        h = (h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
        h
    }

    pub fn lookahead(&self, c: Cid) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, true, &mut LookDigests::default())
    }

    pub fn lookahead_shared(&self, c: Cid, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, true, d)
    }

    pub fn lookahead_fresh(&self, c: Cid) -> [LookRow; LOOK_H] {
        self.lookahead_with(c, false, &mut LookDigests::default())
    }

    fn lookahead_with(&self, c: Cid, cached: bool, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        let cr = self.cr(c);
        if !cr.is_alive() || !cr.in_combat || cr.monster.next_move == NO || cr.is_player || self.stage == Stage::Over {
            return [EMPTY_ROW; LOOK_H];
        }
        #[cfg(feature = "obs_prof")]
        let t0 = unsafe { core::arch::x86_64::_rdtsc() };
        // The relaxed key holds only for projections that read no enemy starting HP or block beyond alive/dead (LOOK_DEP false); else the exact key.
        let rkey = if cached { self.look_key_of(c, d.relaxed(self)) } else { 0 };
        #[cfg(feature = "obs_prof")]
        unsafe { crate::observe::OBS_PROF[10] += core::arch::x86_64::_rdtsc() - t0; }
        #[cfg(feature = "obs_prof")]
        let t1 = unsafe { core::arch::x86_64::_rdtsc() };
        let rows = if !cached { self.look_rows(c, false, d) } else { LOOK_CACHE.with(|t| {
            let mut t = t.borrow_mut();
            #[cfg(feature = "obs_prof")]
            unsafe { crate::observe::OBS_PROF[12] += 1; }
            if let Some(r) = t.get(rkey) {
                if LOOK_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
                    look_verify(self, c, &r);
                }
                return r;
            }
            let key = self.look_key_of(c, d.key(self));
            if let Some(r) = t.get(key) {
                if LOOK_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
                    look_verify(self, c, &r);
                }
                return r;
            }
            #[cfg(feature = "obs_prof")]
            unsafe { crate::observe::OBS_PROF[13] += 1; }
            let outer = LOOK_DEP.with(|d| d.replace(false));
            let r = self.look_rows(c, true, d);
            let dep = LOOK_DEP.with(|d| d.replace(outer || d.get()));
            t.put(key, r);
            if !dep {
                t.put(rkey, r);
            }
            r
        }) };
        #[cfg(feature = "obs_prof")]
        unsafe { crate::observe::OBS_PROF[11] += core::arch::x86_64::_rdtsc() - t1; }
        rows
    }

    fn look_who(&self) -> crate::util::ArrayVec<Cid, MAX_CREATURES> {
        let mut v = crate::util::ArrayVec::new();
        for &e in self.enemies.iter() {
            let cr = self.cr(e);
            if cr.is_alive() && cr.in_combat && cr.monster.next_move != NO {
                v.push(e);
            }
        }
        v
    }

    fn look_project_rows(&self, who: &[Cid], fork: impl Fn(&Combat, Cid) -> bool) -> ([[LookRow; LOOK_H]; MAX_CREATURES], u16) {
        let (lists, random) = self.look_project(who, fork);
        let mut rows = [[EMPTY_ROW; LOOK_H]; MAX_CREATURES];
        for (&e, l) in who.iter().zip(lists.iter()) {
            rows[e as usize] = look_rows_of(l);
        }
        (rows, random)
    }

    fn look_rows(&self, c: Cid, cached: bool, d: &mut LookDigests) -> [LookRow; LOOK_H] {
        let key = if cached { self.look_key_of(NO, d.key(self)) } else { 0 };
        let memo = if cached { LOOK_MODE.with(|m| m.borrow().filter(|m| m.0 == key)) } else { None };
        if let Some((_, rows, random, dep)) = memo {
            if dep {
                look_dep();
            }
            return if random >> c & 1 == 0 { rows[c as usize] } else { look_rows_of(&self.look_project_one(c)) };
        }
        let who = self.look_who();
        let outer = LOOK_DEP.with(|d| d.replace(false));
        let (rows, random) = self.look_project_rows(who.as_slice(), look_fork(self, c));
        let dep = LOOK_DEP.with(|d| d.replace(outer || d.get()));
        if random >> c & 1 == 0 && cached {
            LOOK_MODE.with(|m| *m.borrow_mut() = Some((key, rows, random, dep)));
        }
        rows[c as usize]
    }
}

fn look_fork(cx: &Combat, f: Cid) -> impl Fn(&Combat, Cid) -> bool {
    let fid = cx.cr(f).monster.id;
    let joint = LOOK_JOINT.contains(&fid);
    move |cx: &Combat, e: Cid| e == f || (joint && cx.cr(e).monster.id == fid)
}

fn look_rows_of(lists: &[LookList; LOOK_H]) -> [LookRow; LOOK_H] {
    let mut rows = [EMPTY_ROW; LOOK_H];
    for (row, list) in rows.iter_mut().zip(lists.iter()) {
        for &(node, p, pd) in list.iter() {
            row.prob[look_slot(node)] += p;
            row.exp_damage += pd;
        }
    }
    rows
}

#[derive(Default)]
pub struct LookDigests {
    relaxed: Option<u64>,
    key: Option<u64>,
}

impl LookDigests {
    #[inline]
    fn relaxed(&mut self, cx: &Combat) -> u64 {
        *self.relaxed.get_or_insert_with(|| look_digest_of(cx, Digest::Relaxed))
    }
    #[inline]
    fn key(&mut self, cx: &Combat) -> u64 {
        *self.key.get_or_insert_with(|| look_digest_of(cx, Digest::Key))
    }
}

const LOOK_PLAYER_HOOKS: crate::hooks::Mask = crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_additive)
    .or(crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_multiplicative))
    .or(crate::hooks::Mask::bit(crate::hooks::hookbit::modify_damage_cap));
const LOOK_READ_POWERS: [u16; 1] = [crate::ids::power::DEBILITATE_POWER];
const LOOK_READ_RELICS: [u16; 3] = [crate::ids::relic::PAPER_KRANE, crate::ids::relic::PAPER_PHROG, crate::ids::relic::WHISPERING_EARRING];

#[derive(Clone, Copy, PartialEq)]
enum Digest {
    Key,
    Canon,
    Relaxed,
}

fn look_digest_of(cx: &Combat, mode: Digest) -> u64 {
    #[inline(always)]
    fn mix(h: &mut u64, v: u64) {
        *h = (*h ^ v).wrapping_mul(0x100000001b3).rotate_left(23);
    }
    let canon = mode == Digest::Canon;
    let mut h = if mode == Digest::Relaxed { 0x84222325cbf29ce4u64 } else { 0xcbf29ce484222325u64 };
    for (i, cr) in cx.creatures.iter().enumerate() {
        if !cr.active || (canon && cr.is_player) {
            continue;
        }
        if !cr.is_player {
            if mode == Digest::Relaxed {
                mix(&mut h, i as u64 | (cr.in_combat as u64) << 8 | (cr.slot as u64) << 16 | ((cr.hp > 0) as u64) << 32);
                mix(&mut h, cr.max_hp as u32 as u64);
            } else {
                mix(&mut h, i as u64 | (cr.in_combat as u64) << 8 | (cr.slot as u64) << 16 | (cr.hp as u32 as u64) << 32);
                mix(&mut h, cr.max_hp as u32 as u64 | (cr.block as u32 as u64) << 32);
            }
        }
        for p in cr.powers.as_slice() {
            if cr.is_player && !content::power_mask(p.id).intersects(LOOK_PLAYER_HOOKS) && !LOOK_READ_POWERS.contains(&p.id) {
                continue;
            }
            mix(&mut h, (p.id as u64) << 48 | (if canon { 0 } else { p.uid as u64 }) << 32 | p.amount as u32 as u64);
            mix(&mut h, p.aux as u32 as u64 | (p.applier as u64) << 32 | (p.skip_next_tick as u64) << 40);
        }
        if cr.is_player {
            continue;
        }
        let m = &cr.monster;
        mix(&mut h, m.id as u64 | (m.cur_state as u64) << 16 | (m.next_move as u64) << 24 | (m.performed_first as u64) << 32 | (m.spawned_this_turn as u64) << 33
            | (m.is_performing as u64) << 34 | (m.stunned as u64) << 35 | (m.stun_performed as u64) << 36 | (m.stun_move.is_some() as u64) << 37 | (m.stun_follow_up as u64) << 40);
        if canon {
            let (w, once) = look_memory(m.id);
            let n = (m.log_len as usize).min(w);
            let mut last = 0u64;
            for k in 0..n {
                last = last << 8 | m.log[(m.log_len as usize - 1 - k) & 7] as u64;
            }
            mix(&mut h, last << 8 | n as u64);
            mix(&mut h, m.ever_logged & once);
        } else {
            mix(&mut h, u64::from_le_bytes(m.log));
            mix(&mut h, m.log_len as u64 | (u32::from_le_bytes(m.performed) as u64) << 16);
            mix(&mut h, m.ever_logged);
        }
        mix(&mut h, m.performed_once);
        for v in m.vars {
            mix(&mut h, v as u32 as u64);
        }
    }
    for &e in cx.enemies.iter() {
        mix(&mut h, e as u64);
    }
    if !canon {
        for r in cx.player.relics.iter() {
            if !content::relic_mask(r.id).intersects(LOOK_PLAYER_HOOKS) && !LOOK_READ_RELICS.contains(&r.id) {
                continue;
            }
            mix(&mut h, r.id as u64 | (r.counter as u32 as u64) << 16 | (r.flags as u64) << 48);
            mix(&mut h, r.aux as u32 as u64);
        }
    }
    h
}

fn look_merge(v: &mut Vec<(Box<Combat>, f32)>) {
    let (mut any, mut all) = ([0u8; MAX_CREATURES], [u8::MAX; MAX_CREATURES]);
    for (cx, _) in v.iter() {
        for (k, cr) in cx.creatures.iter().enumerate() {
            if cr.active && !cr.is_player {
                any[k] |= cr.pristine;
                all[k] &= cr.pristine;
            }
        }
    }
    if (0..MAX_CREATURES).any(|k| any[k] & !all[k] != 0) {
        look_dep();
    }
    let mut digests = crate::util::ArrayVec::<u64, 512>::new();
    let mut k = 0;
    for i in 0..v.len() {
        let d = look_digest_of(&v[i].0, Digest::Canon);
        match digests.iter().position(|&x| x == d) {
            Some(j) if j < k => v[j].1 += v[i].1,
            _ => {
                digests.push(d);
                v.swap(k, i);
                k += 1;
            }
        }
    }
    for (cx, _) in v.drain(k..) {
        look_free(cx);
    }
    if v.len() > LOOK_PATHS {
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        for (cx, _) in v.drain(LOOK_PATHS..) {
            look_free(cx);
        }
    }
}

thread_local! {
    static LOOK_VECS: std::cell::RefCell<[Vec<(Box<Combat>, f32)>; 3]> = const { std::cell::RefCell::new([Vec::new(), Vec::new(), Vec::new()]) };
    static LOOK_POOL: std::cell::RefCell<Vec<Box<Combat>>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn look_box(src: &Combat) -> Box<Combat> {
    match LOOK_POOL.with(|p| p.borrow_mut().pop()) {
        Some(mut b) => {
            (*b).clone_from(src);
            b
        }
        None => Box::new(src.clone()),
    }
}

fn look_free(b: Box<Combat>) {
    LOOK_POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() < 2 * LOOK_PATHS {
            p.push(b);
        }
    });
}

fn look_memory(id: u16) -> (usize, u64) {
    static T: std::sync::OnceLock<Vec<(usize, u64)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        (0..crate::ids::monster::COUNT as u16)
            .map(|id| {
                let (mut w, mut once) = (1usize, 0u64);
                if content::monster_implemented(id) {
                    for n in content::monster_def(id).nodes.iter() {
                        if let MonsterNode::Random { branches, .. } = n {
                            for b in branches.iter() {
                                match b.repeat {
                                    Repeat::UseOnlyOnce => once |= 1u64 << (b.target & 63),
                                    Repeat::CanRepeatXTimes(k) => w = w.max(k as usize),
                                    Repeat::CanRepeatForever => {}
                                }
                                w = w.max(b.cooldown as usize);
                            }
                        }
                    }
                }
                (w.min(8), once)
            })
            .collect()
    })[id as usize]
}

fn look_slot(node: u8) -> usize {
    if node == STUN_NODE || node as usize >= LOOK_NODES - 1 { LOOK_NODES - 1 } else { node as usize }
}

fn look_rng() -> &'static RngSet {
    static R: std::sync::OnceLock<RngSet> = std::sync::OnceLock::new();
    R.get_or_init(|| RngSet::from_run_seed(LOOK_SEED))
}

pub const LOOK_CACHE_ENTRIES: usize = 1024;
const LOOK_WAYS: usize = 8;

pub struct LookCache {
    keys: Vec<[u64; LOOK_WAYS]>,
    used: Vec<[u32; LOOK_WAYS]>,
    rows: Vec<[LookRow; LOOK_H]>,
    clock: u32,
}

impl LookCache {
    pub fn new(entries: usize) -> LookCache {
        assert!(entries.is_power_of_two() && entries >= LOOK_WAYS, "look-ahead cache entries must be a power of two >= {LOOK_WAYS}");
        let sets = entries / LOOK_WAYS;
        LookCache { keys: vec![[0; LOOK_WAYS]; sets], used: vec![[0; LOOK_WAYS]; sets], rows: vec![[EMPTY_ROW; LOOK_H]; entries], clock: 0 }
    }

    #[inline]
    fn tick(&mut self) -> u32 {
        if self.clock == u32::MAX {
            self.used.iter_mut().for_each(|u| *u = [0; LOOK_WAYS]);
            self.clock = 0;
        }
        self.clock += 1;
        self.clock
    }

    #[inline]
    fn get(&mut self, key: u64) -> Option<[LookRow; LOOK_H]> {
        let set = (key as usize) & (self.keys.len() - 1);
        let w = (0..LOOK_WAYS).find(|&w| self.keys[set][w] == key && self.used[set][w] != 0)?;
        self.used[set][w] = self.tick();
        Some(self.rows[set * LOOK_WAYS + w])
    }

    fn put(&mut self, key: u64, r: [LookRow; LOOK_H]) {
        let set = (key as usize) & (self.keys.len() - 1);
        let w = (0..LOOK_WAYS).min_by_key(|&w| self.used[set][w]).unwrap_or(0);
        self.keys[set][w] = key;
        self.used[set][w] = self.tick();
        self.rows[set * LOOK_WAYS + w] = r;
    }
}

#[cold]
#[inline(never)]
pub fn look_dep() {
    LOOK_DEP.with(|d| d.set(true));
}

pub static LOOK_VERIFY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static LOOK_VERIFIED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cold]
fn look_verify(cx: &Combat, c: Cid, r: &[LookRow; LOOK_H]) {
    let outer = LOOK_DEP.with(|d| d.get());
    let f = cx.look_rows(c, false, &mut LookDigests::default());
    LOOK_DEP.with(|d| d.set(outer));
    let same = r.iter().zip(f.iter()).all(|(a, b)| a.exp_damage.to_bits() == b.exp_damage.to_bits() && a.prob.iter().zip(b.prob.iter()).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(same, "look-ahead: a cache hit differs from the fresh rows (monster {}, creature {c})", crate::ids::monster::NAMES[cx.cr(c).monster.id as usize]);
    LOOK_VERIFIED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn with_look_cache<R>(cache: &mut Box<LookCache>, f: impl FnOnce() -> R) -> R {
    struct Restore<'a>(&'a mut Box<LookCache>);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            LOOK_CACHE.with(|t| std::mem::swap(&mut *t.borrow_mut(), self.0));
        }
    }
    LOOK_CACHE.with(|t| std::mem::swap(&mut *t.borrow_mut(), cache));
    let _restore = Restore(cache);
    f()
}

thread_local! {
    static LOOK_CACHE: std::cell::RefCell<Box<LookCache>> = std::cell::RefCell::new(Box::new(LookCache::new(LOOK_CACHE_ENTRIES)));
    static LOOK_MODE: std::cell::RefCell<Option<(u64, [[LookRow; LOOK_H]; MAX_CREATURES], u16, bool)>> = const { std::cell::RefCell::new(None) };
    static LOOK_DEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Combat {
    fn bound_enter(&self, c: Cid, mut ms: MonsterState, left: u8, to: u8, first: u8, out: &mut Vec<MonsterState>, depth: u32) {
        if depth > 64 {
            return;
        }
        if left == STUN_NODE {
            ms.stun_performed = false;
        } else {
            ms.performed_once &= !(1u64 << left);
        }
        ms.cur_state = to;
        if self.node_is_move(c, to) {
            let first = if first == NO { to } else { first };
            ms.log[(ms.log_len & 7) as usize] = first;
            ms.log_len += 1;
            if first < 64 {
                ms.ever_logged |= 1u64 << first;
            }
            ms.next_move = to;
            out.push(ms);
            return;
        }
        let def = content::monster_def(ms.id);
        match &def.nodes[to as usize] {
            MonsterNode::Cond { arms, .. } => {
                for (target, _) in arms.iter() {
                    self.bound_enter(c, ms, to, *target, first, out, depth + 1);
                }
            }
            MonsterNode::Random { branches, .. } => {
                for b in branches.iter() {
                    self.bound_enter(c, ms, to, b.target, first, out, depth + 1);
                }
            }
            MonsterNode::Move { .. } => {}
        }
    }

    pub fn bound_next_states(&self, c: Cid, ms: &MonsterState, out: &mut Vec<MonsterState>) {
        let mut ms = *ms;
        ms.performed_first = true;
        let cur = ms.cur_state;
        let def = content::monster_def(ms.id);
        let nxt = if cur == STUN_NODE {
            ms.stun_performed = true;
            if ms.stun_follow_up == NO { def.initial } else { ms.stun_follow_up }
        } else {
            ms.performed_once |= 1u64 << cur;
            match &def.nodes[cur as usize] {
                MonsterNode::Move { follow_up, .. } => {
                    if *follow_up == NO {
                        def.initial
                    } else if *follow_up == crate::defs::FOLLOW_STORED {
                        ms.stun_follow_up
                    } else {
                        *follow_up
                    }
                }
                _ => return,
            }
        };
        self.bound_enter(c, ms, cur, nxt, NO, out, 0);
    }

    pub fn bound_same_state(a: &MonsterState, b: &MonsterState) -> bool {
        a.cur_state == b.cur_state && a.stun_performed == b.stun_performed && a.stun_follow_up == b.stun_follow_up
    }

    pub fn bound_node_damage(&self, c: Cid, node: u8, weak: bool) -> i64 {
        if node == STUN_NODE {
            return 0;
        }
        let def = content::monster_def(self.cr(c).monster.id);
        let MonsterNode::Move { intents, .. } = &def.nodes[node as usize] else { return 0 };
        let hit = |d: i32| -> i64 { if weak { (d as i64 * 3) / 4 } else { d as i64 } };
        let mut total = 0i64;
        for it in intents.iter() {
            match it {
                Intent::Attack { damage, hits } => total += hit(self.intent_damage(c, damage(self, c))) * hits(self, c) as i64,
                Intent::DeathBlowAttack { damage } => total += hit(self.intent_damage(c, damage(self, c))),
                _ => {}
            }
        }
        total
    }
}
