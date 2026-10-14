//! Adaptive Clock-Servo and Drift Estimator (AES67 / PTP IEEE 1588-2019 Principles).
//! Continuously tracks fractional crystal drift between Windows System Clock (YouTube/WASAPI)
//! and Real-Time Kernel Hardware DAC Clock, eliminating clicks and buffer slips without pitch warble.

#[repr(C)]
#[derive(Clone, Debug)]
pub struct ClockDriftServo {
    /// Proportional gain of the PI loop filter
    pub kp: f64,
    /// Integral gain of the PI loop filter
    pub ki: f64,
    /// Integrated phase error (fractional samples)
    pub integrator: f64,
    /// Target nominal buffer fill level (e.g. half ring capacity)
    pub target_fill_level: f64,
    /// Filtered frequency ratio adjustment (e.g. 1.000002 for +2 PPM drift)
    pub smoothed_ratio: f64,
    /// Exponential smoothing alpha
    pub alpha: f64,
}

impl ClockDriftServo {
    pub fn new(target_fill_level: usize) -> Self {
        Self {
            // Critically damped PI loop filter parameters tuned for sub-PPM audio clock tracking
            kp: 0.000005,
            ki: 0.0000001,
            integrator: 0.0,
            target_fill_level: target_fill_level as f64,
            smoothed_ratio: 1.0,
            alpha: 0.001,
        }
    }

    pub fn reset(&mut self) {
        self.integrator = 0.0;
        self.smoothed_ratio = 1.0;
    }

    /// Update the servo with current buffer fill level and return adjusted sample-rate ratio.
    /// Fractional ratio adjustment is micro-bounded to +/- 100 PPM (+/- 0.01%)
    /// which is mathematically inaudible to human hearing (per ITU-R BS.1116).
    #[inline(always)]
    pub fn update(&mut self, current_fill: usize) -> f64 {
        let error = (current_fill as f64) - self.target_fill_level;

        // PI filter calculation
        self.integrator += error * self.ki;
        // Anti-windup clamping to +/- 50 PPM
        self.integrator = self.integrator.clamp(-0.00005, 0.00005);

        let delta_ratio = error * self.kp + self.integrator;
        let clamped_delta = delta_ratio.clamp(-0.0001, 0.0001);

        let raw_ratio = 1.0 + clamped_delta;
        self.smoothed_ratio = self.smoothed_ratio * (1.0 - self.alpha) + raw_ratio * self.alpha;
        self.smoothed_ratio
    }
}
