//! Implementation of the Steinberg IASIO COM Interface.
//! Integrates MMCSS "Pro Audio" scheduling, P-Core CPU affinity, and sub-millisecond audio pump.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use core::ffi::c_void;
use crate::asio_types::*;
use crate::kernel_client::KernelAudioClient;
use audio_core::{AudioFrame, MAX_AUDIO_CHANNELS};

#[derive(Copy, Clone)]
pub struct SendPtr(pub *mut f32);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn SetThreadAffinityMask(hThread: *mut c_void, dwThreadAffinityMask: usize) -> usize;
    fn GetCurrentThread() -> *mut c_void;
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
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
                control_mask: 0x1, // THREAD_POWER_THROTTLING_EXECUTION_SPEED
                state_mask: 0,     // 0 = Disable power throttling (Maximum CPU Speed)
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

pub struct AsioDriverState {
    pub client: KernelAudioClient,
    pub sample_rate: f64,
    pub buffer_size: usize,
    pub num_input_channels: usize,
    pub num_output_channels: usize,
    pub is_running: Arc<AtomicBool>,
    pub thread_handle: Option<JoinHandle<()>>,
    pub callbacks: Option<ASIOCallbacks>,
    pub input_buffers: Vec<Vec<f32>>,
    pub output_buffers: Vec<Vec<f32>>,
}

impl AsioDriverState {
    pub fn new() -> Self {
        let client = KernelAudioClient::new();
        let (in_ch, out_ch, s_rate, q_frames) = if let Some(mem) = client.shared_memory() {
            (
                mem.config.input_channels as usize,
                mem.config.output_channels as usize,
                mem.config.sample_rate as f64,
                mem.config.quantum_frames as usize,
            )
        } else {
            (64, 64, 96000.0, 16)
        };

        Self {
            client,
            sample_rate: s_rate,
            buffer_size: q_frames,
            num_input_channels: in_ch.min(MAX_AUDIO_CHANNELS),
            num_output_channels: out_ch.min(MAX_AUDIO_CHANNELS),
            is_running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            callbacks: None,
            input_buffers: Vec::new(),
            output_buffers: Vec::new(),
        }
    }

    pub fn start_streaming(&mut self) -> ASIOError {
        if self.is_running.load(Ordering::SeqCst) {
            return ASE_OK;
        }

        self.is_running.store(true, Ordering::SeqCst);
        self.client.start_streaming();

        let is_running = self.is_running.clone();
        let callbacks = match self.callbacks {
            Some(cb) => cb,
            None => return ASE_HWMalfunction,
        };

        let buffer_size = self.buffer_size;
        let event_handle = self.client.event_handle() as usize;
        let shared_mem_ptr = self.client.shared_memory().map(|s| s as *const _ as usize).unwrap_or(0);
        let num_inputs = self.num_input_channels;
        let num_outputs = self.num_output_channels;

        // Collect SendPtr wrappers to double buffer halves
        let in_buf_ptrs: Vec<[SendPtr; 2]> = self.input_buffers.iter_mut().map(|b| {
            let half = buffer_size;
            let ptr = b.as_mut_ptr();
            [SendPtr(ptr), SendPtr(unsafe { ptr.add(half) })]
        }).collect();

        let out_buf_ptrs: Vec<[SendPtr; 2]> = self.output_buffers.iter_mut().map(|b| {
            let half = buffer_size;
            let ptr = b.as_mut_ptr();
            [SendPtr(ptr), SendPtr(unsafe { ptr.add(half) })]
        }).collect();

        // Spawn Real-Time Audio Thread with dynamic MMCSS and P-Core Affinity
        let handle = thread::Builder::new()
            .name("ZeroLatencyAsioPump".into())
            .spawn(move || {
                unsafe {
                    // 1. Dynamic MMCSS Pro Audio Registration
                    let mmcss_info = enable_mmcss();

                    // 2. Pin Thread Affinity to P-Core (Core 2, mask = 0b0100)
                    let current_thread = GetCurrentThread();
                    SetThreadAffinityMask(current_thread, 0x04);

                    // 3. Disable Windows Power Throttling / Intel Thread Director downclocking
                    disable_power_throttling(current_thread);

                    let mut double_buffer_index = 0i32;

                    while is_running.load(Ordering::Relaxed) {
                        // Wait for kernel clock tick event (or spin wait for microsecond latency)
                        if event_handle != 0 {
                            WaitForSingleObject(event_handle as *mut c_void, 2);
                        } else {
                            core::hint::spin_loop();
                        }

                        // Transfer input samples from kernel capture ring into DAW input buffers
                        if shared_mem_ptr != 0 {
                            let mem = &mut *(shared_mem_ptr as *mut audio_core::SharedAudioMemory);
                            for frame_i in 0..buffer_size {
                                if let Some(frame) = mem.capture_ring.pop() {
                                    for ch in 0..num_inputs.min(in_buf_ptrs.len()) {
                                        let target = in_buf_ptrs[ch][double_buffer_index as usize].0.add(frame_i);
                                        *target = frame.samples[ch];
                                    }
                                }
                            }
                        }

                        // Trigger Host DAW buffer switch callback
                        if let Some(switch_fn) = callbacks.buffer_switch {
                            switch_fn(double_buffer_index, ASIOTrue);
                        }

                        // Transfer processed output samples from DAW buffers into kernel playback ring
                        if shared_mem_ptr != 0 {
                            let mem = &mut *(shared_mem_ptr as *mut audio_core::SharedAudioMemory);
                            for frame_i in 0..buffer_size {
                                let mut frame = AudioFrame::EMPTY;
                                for ch in 0..num_outputs.min(out_buf_ptrs.len()) {
                                    let src = out_buf_ptrs[ch][double_buffer_index as usize].0.add(frame_i);
                                    frame.samples[ch] = *src;
                                }
                                mem.playback_ring.push(frame);
                            }
                        }

                        // Toggle buffer half: 0 -> 1 -> 0
                        double_buffer_index ^= 1;
                    }

                    if let Some(info) = mmcss_info {
                        disable_mmcss(info);
                    }
                }
            })
            .expect("Failed to spawn audio pump thread");

        self.thread_handle = Some(handle);
        ASE_OK
    }

    pub fn stop_streaming(&mut self) -> ASIOError {
        self.is_running.store(false, Ordering::SeqCst);
        self.client.stop_streaming();

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        ASE_OK
    }
}
