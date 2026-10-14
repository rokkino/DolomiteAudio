//! Windows Kernel IOCTL (I/O Control) definitions for the Zero-Latency Audio Driver.

pub const FILE_DEVICE_ZERO_AUDIO: u32 = 0x8A51; // Custom device identifier 'AS'

pub const METHOD_BUFFERED: u32 = 0;
pub const METHOD_IN_DIRECT: u32 = 1;
pub const METHOD_OUT_DIRECT: u32 = 2;
pub const METHOD_NEITHER: u32 = 3;

pub const FILE_ANY_ACCESS: u32 = 0;
pub const FILE_READ_ACCESS: u32 = 0x0001;
pub const FILE_WRITE_ACCESS: u32 = 0x0002;

/// Standard Windows CTL_CODE macro equivalent in Rust const fn
pub const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

/// IOCTL: Map shared non-paged audio buffer into calling user process address space
pub const IOCTL_AUDIO_MAP_SHARED_MEMORY: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x801,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Unmap shared audio memory
pub const IOCTL_AUDIO_UNMAP_SHARED_MEMORY: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x802,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Start real-time audio streaming and DPC clock
pub const IOCTL_AUDIO_START_STREAMING: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x803,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Stop real-time audio streaming
pub const IOCTL_AUDIO_STOP_STREAMING: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x804,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Set CPU Core Affinity Mask for Audio DPC thread
pub const IOCTL_AUDIO_SET_CORE_AFFINITY: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x805,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Register user-mode event handle for hardware interrupt signaling
pub const IOCTL_AUDIO_REGISTER_EVENT: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x806,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Configure routing matrix crosspoint
pub const IOCTL_AUDIO_SET_ROUTING: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x807,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Set hardware sample rate
pub const IOCTL_AUDIO_SET_SAMPLE_RATE: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x808,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Set buffer quantum size (e.g. 1, 8, 16, 32, 64)
pub const IOCTL_AUDIO_SET_QUANTUM: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x809,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// IOCTL: Reset xrun counters and telemetry
pub const IOCTL_AUDIO_RESET_TELEMETRY: u32 = ctl_code(
    FILE_DEVICE_ZERO_AUDIO,
    0x80A,
    METHOD_BUFFERED,
    FILE_READ_ACCESS | FILE_WRITE_ACCESS,
);

/// Response payload from IOCTL_AUDIO_MAP_SHARED_MEMORY
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MapMemoryResponse {
    pub user_address: u64,
    pub size_bytes: u64,
    pub version: u32,
    pub _reserved: u32,
}

/// Request payload to set CPU Affinity
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SetAffinityRequest {
    pub affinity_mask: u64,
}

/// Request payload to register synchronization Event
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct RegisterEventRequest {
    pub event_handle: u64,
}

/// Request payload to configure routing matrix crosspoint
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SetRoutingRequest {
    pub input_channel: u32,
    pub output_channel: u32,
    pub gain: f32,
    pub phase_inverted: u32, // 1 or 0
    pub is_muted: u32,       // 1 or 0
}

/// Request payload to configure sample rate
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SetSampleRateRequest {
    pub sample_rate: u32,
}

/// Request payload to configure buffer quantum
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SetQuantumRequest {
    pub quantum_frames: u32,
}
