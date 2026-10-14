//! Device Extension and Audio Engine Kernel State.
//! Manages direct physical memory mapping, P-Core CPU affinity, and DPC dispatching.

use core::sync::atomic::{Ordering, AtomicPtr};
use audio_core::{
    SharedAudioMemory, AudioStreamConfig, AudioFrame, status_flags,
    read_cpu_timestamp, MAX_AUDIO_CHANNELS,
};
use crate::nt::*;
use crate::pnp::DevicePnpState;

pub const AUDIO_POOL_TAG: u32 = 0x5541305A; // 'Z0AU'

#[repr(C)]
pub struct AudioDeviceExtension {
    /// Non-paged physical shared memory structure
    pub shared_memory: *mut SharedAudioMemory,
    /// Memory Descriptor List for user mapping
    pub mdl: *mut MDL,
    /// User-mode mapped base address for active process
    pub user_mapped_va: *mut core::ffi::c_void,
    /// High-resolution Kernel DPC
    pub dpc: KDPC,
    /// Kernel Timer driving audio DMA clock
    pub timer: KTIMER,
    /// Referenced user KEVENT to signal audio quantum ready
    pub user_event: *mut core::ffi::c_void,
    /// Is the audio streaming engine active
    pub is_streaming: bool,
    /// State saved during system sleep
    pub was_streaming_before_sleep: bool,
    /// Target CPU core for audio DPC execution
    pub target_core: u8,
    /// Spinlock protecting extension configuration changes
    pub spinlock: KSPIN_LOCK,
    /// Current Plug and Play state
    pub pnp_state: DevicePnpState,
    /// Current Power state
    pub power_state: DEVICE_POWER_STATE,
}

static GLOBAL_EXTENSION: AtomicPtr<AudioDeviceExtension> = AtomicPtr::new(core::ptr::null_mut());

impl AudioDeviceExtension {
    pub unsafe fn new() -> Result<*mut Self, NTSTATUS> {
        // Allocate Device Extension in Non-Paged Pool
        let ext_ptr = ExAllocatePool2(
            POOL_FLAG_NON_PAGED,
            core::mem::size_of::<AudioDeviceExtension>(),
            AUDIO_POOL_TAG,
        ) as *mut AudioDeviceExtension;

        if ext_ptr.is_null() {
            return Err(STATUS_INSUFFICIENT_RESOURCES);
        }

        // Allocate Shared Memory (contains SPSC ring buffers + telemetry + routing matrix + meters)
        let mem_size = core::mem::size_of::<SharedAudioMemory>();
        let shared_mem_ptr = ExAllocatePool2(
            POOL_FLAG_NON_PAGED,
            mem_size,
            AUDIO_POOL_TAG,
        ) as *mut SharedAudioMemory;

        if shared_mem_ptr.is_null() {
            ExFreePoolWithTag(ext_ptr as _, AUDIO_POOL_TAG);
            return Err(STATUS_INSUFFICIENT_RESOURCES);
        }

        // Initialize shared memory
        (*shared_mem_ptr).init(AudioStreamConfig::DEFAULT);

        // Build MDL (Memory Descriptor List) for direct user-mode zero-copy mapping
        let mdl = IoAllocateMdl(
            shared_mem_ptr as _,
            mem_size as u32,
            0,
            0,
            core::ptr::null_mut(),
        );

        if mdl.is_null() {
            ExFreePoolWithTag(shared_mem_ptr as _, AUDIO_POOL_TAG);
            ExFreePoolWithTag(ext_ptr as _, AUDIO_POOL_TAG);
            return Err(STATUS_INSUFFICIENT_RESOURCES);
        }

        MmBuildMdlForNonPagedPool(mdl);

        // Initialize DPC, Timer, Spinlock
        let ext = &mut *ext_ptr;
        ext.shared_memory = shared_mem_ptr;
        ext.mdl = mdl;
        ext.user_mapped_va = core::ptr::null_mut();
        ext.user_event = core::ptr::null_mut();
        ext.is_streaming = false;
        ext.was_streaming_before_sleep = false;
        ext.target_core = 2; // Default to P-Core 2 (Intel Core Ultra)
        ext.spinlock = 0;
        ext.pnp_state = DevicePnpState::NotStarted;
        ext.power_state = DEVICE_POWER_STATE::PowerDeviceD0;

        KeInitializeSpinLock(&mut ext.spinlock);
        KeInitializeDpc(&mut ext.dpc, Some(audio_dpc_routine), ext_ptr as _);
        KeSetTargetProcessorDpc(&mut ext.dpc, ext.target_core);
        KeInitializeTimerEx(&mut ext.timer, 0);

        GLOBAL_EXTENSION.store(ext_ptr, Ordering::SeqCst);
        Ok(ext_ptr)
    }

    /// Map the locked physical pages directly into caller user-space process
    pub unsafe fn map_to_user(&mut self) -> Result<*mut core::ffi::c_void, NTSTATUS> {
        if !self.user_mapped_va.is_null() {
            return Ok(self.user_mapped_va);
        }

        let va = MmMapLockedPagesSpecifyCache(
            self.mdl,
            UserMode,
            MEMORY_CACHING_TYPE::MmCached,
            core::ptr::null_mut(),
            0,
            16, // NormalPagePriority
        );

        if va.is_null() {
            return Err(STATUS_INSUFFICIENT_RESOURCES);
        }

        self.user_mapped_va = va;
        Ok(va)
    }

    /// Unmap from user-space process
    pub unsafe fn unmap_from_user(&mut self) {
        if !self.user_mapped_va.is_null() {
            MmUnmapLockedPages(self.user_mapped_va, self.mdl);
            self.user_mapped_va = core::ptr::null_mut();
        }
    }

    /// Start streaming at requested quantum
    pub unsafe fn start_stream(&mut self) -> NTSTATUS {
        if self.is_streaming {
            return STATUS_SUCCESS;
        }

        if let Some(shared_mem) = self.shared_memory.as_mut() {
            shared_mem.status.fetch_or(status_flags::RUNNING, Ordering::SeqCst);
            shared_mem.telemetry.last_dpc_cycles.store(read_cpu_timestamp(), Ordering::Relaxed);
        }

        self.is_streaming = true;

        // Configure timer period:
        // For 96kHz and 16 samples per quantum: 16 / 96000 = 166.6 microseconds = 1666 (100ns units).
        let due_time: i64 = -1666;
        KeSetTimerEx(&mut self.timer, due_time, 1, &mut self.dpc);

        STATUS_SUCCESS
    }

    /// Stop streaming
    pub unsafe fn stop_stream(&mut self) -> NTSTATUS {
        if !self.is_streaming {
            return STATUS_SUCCESS;
        }

        KeCancelTimer(&mut self.timer);
        self.is_streaming = false;

        if let Some(shared_mem) = self.shared_memory.as_mut() {
            shared_mem.status.fetch_and(!status_flags::RUNNING, Ordering::SeqCst);
        }

        STATUS_SUCCESS
    }

    /// Set CPU core affinity to pin DPC execution to high-performance P-cores
    pub unsafe fn set_core_affinity(&mut self, core_number: u8) {
        self.target_core = core_number;
        KeSetTargetProcessorDpc(&mut self.dpc, core_number);
        if let Some(shared_mem) = self.shared_memory.as_mut() {
            shared_mem.status.fetch_or(status_flags::REALTIME_AFFINITY_SET, Ordering::Relaxed);
        }
    }

    /// Destroy and free all kernel resources
    pub unsafe fn destroy(&mut self) {
        self.stop_stream();
        self.unmap_from_user();

        if !self.user_event.is_null() {
            ObDereferenceObject(self.user_event);
            self.user_event = core::ptr::null_mut();
        }

        if !self.mdl.is_null() {
            IoFreeMdl(self.mdl);
            self.mdl = core::ptr::null_mut();
        }

        if !self.shared_memory.is_null() {
            ExFreePoolWithTag(self.shared_memory as _, AUDIO_POOL_TAG);
            self.shared_memory = core::ptr::null_mut();
        }
    }
}

/// Real-time DPC Routine executed at IRQL DISPATCH_LEVEL (Wait-free, zero lock).
/// Runs on the dedicated P-Core, drives hardware-like audio streaming.
extern "system" fn audio_dpc_routine(
    _dpc: *mut KDPC,
    context: *mut core::ffi::c_void,
    _arg1: *mut core::ffi::c_void,
    _arg2: *mut core::ffi::c_void,
) {
    let ext_ptr = context as *mut AudioDeviceExtension;
    if ext_ptr.is_null() {
        return;
    }

    unsafe {
        let ext = &mut *ext_ptr;
        if !ext.is_streaming || ext.shared_memory.is_null() {
            return;
        }

        let dpc_start_cycles = read_cpu_timestamp();
        let shared_mem = &mut *ext.shared_memory;
        let prev_cycles = shared_mem.telemetry.last_dpc_cycles.swap(dpc_start_cycles, Ordering::Relaxed);
        let delta_cycles = dpc_start_cycles.saturating_sub(prev_cycles);

        // Update telemetry
        let _ = shared_mem.telemetry.min_cycle_latency.fetch_min(delta_cycles, Ordering::Relaxed);
        let _ = shared_mem.telemetry.max_cycle_latency.fetch_max(delta_cycles, Ordering::Relaxed);

        // Quantum loop: process audio frames
        let quantum = shared_mem.config.quantum_frames as usize;
        let mut routed_frame_samples = [0.0f32; MAX_AUDIO_CHANNELS];

        for _ in 0..quantum {
            // Read output frame from ASIO / Native DAW (Playback)
            let out_frame = shared_mem.playback_ring.pop().unwrap_or(AudioFrame::EMPTY);

            // Process through 64x64 Crossbar Routing Matrix
            shared_mem.routing_matrix.process_frame(&out_frame.samples, &mut routed_frame_samples);

            // Update real-time peak meters on active channels
            for ch in 0..MAX_AUDIO_CHANNELS {
                let abs_sample = routed_frame_samples[ch].abs();
                if abs_sample >= 1.0 {
                    shared_mem.output_meters.clip_flags |= 1 << ch;
                }
                // Exponential decay peak ballistics
                let curr_peak = shared_mem.output_meters.peak_linear[ch];
                if abs_sample > curr_peak {
                    shared_mem.output_meters.peak_linear[ch] = abs_sample;
                } else {
                    shared_mem.output_meters.peak_linear[ch] = curr_peak * 0.995;
                }
            }

            // Loopback / Hardware Line-In Capture Frame
            let mut in_frame = AudioFrame::EMPTY;
            in_frame.samples = routed_frame_samples;
            in_frame.timestamp_cycles = dpc_start_cycles;
            in_frame.frame_id = shared_mem.telemetry.total_frames.load(Ordering::Relaxed);

            if !shared_mem.capture_ring.push(in_frame) {
                shared_mem.telemetry.input_xruns.fetch_add(1, Ordering::Relaxed);
            }

            shared_mem.telemetry.total_frames.fetch_add(1, Ordering::Relaxed);
        }

        // Measure DPC execution duration
        let dpc_end_cycles = read_cpu_timestamp();
        shared_mem.telemetry.dpc_duration_cycles.store(
            dpc_end_cycles.saturating_sub(dpc_start_cycles),
            Ordering::Relaxed,
        );

        // Notify user-mode ASIO buffer switch via synchronization counter
        shared_mem.kernel_sync_counter.fetch_add(1, Ordering::Release);

        // Signal user-mode Win32 Event if registered
        if !ext.user_event.is_null() {
            KeSetEvent(ext.user_event, IO_NO_INCREMENT, 0);
        }
    }
}
