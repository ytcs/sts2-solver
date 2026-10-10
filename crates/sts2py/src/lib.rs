use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyReadwriteArray1, PyReadwriteArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
mod sim;
use sts2env::{BatchEnv, PoolScenario, RewardConfig, RoundRobinScenario, StepOut};

fn parse_scenarios(py: Python<'_>, scenarios_json: &[String], validate: bool) -> PyResult<Vec<(sts2sim::Scenario, sts2sim::ScenarioExtras)>> {
    use rayon::prelude::*;
    let parsed: Vec<Result<(sts2sim::Scenario, sts2sim::ScenarioExtras), String>> = py.detach(|| {
        scenarios_json
            .par_iter()
            .map(|s| {
                let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
                let (sc, ex) = sts2diff::convert::scenario_ex(&v)?;
                if validate {
                    sc.validate().map_err(|e| format!("scenario uses unported content: {e:?}"))?;
                }
                Ok((sc, ex))
            })
            .collect()
    });
    parsed.into_iter().map(|r| r.map_err(PyValueError::new_err)).collect()
}

#[pyclass]
struct BatchEnvPy {
    env: BatchEnv,
}

#[pymethods]
impl BatchEnvPy {
    #[new]
    #[pyo3(signature = (n_envs, scenarios_json, seed, max_steps, win, loss, hp_bonus, round_robin=false, turn_cap=0))]
    #[allow(clippy::too_many_arguments)]
    fn new(py: Python<'_>, n_envs: usize, scenarios_json: Vec<String>, seed: u64, max_steps: u32, win: f32, loss: f32, hp_bonus: f32, round_robin: bool, turn_cap: u32) -> PyResult<Self> {
        let scs = parse_scenarios(py, &scenarios_json, true)?;
        let cfg = RewardConfig { win, loss, hp_bonus, turn_cap };
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

    fn set_weights(&mut self, w: PyReadonlyArray1<f32>) -> PyResult<()> {
        let w = w.as_slice().map_err(|e| PyValueError::new_err(e.to_string()))?;
        if self.env.set_weights(w) { Ok(()) } else { Err(PyValueError::new_err("weights need a pool source with one weight per scenario")) }
    }

    fn episode_info(&self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let o = out.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let n = self.env.len();
        if o.len() < n * 7 {
            return Err(PyValueError::new_err("buffer shorter than n_envs * 7"));
        }
        let mut info = vec![sts2env::EpisodeInfo::default(); n];
        self.env.episode_info(&mut info);
        for (k, i) in info.iter().enumerate() {
            o[k * 7..k * 7 + 7].copy_from_slice(&[i.scen as f32, i.hp_lost, i.hp_end, i.len as f32, i.hp_end_abs as f32, i.max_hp_end as f32, i.turns as f32]);
        }
        Ok(())
    }
}

#[pyclass]
struct FightStartsPy {
    starts: sts2env::FightStarts,
}

#[pymethods]
impl FightStartsPy {
    #[new]
    fn new(py: Python<'_>, scenarios_json: Vec<String>) -> PyResult<Self> {
        let scs = parse_scenarios(py, &scenarios_json, true)?;
        let starts = sts2env::FightStarts::try_new(scs).map_err(|e| PyValueError::new_err(format!("cannot create the fight starts: {e:?}")))?;
        Ok(FightStartsPy { starts })
    }

    fn __len__(&self) -> usize {
        self.starts.len()
    }

    fn observe(&self, py: Python<'_>, seed: u64, mut obs: PyReadwriteArray2<f32>, mut mask: PyReadwriteArray2<u8>) -> PyResult<()> {
        let o = obs.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let m = mask.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let st = &self.starts;
        py.detach(|| st.observe(seed, o, m)).map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }
}

#[pyclass]
struct SearchEnginePy {
    eng: sts2env::search::SearchEngine,
}

#[pymethods]
impl SearchEnginePy {
    #[new]
    #[pyo3(signature = (scenarios_json, job_scen, job_seed, n_roots, m, k, roll_cap, max_steps, win, loss, hp_bonus, threads, record=false, starts=None, leaf_turns=1, turn_cap=0, val_w=1, worth=None, clairvoyant=false, cover=false, futures=0, exact=false, ex_loss=-0.9, ex_tie=0.0, ex_dets=8, ex_cap=5000, hp_cap=false, pot_cost=0.0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        scenarios_json: Vec<String>,
        job_scen: PyReadonlyArray1<u32>,
        job_seed: PyReadonlyArray1<u64>,
        n_roots: usize,
        m: usize,
        k: usize,
        roll_cap: u32,
        max_steps: u32,
        win: f32,
        loss: f32,
        hp_bonus: f32,
        threads: usize,
        record: bool,
        starts: Option<Vec<Option<PyRef<'_, sim::Sim>>>>,
        leaf_turns: u32,
        turn_cap: u32,
        val_w: usize,
        worth: Option<PyReadonlyArray2<f32>>,
        // DIAGNOSTIC ONLY: sees hidden information; never for live play.
        clairvoyant: bool,
        cover: bool,
        futures: usize,
        exact: bool,
        ex_loss: f32,
        ex_tie: f32,
        ex_dets: usize,
        ex_cap: usize,
        hp_cap: bool,
        pot_cost: f32,
    ) -> PyResult<Self> {
        let scs = parse_scenarios(py, &scenarios_json, true)?;
        let e = |x: numpy::NotContiguousError| PyValueError::new_err(x.to_string());
        let jobs: Vec<(u32, u64)> = job_scen.as_slice().map_err(e)?.iter().copied().zip(job_seed.as_slice().map_err(e)?.iter().copied()).collect();
        let cfg = sts2env::search::SearchCfg { m, k, roll_cap, leaf_turns, max_steps, win, loss, hp_bonus, turn_cap, val_w, cover, futures, clairvoyant, exact: sts2env::search::ExactCfg { on: exact, loss: ex_loss, tie: ex_tie, dets: ex_dets, cap: ex_cap }, hp_cap, pot_cost };
        let starts: Vec<Option<sts2sim::Combat>> = starts.unwrap_or_default().into_iter().map(|o| o.map(|s| s.cx.clone())).collect();
        let n_scen = scs.len();
        let mut eng = sts2env::search::SearchEngine::new_with_starts(scs, starts, jobs, n_roots, cfg, threads, record).map_err(|e| PyValueError::new_err(format!("cannot create the search engine: {e:?}")))?;
        if let Some(wa) = worth {
            use sts2env::search::{Worth, HEAD_NC};
            let w = wa.as_slice().map_err(e)?;
            let width = 1 + HEAD_NC;
            if w.len() != n_scen * width {
                return Err(PyValueError::new_err(format!("worth must be [n_scenarios, {width}] (flag, {HEAD_NC} class worths)")));
            }
            let ws = (0..n_scen)
                .map(|i| {
                    let r = &w[i * width..(i + 1) * width];
                    let mut x = Worth::linear();
                    x.table = r[0] != 0.0;
                    x.u.copy_from_slice(&r[1..]);
                    x
                })
                .collect();
            eng.set_worth(ws).map_err(|e| PyValueError::new_err(format!("{e:?}")))?;
        }
        Ok(SearchEnginePy { eng })
    }

    fn finished(&self) -> bool {
        self.eng.finished()
    }

    fn shared_rows(&self) -> usize {
        self.eng.shared_rows()
    }

    #[pyo3(signature = (obs, mask, pol_kind, pol_u, val_kind, pol=None, val=None))]
    #[allow(clippy::too_many_arguments)]
    fn advance_shared(
        &mut self,
        py: Python<'_>,
        mut obs: PyReadwriteArray2<f32>,
        mut mask: PyReadwriteArray2<u8>,
        mut pol_kind: PyReadwriteArray1<u8>,
        mut pol_u: PyReadwriteArray1<f32>,
        mut val_kind: PyReadwriteArray1<u8>,
        pol: Option<PyReadonlyArray2<f32>>,
        val: Option<PyReadonlyArray1<f32>>,
    ) -> PyResult<(usize, usize)> {
        let er = |x: numpy::NotContiguousError| PyValueError::new_err(x.to_string());
        let o = obs.as_slice_mut().map_err(er)?;
        let pm = mask.as_slice_mut().map_err(er)?;
        let pk = pol_kind.as_slice_mut().map_err(er)?;
        let pu = pol_u.as_slice_mut().map_err(er)?;
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
        py.detach(|| eng.advance_shared(pa, va, o, pm, pk, pu, vk)).map_err(|e| PyValueError::new_err(format!("{e:?}")))
    }

    fn results(&self, mut out: PyReadwriteArray2<f32>) -> PyResult<()> {
        let w = out.as_array().ncols();
        let o = out.as_slice_mut().map_err(|e| PyValueError::new_err(e.to_string()))?;
        let r = self.eng.results();
        if !(6..=8).contains(&w) || o.len() < r.len() * w {
            return Err(PyValueError::new_err("buffer must be [n_jobs, 6..8]"));
        }
        for (k, j) in r.iter().enumerate() {
            let row = [j.scen as f32, j.outcome as f32, j.hp_lost, j.hp_end, j.len as f32, j.done as u8 as f32, j.hp_end_abs as f32, j.pot_kept as f32];
            o[k * w..k * w + w].copy_from_slice(&row[..w]);
        }
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn moves<'py>(&self, py: Python<'py>, job: usize) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
        use numpy::{PyArray1, PyArrayMethods};
        let mv = self.eng.moves(job);
        let m = sts2env::search::MAX_M;
        let a: Vec<i32> = mv.iter().map(|x| x.action as i32).collect();
        let sr: Vec<u8> = mv.iter().map(|x| x.searched as u8).collect();
        let opts: Vec<i32> = mv.iter().flat_map(|x| x.opts.iter().map(|&o| o as i32)).collect();
        let p: Vec<f32> = mv.iter().flat_map(|x| x.p.iter().copied()).collect();
        let q: Vec<f32> = mv.iter().flat_map(|x| x.q.iter().copied()).collect();
        let lg: Vec<u8> = mv.iter().flat_map(|x| x.legal.iter().map(|&b| b as u8)).collect();
        let ex: Vec<u8> = mv.iter().map(|x| x.exact as u8).collect();
        let n = mv.len();
        let t = pyo3::types::PyTuple::new(
            py,
            [
                PyArray1::from_vec(py, a).into_any(),
                PyArray1::from_vec(py, sr).into_any(),
                PyArray1::from_vec(py, opts).reshape([n, m])?.into_any(),
                PyArray1::from_vec(py, p).reshape([n, m])?.into_any(),
                PyArray1::from_vec(py, q).reshape([n, m])?.into_any(),
                PyArray1::from_vec(py, lg).reshape([n, m])?.into_any(),
                PyArray1::from_vec(py, ex).into_any(),
            ],
        )?;
        Ok(t)
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
        d.set_item("panics", s.panics)?;
        d.set_item("end_turn", s.end_turn)?;
        d.set_item("end_term", s.end_term)?;
        d.set_item("end_cap", s.end_cap)?;
        d.set_item("end_stuck", s.end_stuck)?;
        d.set_item("end_loop", s.end_loop)?;
        d.set_item("fight_loops", s.fight_loops)?;
        d.set_item("lead_branch", s.lead_branch)?;
        d.set_item("lead_clean", s.lead_clean)?;
        d.set_item("carried", s.carried)?;
        d.set_item("lead_prefix_steps", s.lead_prefix_steps)?;
        d.set_item("lead_first_unclean", s.lead_first_unclean)?;
        d.set_item("cy_step", s.cy_step)?;
        d.set_item("cy_legal", s.cy_legal)?;
        d.set_item("cy_obs", s.cy_obs)?;
        d.set_item("cy_fork", s.cy_fork)?;
        d.set_item("cy_main", s.cy_main)?;
        d.set_item("cy_endturn", s.cy_endturn)?;
        d.set_item("n_endturn", s.n_endturn)?;
        d.set_item("cover_actions", s.cover_actions)?;
        d.set_item("cover_classes", s.cover_classes)?;
        d.set_item("cover_capped", s.cover_capped)?;
        d.set_item("ex_triggered", s.ex_triggered)?;
        d.set_item("ex_done", s.ex_done)?;
        d.set_item("ex_capped", s.ex_capped)?;
        d.set_item("ex_changed", s.ex_changed)?;
        d.set_item("ex_states", s.ex_states)?;
        d.set_item("ex_rows", s.ex_rows)?;
        d.set_item("cy_exact", s.cy_exact)?;
        d.set_item("cap_roots", s.cap_roots)?;
        d.set_item("cap_skipped", s.cap_skipped)?;
        Ok(d)
    }
}

#[pyfunction]
fn replay<'py>(py: Python<'py>, scenario_json: &str, seed: u64, actions: PyReadonlyArray1<i32>) -> PyResult<(Bound<'py, numpy::PyArray2<f32>>, Bound<'py, numpy::PyArray2<u8>>)> {
    use numpy::{PyArray1, PyArrayMethods};
    let osz = sts2env::OBS;
    let v: serde_json::Value = serde_json::from_str(scenario_json).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let scen = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
    let acts: Vec<u16> = actions.as_slice().map_err(|e| PyValueError::new_err(e.to_string()))?.iter().map(|&a| a as u16).collect();
    let n = acts.len() + 1;
    let mut obs = vec![0f32; n * osz];
    let mut mask = vec![0u8; n * sts2env::ACTIONS];
    sts2env::search::replay(&scen, seed, &acts, &mut obs, &mut mask).map_err(|e| PyValueError::new_err(format!("{e:?}")))?;
    Ok((PyArray1::from_vec(py, obs).reshape([n, osz])?, PyArray1::from_vec(py, mask).reshape([n, sts2env::ACTIONS])?))
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn replay_rows<'py>(py: Python<'py>, scenarios: Vec<String>, scen: Vec<u32>, seeds: Vec<u64>, actions: PyReadonlyArray1<i32>, off: Vec<usize>,
                    steps: Vec<u32>, soff: Vec<usize>) -> PyResult<(Bound<'py, numpy::PyArray2<f32>>, Bound<'py, numpy::PyArray2<u8>>)> {
    use numpy::PyArrayMethods;
    use rayon::prelude::*;
    let o = sts2env::OBS;
    let parsed = parse_scenarios(py, &scenarios, false)?;
    let acts: Vec<u16> = actions.as_slice().map_err(|e| PyValueError::new_err(e.to_string()))?.iter().map(|&a| a as u16).collect();
    let a = sts2env::ACTIONS;
    let n = seeds.len();
    if scen.len() < n || off.len() <= n || soff.len() <= n || soff[n] > steps.len() || off[n] > acts.len() || scen.iter().take(n).any(|&s| s as usize >= parsed.len()) {
        return Err(PyValueError::new_err("replay_rows: scen / off / soff do not match the fights"));
    }
    let r = soff[n] - soff[0];
    let obs_arr = numpy::PyArray2::<f32>::zeros(py, [r, o], false);
    let mask_arr = numpy::PyArray2::<u8>::zeros(py, [r, a], false);
    {
        // SAFETY: both arrays were just created here and are not shared with Python code while it runs
        let (obs, mask) = unsafe { (obs_arr.as_slice_mut().expect("contiguous"), mask_arr.as_slice_mut().expect("contiguous")) };
        let mut parts = Vec::with_capacity(n);
        let (mut ro, mut rm) = (obs, mask);
        for i in 0..n {
            let k = soff[i + 1].checked_sub(soff[i]).ok_or_else(|| PyValueError::new_err("replay_rows: soff must not decrease"))?;
            let (ho, to) = std::mem::take(&mut ro).split_at_mut(k * o);
            let (hm, tm) = std::mem::take(&mut rm).split_at_mut(k * a);
            (ro, rm) = (to, tm);
            parts.push((i, ho, hm));
        }
        let res: Result<(), String> = py.detach(|| {
            parts.into_par_iter().try_for_each(|(i, ho, hm)| {
                let fa = acts.get(off[i]..off[i + 1]).ok_or_else(|| format!("fight {i}: bad action offsets"))?;
                sts2env::search::replay_steps(&parsed[scen[i] as usize], seeds[i], fa, &steps[soff[i]..soff[i + 1]], ho, hm).map_err(|e| format!("fight {i}: {e:?}"))
            })
        });
        res.map_err(PyValueError::new_err)?;
    }
    Ok((obs_arr, mask_arr))
}

#[pyfunction]
fn obs_size() -> usize {
    sts2env::OBS
}

#[pyfunction]
fn action_space() -> usize {
    sts2env::ACTIONS
}

#[pyfunction]
fn provably_unwinnable(scenario_json: &str) -> PyResult<Option<String>> {
    let v: serde_json::Value = serde_json::from_str(scenario_json).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let (sc, ex) = sts2diff::convert::scenario_ex(&v).map_err(PyValueError::new_err)?;
    Ok(sts2sim::bounds::provably_unwinnable(&sc, &ex).map(|p| p.describe()))
}

#[pyfunction]
fn names(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyDict>> {
    use sts2sim::ids;
    let d = pyo3::types::PyDict::new(py);
    d.set_item("head_nc", sts2env::search::HEAD_NC)?;
    d.set_item("head_bin", sts2env::search::HEAD_BIN)?;
    d.set_item("max_m", sts2env::search::MAX_M)?;
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
    m.add_class::<FightStartsPy>()?;
    m.add_class::<sim::Sim>()?;
    m.add("BatchEnv", m.getattr("BatchEnvPy")?)?;
    m.add_function(wrap_pyfunction!(obs_size, m)?)?;
    m.add_function(wrap_pyfunction!(replay, m)?)?;
    m.add_function(wrap_pyfunction!(replay_rows, m)?)?;
    m.add_function(wrap_pyfunction!(action_space, m)?)?;
    m.add_function(wrap_pyfunction!(layout, m)?)?;
    m.add_function(wrap_pyfunction!(names, m)?)?;
    m.add_function(wrap_pyfunction!(provably_unwinnable, m)?)?;
    Ok(())
}
