//! Stateful streaming stereo resampler (Kaiser-windowed sinc, polyphase table).
//!
//! Unlike a per-block interpolator, this keeps its fractional phase and input
//! history across calls, so block boundaries are seamless and the long-term
//! output rate is exactly `in_rate -> out_rate` (no drift, no clicks).
//!
//! * Same-rate streams should bypass this entirely (bit-perfect path).
//! * When downsampling the cutoff follows the output Nyquist (anti-aliasing).
//! * No allocations after construction as long as each `process` call is
//!   `<= max_block` frames.

use std::vec::Vec;

const HALF_TAPS: usize = 16; // 32-tap kernel
const TAPS: usize = HALF_TAPS * 2;
const PHASES_LINEAR: usize = 256; // fractional resolution (linearly interpolated)
const PHASES_MIN: usize = 1024; // min-phase kernels are steeper: finer grid
const KAISER_BETA: f64 = 8.6; // ~-90 dB stop-band

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half = x * 0.5;
    for k in 1..64 {
        term *= (half / k as f64) * (half / k as f64);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

/// Filter phase response.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Symmetric FIR: constant group delay of 16 input frames.
    Linear,
    /// Minimum-phase version of the same magnitude response (cepstral
    /// method): energy is front-loaded, group delay drops to ~2-3 frames.
    Minimum,
}

pub struct StreamResampler {
    step: f64,       // input frames advanced per output frame (in_rate / out_rate)
    pos: f64,        // position of next output, in input-buffer index units
    lookahead: usize, // input frames needed after floor(pos)
    latency: f64,    // group delay at DC, in input frames
    hist_l: Vec<f32>,
    hist_r: Vec<f32>,
    table: std::sync::Arc<Vec<f32>>, // (phases + 1) * TAPS
    phases: usize,
}

// --- tiny radix-2 FFT (only used once, at construction) --------------------
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * core::f64::consts::PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (ar, ai) = (re[i + k], im[i + k]);
                let (br, bi) = (re[i + k + len / 2], im[i + k + len / 2]);
                let (tr, ti) = (br * cr - bi * ci, br * ci + bi * cr);
                re[i + k] = ar + tr;
                im[i + k] = ai + ti;
                re[i + k + len / 2] = ar - tr;
                im[i + k + len / 2] = ai - ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
    if inverse {
        let s = 1.0 / n as f64;
        for v in re.iter_mut() {
            *v *= s;
        }
        for v in im.iter_mut() {
            *v *= s;
        }
    }
}

/// Homomorphic (real-cepstrum) minimum-phase conversion of a real FIR.
fn minimum_phase(h: &[f64]) -> Vec<f64> {
    let mut m = 1usize;
    while m < h.len() * 4 {
        m <<= 1;
    }
    let (mut re, mut im) = (vec![0.0; m], vec![0.0; m]);
    re[..h.len()].copy_from_slice(h);
    fft(&mut re, &mut im, false);
    let peak = re.iter().zip(&im).map(|(a, b)| (a * a + b * b).sqrt()).fold(0.0, f64::max);
    let floor = peak * 1e-6; // -200 dB: keeps log finite at stop-band nulls
    for i in 0..m {
        let mag = (re[i] * re[i] + im[i] * im[i]).sqrt().max(floor);
        re[i] = mag.ln();
        im[i] = 0.0;
    }
    fft(&mut re, &mut im, true); // real cepstrum
    for i in 1..m / 2 {
        re[i] *= 2.0;
    }
    for i in m / 2 + 1..m {
        re[i] = 0.0;
    }
    for v in im.iter_mut() {
        *v = 0.0;
    }
    fft(&mut re, &mut im, false);
    for i in 0..m {
        let (a, b) = (re[i].exp(), im[i]);
        re[i] = a * b.cos();
        im[i] = a * b.sin();
    }
    fft(&mut re, &mut im, true);
    re.truncate(h.len());
    re
}

type Kernel = (std::sync::Arc<Vec<f32>>, f64);

/// Tables are cached per (cutoff, phase) so switching engines at run time
/// never pays the ~50 ms design cost twice.
fn cached_kernel(cutoff: f64, phase: Phase, build: impl FnOnce() -> (Vec<f32>, f64)) -> Kernel {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Vec<(u64, Phase, Kernel)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let key = cutoff.to_bits();
    if let Ok(c) = cache.lock() {
        if let Some((_, _, k)) = c.iter().find(|(b, p, _)| *b == key && *p == phase) {
            return k.clone();
        }
    }
    let (t, lat) = build();
    let k = (std::sync::Arc::new(t), lat);
    if let Ok(mut c) = cache.lock() {
        if c.len() > 16 {
            c.remove(0);
        }
        c.push((key, phase, k.clone()));
    }
    k
}

impl StreamResampler {
    /// Pre-build (and cache) the table for a rate pair, off the audio thread.
    pub fn prewarm(in_rate: f64, out_rate: f64, phase: Phase) {
        let _ = Self::with_phase(in_rate, out_rate, 16, phase);
    }

    /// Linear-phase resampler (reference quality, 16 frames of delay).
    pub fn new(in_rate: f64, out_rate: f64, max_block: usize) -> Self {
        Self::with_phase(in_rate, out_rate, max_block, Phase::Linear)
    }

    pub fn with_phase(in_rate: f64, out_rate: f64, max_block: usize, phase: Phase) -> Self {
        let step = in_rate / out_rate;
        let phases = if phase == Phase::Minimum { PHASES_MIN } else { PHASES_LINEAR };
        // Cutoff relative to input Nyquist; leave a little transition band.
        let cutoff = (out_rate / in_rate).min(1.0) * 0.94;
        let lookahead = if phase == Phase::Minimum { 0 } else { HALF_TAPS };
        let (table, latency) = cached_kernel(cutoff, phase, || {
            let i0_beta = bessel_i0(KAISER_BETA);

            // Prototype on the fine grid: d = n / phases input frames after the
            // input sample, n = 0..=TAPS*phases, centred at d = HALF_TAPS.
            let len = TAPS * phases + 1;
            let mut proto = vec![0.0f64; len];
            for (n, v) in proto.iter_mut().enumerate() {
                let x = n as f64 / phases as f64 - HALF_TAPS as f64;
                let sinc = if x.abs() < 1e-9 {
                    1.0
                } else {
                    let a = core::f64::consts::PI * x * cutoff;
                    a.sin() / a
                };
                let r = x / HALF_TAPS as f64;
                let w = if r.abs() >= 1.0 { 0.0 } else { bessel_i0(KAISER_BETA * (1.0 - r * r).sqrt()) / i0_beta };
                *v = sinc * w;
            }
            let (proto, lookahead) = match phase {
                Phase::Linear => (proto, HALF_TAPS),
                Phase::Minimum => {
                    (minimum_phase(&proto), 0)
                }
            };
            // DC group delay (centroid), in input frames, relative to "now".
            let (mut num, mut den) = (0.0, 0.0);
            for (n, &v) in proto.iter().enumerate() {
                num += n as f64 * v;
                den += v;
            }
            let centroid = num / den / phases as f64;
            let latency = match phase {
                Phase::Linear => HALF_TAPS as f64,
                Phase::Minimum => centroid,
            };

            // Polyphase table. Output at pos = i0 + frac uses input samples
            // j = i0 - (TAPS-1-lookahead) + k, at distance d = pos - j.
            let back = (TAPS - 1 - lookahead) as f64;
            // Global gain: unity at DC averaged over all phases (per-row
            // normalisation would add phase-dependent gain modulation).
            let gain = phases as f64 / proto.iter().sum::<f64>();
            let mut table = vec![0.0f32; (phases + 1) * TAPS];
            for p in 0..=phases {
                let row = &mut table[p * TAPS..(p + 1) * TAPS];
                let mut sum = 0.0f64;
                let mut vals = [0.0f64; TAPS];
                for k in 0..TAPS {
                    let d = p as f64 / phases as f64 + back - k as f64;
                    // grid index: linear proto is centred at HALF_TAPS, min-phase starts at 0
                    let g = match phase {
                        Phase::Linear => (d + HALF_TAPS as f64) * phases as f64,
                        Phase::Minimum => d * phases as f64,
                    };
                    let gi = g.round();
                    let v = if gi < 0.0 || gi as usize >= len { 0.0 } else { proto[gi as usize] };
                    vals[k] = v;
                    sum += v;
                }
                let _ = sum;
                for k in 0..TAPS {
                    row[k] = (vals[k] * gain) as f32;
                }
            }
            (table, latency)
        });

        let pad = TAPS - 1 - lookahead;
        let cap = max_block.max(1024) + TAPS * 2 + 8;
        let mut hist_l = Vec::with_capacity(cap);
        let mut hist_r = Vec::with_capacity(cap);
        hist_l.resize(pad, 0.0);
        hist_r.resize(pad, 0.0);

        Self { step, pos: pad as f64, lookahead, latency, hist_l, hist_r, table, phases }
    }

    /// Group delay of the filter, in input frames (rounded up).
    pub fn latency_input_frames(&self) -> usize {
        self.latency.ceil() as usize
    }

    /// Feed `n` input frames; every produced output frame is passed to `emit`.
    pub fn process(&mut self, left: &[f32], right: &[f32], mut emit: impl FnMut(f32, f32)) {
        self.hist_l.extend_from_slice(left);
        self.hist_r.extend_from_slice(right);
        let len = self.hist_l.len();
        let back = TAPS - 1 - self.lookahead;

        loop {
            let i0 = self.pos.floor() as usize;
            if i0 + self.lookahead >= len {
                break;
            }
            let frac = self.pos - i0 as f64;
            let phases = self.phases;
            let fp = frac * phases as f64;
            let p = (fp as usize).min(phases - 1);
            let t = (fp - p as f64) as f32;
            let row_a = &self.table[p * TAPS..(p + 1) * TAPS];
            let row_b = &self.table[(p + 1) * TAPS..(p + 2) * TAPS];
            let base = i0 - back;
            let xl = &self.hist_l[base..base + TAPS];
            let xr = &self.hist_r[base..base + TAPS];
            let (mut sl, mut sr) = (0.0f32, 0.0f32);
            for k in 0..TAPS {
                let c = row_a[k] + (row_b[k] - row_a[k]) * t;
                sl += xl[k] * c;
                sr += xr[k] * c;
            }
            emit(sl, sr);
            self.pos += self.step;
        }

        // Discard history no longer needed, keep fractional phase.
        let i0 = self.pos.floor() as usize;
        let keep_from = i0.saturating_sub(back).min(len);
        if keep_from > 0 {
            self.hist_l.drain(0..keep_from);
            self.hist_r.drain(0..keep_from);
            self.pos -= keep_from as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(in_rate: f64, out_rate: f64, block: usize, secs: f64, freq: f64) -> Vec<f32> {
        run_p(in_rate, out_rate, block, secs, freq, Phase::Linear)
    }

    fn run_p(in_rate: f64, out_rate: f64, block: usize, secs: f64, freq: f64, ph: Phase) -> Vec<f32> {
        let mut rs = StreamResampler::with_phase(in_rate, out_rate, block, ph);
        let total = (in_rate * secs) as usize;
        let mut out = Vec::new();
        let mut n = 0;
        let mut l = vec![0.0f32; block];
        while n < total {
            for i in 0..block {
                l[i] = (0.5 * (2.0 * core::f64::consts::PI * freq * (n + i) as f64 / in_rate).sin()) as f32;
            }
            rs.process(&l, &l, |a, _| out.push(a));
            n += block;
        }
        out
    }

    /// Residual vs. an ideal sine after fitting phase: measures distortion + clicks.
    fn thd_db(sig: &[f32], rate: f64, freq: f64) -> f64 {
        let s = &sig[sig.len() / 4..sig.len() * 3 / 4];
        let w = 2.0 * core::f64::consts::PI * freq / rate;
        // Least-squares fit of a*sin + b*cos + dc (window need not hold whole cycles).
        let mut m = [[0.0f64; 4]; 3];
        for (i, &x) in s.iter().enumerate() {
            let v = [(w * i as f64).sin(), (w * i as f64).cos(), 1.0];
            for r in 0..3 {
                for c in 0..3 {
                    m[r][c] += v[r] * v[c];
                }
                m[r][3] += v[r] * x as f64;
            }
        }
        for col in 0..3 {
            for r in col + 1..3 {
                let f = m[r][col] / m[col][col];
                for c in col..4 {
                    m[r][c] -= f * m[col][c];
                }
            }
        }
        let mut sol = [0.0f64; 3];
        for r in (0..3).rev() {
            let mut acc = m[r][3];
            for c in r + 1..3 {
                acc -= m[r][c] * sol[c];
            }
            sol[r] = acc / m[r][r];
        }
        let (a, b, mean) = (sol[0], sol[1], sol[2]);
        let (mut err, mut pow) = (0.0, 0.0);
        let mut max_err: f64 = 0.0;
        for (i, &x) in s.iter().enumerate() {
            let ideal = a * (w * i as f64).sin() + b * (w * i as f64).cos();
            let e = x as f64 - mean - ideal;
            max_err = max_err.max(e.abs());
            err += e * e;
            pow += ideal * ideal;
        }
        assert!(max_err < 0.002, "click/glitch detected: max err {max_err}");
        10.0 * (err / pow).log10()
    }

    #[test]
    fn rate_is_exact_and_clean_44k_to_48k() {
        let out = run(44100.0, 48000.0, 512, 2.0, 1000.0);
        let fed = ((88200 + 511) / 512 * 512) as f64; // whole blocks
        let expected = fed * 48000.0 / 44100.0;
        assert!((out.len() as f64 - expected).abs() <= 20.0, "len {} vs {}", out.len(), expected);
        let d = thd_db(&out, 48000.0, 1000.0);
        assert!(d < -95.0, "THD+N {d} dB");
    }

    #[test]
    fn clean_48k_to_44k_odd_blocks() {
        let out = run(48000.0, 44100.0, 61, 2.0, 3000.0);
        let d = thd_db(&out, 44100.0, 3000.0);
        assert!(d < -95.0, "THD+N {d} dB");
    }

    #[test]
    fn clean_96k_to_48k() {
        let out = run(96000.0, 48000.0, 64, 1.0, 5000.0);
        let d = thd_db(&out, 48000.0, 5000.0);
        assert!(d < -95.0, "THD+N {d} dB");
    }

    #[test]
    fn min_phase_is_clean_and_low_latency() {
        for &(a, b, blk, f) in &[(44100.0, 48000.0, 512usize, 1000.0), (48000.0, 44100.0, 61, 3000.0), (44100.0, 48000.0, 64, 12000.0)] {
            let out = run_p(a, b, blk, 2.0, f, Phase::Minimum);
            let d = thd_db(&out, b, f);
            assert!(d < -95.0, "min-phase {a}->{b} @ {f} Hz: THD+N {d} dB");
        }
        let rs = StreamResampler::with_phase(44100.0, 48000.0, 256, Phase::Minimum);
        assert!(rs.latency_input_frames() <= 5, "latency {}", rs.latency_input_frames());
        println!("min-phase latency = {:.2} frames", rs.latency);
    }
}
