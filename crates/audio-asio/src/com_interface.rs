//! Steinberg IASIO COM Interface VTable and Class Factory.
//! Enables Steinberg Cubase, Cockos Reaper, Ableton Live, FL Studio, etc., to load the Rust driver.

use std::sync::Mutex;
use core::ffi::c_void;
use crate::asio_types::*;
use crate::asio_driver::AsioDriverState;

// CLSID for ZeroLatency Audio: {3E8C1234-A789-4B56-9DEF-0123456789AB}
pub const CLSID_ZERO_LATENCY_AUDIO: [u8; 16] = [
    0x34, 0x12, 0x8C, 0x3E, 0x89, 0xA7, 0x56, 0x4B,
    0x9D, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
];

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
    pub ref_count: u32,
    pub state: Mutex<AsioDriverState>,
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

extern "system" fn asio_query_interface(this: *mut IASIO, _riid: *const [u8; 16], ppv: *mut *mut c_void) -> i32 {
    if ppv.is_null() {
        return -2147467261; // E_POINTER
    }
    unsafe {
        *ppv = this as *mut c_void;
        asio_add_ref(this);
    }
    0 // S_OK
}

extern "system" fn asio_add_ref(this: *mut IASIO) -> u32 {
    unsafe {
        let obj = &mut *this;
        obj.ref_count += 1;
        obj.ref_count
    }
}

extern "system" fn asio_release(this: *mut IASIO) -> u32 {
    unsafe {
        let obj = &mut *this;
        obj.ref_count = obj.ref_count.saturating_sub(1);
        let count = obj.ref_count;
        if count == 0 {
            let _ = Box::from_raw(this);
        }
        count
    }
}

extern "system" fn asio_init(_this: *mut IASIO, _sys_handle: *mut c_void) -> ASIOBool {
    ASIOTrue
}

extern "system" fn asio_get_driver_name(_this: *mut IASIO, name: *mut u8) {
    let driver_name = b"ZeroLatency Rust Kernel ASIO\0";
    unsafe {
        std::ptr::copy_nonoverlapping(driver_name.as_ptr(), name, driver_name.len());
    }
}

extern "system" fn asio_get_driver_version(_this: *mut IASIO) -> i32 {
    1
}

extern "system" fn asio_get_error_message(_this: *mut IASIO, string: *mut u8) {
    let msg = b"No error\0";
    unsafe {
        std::ptr::copy_nonoverlapping(msg.as_ptr(), string, msg.len());
    }
}

extern "system" fn asio_start(this: *mut IASIO) -> ASIOError {
    let obj = unsafe { &*this };
    let mut state = obj.state.lock().unwrap();
    state.start_streaming()
}

extern "system" fn asio_stop(this: *mut IASIO) -> ASIOError {
    let obj = unsafe { &*this };
    let mut state = obj.state.lock().unwrap();
    state.stop_streaming()
}

extern "system" fn asio_get_channels(this: *mut IASIO, num_in: *mut i32, num_out: *mut i32) -> ASIOError {
    let obj = unsafe { &*this };
    let state = obj.state.lock().unwrap();
    unsafe {
        if !num_in.is_null() { *num_in = state.num_input_channels as i32; }
        if !num_out.is_null() { *num_out = state.num_output_channels as i32; }
    }
    ASE_OK
}

extern "system" fn asio_get_latencies(this: *mut IASIO, input_lat: *mut i32, output_lat: *mut i32) -> ASIOError {
    let obj = unsafe { &*this };
    let state = obj.state.lock().unwrap();
    unsafe {
        // True ultra-low latency: buffer size in samples
        if !input_lat.is_null() { *input_lat = state.buffer_size as i32; }
        if !output_lat.is_null() { *output_lat = state.buffer_size as i32; }
    }
    ASE_OK
}

extern "system" fn asio_get_buffer_size(
    _this: *mut IASIO,
    min_size: *mut i32,
    max_size: *mut i32,
    pref_size: *mut i32,
    granularity: *mut i32,
) -> ASIOError {
    unsafe {
        if !min_size.is_null() { *min_size = 1; }         // True 1-sample bufferless minimum!
        if !max_size.is_null() { *max_size = 2048; }
        if !pref_size.is_null() { *pref_size = 16; }       // Preferred: 16 samples (~0.16ms at 96kHz)
        if !granularity.is_null() { *granularity = 1; }    // Step by 1 sample
    }
    ASE_OK
}

extern "system" fn asio_can_sample_rate(_this: *mut IASIO, sample_rate: ASIOSampleRate) -> ASIOError {
    match sample_rate as i64 {
        44100 | 48000 | 88200 | 96000 | 176400 | 192000 | 384000 => ASE_OK,
        _ => ASE_NoClock,
    }
}

extern "system" fn asio_get_sample_rate(this: *mut IASIO, sample_rate: *mut ASIOSampleRate) -> ASIOError {
    let obj = unsafe { &*this };
    let state = obj.state.lock().unwrap();
    unsafe {
        if !sample_rate.is_null() {
            *sample_rate = state.sample_rate;
        }
    }
    ASE_OK
}

extern "system" fn asio_set_sample_rate(this: *mut IASIO, sample_rate: ASIOSampleRate) -> ASIOError {
    let obj = unsafe { &*this };
    let mut state = obj.state.lock().unwrap();
    state.sample_rate = sample_rate;
    ASE_OK
}

extern "system" fn asio_get_clock_sources(_this: *mut IASIO, clocks: *mut ASIOClockSource, num_sources: *mut i32) -> ASIOError {
    unsafe {
        if !num_sources.is_null() { *num_sources = 1; }
        if !clocks.is_null() {
            let clock = &mut *clocks;
            clock.index = 0;
            clock.associated_channel = -1;
            clock.associated_group = -1;
            clock.is_current_source = ASIOTrue;
            let name = b"Internal Ultra-Low-Jitter P-Core Clock\0";
            std::ptr::copy_nonoverlapping(name.as_ptr(), clock.name.as_mut_ptr(), name.len());
        }
    }
    ASE_OK
}

extern "system" fn asio_set_clock_source(_this: *mut IASIO, _ref: i32) -> ASIOError {
    ASE_OK
}

extern "system" fn asio_get_sample_position(
    this: *mut IASIO,
    s_pos: *mut ASIOSamples,
    t_stamp: *mut ASIOTimeStamp,
) -> ASIOError {
    let obj = unsafe { &*this };
    let state = obj.state.lock().unwrap();
    let total = if let Some(mem) = state.client.shared_memory() {
        mem.telemetry.total_frames.load(core::sync::atomic::Ordering::Relaxed)
    } else {
        0
    };

    unsafe {
        if !s_pos.is_null() { *s_pos = total as i64; }
        if !t_stamp.is_null() { *t_stamp = audio_core::read_cpu_timestamp() as i64; }
    }
    ASE_OK
}

extern "system" fn asio_get_channel_info(_this: *mut IASIO, info: *mut ASIOChannelInfo) -> ASIOError {
    unsafe {
        let inf = &mut *info;
        inf.is_active = ASIOTrue;
        inf.channel_group = 0;
        inf.sample_type = ASIOSTFloat32LSB; // 32-bit float audio for maximum dynamic range
        let name = if inf.is_input == ASIOTrue {
            format!("ZeroInput {}\0", inf.channel + 1)
        } else {
            format!("ZeroOutput {}\0", inf.channel + 1)
        };
        std::ptr::copy_nonoverlapping(name.as_ptr(), inf.name.as_mut_ptr(), name.len().min(32));
    }
    ASE_OK
}

extern "system" fn asio_create_buffers(
    this: *mut IASIO,
    buffer_infos: *mut ASIOBufferInfo,
    num_channels: i32,
    buffer_size: i32,
    callbacks: *mut ASIOCallbacks,
) -> ASIOError {
    let obj = unsafe { &*this };
    let mut state = obj.state.lock().unwrap();
    state.buffer_size = buffer_size as usize;
    if !callbacks.is_null() {
        state.callbacks = Some(unsafe { *callbacks });
    }

    // Allocate continuous double buffers (2 halves per channel)
    state.input_buffers.clear();
    state.output_buffers.clear();

    let double_size = (buffer_size * 2) as usize;

    for i in 0..num_channels {
        unsafe {
            let buf_info = &mut *buffer_infos.add(i as usize);
            let mut vec = vec![0.0f32; double_size];
            let ptr = vec.as_mut_ptr();

            buf_info.buffers[0] = ptr as *mut c_void;
            buf_info.buffers[1] = ptr.add(buffer_size as usize) as *mut c_void;

            if buf_info.is_input == ASIOTrue {
                state.input_buffers.push(vec);
            } else {
                state.output_buffers.push(vec);
            }
        }
    }

    ASE_OK
}

extern "system" fn asio_dispose_buffers(this: *mut IASIO) -> ASIOError {
    let obj = unsafe { &*this };
    let mut state = obj.state.lock().unwrap();
    state.input_buffers.clear();
    state.output_buffers.clear();
    ASE_OK
}

extern "system" fn asio_control_panel(_this: *mut IASIO) -> ASIOError {
    let possible_paths = [
        "audio-control.exe",
        "target\\release\\audio-control.exe",
        "C:\\Program Files\\ZeroAudio\\audio-control.exe",
        "C:\\Audio\\target\\release\\audio-control.exe",
    ];

    for path in &possible_paths {
        if std::path::Path::new(path).exists() {
            let _ = std::process::Command::new(path).arg("gui").spawn();
            return ASE_OK;
        }
    }
    let _ = std::process::Command::new("audio-control.exe").arg("gui").spawn();
    ASE_OK
}

extern "system" fn asio_future(_this: *mut IASIO, _selector: i32, _opt: *mut c_void) -> ASIOError {
    ASE_SUCCESS
}

extern "system" fn asio_output_ready(_this: *mut IASIO) -> ASIOError {
    // Fast path: DAW notified that audio samples are written and ready immediately
    ASE_OK
}
