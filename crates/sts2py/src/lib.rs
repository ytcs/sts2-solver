use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyReadwriteArray1, PyReadwriteArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use sts2env::{BatchEnv, PoolScenario, RewardConfig, RoundRobinScenario, StepOut};

#[pyclass]
struct BatchEnvPy {
    env: BatchEnv,
}

#[pymethods]
impl BatchEnvPy {
    #[new]
    #[pyo3(signature = (n_envs, scenarios_json, seed, max_steps, win, loss, hp_bonus, step_reward, round_robin=false))]
    fn new(n_envs: usize, scenarios_json: Vec<String>, seed: u64, max_steps: u32, win: f32, loss: f32, hp_bonus: f32, step_reward: f32, round_robin: bool) -> PyResult<Self> {
        let mut scs = vec![];
        for s in scenarios_json {
            let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))?;
            let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
            sc.validate().map_err(|e| PyValueError::new_err(format!("scenario uses unported content: {e:?}")))?;
            scs.push((sc, ex));
        }
        let cfg = RewardConfig { win, loss, hp_bonus, step: step_reward };
        if scs.is_empty() {
            return Err(PyValueError::new_err("no scenarios"));
        }
        let source: Box<dyn sts2env::ScenarioSource> = if round_robin { Box::new(RoundRobinScenario::with_extras(scs)) } else { Box::new(PoolScenario::with_extras(scs)) };
        let env = BatchEnv::try_new(n_envs, source, cfg, max_steps, seed)
            .map_err(|e| PyValueError::new_err(format!("cannot create the env: {e:?}")))?;
        Ok(BatchEnvPy { env })
    }

    fn observe_all(&mut self, py: Python<'_>, mut obs: PyReadwriteArray2<f32>, mut mask: PyReadwriteArray2<u8>) -> PyResult<()> {
        let o = obs.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let m = mask.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let env = &mut self.env;
        py.detach(|| env.observe_all(o, m)).map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }

    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        py: Python<'_>,
        actions: PyReadonlyArray1<i32>,
        mut obs: PyReadwriteArray2<f32>,
        mut mask: PyReadwriteArray2<u8>,
        mut reward: PyReadwriteArray1<f32>,
        mut done: PyReadwriteArray1<u8>,
        mut outcome: PyReadwriteArray1<i8>,
        mut illegal: PyReadwriteArray1<u8>,
    ) -> PyResult<()> {
        let e = |x: numpy::BorrowError| PyValueError::new_err(x.to_string());
        let _ = e;
        let a = actions.as_slice().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let o = obs.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let m = mask.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let r = reward.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let d = done.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let oc = outcome.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let il = illegal.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let env = &mut self.env;
        py.detach(|| env.step(a, StepOut { obs: o, mask: m, reward: r, done: d, outcome: oc, illegal: il })).map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }

    fn set_autoreset(&mut self, on: bool) {
        self.env.set_autoreset(on);
    }

    /// Copy `src[src_idx[k]]` into this env's slot `dst_idx[k]`, resampling hidden state with `seeds[k]` (see `BatchEnv::fork_from`).
    fn fork_from(&mut self, src: PyRef<'_, BatchEnvPy>, src_idx: PyReadonlyArray1<u32>, dst_idx: PyReadonlyArray1<u32>, seeds: PyReadonlyArray1<u64>) -> PyResult<()> {
        let e = |x: numpy::NotContiguousError| PyValueError::new_err(x.to_string());
        self.env.fork_from(&src.env, src_idx.as_slice().map_err(e)?, dst_idx.as_slice().map_err(e)?, seeds.as_slice().map_err(e)?)
            .map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }

    /// `[n, 4]` f32: scenario index, HP lost fraction, HP left fraction, episode length of the episode each env finished last.
    fn episode_info(&self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let o = out.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let n = self.env.len();
        if o.len() < n * 4 {
            return Err(PyValueError::new_err("buffer shorter than n_envs * 4"));
        }
        let mut info = vec![sts2env::EpisodeInfo::default(); n];
        self.env.episode_info(&mut info);
        for (k, i) in info.iter().enumerate() {
            o[k * 4] = i.scen as f32;
            o[k * 4 + 1] = i.hp_lost;
            o[k * 4 + 2] = i.hp_end;
            o[k * 4 + 3] = i.len as f32;
        }
        Ok(())
    }
}

/// The determinized play-out search as a continuous-batching state machine (`sts2env::search`): the caller only evaluates networks.
#[pyclass]
struct SearchEnginePy {
    eng: sts2env::search::SearchEngine,
}

#[pymethods]
impl SearchEnginePy {
    #[new]
    #[pyo3(signature = (scenarios_json, job_scen, job_seed, n_roots, m, k, conf, pmin, margin, roll_cap, max_steps, win, loss, hp_bonus, threads))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        scenarios_json: Vec<String>,
        job_scen: PyReadonlyArray1<u32>,
        job_seed: PyReadonlyArray1<u64>,
        n_roots: usize,
        m: usize,
        k: usize,
        conf: f32,
        pmin: f32,
        margin: f32,
        roll_cap: u32,
        max_steps: u32,
        win: f32,
        loss: f32,
        hp_bonus: f32,
        threads: usize,
    ) -> PyResult<Self> {
        let mut scs = vec![];
        for s in scenarios_json {
            let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))?;
            let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
            sc.validate().map_err(|e| PyValueError::new_err(format!("scenario uses unported content: {e:?}")))?;
            scs.push((sc, ex));
        }
        let e = |x: numpy::NotContiguousError| PyValueError::new_err(x.to_string());
        let jobs: Vec<(u32, u64)> = job_scen.as_slice().map_err(e)?.iter().copied().zip(job_seed.as_slice().map_err(e)?.iter().copied()).collect();
        let cfg = sts2env::search::SearchCfg { m, k, conf, pmin, margin, roll_cap, max_steps, win, loss, hp_bonus };
        let eng = sts2env::search::SearchEngine::new(scs, jobs, n_roots, cfg, threads).map_err(|e| PyValueError::new_err(format!("cannot create the search engine: {e:?}")))?;
        Ok(SearchEnginePy { eng })
    }

    /// `(max policy rows, max value rows)` one `advance` can request: the sizes of the request buffers.
    fn max_rows(&self) -> (usize, usize) {
        self.eng.max_rows()
    }

    fn n_roots(&self) -> usize {
        self.eng.n_roots()
    }

    fn finished(&self) -> bool {
        self.eng.finished()
    }

    /// One cycle (see `sts2env::search::SearchEngine::advance`); the first call passes `pol = val = None`.
    #[pyo3(signature = (pol_obs, pol_mask, pol_kind, val_obs, val_kind, pol=None, val=None))]
    fn advance(
        &mut self,
        py: Python<'_>,
        mut pol_obs: PyReadwriteArray2<f32>,
        mut pol_mask: PyReadwriteArray2<u8>,
        mut pol_kind: PyReadwriteArray1<u8>,
        mut val_obs: PyReadwriteArray2<f32>,
        mut val_kind: PyReadwriteArray1<u8>,
        pol: Option<PyReadonlyArray2<f32>>,
        val: Option<PyReadonlyArray1<f32>>,
    ) -> PyResult<(usize, usize)> {
        let er = |x: numpy::NotContiguousError| PyValueError::new_err(x.to_string());
        let po = pol_obs.as_slice_mut().map_err(er)?;
        let pm = pol_mask.as_slice_mut().map_err(er)?;
        let pk = pol_kind.as_slice_mut().map_err(er)?;
        let vo = val_obs.as_slice_mut().map_err(er)?;
        let vk = val_kind.as_slice_mut().map_err(er)?;
        let pa = match &pol {
            Some(p) => Some(p.as_slice().map_err(er)?),
            None => None,
        };
        let va = match &val {
            Some(v) => Some(v.as_slice().map_err(er)?),
            None => None,
        };
        let eng = &mut self.eng;
        py.detach(|| eng.advance(pa, va, po, pm, pk, vo, vk)).map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }

    /// `[n_jobs, 6]` f32: scenario index, outcome, HP lost fraction, HP left fraction, length, finished (1/0).
    fn results(&self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let o = out.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let r = self.eng.results();
        if o.len() < r.len() * 6 {
            return Err(PyValueError::new_err("buffer shorter than n_jobs * 6"));
        }
        for (k, j) in r.iter().enumerate() {
            o[k * 6..k * 6 + 6].copy_from_slice(&[j.scen as f32, j.outcome as f32, j.hp_lost, j.hp_end, j.len as f32, j.done as u8 as f32]);
        }
        Ok(())
    }

    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let s = self.eng.stats();
        let d = pyo3::types::PyDict::new(py);
        d.set_item("root_decisions", s.root_decisions)?;
        d.set_item("searched", s.searched)?;
        d.set_item("forced_root", s.forced_root)?;
        d.set_item("sim_steps", s.sim_steps)?;
        d.set_item("forced_sim", s.forced_sim)?;
        d.set_item("policy_rows", s.policy_rows)?;
        d.set_item("value_rows", s.value_rows)?;
        d.set_item("forks", s.forks)?;
        d.set_item("illegal", s.illegal)?;
        d.set_item("end_turn", s.end_turn)?;
        d.set_item("end_term", s.end_term)?;
        d.set_item("end_cap", s.end_cap)?;
        d.set_item("end_stuck", s.end_stuck)?;
        Ok(d)
    }
}

#[pyfunction]
fn obs_size() -> usize {
    sts2env::OBS
}

#[pyfunction]
fn action_space() -> usize {
    sts2env::ACTIONS
}

/// `None` when nothing is proven; otherwise a sentence explaining why the fight cannot be won (see `sts2sim::bounds`).
#[pyfunction]
fn provably_unwinnable(scenario_json: &str) -> PyResult<Option<String>> {
    let v: serde_json::Value = serde_json::from_str(scenario_json).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
    Ok(sts2sim::bounds::provably_unwinnable(&sc, &ex).map(|p| p.describe()))
}

/// Display names (SCREAMING_SNAKE ids) of cards, powers, relics, potions, monsters, encounters, enchantments, afflictions, orbs, indexed by id.
#[pyfunction]
fn names(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyDict>> {
    use sts2sim::ids;
    let d = pyo3::types::PyDict::new(py);
    d.set_item("card", ids::card::NAMES.to_vec())?;
    d.set_item("power", ids::power::NAMES.to_vec())?;
    d.set_item("relic", ids::relic::NAMES.to_vec())?;
    d.set_item("potion", ids::potion::NAMES.to_vec())?;
    d.set_item("monster", ids::monster::NAMES.to_vec())?;
    d.set_item("encounter", ids::encounter::NAMES.to_vec())?;
    d.set_item("enchantment", ids::enchantment::NAMES.to_vec())?;
    d.set_item("affliction", ids::affliction::NAMES.to_vec())?;
    d.set_item("orb", ids::orb::NAMES.to_vec())?;
    Ok(d)
}

/// Observation layout and action-space constants: `{"sections": [(name, offset, size)], "consts": {name: value}}`.
#[pyfunction]
fn layout(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyDict>> {
    use pyo3::types::{PyDict, PyList};
    let d = PyDict::new(py);
    let secs = PyList::empty(py);
    for (name, off, size) in sts2sim::observe::layout() {
        secs.append((name, off, size))?;
    }
    d.set_item("sections", secs)?;
    let c = PyDict::new(py);
    for (name, v) in sts2sim::observe::layout_consts() {
        c.set_item(name, v)?;
    }
    d.set_item("consts", c)?;
    Ok(d)
}

#[pymodule]
fn _sts2(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchEnvPy>()?;
    m.add_class::<SearchEnginePy>()?;
    m.add("BatchEnv", m.getattr("BatchEnvPy")?)?;
    m.add_function(wrap_pyfunction!(obs_size, m)?)?;
    m.add_function(wrap_pyfunction!(action_space, m)?)?;
    m.add_function(wrap_pyfunction!(layout, m)?)?;
    m.add_function(wrap_pyfunction!(names, m)?)?;
    m.add_function(wrap_pyfunction!(provably_unwinnable, m)?)?;
    // `outcome` codes of `step` (set when `done`)
    m.add("OUTCOME_ONGOING", sts2env::OUTCOME_ONGOING)?;
    m.add("OUTCOME_WIN", sts2env::OUTCOME_WIN)?;
    m.add("OUTCOME_LOSS", sts2env::OUTCOME_LOSS)?;
    m.add("OUTCOME_TRUNCATED", sts2env::OUTCOME_TRUNCATED)?;
    m.add("OUTCOME_UNIMPLEMENTED", sts2env::OUTCOME_UNIMPLEMENTED)?;
    m.add("OUTCOME_OVERFLOW", sts2env::OUTCOME_OVERFLOW)?;
    Ok(())
}
