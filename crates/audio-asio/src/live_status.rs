//! Live engine telemetry shared between the ASIO driver (running inside the
//! DAW process, e.g. Studio One) and the control panel (`audio-control`).
//!
//! Implemented as a small named file mapping (`Local\DolomiteAudioLive`).
//! The driver is the only writer; readers just poll. Everything is atomics,
//! so no locks are ever taken on the audio thread.

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const LIVE_MAGIC: u32 = 0x444F_4C4D; // 'DOLM'
pub const LIVE_VERSION: u32 = 4;
const MAPPING_NAME: &str = "Local\\DolomiteAudioLive";

#[repr(C)]
pub struct LiveStatus {
    pub magic: AtomicU32,
    pub version: AtomicU32,
    pub host_pid: AtomicU32,
    pub running: AtomicU32,
    /// GetTickCount64() of the last write – readers treat >1 s as "stopped".
    pub last_update_ms: AtomicU64,

    pub daw_rate: AtomicU32,
    pub device_rate: AtomicU32,
    pub buffer_frames: AtomicU32,
    pub device_period_frames: AtomicU32,
    pub queue_target_frames: AtomicU32,
    pub output_latency_frames: AtomicU32,
    pub input_latency_frames: AtomicU32,
    /// 0 = no device / timer, 1 = classic shared, 2 = low-latency shared (IAudioClient3)
    pub engine_mode: AtomicU32,
    pub resampling: AtomicU32,

    pub callbacks: AtomicU64,
    pub underruns: AtomicU32,
    pub late_wakeups: AtomicU32,
    /// DAW callback time / block duration, in 1/1000 (smoothed)
    pub dsp_load_permille: AtomicU32,
    pub dsp_load_peak_permille: AtomicU32,

    /// f32 bit patterns: out L, out R, in L, in R
    pub peak: [AtomicU32; 4],
    pub rms: [AtomicU32; 4],
    pub clip: [AtomicU32; 4],

    pub device_name: [AtomicU32; 32], // 128 bytes UTF-8, NUL padded

    // ---- v3 ----
    /// Input engine: 0 = off/none, 1 shared, 2 low-latency, 3 exclusive
    pub input_mode: AtomicU32,
    pub input_rate: AtomicU32,
    pub input_period_frames: AtomicU32,
    pub input_underruns: AtomicU32,
    /// Resampler group delay, output side (DAW frames)
    pub resampler_latency_frames: AtomicU32,
    /// Auto-exclusive state: 0 off, 1 holding exclusive, 2 released (DAW in
    /// background), 3 released (another app is playing), 4 device busy
    pub auto_state: AtomicU32,
    /// Number of live engine switches since start
    pub engine_switches: AtomicU32,
    pub input_device_name: [AtomicU32; 32],

    // ---- v4 ----
    /// Adaptive input resampling: 0 off, 1 locking, 2 locked
    pub input_adaptive: AtomicU32,
    /// Measured input-vs-output clock offset, parts per billion (i32 bits)
    pub input_drift_ppb: AtomicU32,
}

impl LiveStatus {
    pub fn set_f32(a: &AtomicU32, v: f32) {
        a.store(v.to_bits(), Ordering::Relaxed);
    }
    pub fn get_f32(a: &AtomicU32) -> f32 {
        f32::from_bits(a.load(Ordering::Relaxed))
    }

    pub fn set_device_name(&self, name: &str) {
        let mut bytes = [0u8; 128];
        let b = name.as_bytes();
        let n = b.len().min(127);
        bytes[..n].copy_from_slice(&b[..n]);
        for (i, w) in self.device_name.iter().enumerate() {
            let v = u32::from_le_bytes([bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2], bytes[i * 4 + 3]]);
            w.store(v, Ordering::Relaxed);
        }
    }

    pub fn set_input_device_name(&self, name: &str) {
        Self::store_name(&self.input_device_name, name)
    }

    pub fn input_device_name(&self) -> String {
        Self::load_name(&self.input_device_name)
    }

    fn store_name(dst: &[AtomicU32; 32], name: &str) {
        let mut bytes = [0u8; 128];
        let b = name.as_bytes();
        let n = b.len().min(127);
        bytes[..n].copy_from_slice(&b[..n]);
        for (i, w) in dst.iter().enumerate() {
            w.store(u32::from_le_bytes([bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2], bytes[i * 4 + 3]]), Ordering::Relaxed);
        }
    }

    fn load_name(src: &[AtomicU32; 32]) -> String {
        let mut bytes = Vec::with_capacity(128);
        for w in src {
            bytes.extend_from_slice(&w.load(Ordering::Relaxed).to_le_bytes());
        }
        let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    pub fn device_name(&self) -> String {
        let mut bytes = Vec::with_capacity(128);
        for w in &self.device_name {
            bytes.extend_from_slice(&w.load(Ordering::Relaxed).to_le_bytes());
        }
        let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    pub fn touch(&self) {
        self.last_update_ms.store(unsafe { GetTickCount64() }, Ordering::Relaxed);
    }

    /// True if a driver instance updated this block within the last second.
    pub fn is_alive(&self) -> bool {
        let last = self.last_update_ms.load(Ordering::Relaxed);
        self.magic.load(Ordering::Relaxed) == LIVE_MAGIC
            && self.running.load(Ordering::Relaxed) != 0
            && unsafe { GetTickCount64() }.saturating_sub(last) < 1000
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileMappingW(h: *mut c_void, sa: *mut c_void, protect: u32, hi: u32, lo: u32, name: *const u16) -> *mut c_void;
    fn OpenFileMappingW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
    fn MapViewOfFile(h: *mut c_void, access: u32, hi: u32, lo: u32, bytes: usize) -> *mut c_void;
    fn UnmapViewOfFile(p: *const c_void) -> i32;
    fn CloseHandle(h: *mut c_void) -> i32;
    fn GetTickCount64() -> u64;
    fn GetCurrentProcessId() -> u32;
}

const PAGE_READWRITE: u32 = 0x04;
const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;
const FILE_MAP_READ: u32 = 0x0004;

/// RAII handle to the mapped block.
pub struct LiveStatusMap {
    handle: *mut c_void,
    view: *mut LiveStatus,
}

unsafe impl Send for LiveStatusMap {}
unsafe impl Sync for LiveStatusMap {}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

impl LiveStatusMap {
    /// Driver side: create (or attach to) the mapping and stamp the header.
    pub fn create() -> Option<Self> {
        let name = wide(MAPPING_NAME);
        let size = core::mem::size_of::<LiveStatus>() as u32;
        unsafe {
            let h = CreateFileMappingW(-1isize as *mut c_void, core::ptr::null_mut(), PAGE_READWRITE, 0, size, name.as_ptr());
            if h.is_null() {
                return None;
            }
            let v = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, size as usize) as *mut LiveStatus;
            if v.is_null() {
                CloseHandle(h);
                return None;
            }
            let s = &*v;
            s.magic.store(LIVE_MAGIC, Ordering::Relaxed);
            s.version.store(LIVE_VERSION, Ordering::Relaxed);
            s.host_pid.store(GetCurrentProcessId(), Ordering::Relaxed);
            Some(Self { handle: h, view: v })
        }
    }

    /// Control-panel side: open read-only if a driver has created it.
    pub fn open_read() -> Option<Self> {
        let name = wide(MAPPING_NAME);
        let size = core::mem::size_of::<LiveStatus>();
        unsafe {
            let h = OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr());
            if h.is_null() {
                return None;
            }
            let v = MapViewOfFile(h, FILE_MAP_READ, 0, 0, size) as *mut LiveStatus;
            if v.is_null() {
                CloseHandle(h);
                return None;
            }
            Some(Self { handle: h, view: v })
        }
    }

    pub fn get(&self) -> &LiveStatus {
        unsafe { &*self.view }
    }
}

impl Drop for LiveStatusMap {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view as *const c_void);
            CloseHandle(self.handle);
        }
    }
}
