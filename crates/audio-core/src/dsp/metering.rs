//! IEC 60268-10 PPM (Peak Programme Meter) and RMS Ballistics Detector.
//! Real-time telemetry monitoring for all audio channels without allocations.

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ChannelMeter {
    /// Peak linear amplitude [0.0, 1.0+]
    pub peak: f32,
    /// RMS linear amplitude [0.0, 1.0+]
    pub rms: f32,
    /// Peak in decibels [-inf, +6.0 dBFS]
    pub peak_db: f32,
    /// RMS in decibels [-inf, +6.0 dBFS]
    pub rms_db: f32,
    /// Clipping flag (true if any sample reached >= 1.0)
    pub is_clipped: bool,
    /// Running sum of squares for RMS calculation
    pub squared_sum: f32,
    /// Count of samples in current RMS window
    pub window_count: usize,
    /// Ballistic decay coefficient per sample
    pub decay_coeff: f32,
}

impl ChannelMeter {
    pub fn new(sample_rate: f32, decay_time_sec: f32) -> Self {
        let decay_coeff = (-1.0 / (decay_time_sec * sample_rate)).exp();
        Self {
            peak: 0.0,
            rms: 0.0,
            peak_db: -120.0,
            rms_db: -120.0,
            is_clipped: false,
            squared_sum: 0.0,
            window_count: 0,
            decay_coeff,
        }
    }

    #[inline(always)]
    pub fn reset_clip(&mut self) {
        self.is_clipped = false;
    }

    /// Process a single audio sample and update peak/RMS ballistics
    #[inline(always)]
    pub fn process_sample(&mut self, sample: f32) {
        let abs_val = sample.abs();

        if abs_val >= 1.0 {
            self.is_clipped = true;
        }

        // Instantaneous peak attack, exponential decay
        if abs_val > self.peak {
            self.peak = abs_val;
        } else {
            self.peak *= self.decay_coeff;
        }

        // Accumulate energy for RMS
        self.squared_sum += sample * sample;
        self.window_count += 1;

        // Window size: 512 samples (~5.3ms at 96kHz)
        if self.window_count >= 512 {
            let mean_square = self.squared_sum / (self.window_count as f32);
            self.rms = mean_square.sqrt();
            self.squared_sum = 0.0;
            self.window_count = 0;

            // Recalculate decibels
            self.peak_db = if self.peak > 1e-6 { 20.0 * self.peak.log10() } else { -120.0 };
            self.rms_db = if self.rms > 1e-6 { 20.0 * self.rms.log10() } else { -120.0 };
        }
    }

    /// Process a block of samples
    #[inline(always)]
    pub fn process_block(&mut self, samples: &[f32]) {
        for &s in samples {
            self.process_sample(s);
        }
    }
}
