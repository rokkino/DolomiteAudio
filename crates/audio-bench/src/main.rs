//! High-Precision Audio Benchmark Harness.
//! Measures Round-Trip Latency (RTL), CPU cycles per sample, and multithreaded lockless throughput.

use std::sync::Arc;
use std::thread;
use std::time::Instant;
use audio_core::{
    SpscRingBuffer, AudioFrame, read_cpu_timestamp,
    convert_f32_to_i32_slice, convert_i32_to_f32_slice,
    BiquadFilter, BiquadCoefficients, FilterType,
    BrickwallLimiter, TpdfDither, AudioRoutingMatrix,
    MAX_AUDIO_CHANNELS,
};

fn main() {
    println!("===============================================================");
    println!("    ZEROLATENCY KERNEL AUDIO: PRODUCTION BENCHMARK SUITE       ");
    println!("===============================================================\n");

    test_single_sample_latency();
    test_64channel_multithreaded_throughput();
    test_dsp_pipeline_latency();
    test_simd_conversion_performance();
}

/// Benchmark 1: Measure latency of 1-sample bufferless transfers
fn test_single_sample_latency() {
    println!("[TEST 1] Single-Sample 'Bufferless' Lockless SPSC Latency...");
    let mut ring = SpscRingBuffer::heap_boxed();

    const ITERATIONS: u64 = 1_000_000;
    let mut total_cycles: u64 = 0;
    let mut min_cycles: u64 = u64::MAX;
    let mut max_cycles: u64 = 0;

    let start_wall = Instant::now();

    for i in 0..ITERATIONS {
        let t0 = read_cpu_timestamp();

        let mut frame = AudioFrame::EMPTY;
        frame.samples[0] = 0.707;
        frame.samples[1] = -0.707;
        frame.frame_id = i;

        // Write
        let pushed = ring.push(frame);
        // Read
        let popped = ring.pop();

        let t1 = read_cpu_timestamp();
        let delta = t1.saturating_sub(t0);

        if pushed && popped.is_some() {
            total_cycles += delta;
            if delta < min_cycles && delta > 0 { min_cycles = delta; }
            if delta > max_cycles { max_cycles = delta; }
        }
    }

    let elapsed = start_wall.elapsed();
    let avg_cycles = total_cycles as f64 / ITERATIONS as f64;
    let approx_ns = avg_cycles / 3.8;

    println!("  Iterations      : {} frames", ITERATIONS);
    println!("  Total Wall Time : {:?}", elapsed);
    println!("  Min Latency     : {} CPU cycles (~{:.2} ns)", min_cycles, min_cycles as f64 / 3.8);
    println!("  Avg Latency     : {:.1} CPU cycles (~{:.2} ns)", avg_cycles, approx_ns);
    println!("  Max Latency     : {} CPU cycles (jitter peak)", max_cycles);
    println!("  Throughput      : {:.2} Million samples/sec\n", (ITERATIONS as f64 / elapsed.as_secs_f64()) / 1_000_000.0);
}

/// Benchmark 2: Concurrent 64-Channel Producer (Kernel) -> Consumer (DAW)
fn test_64channel_multithreaded_throughput() {
    println!("[TEST 2] 64-Channel Multithreaded Zero-Copy Throughput (64-byte Cache-Isolated)...");

    let ring: Arc<SpscRingBuffer> = Arc::from(SpscRingBuffer::heap_boxed());
    const TOTAL_FRAMES: u64 = 3_000_000;
    let ring_producer = ring.clone();
    let ring_consumer = ring.clone();

    let start_time = Instant::now();

    // Producer Thread (Kernel DMA Simulator)
    let prod_handle = thread::spawn(move || {
        for i in 0..TOTAL_FRAMES {
            let mut frame = AudioFrame::EMPTY;
            frame.frame_id = i;
            frame.timestamp_cycles = read_cpu_timestamp();
            // Fill 64 channels
            for ch in 0..MAX_AUDIO_CHANNELS {
                frame.samples[ch] = (ch as f32) * 0.01;
            }

            while !unsafe { (*(ring_producer.as_ref() as *const _ as *mut SpscRingBuffer)).push(frame) } {
                core::hint::spin_loop();
            }
        }
    });

    // Consumer Thread (DAW Simulator)
    let cons_handle = thread::spawn(move || {
        let mut received = 0u64;
        let mut total_latency_cycles = 0u64;

        while received < TOTAL_FRAMES {
            if let Some(frame) = unsafe { (*(ring_consumer.as_ref() as *const _ as *mut SpscRingBuffer)).pop() } {
                let now = read_cpu_timestamp();
                total_latency_cycles += now.saturating_sub(frame.timestamp_cycles);
                received += 1;
            } else {
                core::hint::spin_loop();
            }
        }

        total_latency_cycles
    });

    prod_handle.join().unwrap();
    let total_latency_cycles = cons_handle.join().unwrap();
    let elapsed = start_time.elapsed();

    let avg_roundtrip_ns = (total_latency_cycles as f64 / TOTAL_FRAMES as f64) / 3.8;
    let samples_per_sec = (TOTAL_FRAMES * (MAX_AUDIO_CHANNELS as u64)) as f64 / elapsed.as_secs_f64();

    println!("  Frames Streamed : {} frames (64 channels per frame)", TOTAL_FRAMES);
    println!("  Elapsed Time    : {:?}", elapsed);
    println!("  Avg Inter-Core  : {:.2} ns round-trip handover", avg_roundtrip_ns);
    println!("  Throughput      : {:.2} Million 64-channel frames/sec", (TOTAL_FRAMES as f64 / elapsed.as_secs_f64()) / 1_000_000.0);
    println!("  Audio Bandwidth : {:.2} GigaSamples/second\n", samples_per_sec / 1_000_000_000.0);
}

/// Benchmark 3: Real-Time Kernel DSP Pipeline (EQ + Matrix + Limiter + Dither)
fn test_dsp_pipeline_latency() {
    println!("[TEST 3] Real-Time DSP Pipeline Latency (Biquad EQ + 64x64 Matrix + True-Peak Limiter + TPDF Dither)...");

    let coeffs = BiquadCoefficients::calculate(FilterType::PeakingEQ { gain_db: 3.0, q: 1.0 }, 1000.0, 96000.0, 1.0);
    let mut eq = BiquadFilter::new(coeffs);
    let mut limiter = BrickwallLimiter::new(96000.0, -0.1, 50.0, 0); // 0 lookahead for bufferless mode
    let mut dither = TpdfDither::new(24);
    let matrix = AudioRoutingMatrix::new();

    const SAMPLES: usize = 500_000;
    let mut frame_in = [0.5f32; MAX_AUDIO_CHANNELS];
    let mut frame_out = [0.0f32; MAX_AUDIO_CHANNELS];

    let t0 = Instant::now();

    for _ in 0..SAMPLES {
        // 1. Parametric EQ
        let eq_sample = eq.process_sample(0.707);
        frame_in[0] = eq_sample;

        // 2. 64x64 Crossbar Matrix Routing
        matrix.process_frame(&frame_in, &mut frame_out);

        // 3. True-Peak Limiter
        let lim_sample = limiter.process_sample(frame_out[0]);

        // 4. TPDF Dither to 24-bit DAC
        let _quantized = dither.quantize(lim_sample);
    }

    let elapsed = t0.elapsed();
    let ns_per_sample = (elapsed.as_nanos() as f64) / (SAMPLES as f64);

    println!("  Processed       : {} samples through full mastering DSP chain", SAMPLES);
    println!("  Elapsed Time    : {:?}", elapsed);
    println!("  Processing Time : {:.2} nanoseconds per sample", ns_per_sample);
    println!("  Max DSP Rate    : {:.2} Million samples/sec\n", (SAMPLES as f64 / elapsed.as_secs_f64()) / 1_000_000.0);
}

/// Benchmark 4: SIMD Vectorized Conversion
fn test_simd_conversion_performance() {
    println!("[TEST 4] SIMD Vectorized Sample Format Conversion (AVX2 / Unrolled)...");

    const SAMPLES: usize = 1_000_000;
    let float_samples: Vec<f32> = (0..SAMPLES).map(|i| ((i % 1000) as f32 / 1000.0) * 2.0 - 1.0).collect();
    let mut int_samples = vec![0i32; SAMPLES];
    let mut back_to_float = vec![0.0f32; SAMPLES];

    let t0 = Instant::now();
    convert_f32_to_i32_slice(&float_samples, &mut int_samples);
    let t_f2i = t0.elapsed();

    let t1 = Instant::now();
    convert_i32_to_f32_slice(&int_samples, &mut back_to_float);
    let t_i2f = t1.elapsed();

    println!("  F32 -> I32 (1M samples): {:?}", t_f2i);
    println!("  I32 -> F32 (1M samples): {:?}", t_i2f);
    println!("  Rate                   : {:.2} GigaSamples/second\n",
        (SAMPLES as f64 / t_f2i.as_secs_f64()) / 1_000_000_000.0
    );
}
