use numpy::{PyReadonlyArray1, PyReadwriteArray1, PyReadwriteArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use sts2env::{BatchEnv, PoolScenario, RewardConfig, StepOut};

#[pyclass]
struct BatchEnvPy {
    env: BatchEnv,
}

#[pymethods]
impl BatchEnvPy {
    #[new]
    #[pyo3(signature = (n_envs, scenarios_json, seed, max_steps, win, loss, hp_bonus, step_reward))]
    fn new(n_envs: usize, scenarios_json: Vec<String>, seed: u64, max_steps: u32, win: f32, loss: f32, hp_bonus: f32, step_reward: f32) -> PyResult<Self> {
        let mut scs = vec![];
        for s in scenarios_json {
            let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))?;
            let sc = sts2diff::convert::scenario(&v).map_err(PyValueError::new_err)?;
            sc.validate().map_err(|e| PyValueError::new_err(format!("scenario uses unported content: {e:?}")))?;
            scs.push(sc);
        }
        let cfg = RewardConfig { win, loss, hp_bonus, step: step_reward };
        if scs.is_empty() {
            return Err(PyValueError::new_err("no scenarios"));
        }
        let env = BatchEnv::try_new(n_envs, Box::new(PoolScenario::new(scs)), cfg, max_steps, seed)
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

#[pyfunction]
fn obs_size() -> usize {
    sts2env::OBS
}

#[pyfunction]
fn action_space() -> usize {
    sts2env::ACTIONS
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
    m.add("BatchEnv", m.getattr("BatchEnvPy")?)?;
    m.add_function(wrap_pyfunction!(obs_size, m)?)?;
    m.add_function(wrap_pyfunction!(action_space, m)?)?;
    m.add_function(wrap_pyfunction!(layout, m)?)?;
    // `outcome` codes of `step` (set when `done`)
    m.add("OUTCOME_ONGOING", sts2env::OUTCOME_ONGOING)?;
    m.add("OUTCOME_WIN", sts2env::OUTCOME_WIN)?;
    m.add("OUTCOME_LOSS", sts2env::OUTCOME_LOSS)?;
    m.add("OUTCOME_TRUNCATED", sts2env::OUTCOME_TRUNCATED)?;
    m.add("OUTCOME_UNIMPLEMENTED", sts2env::OUTCOME_UNIMPLEMENTED)?;
    m.add("OUTCOME_OVERFLOW", sts2env::OUTCOME_OVERFLOW)?;
    Ok(())
}
