//! Adaptive resampling control loop (F. Adriaensen, "Controlling adaptive
//! resampling", LAC 2012 — the loop used by zita-ajbridge).
//!
//! Two audio devices never run on exactly the same clock. The consumer side
//! (here: the output device, which drives the ASIO callbacks) is the master;
//! the producer (capture device) is resampled with a ratio that is slowly
//! corrected so the FIFO between them stays at a fixed delay. No sample is
//! ever dropped or repeated, and the latency is constant.
//!
//! The measured delay is time-corrected: the FIFO only grows in device-period
//! sized steps, so we add the frames that have "virtually" arrived since the
//! last capture packet. That removes the sawtooth whose beat frequency would
//! otherwise sit inside the loop bandwidth.
//!
//! Loop: two one-pole smoothers (corner ~20x the loop bandwidth) followed by
//! a PI stage; critically damped (zeta = 1). Bandwidth starts wide for a
//! quick lock and narrows to `BW_FINAL` so the ratio is quasi-static and the
//! resampling is inaudible.

/// Loop bandwidth right after (re)start, Hz.
pub const BW_START: f64 = 1.0;
/// Steady-state loop bandwidth, Hz (Adriaensen uses 0.05 - 0.1 Hz).
pub const BW_FINAL: f64 = 0.05;
/// Time constant of the bandwidth narrowing, seconds.
const BW_TAU: f64 = 4.0;
/// Hard clamp on the correction (crystals are within +/-100 ppm; 0.5 % is
/// room for a badly clocked USB device without ever sounding wrong).
const MAX_CORR: f64 = 0.005;

#[derive(Clone, Debug)]
pub struct DriftLoop {
    target: f64,
    /// seconds per update
    t_upd: f64,
    /// frames per update (consumer side)
    frames_upd: f64,
    bw: f64,
    w0: f64,
    w1: f64,
    w2: f64,
    z1: f64,
    z2: f64,
    z3: f64,
    corr: f64,
    elapsed: f64,
}

impl DriftLoop {
    /// `target`: delay to hold, in consumer frames.
    /// `frames_per_update`: consumer frames between two `update` calls.
    /// `rate`: consumer sample rate.
    pub fn new(target: f64, frames_per_update: usize, rate: f64) -> Self {
        let mut s = Self {
            target,
            t_upd: frames_per_update as f64 / rate,
            frames_upd: frames_per_update as f64,
            bw: BW_START,
            w0: 0.0,
            w1: 0.0,
            w2: 0.0,
            z1: 0.0,
            z2: 0.0,
            z3: 0.0,
            corr: 1.0,
            elapsed: 0.0,
        };
        s.set_bw(BW_START);
        s
    }

    fn set_bw(&mut self, bw: f64) {
        let w = 2.0 * core::f64::consts::PI * bw * self.t_upd;
        let w1_new = 2.0 * w / self.frames_upd;
        // keep the loop output continuous when the gain changes
        if self.w1 > 0.0 {
            let k = w1_new / self.w1;
            self.z1 *= k;
            self.z2 *= k;
        }
        self.bw = bw;
        self.w0 = 1.0 - (-20.0 * w).exp();
        self.w1 = w1_new;
        self.w2 = w / 2.0;
    }

    /// Change the delay set point (e.g. after the jitter cushion grew).
    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    pub fn target(&self) -> f64 {
        self.target
    }

    /// Feed one delay measurement (consumer frames). Returns the ratio
    /// correction: multiply "output frames per input frame" by it.
    /// < 1.0 means the FIFO is too full and the producer must be consumed
    /// faster.
    #[inline]
    pub fn update(&mut self, delay: f64) -> f64 {
        self.elapsed += self.t_upd;
        if self.bw > BW_FINAL {
            let bw = BW_FINAL + (BW_START - BW_FINAL) * (-self.elapsed / BW_TAU).exp();
            if (bw - self.bw).abs() > 1e-4 || bw <= BW_FINAL + 1e-4 {
                self.set_bw(if bw <= BW_FINAL + 1e-4 { BW_FINAL } else { bw });
            }
        }
        let err = delay - self.target;
        self.z1 += self.w0 * (self.w1 * err - self.z1);
        self.z2 += self.w0 * (self.z1 - self.z2);
        self.z3 += self.w2 * self.z2;
        self.z3 = self.z3.clamp(-MAX_CORR, MAX_CORR);
        self.corr = (1.0 - (self.z2 + self.z3)).clamp(1.0 - MAX_CORR, 1.0 + MAX_CORR);
        self.corr
    }

    pub fn correction(&self) -> f64 {
        self.corr
    }

    /// Estimated clock offset of the producer vs the consumer, in ppm
    /// (positive = producer runs fast).
    pub fn drift_ppm(&self) -> f64 {
        (1.0 / self.corr - 1.0) * 1e6
    }

    pub fn locked(&self) -> bool {
        self.bw <= BW_FINAL * 1.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate a producer clocked `ppm` off, delivering in `period`-frame
    /// packets, a consumer pulling `block` frames, and check the loop holds
    /// the delay with no underrun and converges to the right ratio.
    fn sim(ppm: f64, period: usize, block: usize, secs: f64) -> (f64, f64, f64) {
        let rate = 48000.0;
        let target = (block + period + period / 2) as f64;
        let mut lp = DriftLoop::new(target, block, rate);
        let prod_rate = rate * (1.0 + ppm * 1e-6);
        let mut fifo = target; // frames in fifo (fractional ok)
        let mut t = 0.0f64;
        let mut next_packet = 0.0f64;
        let mut last_packet_end = 0.0f64;
        let mut max_err: f64 = 0.0;
        let mut min_fifo = f64::MAX;
        let mut corr = 1.0;
        let dt = block as f64 / rate;
        let steps = (secs / dt) as usize;
        for k in 0..steps {
            // producer packets that arrived up to now
            while next_packet <= t {
                next_packet += period as f64 / prod_rate;
                fifo += period as f64 * corr;
                last_packet_end = next_packet - period as f64 / prod_rate;
            }
            let delay = fifo + (t - last_packet_end) * rate;
            corr = lp.update(delay);
            min_fifo = min_fifo.min(fifo);
            fifo -= block as f64;
            if k as f64 * dt > secs * 0.7 {
                max_err = max_err.max((delay - target).abs());
            }
            t += dt;
        }
        (lp.drift_ppm(), max_err, min_fifo)
    }

    #[test]
    fn locks_to_positive_and_negative_drift() {
        for &ppm in &[-150.0, -20.0, 0.0, 35.0, 200.0] {
            let (est, err, min_fifo) = sim(ppm, 480, 256, 120.0);
            assert!((est - ppm).abs() < 2.0, "ppm {ppm}: estimated {est}");
            assert!(err < 4.0, "ppm {ppm}: steady-state delay error {err} frames");
            assert!(min_fifo >= 256.0, "ppm {ppm}: underrun, min fifo {min_fifo}");
        }
    }

    #[test]
    fn small_blocks_and_periods() {
        let (est, err, min_fifo) = sim(80.0, 144, 64, 90.0);
        assert!((est - 80.0).abs() < 2.0, "estimated {est}");
        assert!(err < 3.0, "error {err}");
        assert!(min_fifo >= 64.0);
    }
}
