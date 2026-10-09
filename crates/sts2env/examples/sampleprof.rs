#[cfg(not(windows))]
fn main() {
    eprintln!("sampleprof is Windows only (use perf / callgrind elsewhere)");
}

#[cfg(windows)]
fn main() {
    win::main();
}

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use sts2env::search::*;
    use sts2env::*;
    use sts2sim::engine::ACTION_SPACE;

    type Handle = *mut c_void;

    #[repr(C)]
    struct ThreadEntry32 {
        size: u32,
        usage: u32,
        thread_id: u32,
        owner_pid: u32,
        base_pri: i32,
        delta_pri: i32,
        flags: u32,
    }

    #[repr(C, align(16))]
    struct Context {
        bytes: [u8; 1232],
    }

    #[repr(C)]
    struct SymbolInfo {
        size_of_struct: u32,
        type_index: u32,
        reserved: [u64; 2],
        index: u32,
        size: u32,
        mod_base: u64,
        flags: u32,
        value: u64,
        address: u64,
        register: u32,
        scope: u32,
        tag: u32,
        name_len: u32,
        max_name_len: u32,
        name: [u8; 512],
    }

    #[repr(C)]
    struct Line64 {
        size_of_struct: u32,
        key: *mut c_void,
        line: u32,
        file: *const u8,
        address: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn GetCurrentProcessId() -> u32;
        fn GetCurrentThreadId() -> u32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Thread32First(snap: Handle, e: *mut ThreadEntry32) -> i32;
        fn Thread32Next(snap: Handle, e: *mut ThreadEntry32) -> i32;
        fn OpenThread(access: u32, inherit: i32, id: u32) -> Handle;
        fn SuspendThread(h: Handle) -> u32;
        fn ResumeThread(h: Handle) -> u32;
        fn GetThreadContext(h: Handle, c: *mut Context) -> i32;
        fn CloseHandle(h: Handle) -> i32;
        fn Sleep(ms: u32);
    }
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(ms: u32) -> u32;
    }
    #[link(name = "dbghelp")]
    extern "system" {
        fn SymSetOptions(o: u32) -> u32;
        fn SymInitialize(p: Handle, path: *const u8, invade: i32) -> i32;
        fn SymFromAddr(p: Handle, addr: u64, disp: *mut u64, s: *mut SymbolInfo) -> i32;
        fn SymGetLineFromAddr64(p: Handle, addr: u64, disp: *mut u32, l: *mut Line64) -> i32;
    }

    const TH32CS_SNAPTHREAD: u32 = 0x4;
    const THREAD_SUSPEND_RESUME: u32 = 0x2;
    const THREAD_GET_CONTEXT: u32 = 0x8;
    const CONTEXT_CONTROL: u32 = 0x0010_0001;

    fn threads(me: u32) -> Vec<u32> {
        let mut v = vec![];
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            let pid = GetCurrentProcessId();
            let mut e = ThreadEntry32 { size: std::mem::size_of::<ThreadEntry32>() as u32, usage: 0, thread_id: 0, owner_pid: 0, base_pri: 0, delta_pri: 0, flags: 0 };
            let mut ok = Thread32First(snap, &mut e);
            while ok != 0 {
                if e.owner_pid == pid && e.thread_id != me {
                    v.push(e.thread_id);
                }
                ok = Thread32Next(snap, &mut e);
            }
            CloseHandle(snap);
        }
        v
    }

    fn sampler(stop: Arc<AtomicBool>) -> Vec<u64> {
        let me = unsafe { GetCurrentThreadId() };
        let mut out = vec![];
        let mut handles: HashMap<u32, Handle> = HashMap::new();
        let mut tick = 0u32;
        unsafe { timeBeginPeriod(1) };
        while !stop.load(Ordering::Relaxed) {
            if tick % 100 == 0 {
                for id in threads(me) {
                    handles.entry(id).or_insert_with(|| unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, 0, id) });
                }
            }
            tick += 1;
            for (_, &h) in handles.iter() {
                if h.is_null() {
                    continue;
                }
                unsafe {
                    if SuspendThread(h) == u32::MAX {
                        continue;
                    }
                    let mut c = Context { bytes: [0; 1232] };
                    c.bytes[0x30..0x34].copy_from_slice(&CONTEXT_CONTROL.to_le_bytes());
                    if GetThreadContext(h, &mut c) != 0 {
                        let rip = u64::from_le_bytes(c.bytes[0xF8..0x100].try_into().unwrap());
                        out.push(rip);
                    }
                    ResumeThread(h);
                }
            }
            unsafe { Sleep(1) };
        }
        out
    }

    fn hash(o: &[f32]) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for (i, x) in o.iter().enumerate() {
            if x.to_bits() != 0 {
                h = (h ^ x.to_bits() as u64 ^ (i as u64) << 32).wrapping_mul(0x100000001b3);
            }
        }
        h ^ (h >> 29)
    }

    fn env_work(path: &str, n: usize, steps: usize) {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut scen = vec![];
        for sj in v.as_array().unwrap().iter().take(4000) {
            let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
            if sc.validate().is_ok() {
                scen.push((sc, ex));
            }
        }
        let cfg = RewardConfig { win: 1.0, loss: -1.0, hp_bonus: 0.5, turn_cap: 30 };
        let mut env = BatchEnv::try_new(n, Box::new(PoolScenario::with_extras(scen)), cfg, 600, 1000).unwrap();
        let osz = OBS;
        let (mut obs, mut mask) = (vec![0f32; n * osz], vec![0u8; n * ACTION_SPACE]);
        let (mut rew, mut done, mut oc, mut ill) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
        env.observe_all(&mut obs, &mut mask).unwrap();
        let mut acts = vec![0i32; n];
        for _ in 0..steps {
            for i in 0..n {
                let hr = hash(&obs[i * osz..(i + 1) * osz]);
                let m = &mask[i * ACTION_SPACE..(i + 1) * ACTION_SPACE];
                let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| m[a] > 0).collect();
                acts[i] = legal[((hr >> 17) % legal.len() as u64) as usize] as i32;
            }
            env.step(&acts, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut rew, done: &mut done, outcome: &mut oc, illegal: &mut ill }).unwrap();
        }
    }

    fn search_work(path: &str, n_fights: usize, roots: usize) {
        let (m, k) = (5, 32);
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut scen = vec![];
        for sj in v.as_array().unwrap() {
            let sj = sj.get("scenario").filter(|_| sj.get("deck").is_none()).unwrap_or(sj);
            let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
            if sc.validate().is_ok() {
                scen.push((sc, ex));
            }
            if scen.len() == n_fights {
                break;
            }
        }
        let jobs: Vec<(u32, u64)> = (0..scen.len()).map(|i| (i as u32, 101 * 1_000_003 + i as u64)).collect();
        let cfg = SearchCfg { m, k, roll_cap: 120, leaf_turns: 2, turn_cap: 30, val_w: HEAD_NC, ..SearchCfg::default() };
        let mut eng = SearchEngine::new(scen, jobs, roots, cfg, 1, true).unwrap();
        let osz = OBS;
        let cap = eng.shared_rows();
        let (mut obs, mut mask, mut pk, mut pu, mut vk) = (vec![0f32; cap * osz], vec![0u8; cap * ACTION_SPACE], vec![0u8; cap], vec![0f32; cap], vec![0u8; cap]);
        let stride = 2 * m + 1;
        let (mut pol, mut val) = (vec![0f32; cap * stride], vec![0f32; cap * HEAD_NC]);
        let (mut np, mut nv) = eng.advance_shared(None, None, &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
        while np + nv > 0 {
            for r in 0..np {
                let h = hash(&obs[r * osz..(r + 1) * osz]);
                let mk = &mask[r * ACTION_SPACE..(r + 1) * ACTION_SPACE];
                let mut legal: Vec<(u64, usize)> = (0..ACTION_SPACE).filter(|&a| mk[a] > 0).map(|a| ((h ^ a as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 16, a)).collect();
                legal.sort_unstable();
                let o = &mut pol[r * stride..(r + 1) * stride];
                for j in 0..m {
                    o[j] = legal.get(j).map_or(0, |x| x.1) as f32;
                    o[m + j] = if j < legal.len() { 1.0 / legal.len().min(m) as f32 } else { 0.0 };
                }
                o[2 * m] = legal[(h as usize >> 7) % legal.len()].1 as f32;
            }
            for r in 0..nv {
                let row = cap - 1 - r;
                let h = hash(&obs[row * osz..(row + 1) * osz]);
                let p = &mut val[r * HEAD_NC..(r + 1) * HEAD_NC];
                p.fill(0.0);
                p[0] = (h % 1000) as f32 / 1000.0;
                p[1] = 1.0 - p[0];
            }
            (np, nv) = eng.advance_shared(Some(&pol[..np * stride]), Some(&val[..nv * HEAD_NC]), &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
        }
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().collect();
        let mode = args.get(1).expect("env | search").clone();
        let path = args.get(2).expect("scenario json").clone();
        let arg = |i: usize, d: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let t = std::thread::spawn(move || sampler(s2));
        let t0 = std::time::Instant::now();
        if mode == "env" {
            env_work(&path, arg(3, 256), arg(4, 200));
        } else {
            search_work(&path, arg(3, 32), arg(4, 16));
        }
        stop.store(true, Ordering::Relaxed);
        let samples = t.join().unwrap();
        println!("{mode}: {:.2}s, {} samples", t0.elapsed().as_secs_f64(), samples.len());
        let proc_ = unsafe { GetCurrentProcess() };
        unsafe {
            SymSetOptions(0x2 | 0x10);
            let dir = std::env::current_exe().unwrap().parent().unwrap().to_string_lossy().into_owned() + " ";
            SymInitialize(proc_, dir.as_ptr(), 1);
        }
        let mut by_fn: HashMap<String, u64> = HashMap::new();
        let mut by_line: HashMap<String, u64> = HashMap::new();
        let mut cache: HashMap<u64, (String, String)> = HashMap::new();
        for &ip in samples.iter() {
            let (f, l) = cache
                .entry(ip)
                .or_insert_with(|| unsafe {
                    let mut s: SymbolInfo = std::mem::zeroed();
                    s.size_of_struct = 88;
                    s.max_name_len = 511;
                    let mut disp = 0u64;
                    let name = if SymFromAddr(proc_, ip, &mut disp, &mut s) != 0 {
                        String::from_utf8_lossy(&s.name[..s.name_len.min(511) as usize]).into_owned()
                    } else {
                        format!("?{ip:x}")
                    };
                    let mut ln: Line64 = std::mem::zeroed();
                    ln.size_of_struct = std::mem::size_of::<Line64>() as u32;
                    let mut d32 = 0u32;
                    let line = if SymGetLineFromAddr64(proc_, ip, &mut d32, &mut ln) != 0 {
                        let file = std::ffi::CStr::from_ptr(ln.file as *const std::ffi::c_char).to_string_lossy().into_owned();
                        let short = file.rsplit(['\\', '/']).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/");
                        format!("{short}:{}", ln.line)
                    } else {
                        String::from("?")
                    };
                    (name, line)
                })
                .clone();
            if !(f.contains("Wait") || f.contains("NtDelayExecution") || f.contains("ZwDelayExecution")) {
                *by_line.entry(l).or_default() += 1;
            }
            *by_fn.entry(f).or_default() += 1;
        }
        let idle: u64 = by_fn.iter().filter(|(f, _)| f.contains("Wait") || f.contains("NtDelayExecution") || f.contains("ZwDelayExecution")).map(|(_, n)| *n).sum();
        by_fn.retain(|f, _| !(f.contains("Wait") || f.contains("NtDelayExecution") || f.contains("ZwDelayExecution")));
        let tot = (samples.len() as u64 - idle).max(1) as f64;
        println!("{} busy samples ({idle} idle)", tot);
        let mut v: Vec<_> = by_fn.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        println!("--- functions (self) ---");
        for (f, n) in v.iter().take(arg(6, 45)) {
            println!("{:6.2}%  {}", 100.0 * *n as f64 / tot, f.chars().take(150).collect::<String>());
        }
        let mut v: Vec<_> = by_line.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        println!("--- lines ---");
        for (f, n) in v.iter().take(40) {
            println!("{:6.2}%  {f}", 100.0 * *n as f64 / tot);
        }
    }
}
