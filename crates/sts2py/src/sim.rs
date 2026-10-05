//! `sts2.Sim`: one simulated combat that Python can step, copy, snapshot and align with an observation of the real game.
//!
//! This is the replay path for the live game: build the fight from the fight-start scenario, replay the player's actions in the oracle's script
//! vocabulary (`play` / `use_potion` / `end_turn` / `choose`), and after each step `sync` the visible state (hand, HP, block, energy) to what the
//! bridge reported. Hidden information (draw / discard / exhaust order, every RNG stream) is the simulator's own random sample; nothing of it is
//! taken from the real game.

use numpy::{PyReadwriteArray1, PyArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::{json, Value};
use sts2sim::engine::{ActionBuf, ObsCard, ACTION_SPACE};
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

/// The snapshot minus everything hidden: no RNG, and the draw pile as a sorted multiset.
fn visible(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("rng");
        // the draw pile's order is hidden; the discard / exhaust orders carry no information a player can act on: compare them as multisets
        for pile in ["draw", "discard", "exhaust"] {
            if let Some(Value::Array(cards)) = o.get_mut(pile) {
                cards.sort_by(|a, b| {
                    let k = |x: &Value| (x["id"].as_str().unwrap_or("").to_string(), x["upgrade"].as_i64().unwrap_or(0));
                    k(a).cmp(&k(b))
                });
            }
        }
    }
    v
}

fn card_ids(name: &str) -> Option<u16> {
    sts2sim::ids::card::NAMES.iter().position(|n| *n == name).map(|i| i as u16)
}

impl Sim {
    /// Name of the card at displayed position `display` of the pending selection.
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
    /// `Sim(scenario_json, seed)`: the fight at its start. `seed` seeds every hidden stream (use a fresh random one).
    #[new]
    fn new(scenario_json: &str, seed: u64) -> PyResult<Self> {
        let v: Value = serde_json::from_str(scenario_json).map_err(err)?;
        let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
        sc.validate().map_err(|e| PyValueError::new_err(format!("scenario uses unported content: {e:?}")))?;
        let mut cx = Combat::try_new_with(&sc, &ex).map_err(err)?;
        cx.reset_validated(&sc, &ex, seed, sts2sim::state::RngSet::from_run_seed_fast(seed)).map_err(err)?;
        Ok(Sim { cx })
    }

    fn copy(&self) -> Sim {
        self.clone()
    }

    /// Resamples the hidden state (the three pile orders and all RNG streams). False while a prompt replay is on screen.
    fn determinize(&mut self, seed: u64) -> bool {
        self.cx.determinize(seed)
    }

    /// "play" (awaiting an action), "choice" (a card selection is pending) or "over".
    fn stage(&self) -> &'static str {
        match self.cx.stage {
            Stage::AwaitAction => "play",
            Stage::AwaitChoice => "choice",
            Stage::Over => "over",
        }
    }

    /// 0 ongoing, 1 win, -1 loss.
    fn outcome(&self) -> i32 {
        match self.cx.outcome {
            Outcome::Victory => 1,
            Outcome::Defeat => -1,
            _ => 0,
        }
    }

    /// Names of unported content the fight touched, if any (the fight is then not faithful).
    fn missing(&self) -> Option<String> {
        self.cx.missing.map(|(k, id)| format!("{k:?} {id}"))
    }

    fn overflow(&self) -> u32 {
        self.cx.overflow as u32
    }

    // ------------------------------------------------------------------ actions (oracle vocabulary)

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

    /// Answers the pending selection with indices into the option list as the game hands it to the selector (the oracle's `choose`).
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

    /// Applies one oracle-script action given as JSON: {"play":{"hand_pos":i,"target":e}}, {"use_potion":{...}}, {"end_turn":true}, {"choose":[...]}.
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

    // ------------------------------------------------------------------ dense actions (for the network / search)

    /// Legal actions as `(dense index, text)`.
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

    /// The oracle-script JSON of dense action `idx` ({"play":{"hand_pos","target"}}, {"use_potion":...}, {"end_turn":true}; selections: {"pick":display_idx} / {"confirm":true}).
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

    /// For a pending selection: the game's own option index of the candidate shown at position `display`.
    fn pick_game_index(&self, display: usize) -> Option<usize> {
        let d = self.cx.decision.as_ref()?;
        self.cx.decision_view(d).get(display).map(|g| g as usize)
    }

    /// Per living enemy `(index, [expected attack damage of each of the next turns after the shown intent])`: the move pattern a veteran knows by heart
    /// (exact for deterministic cycles, probability-weighted at random branches).
    fn lookahead(&self) -> Vec<(usize, Vec<f32>)> {
        self.cx
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, &c)| self.cx.cr(c).is_alive())
            .map(|(i, &c)| (i, self.cx.lookahead(c).iter().map(|r| r.exp_damage).collect()))
            .collect()
    }

    /// Plays dense action `idx`. False if it is not legal.
    fn step(&mut self, idx: usize) -> bool {
        match Action::from_index(idx) {
            Some(a) => self.do_step(a).is_ok(),
            None => false,
        }
    }

    /// Observation row and legal-action mask for the network (`OBS_SIZE`, `ACTION_SPACE`).
    fn observe(&mut self, mut obs: PyReadwriteArray1<f32>, mut mask: PyReadwriteArray1<u8>) -> PyResult<()> {
        let o = obs.as_slice_mut().map_err(err)?;
        let m = mask.as_slice_mut().map_err(err)?;
        if o.len() < OBS_SIZE || m.len() < ACTION_SPACE {
            return Err(PyValueError::new_err("buffers too small"));
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

    // ------------------------------------------------------------------ the visible state and its alignment with the real game

    /// The oracle-schema snapshot of everything a human could see.
    fn snapshot(&self) -> String {
        visible(sts2diff::snapshot::snapshot(&self.cx)).to_string()
    }

    /// Differences between this fight's visible state and a snapshot of the real game (`Snap.State()` of the bridge). Empty = they agree.
    fn diff(&self, real_json: &str) -> PyResult<Vec<String>> {
        let real = visible(serde_json::from_str(real_json).map_err(err)?);
        let mine = visible(sts2diff::snapshot::snapshot(&self.cx));
        let mut out = vec![];
        sts2diff::diff::compare("", &mine, &real, &mut out);
        out.retain(|l| !l.starts_with(".combat_over:"));
        Ok(out)
    }

    /// Puts the visible state on the real one: the hand (cards, order, costs), energy, stars, HP / max HP / block of the player and the enemies
    /// (matched by list index). Returns a JSON report of what had to change.
    fn sync(&mut self, real_json: &str) -> PyResult<String> {
        let real: Value = serde_json::from_str(real_json).map_err(err)?;
        let mut notes: Vec<String> = vec![];
        let mut obs: Vec<ObsCard> = vec![];
        if let Some(h) = real["hand"].as_array() {
            for c in h {
                let name = c["id"].as_str().unwrap_or("");
                let Some(id) = card_ids(name) else {
                    notes.push(format!("unknown card {name}"));
                    continue;
                };
                obs.push(ObsCard { id, upgrade: c["upgrade"].as_u64().unwrap_or(0) as u8, cost: c["cost"].as_i64().map(|x| x as i32) });
            }
        }
        let rep = self.cx.sync_hand(&obs);
        for (key, pile) in [("exhaust", PileType::Exhaust), ("discard", PileType::Discard)] {
            let mut cards: Vec<ObsCard> = vec![];
            if let Some(a) = real[key].as_array() {
                for c in a {
                    if let Some(id) = card_ids(c["id"].as_str().unwrap_or("")) {
                        cards.push(ObsCard { id, upgrade: c["upgrade"].as_u64().unwrap_or(0) as u8, cost: None });
                    }
                }
            }
            let r = self.cx.sync_pile(pile, &cards);
            if r.created > 0 {
                notes.push(format!("{key}: {} cards created", r.created));
            }
        }
        // (a card still resolving sits in the play pile, in no visible pile: the sim may hold it elsewhere, so the draw pile is not reconciled then)
        let playing = real["play_pile"].as_array().map_or(false, |a| !a.is_empty());
        if let (Some(a), false) = (real["draw"].as_array(), playing) {
            let cards: Vec<ObsCard> = a.iter().filter_map(|c| card_ids(c["id"].as_str().unwrap_or("")).map(|id| ObsCard { id, upgrade: c["upgrade"].as_u64().unwrap_or(0) as u8, cost: None })).collect();
            let r = self.cx.sync_draw(&cards);
            if r.created > 0 {
                notes.push(format!("draw: {} cards created", r.created));
            }
        }
        self.cx.sync_energy(real["energy"].as_i64().unwrap_or(0) as i32, real["stars"].as_i64().unwrap_or(0) as i32);
        let p = &real["player"];
        self.cx.sync_creature(PLAYER, p["hp"].as_i64().unwrap_or(1) as i32, p["max_hp"].as_i64().unwrap_or(1) as i32, p["block"].as_i64().unwrap_or(0) as i32);
        if let Some(es) = real["enemies"].as_array() {
            let ids: Vec<Cid> = self.cx.enemies.iter().copied().collect();
            if es.len() != ids.len() {
                notes.push(format!("enemy count: simulator {} vs real {}", ids.len(), es.len()));
            }
            for (e, &cid) in es.iter().zip(ids.iter()) {
                self.cx.sync_creature(cid, e["hp"].as_i64().unwrap_or(0) as i32, e["max_hp"].as_i64().unwrap_or(1) as i32, e["block"].as_i64().unwrap_or(0) as i32);
            }
        }
        Ok(json!({
            "from_draw": rep.from_draw, "from_discard": rep.from_discard, "from_exhaust": rep.from_exhaust,
            "created": rep.created, "returned": rep.returned, "cost_fixes": rep.cost_fixes, "notes": notes,
        })
        .to_string())
    }
}
