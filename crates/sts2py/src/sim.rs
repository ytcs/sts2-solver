// Contract: hidden information (pile order, RNG) is the simulator's own sample; nothing hidden is taken from the real game.
use numpy::PyReadwriteArray1;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::{json, Value};
use sts2sim::engine::{ActionBuf, ObsCard, ObsEnemy, ACTION_SPACE};
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::{Action, Combat};

#[pyclass]
#[derive(Clone)]
pub struct Sim {
    pub(crate) cx: Combat,
}

fn err<E: std::fmt::Debug>(e: E) -> PyErr {
    PyValueError::new_err(format!("{e:?}"))
}

fn visible(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("rng");
        for pile in ["draw", "discard", "exhaust"] {
            if let Some(Value::Array(cards)) = o.get_mut(pile) {
                cards.sort_by(|a, b| {
                    let k = |x: &Value| (x["id"].as_str().unwrap_or("").to_string(), x["upgrade"].as_i64().unwrap_or(0), x["affliction"]["id"].as_str().unwrap_or("").to_string());
                    k(a).cmp(&k(b))
                });
            }
        }
    }
    v
}

fn power_ids(name: &str) -> Option<u16> {
    sts2sim::ids::power::NAMES.iter().position(|n| *n == name).map(|i| i as u16)
}

fn obs_powers(v: &Value) -> Option<Vec<(u16, i32)>> {
    let a = v["powers"].as_array()?;
    Some(a.iter().filter_map(|p| power_ids(p["id"].as_str()?).map(|id| (id, p["amount"].as_i64().unwrap_or(0) as i32))).collect())
}

fn obs_powers_turn_start(v: &Value) -> Option<Vec<(u16, i32)>> {
    let a = v["powers"].as_array()?;
    Some(a.iter().filter_map(|p| power_ids(p["id"].as_str()?).map(|id| (id, p["amount_on_turn_start"].as_i64().or_else(|| p["amount"].as_i64()).unwrap_or(0) as i32))).collect())
}

fn card_ids(name: &str) -> Option<u16> {
    sts2sim::ids::card::NAMES.iter().position(|n| *n == name).map(|i| i as u16)
}

fn obs_belt(v: &Value, notes: &mut Vec<String>) -> Option<Vec<(usize, u16)>> {
    let mut belt = vec![];
    for (i, p) in v.as_array()?.iter().enumerate() {
        let name = p.as_str().or_else(|| p["id"].as_str()).unwrap_or("");
        match sts2sim::ids::potion::NAMES.iter().position(|n| *n == name) {
            Some(id) => belt.push((p["slot"].as_u64().map_or(i, |s| s as usize), id as u16)),
            None => notes.push(format!("unknown potion {name}")),
        }
    }
    Some(belt)
}

fn obs_card(c: &Value, with_cost: bool) -> Option<ObsCard> {
    let id = card_ids(c["id"].as_str().unwrap_or(""))?;
    let e = &c["enchantment"];
    let enchant = e.as_str().or_else(|| e["id"].as_str()).and_then(|n| sts2sim::ids::enchantment::NAMES.iter().position(|x| *x == n)).map_or(0, |i| i as u8 + 1);
    let a = &c["affliction"];
    let affliction = a.as_str().or_else(|| a["id"].as_str()).and_then(|n| sts2sim::ids::affliction::NAMES.iter().position(|x| *x == n)).map_or(0, |i| i as u8 + 1);
    Some(ObsCard {
        id,
        upgrade: c["upgrade"].as_u64().unwrap_or(0) as u8,
        cost: if with_cost { c["cost"].as_i64().map(|x| x as i32) } else { None },
        enchant,
        enchant_amount: if enchant == 0 { 0 } else { e["amount"].as_i64().unwrap_or(1) as i16 },
        affliction,
        affliction_amount: if affliction == 0 { 0 } else { a["amount"].as_i64().unwrap_or(1) as i16 },
    })
}

impl Sim {
    fn pick_card_name(&self, display: usize) -> Option<&'static str> {
        let d = self.cx.decision.as_ref()?;
        let g = self.cx.decision_view(d).get(display)? as usize;
        let c = d.cands.get(g)?;
        Some(sts2sim::ids::card::NAMES[self.cx.cards[c as usize].id as usize])
    }

    fn enemy_cid(&self, i: usize) -> PyResult<Cid> {
        self.cx.enemies.as_slice().get(i).copied().ok_or_else(|| PyValueError::new_err(format!("no enemy {i}")))
    }

    fn do_step(&mut self, a: Action) -> PyResult<()> {
        let mut buf = ActionBuf::new();
        self.cx.legal_actions(&mut buf);
        if !buf.iter().any(|x| *x == a) {
            return Err(PyValueError::new_err(format!("{a:?} is not legal here")));
        }
        if !self.cx.step(a) {
            return Err(PyValueError::new_err(format!("{a:?} was rejected")));
        }
        Ok(())
    }
}

#[pymethods]
impl Sim {
    #[new]
    fn new(scenario_json: &str, seed: u64) -> PyResult<Self> {
        let v: Value = serde_json::from_str(scenario_json).map_err(err)?;
        let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
        sc.validate().map_err(|e| PyValueError::new_err(format!("scenario uses unported content: {e:?}")))?;
        let mut cx = Combat::try_new_with(&sc, &ex).map_err(err)?;
        cx.reset_validated(&sc, &ex, seed, sts2sim::state::RngSet::from_run_seed_fast(seed)).map_err(err)?;
        if let Some(belt) = obs_belt(&v["potions"], &mut vec![]) {
            let slots = cx.player.potion_slots;
            cx.sync_potions(slots, &belt);
        }
        Ok(Sim { cx })
    }

    fn copy(&self) -> Sim {
        self.clone()
    }

    fn without_potions(&self, slots: Vec<usize>) -> Sim {
        let mut s = self.clone();
        for i in slots {
            if i < s.cx.player.potions.len() {
                s.cx.player.potions[i] = None;
            }
        }
        s
    }

    fn belt(&self) -> Vec<(usize, &'static str)> {
        self.cx.player.potions.iter().enumerate().filter_map(|(i, p)| p.map(|p| (i, sts2sim::ids::potion::NAMES[p.id as usize]))).collect()
    }

    fn determinize(&mut self, seed: u64) -> bool {
        self.cx.determinize(seed)
    }

    fn stage(&self) -> &'static str {
        match self.cx.stage {
            Stage::AwaitAction => "play",
            Stage::AwaitChoice => "choice",
            Stage::Over => "over",
        }
    }

    fn outcome(&self) -> i32 {
        match self.cx.outcome {
            Outcome::Victory => 1,
            Outcome::Defeat => -1,
            _ => 0,
        }
    }

    fn missing(&self) -> Option<String> {
        self.cx.missing.map(|(k, id)| format!("{k:?} {id}"))
    }

    #[pyo3(signature = (hand_pos, target=None))]
    fn play(&mut self, hand_pos: usize, target: Option<usize>) -> PyResult<()> {
        let t = match target {
            Some(i) => self.enemy_cid(i)?,
            None => NO,
        };
        self.do_step(Action::PlayCard { hand_pos: hand_pos as u8, target: t })
    }

    #[pyo3(signature = (slot, target=None))]
    fn use_potion(&mut self, slot: usize, target: Option<usize>) -> PyResult<()> {
        let t = match target {
            Some(i) => self.enemy_cid(i)?,
            None => NO,
        };
        self.do_step(Action::UsePotion { slot: slot as u8, target: t })
    }

    fn end_turn(&mut self) -> PyResult<()> {
        self.do_step(Action::EndTurn)
    }

    fn choose(&mut self, picks: Vec<usize>) -> PyResult<()> {
        if self.cx.stage != Stage::AwaitChoice {
            return Err(PyValueError::new_err("no selection is pending"));
        }
        let seq = self.cx.decision_seq;
        for p in picks {
            if !self.cx.step_pick_game_order(p as u8) {
                return Err(PyValueError::new_err(format!("pick {p} rejected")));
            }
            if self.cx.stage != Stage::AwaitChoice || self.cx.decision_seq != seq {
                break;
            }
        }
        if self.cx.stage == Stage::AwaitChoice && self.cx.decision_seq == seq && !self.cx.step(Action::Confirm) {
            return Err(PyValueError::new_err("selection still pending after the picks"));
        }
        Ok(())
    }

    fn apply(&mut self, action_json: &str) -> PyResult<()> {
        let a: Value = serde_json::from_str(action_json).map_err(err)?;
        if a.get("end_turn").is_some() {
            return self.end_turn();
        }
        if let Some(p) = a.get("play") {
            return self.play(p["hand_pos"].as_u64().ok_or_else(|| PyValueError::new_err("hand_pos"))? as usize, p["target"].as_u64().map(|x| x as usize));
        }
        if let Some(p) = a.get("use_potion") {
            return self.use_potion(p["slot"].as_u64().ok_or_else(|| PyValueError::new_err("slot"))? as usize, p["target"].as_u64().map(|x| x as usize));
        }
        if let Some(c) = a.get("choose") {
            let v: Vec<usize> = c.as_array().map(|x| x.iter().filter_map(|y| y.as_u64()).map(|y| y as usize).collect()).unwrap_or_default();
            return self.choose(v);
        }
        Err(PyValueError::new_err(format!("unsupported action {a}")))
    }

    fn legal(&self) -> Vec<(usize, String)> {
        let mut buf = ActionBuf::new();
        self.cx.legal_actions(&mut buf);
        let enemy_idx = |t: Cid| self.cx.enemies.iter().position(|&e| e == t);
        buf.iter()
            .map(|a| {
                let text = match *a {
                    Action::EndTurn => "end turn".to_string(),
                    Action::PlayCard { hand_pos, target } => {
                        let c = self.cx.player.hand.as_slice()[hand_pos as usize];
                        let name = sts2sim::ids::card::NAMES[self.cx.cards[c as usize].id as usize];
                        match enemy_idx(target) {
                            Some(e) => format!("play {name} #{hand_pos} -> e{e}"),
                            None => format!("play {name} #{hand_pos}"),
                        }
                    }
                    Action::UsePotion { slot, target } => match enemy_idx(target) {
                        Some(e) => format!("potion {slot} -> e{e}"),
                        None => format!("potion {slot}"),
                    },
                    Action::DiscardPotion { slot } => format!("discard potion {slot}"),
                    Action::Pick { idx } => match self.pick_card_name(idx as usize) {
                        Some(n) => format!("pick {idx} ({n})"),
                        None => format!("pick {idx}"),
                    },
                    Action::Confirm => "confirm".to_string(),
                };
                (a.index(), text)
            })
            .collect()
    }

    fn action_json(&self, idx: usize) -> PyResult<String> {
        let a = Action::from_index(idx).ok_or_else(|| PyValueError::new_err("bad action index"))?;
        let enemy_idx = |t: Cid| self.cx.enemies.iter().position(|&e| e == t);
        let v = match a {
            Action::EndTurn => json!({"end_turn": true}),
            Action::PlayCard { hand_pos, target } => {
                let mut p = json!({"hand_pos": hand_pos});
                if let Some(e) = enemy_idx(target) {
                    p["target"] = json!(e);
                }
                json!({"play": p})
            }
            Action::UsePotion { slot, target } => {
                let mut p = json!({"slot": slot});
                if let Some(e) = enemy_idx(target) {
                    p["target"] = json!(e);
                }
                json!({"use_potion": p})
            }
            Action::DiscardPotion { slot } => json!({"discard_potion": slot}),
            Action::Pick { idx } => json!({"pick": idx}),
            Action::Confirm => json!({"confirm": true}),
        };
        Ok(v.to_string())
    }

    fn pick_game_index(&self, display: usize) -> Option<usize> {
        let d = self.cx.decision.as_ref()?;
        self.cx.decision_view(d).get(display).map(|g| g as usize)
    }

    fn lookahead(&self) -> Vec<(usize, Vec<f32>)> {
        self.cx
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, &c)| self.cx.cr(c).is_alive())
            .map(|(i, &c)| (i, self.cx.lookahead(c).iter().map(|r| r.exp_damage).collect()))
            .collect()
    }

    fn intent_plan(&self) -> Vec<(usize, Vec<Vec<(String, f32, String)>>)> {
        self.cx
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, &c)| self.cx.cr(c).is_alive())
            .map(|(i, &c)| (i, self.cx.intent_plan(c)))
            .collect()
    }

    fn intent_now(&self) -> Vec<(usize, String, String)> {
        self.cx
            .enemies
            .iter()
            .enumerate()
            .filter_map(|(i, &c)| self.cx.intent_now(c).map(|(id, text)| (i, id, text)))
            .collect()
    }

    fn step(&mut self, idx: usize) -> bool {
        match Action::from_index(idx) {
            Some(a) => self.do_step(a).is_ok(),
            None => false,
        }
    }

    fn observe(&mut self, mut obs: PyReadwriteArray1<f32>, mut mask: PyReadwriteArray1<u8>) -> PyResult<()> {
        let o = obs.as_slice_mut().map_err(err)?;
        let m = mask.as_slice_mut().map_err(err)?;
        if o.len() < OBS_SIZE || m.len() < ACTION_SPACE {
            return Err(PyValueError::new_err(format!("buffers too small (an observation has {OBS_SIZE} floats)")));
        }
        let mut buf = ActionBuf::new();
        let mut playable = 0u16;
        self.cx.legal_actions_ex(&mut buf, &mut playable);
        self.cx.observe_ex(&mut o[..OBS_SIZE], Some(playable));
        m[..ACTION_SPACE].fill(0);
        for a in buf.iter() {
            m[a.index()] = 1;
        }
        Ok(())
    }

    /// Probe edit (value-response tests): set `power` to `amount` on the player / enemy `index` / Osty (0 removes it); returns false if refused.
    fn set_power(&mut self, side: &str, index: usize, power: &str, amount: i32) -> PyResult<bool> {
        let cid = match side {
            "player" => PLAYER,
            "enemy" => self.enemy_cid(index)?,
            "osty" => self.cx.osty().ok_or_else(|| PyValueError::new_err("no Osty"))?,
            _ => return Err(PyValueError::new_err(format!("side {side}: want player / enemy / osty"))),
        };
        let id = power_ids(power).ok_or_else(|| PyValueError::new_err(format!("unknown power {power}")))?;
        let mut want: Vec<(u16, i32)> = self.cx.cr(cid).powers.iter().map(|p| (p.id, p.amount)).filter(|p| p.0 != id).collect();
        if amount != 0 {
            want.push((id, amount));
        }
        self.cx.sync_powers(cid, &want);
        Ok(self.cx.cr(cid).power_amount(id) == amount)
    }

    /// Diagnostics (observation audits): pile cards with their counters / enchant / affliction, power counts, decision source, relic display.
    fn diag(&self) -> String {
        let cx = &self.cx;
        let card = |c: &CardIdx| {
            let k = &cx.cards[*c as usize];
            json!([k.id, k.upgrade, k.counter[0], k.counter[1], k.enchant, k.enchant_amount, k.affliction, k.affliction_amount])
        };
        let creatures: Vec<usize> = std::iter::once(PLAYER).chain(cx.enemies.iter().copied()).chain(cx.osty()).map(|c| cx.cr(c).powers.len()).collect();
        let dec = cx.decision.as_ref().map(|d| {
            let (kind, id) = Combat::decision_source(d);
            json!([kind, id, d.cands.len(), d.min, d.max])
        });
        let relics: Vec<Value> = cx
            .player
            .relics
            .as_slice()
            .iter()
            .map(|r| {
                let l = sts2sim::content::relic_listener(r.id);
                let props: Vec<(&str, i32)> = l.meta_props().iter().filter(|d| d.lit.is_empty()).map(|d| (d.name, r.get(d.slot))).collect();
                json!([r.id, sts2sim::relic_mask::OBSERVED.get(r.id as usize).copied().unwrap_or(true), l.meta_display(cx, r), props])
            })
            .collect();
        json!({
            "draw": cx.player.draw.iter().map(card).collect::<Vec<_>>(),
            "discard": cx.player.discard.iter().map(card).collect::<Vec<_>>(),
            "exhaust": cx.player.exhaust.iter().map(card).collect::<Vec<_>>(),
            "hand": cx.player.hand.iter().map(card).collect::<Vec<_>>(),
            "powers": creatures,
            "decision": dec,
            "relics": relics,
        })
        .to_string()
    }

    fn snapshot(&self) -> String {
        visible(sts2diff::snapshot::snapshot(&self.cx)).to_string()
    }

    fn diff(&self, real_json: &str) -> PyResult<Vec<String>> {
        let real = visible(serde_json::from_str(real_json).map_err(err)?);
        let mine = visible(sts2diff::snapshot::snapshot(&self.cx));
        let mut out = vec![];
        sts2diff::diff::compare("", &mine, &real, &mut out);
        out.retain(|l| !l.starts_with(".combat_over:"));
        Ok(out)
    }

    fn sync_choice(&mut self, real_json: &str, options: Vec<(String, u8)>) -> PyResult<bool> {
        if self.cx.decision.is_none() || self.cx.replay.is_some() {
            return Ok(false);
        }
        if matches!(self.cx.decision, Some(sts2sim::state::Decision { source: DecisionSource::Options, .. })) {
            let mut want = vec![];
            for (name, up) in &options {
                match card_ids(name) {
                    Some(id) => want.push((id, *up)),
                    None => return Ok(false),
                }
            }
            return Ok(self.cx.sync_options(&want));
        }
        let real: Value = serde_json::from_str(real_json).map_err(err)?;
        let hand: Vec<ObsCard> = real["hand"].as_array().map_or(vec![], |h| h.iter().filter_map(|c| obs_card(c, true)).collect());
        let mut want = vec![];
        for (name, up) in &options {
            match card_ids(name) {
                Some(id) => want.push((id, *up)),
                None => return Ok(false),
            }
        }
        let fits = |cx: &Combat| {
            let mut used = vec![false; cx.player.hand.len()];
            let mut out = Pile::new();
            for &(id, up) in &want {
                let j = cx.player.hand.iter().enumerate().position(|(j, &c)| !used[j] && cx.cards[c as usize].id == id && cx.cards[c as usize].upgrade == up)?;
                used[j] = true;
                out.push(cx.player.hand.as_slice()[j]);
            }
            Some(out)
        };
        let mut trial = self.cx.clone();
        trial.sync_hand(&hand);
        let Some(cands) = fits(&trial) else { return Ok(false) };
        let d = trial.decision.as_mut().expect("pending decision");
        d.cands = cands;
        d.selected.clear();
        self.cx = trial;
        Ok(true)
    }

    fn sync(&mut self, real_json: &str) -> PyResult<String> {
        let real: Value = serde_json::from_str(real_json).map_err(err)?;
        let mut notes: Vec<String> = vec![];
        let mut obs: Vec<ObsCard> = vec![];
        if let Some(h) = real["hand"].as_array() {
            for c in h {
                match obs_card(c, true) {
                    Some(o) => obs.push(o),
                    None => notes.push(format!("unknown card {}", c["id"].as_str().unwrap_or(""))),
                }
            }
        }
        let rep = self.cx.sync_hand(&obs);
        for (key, pile) in [("exhaust", PileType::Exhaust), ("discard", PileType::Discard)] {
            let cards: Vec<ObsCard> = real[key].as_array().map_or(vec![], |a| a.iter().filter_map(|c| obs_card(c, false)).collect());
            let r = self.cx.sync_pile(pile, &cards);
            if r.created > 0 {
                notes.push(format!("{key}: {} cards created", r.created));
            }
        }
        let playing = real["play_pile"].as_array().map_or(false, |a| !a.is_empty());
        if let (Some(a), false) = (real["draw"].as_array(), playing) {
            let cards: Vec<ObsCard> = a.iter().filter_map(|c| obs_card(c, false)).collect();
            let r = self.cx.sync_draw(&cards);
            if r.created > 0 {
                notes.push(format!("draw: {} cards created", r.created));
            }
        }
        self.cx.sync_energy(real["energy"].as_i64().unwrap_or(0) as i32, real["stars"].as_i64().unwrap_or(0) as i32);
        let p = &real["player"];
        self.cx.sync_creature(PLAYER, p["hp"].as_i64().unwrap_or(1) as i32, p["max_hp"].as_i64().unwrap_or(1) as i32, p["block"].as_i64().unwrap_or(0) as i32);
        let mut powers_changed = 0u32;
        if let Some(ob) = obs_powers(p) {
            powers_changed += self.cx.sync_powers(PLAYER, &ob) as u32;
            if let Some(ts) = obs_powers_turn_start(p) {
                self.cx.sync_turn_start(PLAYER, &ts);
            }
        }
        if let Some(es) = real["enemies"].as_array() {
            let monster = |name: &str| sts2sim::ids::monster::NAMES.iter().position(|n| *n == name).map_or(u16::MAX, |i| i as u16);
            let obs: Vec<ObsEnemy> = es
                .iter()
                .map(|e| ObsEnemy {
                    monster: monster(e["id"].as_str().unwrap_or("")),
                    hp: e["hp"].as_i64().unwrap_or(0) as i32,
                    max_hp: e["max_hp"].as_i64().unwrap_or(1) as i32,
                    block: e["block"].as_i64().unwrap_or(0) as i32,
                    alive: e["alive"].as_bool().unwrap_or(true),
                })
                .collect();
            let n_sim = self.cx.enemies.len();
            let er = self.cx.sync_enemies(&obs);
            if es.len() != n_sim {
                notes.push(format!("enemy count: simulator {} vs real {}", n_sim, es.len()));
            }
            if er.revived + er.removed + er.missing > 0 {
                notes.push(format!("enemies: {} re-attached, {} detached, {} with no simulated monster of that id", er.revived, er.removed, er.missing));
            }
            for (e, cid) in es.iter().zip(er.pairs.iter()).filter_map(|(e, c)| c.map(|c| (e, c))) {
                if let Some(ob) = obs_powers(e) {
                    powers_changed += self.cx.sync_powers(cid, &ob) as u32;
                    if let Some(ts) = obs_powers_turn_start(e) {
                        self.cx.sync_turn_start(cid, &ts);
                    }
                }
            }
        }
        let mut relics_changed = 0u32;
        if real.get("relics").is_some() {
            let rr = self.cx.sync_relics(&sts2diff::convert::obs_relics(&real["relics"]));
            relics_changed = rr.changed as u32;
            if rr.unpaired > 0 {
                notes.push(format!("relics: {} with no simulated relic of that id", rr.unpaired));
            }
            if !rr.unknown_props.is_empty() {
                notes.push(format!("relic props not modelled: {}", rr.unknown_props.join(", ")));
            }
            for id in rr.counter_mismatch {
                notes.push(format!("relic {}: shown counter not reproducible", sts2sim::ids::relic::NAMES[id as usize]));
            }
        }
        let mut potions_changed = 0u32;
        if let Some(belt) = obs_belt(&real["potions"], &mut notes) {
            let slots = real["potion_slots"].as_u64().map_or(self.cx.player.potion_slots, |n| n as u8);
            potions_changed = self.cx.sync_potions(slots, &belt) as u32;
        }
        Ok(json!({
            "from_draw": rep.from_draw, "from_discard": rep.from_discard, "from_exhaust": rep.from_exhaust,
            "created": rep.created, "returned": rep.returned, "cost_fixes": rep.cost_fixes, "powers": powers_changed, "relics": relics_changed,
            "potions": potions_changed, "notes": notes,
        })
        .to_string())
    }
}
