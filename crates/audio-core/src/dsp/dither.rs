//! TPDF (Triangular Probability Density Function) Dither with High-Pass Noise Shaping.
//! Eliminates quantization distortion when converting 32-bit float audio to 24-bit or 16-bit DAC outputs.

#[repr(C)]
#[derive(Clone, Debug)]
pub struct TpdfDither {
    /// Linear congruential generator state for high-speed uniform pseudo-random numbers
    pub lcg_state: u64,
    /// Noise shaping error filter history (Lipshitz/Vanderkooy filter)
    pub error_history: [f32; 2],
    /// Target bit depth (16, 24, or 32)
    pub bit_depth: u32,
    /// Precomputed scale factor
    pub scale: f32,
    pub inv_scale: f32,
}

impl TpdfDither {
    pub fn new(bit_depth: u32) -> Self {
        let max_val = match bit_depth {
            16 => 32767.0,
            24 => 8388607.0,
            _ => 2147483647.0,
        };

        Self {
            lcg_state: 0x9E3779B97F4A7C15, // Golden ratio constant
            error_history: [0.0, 0.0],
            bit_depth,
            scale: max_val,
            inv_scale: 1.0 / max_val,
        }
    }

    /// Fast uniform random float between -1.0 and 1.0 using 64-bit LCG
    #[inline(always)]
    fn next_uniform(&mut self) -> f32 {
        self.lcg_state = self.lcg_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let val = (self.lcg_state >> 32) as u32;
        (val as f32 / (u32::MAX as f32)) * 2.0 - 1.0
    }

    /// Generate Triangular PDF dither by summing two independent uniform random variables
    #[inline(always)]
    fn next_tpdf(&mut self) -> f32 {
        let r1 = self.next_uniform();
        let r2 = self.next_uniform();
        (r1 + r2) * 0.5
    }

    /// Quantize a single 32-bit float [-1.0, 1.0] to a 32-bit signed integer with TPDF noise shaping
    #[inline(always)]
    pub fn quantize(&mut self, input: f32) -> i32 {
        // High-pass noise shaping filter: E(z) = 1.0 * e[n-1] - 0.5 * e[n-2]
        // Shifts quantization noise energy upward towards Nyquist frequency
        let shaped_error = 1.0 * self.error_history[0] - 0.5 * self.error_history[1];

        // Scale to integer range and add dither + shaped error
        let scaled = input * self.scale - shaped_error;
        let dither_noise = self.next_tpdf();
        let target = scaled + dither_noise;

        // Truncate to nearest integer
        let quantized = target.round().clamp(-self.scale, self.scale) as i32;

        // Compute quantization error for feedback
        let error = (quantized as f32) - scaled;
        self.error_history[1] = self.error_history[0];
        self.error_history[0] = error;

        quantized
    }
}
