//! ZeroAudioStream: Modern Safe Rust Native Audio Stream Engine.
//! Eliminates Steinberg COM overhead and rigid double-buffering latency.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use core::ffi::c_void;
use audio_core::{
    SharedAudioMemory, AudioFrame, AudioStreamConfig,
    IOCTL_AUDIO_MAP_SHARED_MEMORY, IOCTL_AUDIO_UNMAP_SHARED_MEMORY,
    IOCTL_AUDIO_START_STREAMING, IOCTL_AUDIO_STOP_STREAMING,
    IOCTL_AUDIO_REGISTER_EVENT,
    MapMemoryResponse, RegisterEventRequest,
};
use crate::telemetry::TelemetrySnapshot;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: *mut c_void,
    ) -> *mut c_void;
    fn DeviceIoControl(
        hDevice: *mut c_void,
        dwIoControlCode: u32,
        lpInBuffer: *const c_void,
        nInBufferSize: u32,
        lpOutBuffer: *mut c_void,
        nOutBufferSize: u32,
        lpBytesReturned: *mut u32,
        lpOverlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(hObject: *mut c_void) -> i32;
    fn CreateEventW(
        lpEventAttributes: *mut c_void,
        bManualReset: i32,
        bInitialState: i32,
        lpName: *const u16,
    ) -> *mut c_void;
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
    fn SetThreadAffinityMask(hThread: *mut c_void, dwThreadAffinityMask: usize) -> usize;
    fn SetThreadPriority(hThread: *mut c_void, nPriority: i32) -> i32;
    fn GetCurrentThread() -> *mut c_void;
}

type AvSetFn = unsafe extern "system" fn(*const u16, *mut u32) -> *mut c_void;
type AvRevertFn = unsafe extern "system" fn(*mut c_void) -> i32;

unsafe fn enable_mmcss() -> Option<(*mut c_void, *mut c_void, AvRevertFn)> {
    let dll_name: [u16; 9] = ['a' as u16, 'v' as u16, 'r' as u16, 't' as u16, '.' as u16, 'd' as u16, 'l' as u16, 'l' as u16, 0];
    let h_mod = LoadLibraryW(dll_name.as_ptr());
    if h_mod.is_null() {
        return None;
    }

    let set_proc = GetProcAddress(h_mod, b"AvSetMmThreadCharacteristicsW\0".as_ptr());
    let revert_proc = GetProcAddress(h_mod, b"AvRevertMmThreadCharacteristics\0".as_ptr());

    if set_proc.is_null() || revert_proc.is_null() {
        FreeLibrary(h_mod);
        return None;
    }

    let set_fn: AvSetFn = core::mem::transmute(set_proc);
    let revert_fn: AvRevertFn = core::mem::transmute(revert_proc);

    let task_name: [u16; 10] = ['P' as u16, 'r' as u16, 'o' as u16, ' ' as u16, 'A' as u16, 'u' as u16, 'd' as u16, 'i' as u16, 'o' as u16, 0];
    let mut task_index = 0u32;
    let handle = set_fn(task_name.as_ptr(), &mut task_index);

    Some((h_mod, handle, revert_fn))
}

#[repr(C)]
struct ThreadPowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

type SetThreadInformationFn = unsafe extern "system" fn(*mut c_void, i32, *const c_void, u32) -> i32;

unsafe fn disable_power_throttling(h_thread: *mut c_void) {
    let dll_name: [u16; 13] = [
        'k' as u16, 'e' as u16, 'r' as u16, 'n' as u16, 'e' as u16, 'l' as u16,
        '3' as u16, '2' as u16, '.' as u16, 'd' as u16, 'l' as u16, 'l' as u16, 0,
    ];
    let k32 = LoadLibraryW(dll_name.as_ptr());
    if !k32.is_null() {
        let proc = GetProcAddress(k32, b"SetThreadInformation\0".as_ptr());
        if !proc.is_null() {
            let set_info: SetThreadInformationFn = core::mem::transmute(proc);
            let state = ThreadPowerThrottlingState {
                version: 1,
                control_mask: 0x1,
                state_mask: 0,
            };
            set_info(h_thread, 1, &state as *const _ as *const c_void, core::mem::size_of::<ThreadPowerThrottlingState>() as u32);
        }
        FreeLibrary(k32);
    }
}

unsafe fn disable_mmcss(info: (*mut c_void, *mut c_void, AvRevertFn)) {
    let (h_mod, handle, revert_fn) = info;
    if !handle.is_null() {
        revert_fn(handle);
    }
    FreeLibrary(h_mod);
}

const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

static DEVICE_PATH: [u16; 23] = [
    '\\' as u16, '\\' as u16, '.' as u16, '\\' as u16,
    'Z' as u16, 'e' as u16, 'r' as u16, 'o' as u16, 'L' as u16, 'a' as u16, 't' as u16, 'e' as u16,
    'n' as u16, 'c' as u16, 'y' as u16, 'A' as u16, 'u' as u16, 'd' as u16, 'i' as u16, 'o' as u16,
    0, 0, 0,
];

pub struct ZeroAudioStream {
    device_handle: *mut c_void,
    shared_mem: *mut SharedAudioMemory,
    event_handle: *mut c_void,
    is_kernel_connected: bool,
    _fallback_mem: Option<Box<SharedAudioMemory>>,
    is_streaming: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

unsafe impl Send for ZeroAudioStream {}
unsafe impl Sync for ZeroAudioStream {}

impl ZeroAudioStream {
    pub fn open() -> Result<Self, &'static str> {
        let mut stream = Self {
            device_handle: INVALID_HANDLE_VALUE,
            shared_mem: core::ptr::null_mut(),
            event_handle: core::ptr::null_mut(),
            is_kernel_connected: false,
            _fallback_mem: None,
            is_streaming: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        };

        unsafe {
            let h_device = CreateFileW(
                DEVICE_PATH.as_ptr(),
                0xC0000000, // GENERIC_READ | GENERIC_WRITE
                0,
                core::ptr::null_mut(),
                3, // OPEN_EXISTING
                0x80, // FILE_ATTRIBUTE_NORMAL
                core::ptr::null_mut(),
            );

            if h_device != INVALID_HANDLE_VALUE {
                stream.device_handle = h_device;
                stream.is_kernel_connected = true;

                let mut resp = MapMemoryResponse {
                    user_address: 0,
                    size_bytes: 0,
                    version: 0,
                    _reserved: 0,
                };
                let mut bytes_returned: u32 = 0;

                let success = DeviceIoControl(
                    h_device,
                    IOCTL_AUDIO_MAP_SHARED_MEMORY,
                    core::ptr::null(),
                    0,
                    &mut resp as *mut _ as _,
                    core::mem::size_of::<MapMemoryResponse>() as u32,
                    &mut bytes_returned,
                    core::ptr::null_mut(),
                );

                if success != 0 && resp.user_address != 0 {
                    stream.shared_mem = resp.user_address as *mut SharedAudioMemory;

                    let ev = CreateEventW(core::ptr::null_mut(), 0, 0, core::ptr::null());
                    if !ev.is_null() {
                        stream.event_handle = ev;
                        let mut reg = RegisterEventRequest { event_handle: ev as u64 };
                        let mut ret: u32 = 0;
                        let _ = DeviceIoControl(
                            h_device,
                            IOCTL_AUDIO_REGISTER_EVENT,
                            &mut reg as *mut _ as _,
                            core::mem::size_of::<RegisterEventRequest>() as u32,
                            core::ptr::null_mut(),
                            0,
                            &mut ret,
                            core::ptr::null_mut(),
                        );
                    }
                }
            } else {
                // High-performance Userland Memory Engine Fallback
                let mem = SharedAudioMemory::heap_boxed(AudioStreamConfig::DEFAULT);
                stream.shared_mem = Box::into_raw(mem);
                stream.event_handle = CreateEventW(core::ptr::null_mut(), 0, 0, core::ptr::null());
            }
        }

        if stream.shared_mem.is_null() {
            Err("Failed to initialize shared audio memory")
        } else {
            Ok(stream)
        }
    }

    /// Direct single-sample write (Wait-free, sub-nanosecond, no double-buffering!)
    #[inline(always)]
    pub fn push_sample(&mut self, frame: AudioFrame) -> bool {
        unsafe {
            if let Some(mem) = self.shared_mem.as_mut() {
                mem.playback_ring.push(frame)
            } else {
                false
            }
        }
    }

    /// Direct single-sample read
    #[inline(always)]
    pub fn pop_sample(&mut self) -> Option<AudioFrame> {
        unsafe {
            if let Some(mem) = self.shared_mem.as_mut() {
                mem.capture_ring.pop()
            } else {
                None
            }
        }
    }

    /// Start streaming with a high-performance audio callback
    pub fn start<F>(&mut self, mut callback: F) -> Result<(), &'static str>
    where
        F: FnMut(&[AudioFrame], &mut [AudioFrame]) + Send + 'static,
    {
        if self.is_streaming.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_streaming.store(true, Ordering::SeqCst);

        if self.is_kernel_connected {
            unsafe {
                let mut ret: u32 = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_START_STREAMING,
                    core::ptr::null(),
                    0,
                    core::ptr::null_mut(),
                    0,
                    &mut ret,
                    core::ptr::null_mut(),
                );
            }
        }

        let is_running = self.is_streaming.clone();
        let shared_mem_ptr = self.shared_mem as usize;
        let event_handle = self.event_handle as usize;

        let thread_handle = thread::Builder::new()
            .name("ZeroAudioNativePump".into())
            .spawn(move || {
                unsafe {
                    // Dynamic MMCSS Pro Audio Registration
                    let mmcss_info = enable_mmcss();

                    // Set Thread Priority to TIME_CRITICAL (15) inside MMCSS Pro Audio
                    let current_thread = GetCurrentThread();
                    SetThreadPriority(current_thread, 15);

                    // Pin Thread Affinity to Core 2 (P-Core / Zen 4 isolated core)
                    SetThreadAffinityMask(current_thread, 0x04);

                    // Disable Windows Power Throttling / CPU core downclocking
                    disable_power_throttling(current_thread);

                    let mem = &mut *(shared_mem_ptr as *mut SharedAudioMemory);
                    let quantum = (mem.config.quantum_frames as usize).clamp(16, 512);
                    let sample_rate = mem.config.sample_rate;

                    let mut pacer = audio_core::AudioClockPacer::new(sample_rate, quantum as u32);
                    let mut in_buffer = vec![AudioFrame::EMPTY; quantum];
                    let mut out_buffer = vec![AudioFrame::EMPTY; quantum];

                    while is_running.load(Ordering::Relaxed) {
                        if event_handle != 0 {
                            WaitForSingleObject(event_handle as *mut c_void, 1);
                        } else {
                            let tick_info = pacer.wait_next_tick();
                            if tick_info.is_dpc_spike {
                                mem.telemetry.last_dpc_cycles.store(audio_core::read_cpu_timestamp(), Ordering::Relaxed);
                            }
                        }

                        // Read input slice from capture ring
                        mem.capture_ring.read_slice(quantum, |slice| {
                            in_buffer[..slice.len()].copy_from_slice(slice);
                        });

                        // Run user DSP callback
                        callback(&in_buffer[..quantum], &mut out_buffer[..quantum]);

                        // Write output slice to playback ring
                        mem.playback_ring.write_slice(quantum, |slice| {
                            slice.copy_from_slice(&out_buffer[..slice.len()]);
                        });
                    }

                    if let Some(info) = mmcss_info {
                        disable_mmcss(info);
                    }
                }
            })
            .map_err(|_| "Failed to spawn real-time pump thread")?;

        self.thread_handle = Some(thread_handle);
        Ok(())
    }

    /// Stop streaming
    pub fn stop(&mut self) {
        self.is_streaming.store(false, Ordering::SeqCst);

        if self.is_kernel_connected {
            unsafe {
                let mut ret: u32 = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_STOP_STREAMING,
                    core::ptr::null(),
                    0,
                    core::ptr::null_mut(),
                    0,
                    &mut ret,
                    core::ptr::null_mut(),
                );
            }
        }

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Get real-time telemetry snapshot
    pub fn telemetry(&self) -> Option<TelemetrySnapshot> {
        unsafe {
            self.shared_mem.as_ref().map(TelemetrySnapshot::capture)
        }
    }

    /// Access immutable shared memory (config, meters, routing matrix)
    pub fn shared_memory(&self) -> Option<&SharedAudioMemory> {
        unsafe { self.shared_mem.as_ref() }
    }

    /// Access mutable shared memory for control panel parameter updates
    pub fn shared_memory_mut(&mut self) -> Option<&mut SharedAudioMemory> {
        unsafe { self.shared_mem.as_mut() }
    }
}

impl Drop for ZeroAudioStream {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            if self.is_kernel_connected {
                let mut ret: u32 = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_UNMAP_SHARED_MEMORY,
                    core::ptr::null(),
                    0,
                    core::ptr::null_mut(),
                    0,
                    &mut ret,
                    core::ptr::null_mut(),
                );
                CloseHandle(self.device_handle);
            } else if !self.shared_mem.is_null() {
                let _ = Box::from_raw(self.shared_mem);
            }

            if !self.event_handle.is_null() {
                CloseHandle(self.event_handle);
            }
        }
    }
}
