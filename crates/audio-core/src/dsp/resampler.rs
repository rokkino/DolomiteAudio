//! Bandlimited Sinc Interpolation Resampler.
//! Provides real-time Sample Rate Conversion (SRC) between arbitrary sample rates
//! (e.g. 44.1k, 48k, 88.2k, 96k, 192k, 384k) without heap allocations in the audio path.

pub const RESAMPLER_TAPS: usize = 16;
pub const HISTORY_SIZE: usize = 32;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SincResampler {
    pub input_rate: f64,
    pub output_rate: f64,
    pub ratio: f64, // input_rate / output_rate
    pub fractional_pos: f64,
    pub history: [f32; HISTORY_SIZE],
    pub history_idx: usize,
}

impl SincResampler {
    pub fn new(input_rate: f64, output_rate: f64) -> Self {
        let ratio = input_rate / output_rate;
        Self {
            input_rate,
            output_rate,
            ratio,
            fractional_pos: 0.0,
            history: [0.0; HISTORY_SIZE],
            history_idx: 0,
        }
    }

    pub fn reset(&mut self) {
        self.fractional_pos = 0.0;
        self.history.fill(0.0);
        self.history_idx = 0;
    }

    pub fn set_rates(&mut self, input_rate: f64, output_rate: f64) {
        self.input_rate = input_rate;
        self.output_rate = output_rate;
        self.ratio = input_rate / output_rate;
        self.reset();
    }

    /// Push an input sample into the circular history buffer
    #[inline(always)]
    pub fn push_input(&mut self, sample: f32) {
        self.history[self.history_idx] = sample;
        self.history_idx = (self.history_idx + 1) % HISTORY_SIZE;
    }

    /// Normalized sinc function: sinc(x) = sin(pi * x) / (pi * x)
    #[inline(always)]
    fn sinc(x: f32) -> f32 {
        if x.abs() < 1e-6 {
            1.0
        } else {
            let px = core::f32::consts::PI * x;
            px.sin() / px
        }
    }

    /// Blackman-Harris 4-term window function for sidelobe suppression > 92 dB
    #[inline(always)]
    fn window(x: f32, n: f32) -> f32 {
        const A0: f32 = 0.35875;
        const A1: f32 = 0.48829;
        const A2: f32 = 0.14128;
        const A3: f32 = 0.01168;

        let frac = (x + n * 0.5) / n;
        if !(0.0..=1.0).contains(&frac) {
            return 0.0;
        }

        let theta = 2.0 * core::f32::consts::PI * frac;
        A0 - A1 * theta.cos() + A2 * (2.0 * theta).cos() - A3 * (3.0 * theta).cos()
    }

    /// Synthesize an output sample using polyphase sinc interpolation
    #[inline(always)]
    pub fn interpolate_sample(&self, frac: f32) -> f32 {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;
        let taps = RESAMPLER_TAPS as i32;
        let half_taps = taps / 2;

        for i in -half_taps..half_taps {
            let offset = (i as f32) - frac;
            let sinc_val = Self::sinc(offset);
            let win_val = Self::window(offset, taps as f32);
            let weight = sinc_val * win_val;

            // Fetch sample from history
            let idx = ((self.history_idx as i32) - half_taps + i).rem_euclid(HISTORY_SIZE as i32) as usize;
            let sample = self.history[idx];

            sum += sample * weight;
            weight_sum += weight;
        }

        if weight_sum.abs() > 1e-6 {
            sum / weight_sum
        } else {
            sum
        }
    }

    /// Process input buffer to output buffer, returning number of samples written
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> (usize, usize) {
        let mut in_idx = 0;
        let mut out_idx = 0;

        while out_idx < output.len() && in_idx < input.len() {
            while self.fractional_pos < 1.0 && in_idx < input.len() {
                self.push_input(input[in_idx]);
                in_idx += 1;
                self.fractional_pos += 1.0;
            }

            if self.fractional_pos >= 1.0 {
                let frac = (self.fractional_pos - 1.0) as f32;
                output[out_idx] = self.interpolate_sample(frac);
                out_idx += 1;
                self.fractional_pos -= self.ratio;
            }
        }

        (in_idx, out_idx)
    }
}
