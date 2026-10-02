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
        Ok(BatchEnvPy { env: BatchEnv::new(n_envs, Box::new(PoolScenario::new(scs)), cfg, max_steps, seed) })
    }

    fn observe_all(&self, py: Python<'_>, mut obs: PyReadwriteArray2<f32>, mut mask: PyReadwriteArray2<u8>) -> PyResult<()> {
        let o = obs.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let m = mask.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        py.detach(|| self.env.observe_all(o, m));
        Ok(())
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
        py.detach(|| env.step(a, StepOut { obs: o, mask: m, reward: r, done: d, outcome: oc, illegal: il }));
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

#[pymodule]
fn _sts2(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchEnvPy>()?;
    m.add("BatchEnv", m.getattr("BatchEnvPy")?)?;
    m.add_function(wrap_pyfunction!(obs_size, m)?)?;
    m.add_function(wrap_pyfunction!(action_space, m)?)?;
    Ok(())
}
