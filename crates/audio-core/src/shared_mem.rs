//! Shared Memory Layout between Kernel Driver (.sys) and User-Mode DAWs / Clients.
//! Mapped directly into calling processes via MmMapLockedPagesSpecifyCache.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::ring_buffer::{SpscRingBuffer, CachePadded, MAX_AUDIO_CHANNELS};
use crate::dsp::matrix::AudioRoutingMatrix;

pub const ZERO_AUDIO_MAGIC: u32 = 0x5A4C4155; // 'ZLAU'
pub const PROTOCOL_VERSION: u32 = 2;

/// Driver status flags
pub mod status_flags {
    pub const STOPPED: u32 = 0;
    pub const RUNNING: u32 = 1 << 0;
    pub const XRUN_INPUT: u32 = 1 << 1;
    pub const XRUN_OUTPUT: u32 = 1 << 2;
    pub const REALTIME_AFFINITY_SET: u32 = 1 << 3;
    pub const CLOCK_LOCKED: u32 = 1 << 4;
    pub const MULTI_CLIENT_ACTIVE: u32 = 1 << 5;
    pub const DSP_LIMITER_ACTIVE: u32 = 1 << 6;
}

/// Supported studio clock synchronization sources
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ClockSource {
    InternalPcore = 0,
    ExternalWordClock = 1,
    PtpIeee1588 = 2,
    SpdifAdatOptical = 3,
}

/// Real-time Kernel/User Telemetry Header for Sub-Nanosecond Jitter Tracking
#[repr(C, align(64))]
pub struct DriverTelemetry {
    /// Total audio frames processed since streaming started
    pub total_frames: AtomicU64,
    /// Count of input underruns/overruns
    pub input_xruns: AtomicU32,
    /// Count of output underruns/overruns
    pub output_xruns: AtomicU32,
    /// Minimum round-trip CPU cycles measured
    pub min_cycle_latency: AtomicU64,
    /// Maximum round-trip CPU cycles measured
    pub max_cycle_latency: AtomicU64,
    /// Last DPC execution timestamp (RDTSC)
    pub last_dpc_cycles: AtomicU64,
    /// Estimated CPU frequency in Hz (calibrated at boot)
    pub cpu_freq_hz: AtomicU64,
    /// Real-time DPC execution duration in CPU cycles
    pub dpc_duration_cycles: AtomicU64,
}

impl DriverTelemetry {
    pub const fn new() -> Self {
        Self {
            total_frames: AtomicU64::new(0),
            input_xruns: AtomicU32::new(0),
            output_xruns: AtomicU32::new(0),
            min_cycle_latency: AtomicU64::new(u64::MAX),
            max_cycle_latency: AtomicU64::new(0),
            last_dpc_cycles: AtomicU64::new(0),
            cpu_freq_hz: AtomicU64::new(0),
            dpc_duration_cycles: AtomicU64::new(0),
        }
    }
}

/// Audio hardware streaming configuration
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct AudioStreamConfig {
    pub sample_rate: u32,       // e.g. 44100, 48000, 96000, 192000, 384000
    pub input_channels: u32,    // Up to 64 channels
    pub output_channels: u32,   // Up to 64 channels
    pub quantum_frames: u32,    // Buffer block size: 1, 8, 16, 32, 64, 128, 256
    pub cpu_affinity_mask: u64, // Mask specifying target P-Core for audio DPCs
    pub clock_source: u32,      // ClockSource enum
    pub enable_limiter: u32,    // 1 = enabled, 0 = disabled
    pub enable_dither: u32,     // 1 = enabled, 0 = disabled
}

impl AudioStreamConfig {
    pub const DEFAULT: Self = Self {
        sample_rate: 96000,
        input_channels: 64,
        output_channels: 64,
        quantum_frames: 16, // Ultra-low latency: ~0.16ms at 96kHz!
        cpu_affinity_mask: 0x04, // Default Core 2 (P-Core)
        clock_source: ClockSource::InternalPcore as u32,
        enable_limiter: 1,
        enable_dither: 1,
    };
}

/// Per-Channel Real-time Metering State in Shared Memory
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct ChannelMeterState {
    pub peak_linear: [f32; MAX_AUDIO_CHANNELS],
    pub rms_linear: [f32; MAX_AUDIO_CHANNELS],
    pub clip_flags: u64, // Bitmask: bit N is set if channel N clipped
}

impl ChannelMeterState {
    pub const fn new() -> Self {
        Self {
            peak_linear: [0.0; MAX_AUDIO_CHANNELS],
            rms_linear: [0.0; MAX_AUDIO_CHANNELS],
            clip_flags: 0,
        }
    }
}

/// The entire physical memory structure allocated in Non-Paged Pool
/// and mapped into User-Mode DAW / Client memory with READ/WRITE permissions.
#[repr(C, align(4096))] // 4KB Page Aligned for direct physical memory mapping
pub struct SharedAudioMemory {
    /// Magic signature to verify correct memory mapping
    pub magic: u32,
    /// Protocol version
    pub version: u32,
    /// Current driver status flags
    pub status: CachePadded<AtomicU32>,

    /// Stream configuration
    pub config: AudioStreamConfig,

    /// Ultra-fast telemetry
    pub telemetry: DriverTelemetry,

    /// Real-time metering for inputs and outputs
    pub input_meters: ChannelMeterState,
    pub output_meters: ChannelMeterState,

    /// 64x64 Routing Matrix
    pub routing_matrix: AudioRoutingMatrix,

    /// Synchronization counter incremented by Kernel on each buffer quantum tick
    pub kernel_sync_counter: CachePadded<AtomicU64>,

    /// Capture Ring Buffer: Kernel -> User (ASIO / Native Input)
    pub capture_ring: SpscRingBuffer,

    /// Playback Ring Buffer: User (ASIO / Native Output) -> Kernel
    pub playback_ring: SpscRingBuffer,
}

impl SharedAudioMemory {
    /// Initialize fresh shared audio memory block
    pub fn init(&mut self, config: AudioStreamConfig) {
        self.magic = ZERO_AUDIO_MAGIC;
        self.version = PROTOCOL_VERSION;
        self.status.store(status_flags::STOPPED, Ordering::SeqCst);
        self.config = config;
        self.telemetry = DriverTelemetry::new();
        self.input_meters = ChannelMeterState::new();
        self.output_meters = ChannelMeterState::new();
        self.routing_matrix = AudioRoutingMatrix::new();
        self.kernel_sync_counter.store(0, Ordering::SeqCst);
        self.capture_ring.reset();
        self.playback_ring.reset();
    }

    /// Check if magic header matches
    pub fn is_valid(&self) -> bool {
        self.magic == ZERO_AUDIO_MAGIC && self.version == PROTOCOL_VERSION
    }

    #[cfg(feature = "std")]
    pub fn heap_boxed(config: AudioStreamConfig) -> std::boxed::Box<Self> {
        unsafe {
            let layout = std::alloc::Layout::new::<Self>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut Self;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            (*ptr).init(config);
            std::boxed::Box::from_raw(ptr)
        }
    }
}
