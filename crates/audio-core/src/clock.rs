//! High-Precision Audio Clock Pacer and DPC Jitter Guard.
//! Provides deterministic sub-microsecond timing for real-time audio pumping on Windows.

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn QueryPerformanceCounter(lpPerformanceCount: *mut i64) -> i32;
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
}

#[derive(Copy, Clone, Debug)]
pub struct ClockTickInfo {
    pub frame_index: u64,
    pub target_qpc: i64,
    pub actual_qpc: i64,
    pub jitter_nanos: i64,
    pub is_dpc_spike: bool,
    pub spike_duration_nanos: u64,
}

/// High-Precision Deterministic Clock Pacer for 16/32-sample quantums
pub struct AudioClockPacer {
    pub sample_rate: u32,
    pub quantum_frames: u32,
    qpc_frequency: i64,
    qpc_start: i64,
    tick_counter: u64,
    qpc_per_quantum: f64,
    pub total_dpc_spikes: AtomicU32,
    pub max_dpc_spike_nanos: AtomicU64,
}

impl AudioClockPacer {
    pub fn new(sample_rate: u32, quantum_frames: u32) -> Self {
        let mut freq = 10_000_000i64; // Default fallback: 10 MHz
        let mut start = 0i64;

        #[cfg(target_os = "windows")]
        unsafe {
            let _ = QueryPerformanceFrequency(&mut freq);
            let _ = QueryPerformanceCounter(&mut start);
        }

        let qpc_per_quantum = (freq as f64 * quantum_frames as f64) / (sample_rate as f64);

        Self {
            sample_rate,
            quantum_frames,
            qpc_frequency: freq.max(1),
            qpc_start: start,
            tick_counter: 0,
            qpc_per_quantum,
            total_dpc_spikes: AtomicU32::new(0),
            max_dpc_spike_nanos: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    pub fn now_qpc() -> i64 {
        let mut now = 0i64;
        #[cfg(target_os = "windows")]
        unsafe {
            let _ = QueryPerformanceCounter(&mut now);
        }
        now
    }

    #[inline(always)]
    pub fn qpc_to_nanos(&self, qpc_ticks: i64) -> i64 {
        (qpc_ticks * 1_000_000_000) / self.qpc_frequency
    }

    /// Wait until the next quantum boundary with hybrid yield/spin pacing.
    /// Returns ClockTickInfo with measured jitter and DPC spike detection.
    pub fn wait_next_tick(&mut self) -> ClockTickInfo {
        self.tick_counter += 1;
        let target_qpc = self.qpc_start + (self.tick_counter as f64 * self.qpc_per_quantum) as i64;

        // Threshold for DPC spike detection: 100 microseconds
        let dpc_spike_qpc_threshold = (self.qpc_frequency * 100) / 1_000_000;

        loop {
            let now = Self::now_qpc();
            let delta = target_qpc - now;

            if delta <= 0 {
                // Target time reached or passed
                let overdue = -delta;
                let is_dpc_spike = overdue > dpc_spike_qpc_threshold;
                let spike_duration_nanos = if is_dpc_spike {
                    let nanos = ((overdue * 1_000_000_000) / self.qpc_frequency) as u64;
                    self.total_dpc_spikes.fetch_add(1, Ordering::Relaxed);
                    let mut prev_max = self.max_dpc_spike_nanos.load(Ordering::Relaxed);
                    while nanos > prev_max {
                        match self.max_dpc_spike_nanos.compare_exchange_weak(
                            prev_max, nanos, Ordering::Relaxed, Ordering::Relaxed
                        ) {
                            Ok(_) => break,
                            Err(actual) => prev_max = actual,
                        }
                    }
                    nanos
                } else {
                    0
                };

                let jitter_nanos = ((-delta) * 1_000_000_000) / self.qpc_frequency;

                return ClockTickInfo {
                    frame_index: self.tick_counter * (self.quantum_frames as u64),
                    target_qpc,
                    actual_qpc: now,
                    jitter_nanos,
                    is_dpc_spike,
                    spike_duration_nanos,
                };
            }

            // High-resolution pacing:
            let delta_nanos = (delta * 1_000_000_000) / self.qpc_frequency;

            #[cfg(feature = "std")]
            {
                if delta_nanos > 2_000_000 {
                    // Sleep if more than 2ms remaining
                    std::thread::sleep(std::time::Duration::from_millis(1));
                } else if delta_nanos > 100_000 {
                    // Yield to other threads if more than 100us remaining
                    std::thread::yield_now();
                } else {
                    // Micro-spin for the last few microseconds for sub-microsecond accuracy
                    core::hint::spin_loop();
                }
            }

            #[cfg(not(feature = "std"))]
            {
                core::hint::spin_loop();
            }
        }
    }
}

/// Returns the current system high-resolution monotonic time in nanoseconds (based on Windows QPC).
#[inline(always)]
pub fn current_time_nanos() -> i64 {
    let mut freq = 10_000_000i64;
    let mut now = 0i64;
    #[cfg(target_os = "windows")]
    unsafe {
        let _ = QueryPerformanceFrequency(&mut freq);
        let _ = QueryPerformanceCounter(&mut now);
    }
    if freq <= 0 {
        freq = 10_000_000;
    }
    ((now as u128 * 1_000_000_000u128) / (freq as u128)) as i64
}
