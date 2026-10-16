//! Steinberg IASIO COM interface (vtable) for DolomiteAudioRT.
#![allow(non_upper_case_globals)]
//! Loaded by PreSonus Studio One, Cubase, Reaper, Ableton Live, FL Studio, ...

use core::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::asio_driver::{AsioDriverState, EngineShared};
use crate::asio_types::*;

// CLSID for DolomiteAudioRT: {3E8C1234-A789-4B56-9DEF-0123456789AB}
pub const CLSID_ZERO_LATENCY_AUDIO: [u8; 16] = [
    0x34, 0x12, 0x8C, 0x3E, 0x89, 0xA7, 0x56, 0x4B,
    0x9D, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
];

pub const SUPPORTED_RATES: [f64; 7] = [44100.0, 48000.0, 88200.0, 96000.0, 176400.0, 192000.0, 384000.0];

#[repr(C)]
pub struct IASIOVtbl {
    pub query_interface: extern "system" fn(*mut IASIO, *const [u8; 16], *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut IASIO) -> u32,
    pub release: extern "system" fn(*mut IASIO) -> u32,
    pub init: extern "system" fn(*mut IASIO, *mut c_void) -> ASIOBool,
    pub get_driver_name: extern "system" fn(*mut IASIO, *mut u8),
    pub get_driver_version: extern "system" fn(*mut IASIO) -> i32,
    pub get_error_message: extern "system" fn(*mut IASIO, *mut u8),
    pub start: extern "system" fn(*mut IASIO) -> ASIOError,
    pub stop: extern "system" fn(*mut IASIO) -> ASIOError,
    pub get_channels: extern "system" fn(*mut IASIO, *mut i32, *mut i32) -> ASIOError,
    pub get_latencies: extern "system" fn(*mut IASIO, *mut i32, *mut i32) -> ASIOError,
    pub get_buffer_size: extern "system" fn(*mut IASIO, *mut i32, *mut i32, *mut i32, *mut i32) -> ASIOError,
    pub can_sample_rate: extern "system" fn(*mut IASIO, ASIOSampleRate) -> ASIOError,
    pub get_sample_rate: extern "system" fn(*mut IASIO, *mut ASIOSampleRate) -> ASIOError,
    pub set_sample_rate: extern "system" fn(*mut IASIO, ASIOSampleRate) -> ASIOError,
    pub get_clock_sources: extern "system" fn(*mut IASIO, *mut ASIOClockSource, *mut i32) -> ASIOError,
    pub set_clock_source: extern "system" fn(*mut IASIO, i32) -> ASIOError,
    pub get_sample_position: extern "system" fn(*mut IASIO, *mut ASIOSamples, *mut ASIOTimeStamp) -> ASIOError,
    pub get_channel_info: extern "system" fn(*mut IASIO, *mut ASIOChannelInfo) -> ASIOError,
    pub create_buffers: extern "system" fn(*mut IASIO, *mut ASIOBufferInfo, i32, i32, *mut ASIOCallbacks) -> ASIOError,
    pub dispose_buffers: extern "system" fn(*mut IASIO) -> ASIOError,
    pub control_panel: extern "system" fn(*mut IASIO) -> ASIOError,
    pub future: extern "system" fn(*mut IASIO, i32, *mut c_void) -> ASIOError,
    pub output_ready: extern "system" fn(*mut IASIO) -> ASIOError,
}

#[repr(C)]
pub struct IASIO {
    pub vtbl: *const IASIOVtbl,
    pub ref_count: AtomicU32,
    /// Lock-free state readable from inside the host's bufferSwitch.
    pub shared: Arc<EngineShared>,
    pub state: Mutex<AsioDriverState>,
}

impl IASIO {
    pub fn new_boxed() -> Box<Self> {
        let shared = Arc::new(EngineShared::new());
        Box::new(IASIO {
            vtbl: &ASIO_VTBL,
            ref_count: AtomicU32::new(1),
            shared: shared.clone(),
            state: Mutex::new(AsioDriverState::new(shared)),
        })
    }

    fn lock(&self) -> MutexGuard<'_, AsioDriverState> {
        // A panic on another thread must not take the DAW down with it.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub static ASIO_VTBL: IASIOVtbl = IASIOVtbl {
    query_interface: asio_query_interface,
    add_ref: asio_add_ref,
    release: asio_release,
    init: asio_init,
    get_driver_name: asio_get_driver_name,
    get_driver_version: asio_get_driver_version,
    get_error_message: asio_get_error_message,
    start: asio_start,
    stop: asio_stop,
    get_channels: asio_get_channels,
    get_latencies: asio_get_latencies,
    get_buffer_size: asio_get_buffer_size,
    can_sample_rate: asio_can_sample_rate,
    get_sample_rate: asio_get_sample_rate,
    set_sample_rate: asio_set_sample_rate,
    get_clock_sources: asio_get_clock_sources,
    set_clock_source: asio_set_clock_source,
    get_sample_position: asio_get_sample_position,
    get_channel_info: asio_get_channel_info,
    create_buffers: asio_create_buffers,
    dispose_buffers: asio_dispose_buffers,
    control_panel: asio_control_panel,
    future: asio_future,
    output_ready: asio_output_ready,
};

fn obj<'a>(this: *mut IASIO) -> &'a IASIO {
    unsafe { &*this }
}

unsafe fn write_cstr(dst: *mut u8, s: &[u8], cap: usize) {
    if dst.is_null() {
        return;
    }
    let n = s.len().min(cap - 1);
    core::ptr::copy_nonoverlapping(s.as_ptr(), dst, n);
    *dst.add(n) = 0;
}

extern "system" fn asio_query_interface(this: *mut IASIO, _riid: *const [u8; 16], ppv: *mut *mut c_void) -> i32 {
    if ppv.is_null() {
        return -2147467261; // E_POINTER
    }
    unsafe { *ppv = this as *mut c_void };
    asio_add_ref(this);
    0
}

extern "system" fn asio_add_ref(this: *mut IASIO) -> u32 {
    obj(this).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

extern "system" fn asio_release(this: *mut IASIO) -> u32 {
    let prev = obj(this).ref_count.fetch_sub(1, Ordering::AcqRel);
    if prev == 1 {
        // Last reference: make sure the pump is gone before freeing buffers.
        let handle = obj(this).lock().request_stop();
        if let Some(h) = handle {
            let _ = h.join();
        }
        unsafe { drop(Box::from_raw(this)) };
        return 0;
    }
    prev.saturating_sub(1)
}

extern "system" fn asio_init(_this: *mut IASIO, _sys_handle: *mut c_void) -> ASIOBool {
    crate::asio_driver::log_debug("IASIO::init");
    ASIOTrue
}

extern "system" fn asio_get_driver_name(_this: *mut IASIO, name: *mut u8) {
    unsafe { write_cstr(name, b"DolomiteAudioRT", 32) };
}

extern "system" fn asio_get_driver_version(_this: *mut IASIO) -> i32 {
    2
}

extern "system" fn asio_get_error_message(_this: *mut IASIO, string: *mut u8) {
    unsafe { write_cstr(string, b"No error", 124) };
}

extern "system" fn asio_start(this: *mut IASIO) -> ASIOError {
    crate::asio_driver::log_debug("IASIO::start");
    obj(this).lock().start_streaming()
}

extern "system" fn asio_stop(this: *mut IASIO) -> ASIOError {
    crate::asio_driver::log_debug("IASIO::stop");
    // Join the pump OUTSIDE the mutex: the host may call getSamplePosition()
    // (or anything else) from inside the bufferSwitch that is finishing.
    let handle = obj(this).lock().request_stop();
    if let Some(h) = handle {
        let _ = h.join();
    }
    obj(this).lock().client.stop_streaming();
    ASE_OK
}

extern "system" fn asio_get_channels(this: *mut IASIO, num_in: *mut i32, num_out: *mut i32) -> ASIOError {
    let s = obj(this).lock();
    unsafe {
        if !num_in.is_null() {
            *num_in = s.num_input_channels as i32;
        }
        if !num_out.is_null() {
            *num_out = s.num_output_channels as i32;
        }
    }
    ASE_OK
}

extern "system" fn asio_get_latencies(this: *mut IASIO, input_lat: *mut i32, output_lat: *mut i32) -> ASIOError {
    let sh = &obj(this).shared;
    unsafe {
        if !input_lat.is_null() {
            *input_lat = sh.input_latency.load(Ordering::Relaxed) as i32;
        }
        if !output_lat.is_null() {
            *output_lat = sh.output_latency.load(Ordering::Relaxed) as i32;
        }
    }
    ASE_OK
}

extern "system" fn asio_get_buffer_size(this: *mut IASIO, min_size: *mut i32, max_size: *mut i32, pref_size: *mut i32, granularity: *mut i32) -> ASIOError {
    let pref = obj(this).lock().buffer_size as i32;
    unsafe {
        if !min_size.is_null() {
            *min_size = 32;
        }
        if !max_size.is_null() {
            *max_size = 2048;
        }
        if !pref_size.is_null() {
            *pref_size = (pref.clamp(32, 2048) as u32).next_power_of_two() as i32;
        }
        if !granularity.is_null() {
            *granularity = -1; // powers of two
        }
    }
    ASE_OK
}

extern "system" fn asio_can_sample_rate(_this: *mut IASIO, sample_rate: ASIOSampleRate) -> ASIOError {
    if SUPPORTED_RATES.iter().any(|r| (r - sample_rate).abs() < 0.5) {
        ASE_OK
    } else {
        ASE_NoClock
    }
}

extern "system" fn asio_get_sample_rate(this: *mut IASIO, sample_rate: *mut ASIOSampleRate) -> ASIOError {
    if sample_rate.is_null() {
        return ASE_InvalidParameter;
    }
    unsafe { *sample_rate = obj(this).lock().sample_rate };
    ASE_OK
}

extern "system" fn asio_set_sample_rate(this: *mut IASIO, sample_rate: ASIOSampleRate) -> ASIOError {
    if sample_rate == 0.0 {
        return ASE_OK; // "external clock" request: we always run on the device clock
    }
    if asio_can_sample_rate(this, sample_rate) != ASE_OK {
        return ASE_NoClock;
    }
    let (changed, cb) = {
        let mut s = obj(this).lock();
        let changed = (s.sample_rate - sample_rate).abs() > 0.5;
        s.sample_rate = sample_rate;
        s.update_latency_estimate(None);
        (changed, s.callbacks)
    };
    crate::asio_driver::log_debug(&format!("IASIO::setSampleRate({})", sample_rate));
    if changed {
        if let Some(f) = cb.and_then(|c| c.sample_rate_did_change) {
            f(sample_rate);
        }
    }
    ASE_OK
}

extern "system" fn asio_get_clock_sources(_this: *mut IASIO, clocks: *mut ASIOClockSource, num_sources: *mut i32) -> ASIOError {
    unsafe {
        if !clocks.is_null() {
            let c = &mut *clocks;
            c.index = 0;
            c.associated_channel = -1;
            c.associated_group = -1;
            c.is_current_source = ASIOTrue;
            c.name = [0; 32];
            write_cstr(c.name.as_mut_ptr(), b"Internal (sound card)", 32);
        }
        if !num_sources.is_null() {
            *num_sources = 1;
        }
    }
    ASE_OK
}

extern "system" fn asio_set_clock_source(_this: *mut IASIO, reference: i32) -> ASIOError {
    if reference == 0 { ASE_OK } else { ASE_InvalidParameter }
}

/// Lock-free: Studio One and others call this from inside bufferSwitch.
extern "system" fn asio_get_sample_position(this: *mut IASIO, s_pos: *mut ASIOSamples, t_stamp: *mut ASIOTimeStamp) -> ASIOError {
    let sh = &obj(this).shared;
    unsafe {
        if !s_pos.is_null() {
            *s_pos = ASIO64::from_i64(sh.sample_position.load(Ordering::Acquire));
        }
        if !t_stamp.is_null() {
            *t_stamp = ASIO64::from_i64(sh.system_time_ns.load(Ordering::Acquire));
        }
    }
    ASE_OK
}

extern "system" fn asio_get_channel_info(this: *mut IASIO, info: *mut ASIOChannelInfo) -> ASIOError {
    if info.is_null() {
        return ASE_InvalidParameter;
    }
    let s = obj(this).lock();
    unsafe {
        let inf = &mut *info;
        let is_in = inf.is_input != ASIOFalse;
        let max_ch = if is_in { s.num_input_channels } else { s.num_output_channels };
        if inf.channel < 0 || (inf.channel as usize) >= max_ch {
            return ASE_InvalidParameter;
        }
        inf.is_active = ASIOTrue;
        inf.channel_group = 0;
        inf.sample_type = ASIOSTFloat32LSB;
        inf.name = [0; 32];
        let side = if inf.channel % 2 == 0 { "L" } else { "R" };
        let label = if is_in { format!("Dolomite In {}", side) } else { format!("Dolomite Out {}", side) };
        write_cstr(inf.name.as_mut_ptr(), label.as_bytes(), 32);
    }
    ASE_OK
}

extern "system" fn asio_create_buffers(this: *mut IASIO, buffer_infos: *mut ASIOBufferInfo, num_channels: i32, buffer_size: i32, callbacks: *mut ASIOCallbacks) -> ASIOError {
    crate::asio_driver::log_debug(&format!("IASIO::createBuffers channels={} size={}", num_channels, buffer_size));
    if buffer_size < 16 || buffer_size > 8192 || buffer_infos.is_null() || callbacks.is_null() || num_channels <= 0 {
        return ASE_InvalidParameter;
    }

    // Recreating buffers while running would free memory the pump is using.
    let handle = obj(this).lock().request_stop();
    if let Some(h) = handle {
        let _ = h.join();
    }

    let mut s = obj(this).lock();
    let cb = unsafe { *callbacks };
    s.callbacks = Some(cb);
    s.buffer_size = buffer_size as usize;

    // Only use bufferSwitchTimeInfo if the host says it supports it (ASIO 2 rule).
    s.use_time_info = match (cb.asio_message, cb.buffer_switch_time_info) {
        (Some(msg), Some(_)) => {
            let mut sel = kAsioSupportsTimeInfo;
            msg(kAsioSelectorSupported, kAsioSupportsTimeInfo, &mut sel as *mut _ as *mut c_void, core::ptr::null_mut()) != 0
                && msg(kAsioSupportsTimeInfo, 0, core::ptr::null_mut(), core::ptr::null_mut()) != 0
        }
        _ => false,
    };

    let double = s.buffer_size * 2;
    let (ni, no) = (s.num_input_channels, s.num_output_channels);
    s.input_buffers = (0..ni).map(|_| vec![0.0f32; double]).collect();
    s.output_buffers = (0..no).map(|_| vec![0.0f32; double]).collect();

    for i in 0..num_channels as usize {
        unsafe {
            let bi = &mut *buffer_infos.add(i);
            let ch = bi.channel_num;
            let list = if bi.is_input != ASIOFalse { &mut s.input_buffers } else { &mut s.output_buffers };
            if ch < 0 || ch as usize >= list.len() {
                return ASE_InvalidParameter;
            }
            let p = list[ch as usize].as_mut_ptr();
            bi.buffers[0] = p as *mut c_void;
            bi.buffers[1] = p.add(buffer_size as usize) as *mut c_void;
        }
    }
    s.update_latency_estimate(None);
    crate::asio_driver::log_debug(&format!("createBuffers ok (timeInfo={})", s.use_time_info));
    ASE_OK
}

extern "system" fn asio_dispose_buffers(this: *mut IASIO) -> ASIOError {
    let handle = obj(this).lock().request_stop();
    if let Some(h) = handle {
        let _ = h.join();
    }
    let mut s = obj(this).lock();
    s.client.stop_streaming();
    s.input_buffers.clear();
    s.output_buffers.clear();
    s.callbacks = None;
    ASE_OK
}

extern "system" fn asio_control_panel(_this: *mut IASIO) -> ASIOError {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    // 1) next to this DLL (installed layout)
    if let Some(dir) = crate::module_dir() {
        candidates.push(dir.join("audio-control.exe"));
    }
    candidates.push(r"C:\Program Files\DolomiteAudio\audio-control.exe".into());
    candidates.push(r"C:\Program Files\ZeroAudio\audio-control.exe".into());
    for p in candidates {
        if p.exists() {
            let _ = std::process::Command::new(p).arg("gui").spawn();
            return ASE_OK;
        }
    }
    let _ = std::process::Command::new("audio-control.exe").arg("gui").spawn();
    ASE_OK
}

extern "system" fn asio_future(_this: *mut IASIO, selector: i32, _opt: *mut c_void) -> ASIOError {
    match selector {
        kAsioEnableTimeCodeRead | kAsioDisableTimeCodeRead | kAsioCanTimeInfo => ASE_SUCCESS,
        _ => ASE_NotPresent,
    }
}

extern "system" fn asio_output_ready(_this: *mut IASIO) -> ASIOError {
    ASE_NotPresent
}
