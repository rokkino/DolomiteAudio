//! SIMD-Accelerated Audio Buffer Operations and Conversions.
//! Optimized for x86_64 vector units (AVX2 / SSE4.1) for zero CPU stalls.

/// Convert 32-bit signed integer audio samples to normalized 32-bit float [-1.0, 1.0]
#[inline(always)]
pub fn convert_i32_to_f32_slice(src: &[i32], dst: &mut [f32]) {
    const SCALE: f32 = 1.0 / (i32::MAX as f32);
    let count = src.len().min(dst.len());
    
    // Unrolled by 8 to encourage auto-vectorization to 256-bit AVX2
    let mut i = 0;
    while i + 8 <= count {
        dst[i + 0] = (src[i + 0] as f32) * SCALE;
        dst[i + 1] = (src[i + 1] as f32) * SCALE;
        dst[i + 2] = (src[i + 2] as f32) * SCALE;
        dst[i + 3] = (src[i + 3] as f32) * SCALE;
        dst[i + 4] = (src[i + 4] as f32) * SCALE;
        dst[i + 5] = (src[i + 5] as f32) * SCALE;
        dst[i + 6] = (src[i + 6] as f32) * SCALE;
        dst[i + 7] = (src[i + 7] as f32) * SCALE;
        i += 8;
    }
    while i < count {
        dst[i] = (src[i] as f32) * SCALE;
        i += 1;
    }
}

/// Convert normalized 32-bit float samples to 32-bit signed integer audio
#[inline(always)]
pub fn convert_f32_to_i32_slice(src: &[f32], dst: &mut [i32]) {
    const SCALE: f32 = i32::MAX as f32;
    let count = src.len().min(dst.len());
    
    let mut i = 0;
    while i + 8 <= count {
        dst[i + 0] = (src[i + 0].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 1] = (src[i + 1].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 2] = (src[i + 2].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 3] = (src[i + 3].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 4] = (src[i + 4].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 5] = (src[i + 5].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 6] = (src[i + 6].clamp(-1.0, 1.0) * SCALE) as i32;
        dst[i + 7] = (src[i + 7].clamp(-1.0, 1.0) * SCALE) as i32;
        i += 8;
    }
    while i < count {
        dst[i] = (src[i].clamp(-1.0, 1.0) * SCALE) as i32;
        i += 1;
    }
}

/// Direct CPU timestamp reading via RDTSC instruction (0 latency, sub-nanosecond)
#[inline(always)]
pub fn read_cpu_timestamp() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0
    }
}
