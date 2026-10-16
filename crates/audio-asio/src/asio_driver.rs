//! IASIO engine: real-time pump between the DAW's ASIO double buffers and the
//! physical sound card (WASAPI).
//!
//! Design (v2):
//! * The **sound card is the master clock.** The pump sleeps on the WASAPI
//!   engine event and only asks the DAW for a new block when the device queue
//!   drops below its target. The previous build paced on a 1 ms timeout and
//!   ran ~7x faster than real time at 512-sample buffers, which overflowed the
//!   FIFO and chopped the audio.
//! * DAW rate == device rate -> bit-perfect copy. Otherwise a stateful
//!   windowed-sinc resampler (no per-block phase reset, no clicks).
//! * Sample position / timestamps live in atomics outside the driver mutex,
//!   so `getSamplePosition` from inside `bufferSwitch` can never deadlock
//!   against `stop()`.
//! * Live meters / load / xruns are published to the control panel through
//!   the `live_status` shared-memory block.

use core::ffi::c_void;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::asio_types::*;
use crate::kernel_client::KernelAudioClient;
use crate::live_status::{LiveStatus, LiveStatusMap};
use crate::wasapi::{audible_sessions, ComGuard, DolomiteConfig, Engine, WasapiCapture, WasapiRender, AUDCLNT_E_DEVICE_INVALIDATED};
use audio_core::{ResamplerPhase, StreamResampler, MAX_AUDIO_CHANNELS};

#[derive(Copy, Clone)]
pub struct SendPtr(pub *mut f32);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

// ---------------------------------------------------------------------------
// Logging (never called from the steady-state audio path)
// ---------------------------------------------------------------------------

pub fn log_path() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".into());
    let dir = std::path::PathBuf::from(appdata).join("DolomiteAudio");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("asio_debug.log")
}

pub fn log_debug(msg: &str) {
    use std::io::Write;
    let path = log_path();
    // keep the log from growing forever
    if std::fs::metadata(&path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "[{}] {}", t, msg);
    }
}

// ---------------------------------------------------------------------------
// Win32
// ---------------------------------------------------------------------------

#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(uPeriod: u32) -> u32;
    fn timeEndPeriod(uPeriod: u32) -> u32;
    fn timeGetTime() -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn SetThreadPriority(hThread: *mut c_void, nPriority: i32) -> i32;
    fn GetCurrentThread() -> *mut c_void;
    fn Sleep(dwMilliseconds: u32);
    fn QueryPerformanceCounter(lpPerformanceCount: *mut i64) -> i32;
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

fn qpc() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceCounter(&mut v) };
    v
}

fn qpc_freq() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceFrequency(&mut v) };
    v.max(1)
}

type AvSetFn = unsafe extern "system" fn(*const u16, *mut u32) -> *mut c_void;
type AvRevertFn = unsafe extern "system" fn(*mut c_void) -> i32;
type AvPrioFn = unsafe extern "system" fn(*mut c_void, i32) -> i32;

struct Mmcss {
    module: *mut c_void,
    handle: *mut c_void,
    revert: Option<AvRevertFn>,
}

impl Mmcss {
    /// Register the current thread with MMCSS "Pro Audio" at CRITICAL priority.
    unsafe fn enable() -> Self {
        let module = LoadLibraryW(wide("avrt.dll").as_ptr());
        let mut out = Mmcss { module, handle: core::ptr::null_mut(), revert: None };
        if module.is_null() {
            return out;
        }
        let set = GetProcAddress(module, b"AvSetMmThreadCharacteristicsW\0".as_ptr());
        let rev = GetProcAddress(module, b"AvRevertMmThreadCharacteristics\0".as_ptr());
        let prio = GetProcAddress(module, b"AvSetMmThreadPriority\0".as_ptr());
        if set.is_null() || rev.is_null() {
            return out;
        }
        let set: AvSetFn = core::mem::transmute(set);
        out.revert = Some(core::mem::transmute(rev));
        let mut idx = 0u32;
        out.handle = set(wide("Pro Audio").as_ptr(), &mut idx);
        if !out.handle.is_null() && !prio.is_null() {
            let prio: AvPrioFn = core::mem::transmute(prio);
            prio(out.handle, 2 /* AVRT_PRIORITY_CRITICAL */);
        }
        out
    }
}

impl Drop for Mmcss {
    fn drop(&mut self) {
        unsafe {
            if let (Some(rev), false) = (self.revert, self.handle.is_null()) {
                rev(self.handle);
            }
            if !self.module.is_null() {
                FreeLibrary(self.module);
            }
        }
    }
}

#[repr(C)]
struct ThreadPowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}
type SetThreadInformationFn = unsafe extern "system" fn(*mut c_void, i32, *const c_void, u32) -> i32;

unsafe fn disable_power_throttling(h_thread: *mut c_void) {
    let k32 = LoadLibraryW(wide("kernel32.dll").as_ptr());
    if k32.is_null() {
        return;
    }
    let proc = GetProcAddress(k32, b"SetThreadInformation\0".as_ptr());
    if !proc.is_null() {
        let set_info: SetThreadInformationFn = core::mem::transmute(proc);
        let state = ThreadPowerThrottlingState { version: 1, control_mask: 0x1, state_mask: 0 };
        set_info(h_thread, 3 /* ThreadPowerThrottling */, &state as *const _ as *const c_void, core::mem::size_of::<ThreadPowerThrottlingState>() as u32);
    }
    FreeLibrary(k32);
}

// ---------------------------------------------------------------------------
// State shared lock-free between host threads and the pump
// ---------------------------------------------------------------------------

pub struct EngineShared {
    pub sample_position: AtomicI64,
    pub system_time_ns: AtomicI64,
    pub input_latency: AtomicU32,
    pub output_latency: AtomicU32,
}

impl EngineShared {
    pub fn new() -> Self {
        Self {
            sample_position: AtomicI64::new(0),
            system_time_ns: AtomicI64::new(0),
            input_latency: AtomicU32::new(0),
            output_latency: AtomicU32::new(0),
        }
    }
}

/// ASIO spec: system time "on Windows must be derived from timeGetTime()".
/// We anchor to timeGetTime() and refine with QPC for sub-ms resolution.
struct SysClock {
    base_ms: i64,
    base_qpc: i64,
    freq: i64,
}
impl SysClock {
    fn new() -> Self {
        Self { base_ms: unsafe { timeGetTime() } as i64, base_qpc: qpc(), freq: qpc_freq() }
    }
    fn now_ns(&self) -> i64 {
        let d = qpc() - self.base_qpc;
        self.base_ms * 1_000_000 + ((d as i128 * 1_000_000_000) / self.freq as i128) as i64
    }
}

pub struct AsioDriverState {
    pub client: KernelAudioClient,
    pub sample_rate: f64,
    pub buffer_size: usize,
    pub num_input_channels: usize,
    pub num_output_channels: usize,
    pub is_running: Arc<AtomicBool>,
    pub thread_handle: Option<JoinHandle<()>>,
    pub callbacks: Option<ASIOCallbacks>,
    pub use_time_info: bool,
    pub input_buffers: Vec<Vec<f32>>,
    pub output_buffers: Vec<Vec<f32>>,
    pub shared: Arc<EngineShared>,
    pub live: Option<Arc<LiveStatusMap>>,
    policy_thread: Option<JoinHandle<()>>,
    policy: Option<Arc<Policy>>,
}

unsafe impl Send for AsioDriverState {}

impl AsioDriverState {
    pub fn new(shared: Arc<EngineShared>) -> Self {
        let cfg = DolomiteConfig::load();
        let rate = if cfg.sample_rate > 0 { cfg.sample_rate as f64 } else { 48000.0 };
        let block = if cfg.buffer_size > 0 { cfg.buffer_size as usize } else { 256 };
        let mut s = Self {
            client: KernelAudioClient::new(),
            sample_rate: rate,
            buffer_size: block.clamp(32, 2048),
            num_input_channels: 2usize.min(MAX_AUDIO_CHANNELS),
            num_output_channels: 2usize.min(MAX_AUDIO_CHANNELS),
            is_running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            callbacks: None,
            use_time_info: false,
            input_buffers: Vec::new(),
            output_buffers: Vec::new(),
            shared,
            live: LiveStatusMap::create().map(Arc::new),
            policy_thread: None,
            policy: None,
        };
        s.update_latency_estimate(None);
        s
    }

    /// Latency reported to the host before the device is open (refined once running).
    pub fn update_latency_estimate(&mut self, queue_daw_frames: Option<u32>) {
        let block = self.buffer_size as u32;
        let queue = queue_daw_frames.unwrap_or((self.sample_rate * 0.010) as u32 + block);
        self.shared.input_latency.store(block, Ordering::Relaxed);
        self.shared.output_latency.store(block + queue, Ordering::Relaxed);
    }

    pub fn start_streaming(&mut self) -> ASIOError {
        if self.is_running.load(Ordering::SeqCst) {
            return ASE_OK;
        }
        let callbacks = match self.callbacks {
            Some(cb) => cb,
            None => return ASE_InvalidMode,
        };
        if self.output_buffers.is_empty() && self.input_buffers.is_empty() {
            return ASE_InvalidMode;
        }

        self.client.start_streaming();
        self.shared.sample_position.store(0, Ordering::SeqCst);
        self.is_running.store(true, Ordering::SeqCst);

        let block = self.buffer_size;
        let ptrs = |bufs: &mut Vec<Vec<f32>>| -> Vec<[SendPtr; 2]> {
            bufs.iter_mut()
                .map(|b| {
                    let p = b.as_mut_ptr();
                    [SendPtr(p), SendPtr(unsafe { p.add(block) })]
                })
                .collect()
        };

        let config = DolomiteConfig::load();
        let base = if config.low_latency { ENG_LOW } else { ENG_SHARED };
        // Start exclusive only if the DAW is in front (it normally is when
        // the user presses play); the policy thread refines this live.
        let first = if config.exclusive && (!config.auto_release || host_in_front(unsafe { GetCurrentProcessId() })) { ENG_EXCL } else { base };
        let policy = Arc::new(Policy {
            desired: AtomicU8::new(first),
            actual: AtomicU8::new(base),
            state: AtomicU8::new(0),
            excl_failed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            log: RtLog::new(),
        });
        let cfg_shared = Arc::new(std::sync::Mutex::new(config.clone()));
        // Pre-build min-phase resampler kernels off the audio thread.
        let rate = self.sample_rate;
        let _ = thread::Builder::new().name("DolomiteKernelPrewarm".into()).spawn(move || {
            for dev in [44100.0, 48000.0, 88200.0, 96000.0] {
                if (dev - rate).abs() > 0.5 {
                    StreamResampler::prewarm(rate, dev, ResamplerPhase::Minimum);
                    StreamResampler::prewarm(dev, rate, ResamplerPhase::Minimum);
                }
            }
        });
        {
            let (pol, cfg) = (policy.clone(), cfg_shared.clone());
            self.policy_thread = thread::Builder::new().name("DolomitePolicy".into()).spawn(move || policy_thread(pol, cfg)).ok();
        }
        self.policy = Some(policy.clone());

        let ctx = PumpCtx {
            running: self.is_running.clone(),
            callbacks,
            use_time_info: self.use_time_info,
            block,
            daw_rate: self.sample_rate,
            in_ptrs: ptrs(&mut self.input_buffers),
            out_ptrs: ptrs(&mut self.output_buffers),
            shared: self.shared.clone(),
            live: self.live.clone(),
            kernel_mem: if self.client.is_connected_to_kernel() {
                self.client.shared_memory().map(|s| s as *const _ as usize).unwrap_or(0)
            } else {
                0
            },
            config,
            policy,
            cfg_shared,
        };

        match thread::Builder::new().name("DolomiteAsioPump".into()).spawn(move || unsafe { pump(ctx) }) {
            Ok(h) => {
                self.thread_handle = Some(h);
                ASE_OK
            }
            Err(_) => {
                self.is_running.store(false, Ordering::SeqCst);
                ASE_HWMalfunction
            }
        }
    }

    /// Signals the pump to stop and hands back its join handle. The caller
    /// must join **after** releasing the driver mutex.
    pub fn request_stop(&mut self) -> Option<JoinHandle<()>> {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(p) = self.policy.take() {
            p.stop.store(true, Ordering::Relaxed);
        }
        let pump = self.thread_handle.take();
        let policy = self.policy_thread.take();
        // One handle for the caller to join: pump first, then policy thread.
        match (pump, policy) {
            (Some(a), Some(b)) => thread::Builder::new()
                .spawn(move || {
                    let _ = a.join();
                    let _ = b.join();
                })
                .ok(),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }

    pub fn stop_streaming(&mut self) -> ASIOError {
        if let Some(h) = self.request_stop() {
            let _ = h.join();
        }
        self.client.stop_streaming();
        ASE_OK
    }
}

// ---------------------------------------------------------------------------
// RT-safe event log: the audio thread only stores numbers, the policy thread
// formats and writes them to disk.
// ---------------------------------------------------------------------------

const EV_DROPOUT: u32 = 1; // value = new safety margin
const EV_STALL: u32 = 2; // exclusive event timed out
const EV_WRITE_ERR: u32 = 3; // value = HRESULT
const EV_DEVICE_ERR: u32 = 4; // value = HRESULT
const EV_IN_UNDERRUN: u32 = 5; // value = new input cushion
const EV_IN_OVERRUN: u32 = 6; // value = frames dropped

struct RtLog {
    slots: [AtomicU64; 64],
    head: AtomicU32,
    tail: AtomicU32,
}

impl RtLog {
    fn new() -> Self {
        Self { slots: core::array::from_fn(|_| AtomicU64::new(0)), head: AtomicU32::new(0), tail: AtomicU32::new(0) }
    }
    /// Wait-free; drops the event if the ring is full.
    fn push(&self, code: u32, value: u32) {
        let h = self.head.load(Ordering::Relaxed);
        if h.wrapping_sub(self.tail.load(Ordering::Acquire)) >= 64 {
            return;
        }
        self.slots[(h % 64) as usize].store(((code as u64) << 32) | value as u64, Ordering::Relaxed);
        self.head.store(h.wrapping_add(1), Ordering::Release);
    }
    fn drain(&self) {
        let h = self.head.load(Ordering::Acquire);
        let mut t = self.tail.load(Ordering::Relaxed);
        while t != h {
            let v = self.slots[(t % 64) as usize].load(Ordering::Relaxed);
            let (code, val) = ((v >> 32) as u32, v as u32);
            log_debug(&match code {
                EV_DROPOUT => format!("output dropout -> safety margin now {} frames", val),
                EV_STALL => "exclusive stream stalled (no device event) -> reopening".to_string(),
                EV_WRITE_ERR => format!("render write error 0x{:08X} -> reopening", val),
                EV_DEVICE_ERR => format!("device error 0x{:08X} -> reopening", val),
                EV_IN_UNDERRUN => format!("input underrun -> input cushion now {} frames", val),
                EV_IN_OVERRUN => format!("input overrun -> dropped {} frames", val),
                _ => format!("event {} {}", code, val),
            });
            t = t.wrapping_add(1);
        }
        self.tail.store(t, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// Auto-exclusive policy (runs on its own low-priority thread)
// ---------------------------------------------------------------------------

const ENG_SHARED: u8 = 0;
const ENG_LOW: u8 = 1;
const ENG_EXCL: u8 = 2;

fn engine_code(e: Engine) -> u8 {
    match e {
        Engine::Shared => ENG_SHARED,
        Engine::LowLatency => ENG_LOW,
        Engine::Exclusive => ENG_EXCL,
    }
}

fn engine_of(c: u8) -> Engine {
    match c {
        ENG_EXCL => Engine::Exclusive,
        ENG_LOW => Engine::LowLatency,
        _ => Engine::Shared,
    }
}

pub struct Policy {
    /// Engine the pump should run (written by the policy thread).
    desired: AtomicU8,
    /// Engine actually running (written by the pump).
    actual: AtomicU8,
    /// auto_state for the control panel (see live_status)
    state: AtomicU8,
    /// set by the pump when exclusive could not be obtained
    excl_failed: AtomicBool,
    stop: AtomicBool,
    log: RtLog,
}

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    fn GetWindowTextW(hwnd: *mut c_void, buf: *mut u16, max: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcessId() -> u32;
}

/// Is the DAW (this process) - or our own control panel - in front?
fn host_in_front(own_pid: u32) -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == own_pid {
            return true;
        }
        let mut buf = [0u16; 64];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n]).starts_with("Dolomite")
    }
}

/// Decides, ~20 times a second, whether the pump may hold the device
/// exclusively:
/// * release (within ~150 ms) when the user switches to another program, or
///   when another program is audibly playing on the same output;
/// * re-acquire only after the DAW has been in front and the output has been
///   free of other audio for ~1 s (hysteresis, no flapping);
/// * back off for 3 s whenever exclusive open fails (device busy).
fn policy_thread(policy: Arc<Policy>, cfg_rx: Arc<std::sync::Mutex<DolomiteConfig>>) {
    let _com = ComGuard::mta();
    let own_pid = unsafe { GetCurrentProcessId() };
    let freq = qpc_freq();
    let ms = |t: i64| (t * 1000 / freq) as i64;
    let mut away_since: Option<i64> = None;
    let mut ok_since: Option<i64> = None;
    let mut last_other: i64 = i64::MIN / 2;
    let mut last_probe: i64 = 0;
    let mut backoff_until: i64 = 0;

    while !policy.stop.load(Ordering::Relaxed) {
        policy.log.drain();
        let cfg = cfg_rx.lock().map(|c| c.clone()).unwrap_or_default();
        let base = if cfg.low_latency { ENG_LOW } else { ENG_SHARED };
        let now = qpc();

        let (desired, state) = if !cfg.exclusive {
            (base, 0u8)
        } else if !cfg.auto_release {
            (ENG_EXCL, 1u8)
        } else {
            if policy.excl_failed.swap(false, Ordering::Relaxed) {
                backoff_until = now + freq * 3;
            }
            let front = host_in_front(own_pid);
            let actual = policy.actual.load(Ordering::Relaxed);
            // Other apps can only be heard while we are NOT exclusive.
            if actual != ENG_EXCL && ms(now - last_probe) >= 250 {
                last_probe = now;
                if !audible_sessions(Some(&cfg.output_device_id), 0, own_pid).is_empty() {
                    last_other = now;
                }
            }
            let other_recent = ms(now - last_other) < 1500;
            let currently = policy.desired.load(Ordering::Relaxed);

            if front {
                away_since = None;
            } else if away_since.is_none() {
                away_since = Some(now);
            }
            let want = front && !other_recent && now >= backoff_until;
            if want {
                ok_since.get_or_insert(now);
            } else {
                ok_since = None;
            }

            let next = if currently == ENG_EXCL {
                let away_long = away_since.map(|t| ms(now - t) >= 120).unwrap_or(false);
                if away_long || other_recent || now < backoff_until { base } else { ENG_EXCL }
            } else if ok_since.map(|t| ms(now - t) >= 1000).unwrap_or(false) {
                ENG_EXCL
            } else {
                base
            };
            let state = if next == ENG_EXCL {
                1
            } else if now < backoff_until {
                4
            } else if other_recent {
                3
            } else {
                2
            };
            (next, state)
        };
        policy.desired.store(desired, Ordering::Relaxed);
        policy.state.store(state, Ordering::Relaxed);
        thread::sleep(std::time::Duration::from_millis(40));
    }
    policy.log.drain();
}

// ---------------------------------------------------------------------------
// The real-time pump
// ---------------------------------------------------------------------------

struct PumpCtx {
    running: Arc<AtomicBool>,
    callbacks: ASIOCallbacks,
    use_time_info: bool,
    block: usize,
    daw_rate: f64,
    in_ptrs: Vec<[SendPtr; 2]>,
    out_ptrs: Vec<[SendPtr; 2]>,
    shared: Arc<EngineShared>,
    live: Option<Arc<LiveStatusMap>>,
    kernel_mem: usize,
    config: DolomiteConfig,
    policy: Arc<Policy>,
    cfg_shared: Arc<std::sync::Mutex<DolomiteConfig>>,
}

/// Running meter state (published to the control panel).
struct Meters {
    peak: [f32; 4],
    ms: [f32; 4],
    load: f32,
    load_peak: f32,
    peak_decay: f32,
    rms_coef: f32,
    load_coef: f32,
}

impl Meters {
    fn new(block_secs: f64) -> Self {
        Self {
            peak: [0.0; 4],
            ms: [0.0; 4],
            load: 0.0,
            load_peak: 0.0,
            // 20 dB/s peak fall-back, 300 ms RMS integration, 500 ms load smoothing
            peak_decay: 10f64.powf(-20.0 * block_secs / 20.0) as f32,
            rms_coef: (1.0 - (-block_secs / 0.3).exp()) as f32,
            load_coef: (1.0 - (-block_secs / 0.5).exp()) as f32,
        }
    }
}

/// Input side: capture device -> (resampler) -> DAW-rate FIFO.
struct InputChain {
    dev: WasapiCapture,
    rs: Option<StreamResampler>,
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
    fifo: VecDeque<(f32, f32)>,
    /// frames kept in the FIFO on top of one DAW block (jitter cushion)
    cushion: usize,
    cushion_max: usize,
    primed: bool,
    period_daw: usize,
    rs_lat: usize,
    failed: bool,
}

const SCRATCH: usize = 16384;

impl InputChain {
    /// Pull everything the device has captured. Never blocks, never allocates.
    fn pull(&mut self, block: usize, log: &RtLog) {
        if self.failed {
            return;
        }
        self.scratch_l.clear();
        self.scratch_r.clear();
        if self.dev.read_into(&mut self.scratch_l, &mut self.scratch_r, SCRATCH).is_err() {
            self.failed = true;
            return;
        }
        if self.scratch_l.is_empty() {
            return;
        }
        let fifo = &mut self.fifo;
        match self.rs.as_mut() {
            Some(rs) => {
                for (l, r) in self.scratch_l.chunks(1024).zip(self.scratch_r.chunks(1024)) {
                    rs.process(l, r, |a, b| fifo.push_back((a, b)));
                }
            }
            None => {
                for (&l, &r) in self.scratch_l.iter().zip(self.scratch_r.iter()) {
                    fifo.push_back((l, r));
                }
            }
        }
        // Clock drift / scheduling burst: never let the input lag grow.
        let limit = block + self.cushion + self.period_daw * 2;
        if self.fifo.len() > limit {
            let drop = self.fifo.len() - (block + self.cushion);
            self.fifo.drain(..drop);
            log.push(EV_IN_OVERRUN, drop as u32);
        }
    }
}

struct Pump {
    ctx: PumpCtx,
    idx: usize,
    position: i64,
    fifo: VecDeque<(f32, f32)>,
    resampler: Option<StreamResampler>,
    input: Option<InputChain>,
    meters: Meters,
    sys: SysClock,
    block_qpc: f64,
}

impl Pump {
    /// Run one DAW callback and queue its output (resampled if needed).
    unsafe fn process_block(&mut self) {
        let block = self.ctx.block;
        let idx = self.idx;

        // ---- inputs ----
        let mut in_pk = [0.0f32; 2];
        let mut in_sq = [0.0f32; 2];
        if self.ctx.kernel_mem != 0 {
            let mem = &mut *(self.ctx.kernel_mem as *mut audio_core::SharedAudioMemory);
            for f in 0..block {
                let frame = mem.capture_ring.pop();
                for (ch, p) in self.ctx.in_ptrs.iter().enumerate() {
                    *p[idx].0.add(f) = frame.map(|fr| fr.samples[ch]).unwrap_or(0.0);
                }
            }
        } else if let Some(inp) = self.input.as_mut() {
            if !inp.primed && inp.fifo.len() >= block + inp.cushion {
                inp.primed = true;
            }
            let have = if inp.primed { inp.fifo.len().min(block) } else { 0 };
            if inp.primed && have < block {
                // underrun: grow the cushion and re-prime
                inp.cushion = (inp.cushion + inp.period_daw / 2 + 8).min(inp.cushion_max);
                inp.primed = false;
                self.ctx.policy.log.push(EV_IN_UNDERRUN, inp.cushion as u32);
                if let Some(l) = &self.ctx.live {
                    l.get().input_underruns.fetch_add(1, Ordering::Relaxed);
                }
            }
            let pl = self.ctx.in_ptrs.get(0).map(|p| p[idx].0);
            let pr = self.ctx.in_ptrs.get(1).map(|p| p[idx].0);
            for f in 0..block {
                let (l, r) = if f < have { inp.fifo.pop_front().unwrap_or((0.0, 0.0)) } else { (0.0, 0.0) };
                if let Some(p) = pl {
                    *p.add(f) = l;
                }
                if let Some(p) = pr {
                    *p.add(f) = r;
                }
                in_pk[0] = in_pk[0].max(l.abs());
                in_pk[1] = in_pk[1].max(r.abs());
                in_sq[0] += l * l;
                in_sq[1] += r * r;
            }
            for p in self.ctx.in_ptrs.iter().skip(2) {
                core::ptr::write_bytes(p[idx].0, 0, block);
            }
        } else {
            for p in &self.ctx.in_ptrs {
                core::ptr::write_bytes(p[idx].0, 0, block);
            }
        }

        let c = &self.ctx;
        let now_ns = self.sys.now_ns();
        c.shared.sample_position.store(self.position, Ordering::Release);
        c.shared.system_time_ns.store(now_ns, Ordering::Release);

        let t0 = qpc();
        if c.use_time_info {
            if let Some(f) = c.callbacks.buffer_switch_time_info {
                let mut t = ASIOTime::new(self.position, c.daw_rate, now_ns);
                f(&mut t as *mut _ as *mut c_void, idx as i32, ASIOTrue);
            }
        } else if let Some(f) = c.callbacks.buffer_switch {
            f(idx as i32, ASIOTrue);
        }
        let dt = (qpc() - t0) as f32 / self.block_qpc as f32;

        // ---- outputs (mono hosts: duplicate channel 0) ----
        let l = c.out_ptrs.get(0).map(|p| p[idx].0 as *const f32);
        let r = c.out_ptrs.get(1).map(|p| p[idx].0 as *const f32).or(l);
        let (mut pk, mut sq) = ([0.0f32; 2], [0.0f32; 2]);
        if let (Some(l), Some(r)) = (l, r) {
            let sl = core::slice::from_raw_parts(l, block);
            let sr = core::slice::from_raw_parts(r, block);
            for i in 0..block {
                let (a, b) = (sl[i], sr[i]);
                pk[0] = pk[0].max(a.abs());
                pk[1] = pk[1].max(b.abs());
                sq[0] += a * a;
                sq[1] += b * b;
            }
            match self.resampler.as_mut() {
                None => {
                    for i in 0..block {
                        self.fifo.push_back((sl[i], sr[i]));
                    }
                }
                Some(rs) => {
                    let fifo = &mut self.fifo;
                    rs.process(sl, sr, |a, b| fifo.push_back((a, b)));
                }
            }
        } else {
            for _ in 0..block {
                self.fifo.push_back((0.0, 0.0));
            }
        }

        // ---- meters & load ----
        let m = &mut self.meters;
        let all_pk = [pk[0], pk[1], in_pk[0], in_pk[1]];
        let all_sq = [sq[0], sq[1], in_sq[0], in_sq[1]];
        for ch in 0..4 {
            m.peak[ch] = all_pk[ch].max(m.peak[ch] * m.peak_decay);
            m.ms[ch] += (all_sq[ch] / block as f32 - m.ms[ch]) * m.rms_coef;
        }
        m.load += (dt - m.load) * m.load_coef;
        m.load_peak = dt.max(m.load_peak * 0.999);

        if let Some(live) = &c.live {
            let s = live.get();
            for ch in 0..4 {
                LiveStatus::set_f32(&s.peak[ch], m.peak[ch]);
                LiveStatus::set_f32(&s.rms[ch], m.ms[ch].sqrt());
                if all_pk[ch] >= 1.0 {
                    s.clip[ch].fetch_add(1, Ordering::Relaxed);
                }
            }
            s.dsp_load_permille.store((m.load * 1000.0) as u32, Ordering::Relaxed);
            s.dsp_load_peak_permille.store((m.load_peak * 1000.0) as u32, Ordering::Relaxed);
            s.callbacks.fetch_add(1, Ordering::Relaxed);
            s.touch();
        }

        if c.kernel_mem != 0 {
            let mem = &mut *(c.kernel_mem as *mut audio_core::SharedAudioMemory);
            mem.output_meters.peak_linear[0] = m.peak[0];
            mem.output_meters.peak_linear[1] = m.peak[1];
            mem.telemetry.total_frames.fetch_add(block as u64, Ordering::Relaxed);
        }

        self.position += block as i64;
        self.idx ^= 1;
    }

    fn host_message(&self, selector: i32) -> i32 {
        match self.ctx.callbacks.asio_message {
            Some(f) => f(selector, 0, core::ptr::null_mut(), core::ptr::null_mut()),
            None => 0,
        }
    }

    fn pull_input(&mut self) {
        let block = self.ctx.block;
        if let Some(inp) = self.input.as_mut() {
            inp.pull(block, &self.ctx.policy.log);
        }
    }
}

/// Denormals are flushed to zero on the audio thread (the DAW's callback
/// runs here too): avoids 100x CPU spikes in decaying reverbs/filters.
fn enable_ftz_daz() -> u32 {
    #[cfg(target_arch = "x86_64")]
    #[allow(deprecated)]
    unsafe {
        use core::arch::x86_64::{_mm_getcsr, _mm_setcsr};
        let old = _mm_getcsr();
        _mm_setcsr(old | 0x8040);
        return old;
    }
    #[allow(unreachable_code)]
    0
}

fn restore_csr(_old: u32) {
    #[cfg(target_arch = "x86_64")]
    #[allow(deprecated)]
    unsafe {
        core::arch::x86_64::_mm_setcsr(_old);
    }
}

/// Everything that depends on the currently open device.
struct Io {
    device: Option<WasapiRender>,
    exclusive: bool,
    device_rate: f64,
    period: usize,
    block_dev: usize,
    rs_lat: u32,
    margin: usize,
    margin_min: usize,
    margin_max: usize,
    target: usize,
    started: bool,
}

/// Close whatever is open (this releases an exclusive device immediately)
/// and open the requested engine. Also rebuilds resamplers and the input chain.
unsafe fn open_io(p: &mut Pump, engine: Engine, old: Option<Io>) -> Io {
    if let Some(mut o) = old {
        if let Some(d) = o.device.take() {
            d.stop();
            drop(d);
        }
    }
    p.input = None; // drops (and releases) the capture stream

    let cfg = p.ctx.config.clone();
    let block_secs = p.ctx.block as f64 / p.ctx.daw_rate;
    let device = match WasapiRender::open(Some(&cfg.output_device_id), engine, p.ctx.daw_rate as u32, block_secs) {
        Ok(d) => Some(d),
        Err(e) => {
            log_debug(&format!("No WASAPI output ({}); running on timer clock without sound", e));
            None
        }
    };
    if engine == Engine::Exclusive && !device.as_ref().map(|d| d.exclusive).unwrap_or(false) {
        p.ctx.policy.excl_failed.store(true, Ordering::Relaxed);
    }
    let exclusive = device.as_ref().map(|d| d.exclusive).unwrap_or(false);
    let actual = match &device {
        Some(d) if d.exclusive => Engine::Exclusive,
        Some(d) if d.low_latency => Engine::LowLatency,
        _ => Engine::Shared,
    };
    p.ctx.policy.actual.store(engine_code(actual), Ordering::Relaxed);

    let device_rate = device.as_ref().map(|d| d.sample_rate as f64).unwrap_or(p.ctx.daw_rate);
    let resample = (device_rate - p.ctx.daw_rate).abs() > 0.5;
    let block_dev = (p.ctx.block as f64 * device_rate / p.ctx.daw_rate).ceil() as usize;
    let period = device.as_ref().map(|d| d.period_frames as usize).unwrap_or(p.ctx.block);

    p.fifo.clear();
    p.resampler = if resample {
        Some(StreamResampler::with_phase(p.ctx.daw_rate, device_rate, p.ctx.block, ResamplerPhase::Minimum))
    } else {
        None
    };
    let rs_lat = p.resampler.as_ref().map(|r| r.latency_input_frames() as u32).unwrap_or(0);

    // ---- input ----
    if cfg.input_enabled && !p.ctx.in_ptrs.is_empty() && p.ctx.kernel_mem == 0 {
        // Exclusive capture only together with exclusive output.
        let in_engine = if exclusive { Engine::Exclusive } else if actual == Engine::LowLatency { Engine::LowLatency } else { Engine::Shared };
        match WasapiCapture::open(Some(&cfg.input_device_id), in_engine, p.ctx.daw_rate as u32) {
            Ok(dev) => {
                let in_rate = dev.sample_rate as f64;
                let rs = if (in_rate - p.ctx.daw_rate).abs() > 0.5 {
                    Some(StreamResampler::with_phase(in_rate, p.ctx.daw_rate, 1024, ResamplerPhase::Minimum))
                } else {
                    None
                };
                let rs_in_lat = rs.as_ref().map(|r| r.latency_input_frames()).unwrap_or(0);
                let period_daw = ((dev.period_frames as f64) * p.ctx.daw_rate / in_rate).ceil() as usize;
                let hr = dev.start();
                if hr != 0 {
                    log_debug(&format!("capture start failed 0x{:08X}; inputs silent", hr));
                } else {
                    p.input = Some(InputChain {
                        rs,
                        scratch_l: Vec::with_capacity(SCRATCH),
                        scratch_r: Vec::with_capacity(SCRATCH),
                        fifo: VecDeque::with_capacity(SCRATCH * 2),
                        cushion: period_daw,
                        cushion_max: period_daw * 4 + p.ctx.block,
                        primed: false,
                        period_daw,
                        rs_lat: rs_in_lat,
                        failed: false,
                        dev,
                    });
                }
            }
            Err(e) => log_debug(&format!("No WASAPI input ({}); inputs silent", e)),
        }
    }

    let margin_min = 16usize;
    let mut io = Io {
        device,
        exclusive,
        device_rate,
        period,
        block_dev,
        rs_lat,
        margin: (period / 4).max(margin_min),
        margin_min,
        margin_max: period * 2,
        target: 0,
        started: false,
    };
    io.target = io.period + io.block_dev + io.margin;

    if let Some(live) = &p.ctx.live {
        let s = live.get();
        s.running.store(1, Ordering::Relaxed);
        s.daw_rate.store(p.ctx.daw_rate as u32, Ordering::Relaxed);
        s.device_rate.store(device_rate as u32, Ordering::Relaxed);
        s.buffer_frames.store(p.ctx.block as u32, Ordering::Relaxed);
        s.device_period_frames.store(period as u32, Ordering::Relaxed);
        s.resampling.store(resample as u32, Ordering::Relaxed);
        s.resampler_latency_frames.store(rs_lat, Ordering::Relaxed);
        let (mode, name) = match &io.device {
            Some(d) => (if d.exclusive { 3 } else if d.low_latency { 2 } else { 1 }, d.name.clone()),
            None => (0, "No output device".to_string()),
        };
        s.engine_mode.store(mode, Ordering::Relaxed);
        s.set_device_name(&name);
        match &p.input {
            Some(i) => {
                s.input_mode.store(if i.dev.exclusive { 3 } else if i.dev.low_latency { 2 } else { 1 }, Ordering::Relaxed);
                s.input_rate.store(i.dev.sample_rate, Ordering::Relaxed);
                s.input_period_frames.store(i.dev.period_frames, Ordering::Relaxed);
                s.set_input_device_name(&i.dev.name);
            }
            None => {
                s.input_mode.store(0, Ordering::Relaxed);
                s.set_input_device_name(if cfg.input_enabled { "No input" } else { "Inputs off" });
            }
        }
        s.touch();
    }

    log_debug(&format!(
        "Engine open: DAW {} Hz / block {} -> '{}' {} Hz, period {}, {}, resample={} (min-phase, {} smp), input={}",
        p.ctx.daw_rate,
        p.ctx.block,
        io.device.as_ref().map(|d| d.name.as_str()).unwrap_or("-"),
        device_rate,
        period,
        if exclusive { "EXCLUSIVE ping-pong".to_string() } else { format!("{:?} target {}", actual, io.target) },
        resample,
        rs_lat,
        p.input.as_ref().map(|i| format!("{} Hz {}", i.dev.sample_rate, if i.dev.exclusive { "exclusive" } else { "shared" })).unwrap_or_else(|| "none".into())
    ));
    io
}

unsafe fn pump(ctx: PumpCtx) {
    let _com = ComGuard::mta();
    let mmcss = Mmcss::enable();
    if mmcss.handle.is_null() {
        SetThreadPriority(GetCurrentThread(), 15 /* TIME_CRITICAL */);
    }
    disable_power_throttling(GetCurrentThread());
    timeBeginPeriod(1);
    let old_csr = enable_ftz_daz();

    let block_secs = ctx.block as f64 / ctx.daw_rate;
    let freq = qpc_freq();

    let mut p = Pump {
        idx: 0,
        position: 0,
        fifo: VecDeque::with_capacity(SCRATCH * 2),
        resampler: None,
        input: None,
        meters: Meters::new(block_secs),
        sys: SysClock::new(),
        block_qpc: block_secs * freq as f64,
        ctx,
    };

    if let Some(live) = &p.ctx.live {
        let s = live.get();
        s.underruns.store(0, Ordering::Relaxed);
        s.late_wakeups.store(0, Ordering::Relaxed);
        s.callbacks.store(0, Ordering::Relaxed);
        s.input_underruns.store(0, Ordering::Relaxed);
        s.engine_switches.store(0, Ordering::Relaxed);
        for c in &s.clip {
            c.store(0, Ordering::Relaxed);
        }
    }

    let mut want = p.ctx.policy.desired.load(Ordering::Relaxed);
    let mut io = open_io(&mut p, engine_of(want), None);
    let mut reported_out = 0u32;
    let mut reported_in = 0u32;

    let cfg_path = DolomiteConfig::config_path();
    let mut cfg_mtime = std::fs::metadata(&cfg_path).and_then(|m| m.modified()).ok();
    let mut last_cfg_check = qpc();
    let mut timer_next = qpc() as f64;
    let mut last_wake = qpc();
    let mut last_trouble = qpc();
    let mut last_notify = qpc();
    let mut last_retry = qpc();
    let mut reopen = false;

    while p.ctx.running.load(Ordering::Relaxed) {
        // ---- 0) engine switch / self-healing reopen ----
        let desired = p.ctx.policy.desired.load(Ordering::Relaxed);
        let no_dev_retry = io.device.is_none() && (qpc() - last_retry) > freq * 2;
        if desired != want || reopen || no_dev_retry {
            want = desired;
            reopen = false;
            last_retry = qpc();
            io = open_io(&mut p, engine_of(want), Some(io));
            if let Some(l) = &p.ctx.live {
                l.get().engine_switches.fetch_add(1, Ordering::Relaxed);
            }
            last_wake = qpc();
            timer_next = qpc() as f64;
        }

        // ---- latency bookkeeping (reported to the host) ----
        let daw_rate = p.ctx.daw_rate;
        let to_daw = |frames: usize| ((frames as f64) * daw_rate / io.device_rate).round() as u32;
        let out_lat = p.ctx.block as u32
            + if io.exclusive { to_daw(io.period * 2) } else { to_daw(io.target) }
            + io.rs_lat;
        let in_lat = p.ctx.block as u32
            + p.input.as_ref().map(|i| (i.cushion + i.rs_lat) as u32).unwrap_or(0);
        if out_lat != reported_out || in_lat != reported_in {
            let now = qpc();
            let grew = out_lat > reported_out || in_lat > reported_in;
            // increases are reported at once; decreases at most every 10 s
            if grew || (now - last_notify) > freq * 10 || reported_out == 0 {
                reported_out = out_lat;
                reported_in = in_lat;
                p.ctx.shared.output_latency.store(out_lat, Ordering::Relaxed);
                p.ctx.shared.input_latency.store(in_lat, Ordering::Relaxed);
                p.host_message(kAsioLatenciesChanged);
                last_notify = now;
            }
            if let Some(l) = &p.ctx.live {
                let s = l.get();
                s.output_latency_frames.store(out_lat, Ordering::Relaxed);
                s.input_latency_frames.store(in_lat, Ordering::Relaxed);
                s.queue_target_frames.store(if io.exclusive { (io.period * 2) as u32 } else { io.target as u32 }, Ordering::Relaxed);
            }
        }
        if let Some(l) = &p.ctx.live {
            l.get().auto_state.store(p.ctx.policy.state.load(Ordering::Relaxed) as u32, Ordering::Relaxed);
        }

        match io.device.as_mut() {
            Some(dev) if dev.exclusive => {
                if !io.started {
                    // Pre-fill the first hardware buffer before starting (MS pattern).
                    while p.fifo.len() < io.period && p.ctx.running.load(Ordering::Relaxed) {
                        p.process_block();
                    }
                    let _ = dev.write_from(&mut p.fifo);
                    let hr = dev.start();
                    if hr != 0 {
                        log_debug(&format!("exclusive start failed 0x{:08X}", hr));
                        p.ctx.policy.excl_failed.store(true, Ordering::Relaxed);
                        reopen = true;
                        continue;
                    }
                    io.started = true;
                    last_wake = qpc();
                }
                let timeout = ((io.period as f64 / io.device_rate * 1000.0) as u32) * 4 + 20;
                if !dev.wait(timeout) {
                    if let Some(l) = &p.ctx.live {
                        l.get().late_wakeups.fetch_add(1, Ordering::Relaxed);
                    }
                    p.ctx.policy.log.push(EV_STALL, 0);
                    p.ctx.policy.excl_failed.store(true, Ordering::Relaxed);
                    reopen = true;
                    continue;
                }
                let now = qpc();
                let period_qpc = io.period as f64 / io.device_rate * freq as f64;
                if (now - last_wake) as f64 > period_qpc * 1.75 {
                    // scheduled too late: a hardware buffer was likely repeated
                    if let Some(l) = &p.ctx.live {
                        l.get().underruns.fetch_add(1, Ordering::Relaxed);
                    }
                }
                last_wake = now;
                p.pull_input();
                while p.fifo.len() < io.period && p.ctx.running.load(Ordering::Relaxed) {
                    p.process_block();
                }
                if let Err(hr) = dev.write_from(&mut p.fifo) {
                    p.ctx.policy.log.push(EV_WRITE_ERR, hr as u32);
                    reopen = true;
                }
            }
            Some(dev) => {
                // 1) Fill the device queue up to target (bursting callbacks if needed)
                let pad = match dev.padding() {
                    Ok(v) => v as usize,
                    Err(hr) => {
                        p.ctx.policy.log.push(EV_DEVICE_ERR, hr as u32);
                        if hr == AUDCLNT_E_DEVICE_INVALIDATED {
                            // endpoint vanished / format changed: reopen (default device if needed)
                        }
                        reopen = true;
                        Sleep(5);
                        continue;
                    }
                };
                let now = qpc();
                if io.started && pad == 0 {
                    if let Some(l) = &p.ctx.live {
                        l.get().underruns.fetch_add(1, Ordering::Relaxed);
                    }
                    // adaptive margin: grow fast on trouble
                    let grown = (io.margin * 2 + io.margin_min).min(io.margin_max);
                    if grown != io.margin {
                        io.margin = grown;
                        io.target = io.period + io.block_dev + io.margin;
                        p.ctx.policy.log.push(EV_DROPOUT, io.margin as u32);
                    }
                    last_trouble = now;
                } else if io.started && (now - last_trouble) > freq * 10 && io.margin > io.margin_min {
                    // ...and shrink slowly after 10 s without trouble
                    io.margin = (io.margin - io.margin / 8).max(io.margin_min);
                    io.target = io.period + io.block_dev + io.margin;
                    last_trouble = now;
                }

                p.pull_input();
                let mut queued = pad + p.fifo.len();
                let mut guard = 0;
                while queued < io.target && guard < 64 && p.ctx.running.load(Ordering::Relaxed) {
                    p.process_block();
                    queued = pad + p.fifo.len();
                    guard += 1;
                }
                let _ = dev.write_from(&mut p.fifo);
                if !io.started {
                    let hr = dev.start();
                    log_debug(&format!("WASAPI stream started: hr=0x{:08X}", hr));
                    io.started = true;
                }
                // 2) Sleep until the engine consumed a period
                let timeout = (((io.period as f64 / io.device_rate) * 1000.0) as u32).max(1) * 4 + 20;
                if !dev.wait(timeout) {
                    if let Some(l) = &p.ctx.live {
                        l.get().late_wakeups.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            None => {
                // No device: keep the host alive on a QPC timer at the DAW rate.
                p.pull_input();
                p.process_block();
                p.fifo.clear();
                timer_next += p.block_qpc;
                loop {
                    let now = qpc() as f64;
                    if now >= timer_next || !p.ctx.running.load(Ordering::Relaxed) {
                        break;
                    }
                    if timer_next - now > 0.002 * freq as f64 {
                        Sleep(1);
                    } else {
                        core::hint::spin_loop();
                    }
                }
            }
        }

        if p.input.as_ref().map(|i| i.failed).unwrap_or(false) {
            p.ctx.policy.log.push(EV_DEVICE_ERR, 0xCA97);
            reopen = true;
        }

        // 3) Twice a second: pick up control-panel changes. Device / engine /
        //    input changes are applied live (no host reset needed).
        if (qpc() - last_cfg_check) > freq / 2 {
            last_cfg_check = qpc();
            let m = std::fs::metadata(&cfg_path).and_then(|m| m.modified()).ok();
            if m != cfg_mtime {
                cfg_mtime = m;
                let new_cfg = DolomiteConfig::load();
                let c = &p.ctx.config;
                let device_changed = new_cfg.output_device_id != c.output_device_id
                    || new_cfg.input_device_id != c.input_device_id
                    || new_cfg.input_enabled != c.input_enabled;
                if let Ok(mut g) = p.ctx.cfg_shared.lock() {
                    *g = new_cfg.clone();
                }
                p.ctx.config = new_cfg;
                if device_changed {
                    log_debug("Config changed (device/input) -> reopening live");
                    reopen = true;
                }
            }
        }
    }

    if let Some(dev) = io.device.take() {
        dev.stop();
    }
    p.input = None;
    if let Some(live) = &p.ctx.live {
        live.get().running.store(0, Ordering::Relaxed);
        live.get().input_mode.store(0, Ordering::Relaxed);
        live.get().touch();
    }
    restore_csr(old_csr);
    drop(mmcss);
    timeEndPeriod(1);
    log_debug(&format!("Pump stopped after {} samples", p.position));
}
