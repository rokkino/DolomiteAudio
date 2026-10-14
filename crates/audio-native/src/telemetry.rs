//! Real-Time Audio Telemetry and Performance Monitor.

use std::sync::atomic::Ordering;
use audio_core::SharedAudioMemory;

#[derive(Clone, Debug, Default)]
pub struct TelemetrySnapshot {
    pub total_frames: u64,
    pub input_xruns: u32,
    pub output_xruns: u32,
    pub min_latency_nanos: f64,
    pub max_latency_nanos: f64,
    pub dpc_duration_nanos: f64,
    pub sample_rate: u32,
    pub quantum_frames: u32,
    pub is_running: bool,
}

impl TelemetrySnapshot {
    pub fn capture(shared_mem: &SharedAudioMemory) -> Self {
        let cpu_freq = 3.8e9; // Estimated 3.8 GHz clock
        let cycles_to_ns = |c: u64| (c as f64 / cpu_freq) * 1e9;

        let total = shared_mem.telemetry.total_frames.load(Ordering::Relaxed);
        let in_xruns = shared_mem.telemetry.input_xruns.load(Ordering::Relaxed);
        let out_xruns = shared_mem.telemetry.output_xruns.load(Ordering::Relaxed);
        let min_c = shared_mem.telemetry.min_cycle_latency.load(Ordering::Relaxed);
        let max_c = shared_mem.telemetry.max_cycle_latency.load(Ordering::Relaxed);
        let dpc_c = shared_mem.telemetry.dpc_duration_cycles.load(Ordering::Relaxed);
        let status = shared_mem.status.load(Ordering::Relaxed);

        Self {
            total_frames: total,
            input_xruns: in_xruns,
            output_xruns: out_xruns,
            min_latency_nanos: if min_c == u64::MAX { 0.0 } else { cycles_to_ns(min_c) },
            max_latency_nanos: cycles_to_ns(max_c),
            dpc_duration_nanos: cycles_to_ns(dpc_c),
            sample_rate: shared_mem.config.sample_rate,
            quantum_frames: shared_mem.config.quantum_frames,
            is_running: (status & audio_core::status_flags::RUNNING) != 0,
        }
    }
}
