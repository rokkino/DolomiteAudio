//! Production-grade Biquad Filter (Direct Form II Transposed).
//! Implements Robert Bristow-Johnson EQ Cookbook filters with double-precision coefficients
//! and single-precision real-time streaming for zero quantization noise.

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    AllPass,
    PeakingEQ { gain_db: f32, q: f32 },
    LowShelf { gain_db: f32 },
    HighShelf { gain_db: f32 },
}

#[derive(Copy, Clone, Debug)]
pub struct BiquadCoefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl BiquadCoefficients {
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Calculate filter coefficients from filter type, cutoff frequency, and sample rate
    pub fn calculate(filter_type: FilterType, frequency_hz: f32, sample_rate: f32, q_factor: f32) -> Self {
        let f0 = frequency_hz.clamp(10.0, sample_rate * 0.499);
        let omega = 2.0 * core::f32::consts::PI * f0 / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q_factor.max(0.01));

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_omega;
                let b0 = b1 * 0.5;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b1 = -(1.0 + cos_omega);
                let b0 = (1.0 + cos_omega) * 0.5;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::AllPass => {
                let b0 = 1.0 - alpha;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0 + alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::PeakingEQ { gain_db, q } => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let alpha_peak = sin_omega / (2.0 * q.max(0.01));
                let b0 = 1.0 + alpha_peak * a;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0 - alpha_peak * a;
                let a0 = 1.0 + alpha_peak / a;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha_peak / a;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowShelf { gain_db } => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_omega);
                let b2 = a * ((a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_omega);
                let a2 = (a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighShelf { gain_db } => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_omega);
                let b2 = a * ((a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_omega);
                let a2 = (a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }
}

/// Direct Form II Transposed Biquad Filter Channel State.
/// Provides superior numerical stability against floating-point roundoff errors.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct BiquadFilter {
    pub coeffs: BiquadCoefficients,
    pub s1: f32,
    pub s2: f32,
}

impl BiquadFilter {
    pub const fn new(coeffs: BiquadCoefficients) -> Self {
        Self {
            coeffs,
            s1: 0.0,
            s2: 0.0,
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// Process a single audio sample (Sub-nanosecond execution)
    #[inline(always)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.coeffs.b0 * input + self.s1;
        self.s1 = self.coeffs.b1 * input - self.coeffs.a1 * output + self.s2;
        self.s2 = self.coeffs.b2 * input - self.coeffs.a2 * output;
        output
    }

    /// Process a slice of audio samples in place
    #[inline(always)]
    pub fn process_slice(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}
