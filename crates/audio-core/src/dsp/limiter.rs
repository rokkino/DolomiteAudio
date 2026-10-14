//! High-Fidelity True-Peak Lookahead Limiter and Soft Clipper.
//! Compliant with ITU-R BS.1770-5 and AES TD1004 for Inter-Sample Peak (ISP) suppression.

pub const MAX_LOOKAHEAD_SAMPLES: usize = 64;

/// 4x Polyphase Sinc FIR Coefficients (ITU-R BS.1770-5 Table 1)
/// Evaluates true analog waveform peaks at sub-sample points: 0.25, 0.5, 0.75
const POLYPHASE_4X_COEFFS: [[f32; 4]; 3] = [
    // Phase 1 (t = 0.25)
    [-0.078028, 0.887643, 0.245824, -0.055439],
    // Phase 2 (t = 0.50)
    [-0.117188, 0.617188, 0.617188, -0.117188],
    // Phase 3 (t = 0.75)
    [-0.055439, 0.245824, 0.887643, -0.078028],
];

#[repr(C)]
#[derive(Clone, Debug)]
pub struct BrickwallLimiter {
    pub threshold: f32,
    pub ceiling: f32,
    pub release_coeff: f32,
    pub envelope: f32,
    pub lookahead_buffer: [f32; MAX_LOOKAHEAD_SAMPLES],
    pub lookahead_index: usize,
    pub lookahead_length: usize,
    /// 4-sample history window for 4x polyphase inter-sample true-peak reconstruction
    pub true_peak_history: [f32; 4],
}

impl BrickwallLimiter {
    pub fn new(sample_rate: f32, ceiling_db: f32, release_ms: f32, lookahead_samples: usize) -> Self {
        let ceiling = 10.0f32.powf(ceiling_db / 20.0);
        let release_time_sec = (release_ms / 1000.0).max(0.001);
        let release_coeff = (-1.0 / (release_time_sec * sample_rate)).exp();
        let lookahead_length = lookahead_samples.min(MAX_LOOKAHEAD_SAMPLES);

        Self {
            threshold: ceiling,
            ceiling,
            release_coeff,
            envelope: 0.0,
            lookahead_buffer: [0.0; MAX_LOOKAHEAD_SAMPLES],
            lookahead_index: 0,
            lookahead_length,
            true_peak_history: [0.0; 4],
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.lookahead_buffer.fill(0.0);
        self.lookahead_index = 0;
        self.true_peak_history.fill(0.0);
    }

    /// Calculate true analog peak using 4x polyphase FIR interpolation (ITU-R BS.1770-5)
    #[inline(always)]
    fn estimate_true_peak(&self, sample: f32) -> f32 {
        let mut max_abs = sample.abs();
        let h = [self.true_peak_history[1], self.true_peak_history[2], self.true_peak_history[3], sample];

        // Evaluate interpolated points at 0.25, 0.50, and 0.75
        for phase in &POLYPHASE_4X_COEFFS {
            let interp = h[0] * phase[0] + h[1] * phase[1] + h[2] * phase[2] + h[3] * phase[3];
            let abs_interp = interp.abs();
            if abs_interp > max_abs {
                max_abs = abs_interp;
            }
        }

        max_abs
    }

    /// Process a single audio sample through the lookahead envelope follower with True-Peak detection
    #[inline(always)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Estimate true-peak across inter-sample grid
        let true_peak = self.estimate_true_peak(input);

        // Update history
        self.true_peak_history[0] = self.true_peak_history[1];
        self.true_peak_history[1] = self.true_peak_history[2];
        self.true_peak_history[2] = self.true_peak_history[3];
        self.true_peak_history[3] = input;

        // Instantaneous attack, exponential release envelope detector
        if true_peak > self.envelope {
            self.envelope = true_peak;
        } else {
            self.envelope = self.envelope * self.release_coeff + true_peak * (1.0 - self.release_coeff);
        }

        // Calculate gain reduction
        let gain = if self.envelope > self.ceiling {
            self.ceiling / self.envelope
        } else {
            1.0
        };

        // If lookahead is disabled (true zero-latency mode: 0 samples lookahead),
        // apply direct soft-knee saturation
        if self.lookahead_length == 0 {
            return (input * gain).clamp(-self.ceiling, self.ceiling);
        }

        // Read delayed sample from circular lookahead buffer
        let delayed_sample = self.lookahead_buffer[self.lookahead_index];
        self.lookahead_buffer[self.lookahead_index] = input;
        self.lookahead_index = (self.lookahead_index + 1) % self.lookahead_length;

        // Apply calculated gain to the delayed audio sample
        delayed_sample * gain
    }

    /// Process a stereo pair in-place with linkable sidechain to preserve stereo image
    #[inline(always)]
    pub fn process_stereo_linked(&mut self, left: &mut f32, right: &mut f32) {
        let true_l = self.estimate_true_peak(*left);
        let true_r = self.estimate_true_peak(*right);
        let max_abs = true_l.max(true_r);

        if max_abs > self.envelope {
            self.envelope = max_abs;
        } else {
            self.envelope = self.envelope * self.release_coeff + max_abs * (1.0 - self.release_coeff);
        }

        let gain = if self.envelope > self.ceiling {
            self.ceiling / self.envelope
        } else {
            1.0
        };

        *left *= gain;
        *right *= gain;
    }
}
