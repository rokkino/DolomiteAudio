//! 64x64 Studio Crossbar Routing and Mixing Matrix.
//! Allows arbitrary non-blocking summing, gain adjustment, and phase inversion between any input and output.

use crate::ring_buffer::MAX_AUDIO_CHANNELS;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MatrixCrosspoint {
    /// Linear gain coefficient (0.0 = silent, 1.0 = unity, 2.0 = +6dB)
    pub gain: f32,
    /// Phase inversion flag (true = 180 degrees phase reversed)
    pub phase_inverted: bool,
    /// Mute flag for this specific routing connection
    pub is_muted: bool,
}

impl MatrixCrosspoint {
    pub const OFF: Self = Self {
        gain: 0.0,
        phase_inverted: false,
        is_muted: true,
    };

    pub const UNITY: Self = Self {
        gain: 1.0,
        phase_inverted: false,
        is_muted: false,
    };
}

#[repr(C)]
pub struct AudioRoutingMatrix {
    /// [OutputChannel][InputChannel] routing grid
    pub crosspoints: [[MatrixCrosspoint; MAX_AUDIO_CHANNELS]; MAX_AUDIO_CHANNELS],
    /// Master mute per output channel
    pub master_mute: [bool; MAX_AUDIO_CHANNELS],
    /// Master gain per output channel
    pub master_gain: [f32; MAX_AUDIO_CHANNELS],
}

impl AudioRoutingMatrix {
    pub const fn new() -> Self {
        let mut matrix = Self {
            crosspoints: [[MatrixCrosspoint::OFF; MAX_AUDIO_CHANNELS]; MAX_AUDIO_CHANNELS],
            master_mute: [false; MAX_AUDIO_CHANNELS],
            master_gain: [1.0; MAX_AUDIO_CHANNELS],
        };

        // Initialize 1-to-1 diagonal unity passthrough
        let mut i = 0;
        while i < MAX_AUDIO_CHANNELS {
            matrix.crosspoints[i][i] = MatrixCrosspoint::UNITY;
            i += 1;
        }

        matrix
    }

    /// Set gain and routing between an input and output channel
    pub fn set_routing(&mut self, input_ch: usize, output_ch: usize, gain: f32, phase_inverted: bool, muted: bool) {
        if input_ch < MAX_AUDIO_CHANNELS && output_ch < MAX_AUDIO_CHANNELS {
            self.crosspoints[output_ch][input_ch] = MatrixCrosspoint {
                gain,
                phase_inverted,
                is_muted: muted,
            };
        }
    }

    /// Process a multi-channel frame through the routing matrix in-place or into target buffer
    #[inline(always)]
    pub fn process_frame(&self, inputs: &[f32; MAX_AUDIO_CHANNELS], outputs: &mut [f32; MAX_AUDIO_CHANNELS]) {
        for out_idx in 0..MAX_AUDIO_CHANNELS {
            if self.master_mute[out_idx] {
                outputs[out_idx] = 0.0;
                continue;
            }

            let mut sum = 0.0f32;
            let row = &self.crosspoints[out_idx];

            for in_idx in 0..MAX_AUDIO_CHANNELS {
                let cp = &row[in_idx];
                if !cp.is_muted && cp.gain > 1e-6 {
                    let mut sample = inputs[in_idx] * cp.gain;
                    if cp.phase_inverted {
                        sample = -sample;
                    }
                    sum += sample;
                }
            }

            outputs[out_idx] = sum * self.master_gain[out_idx];
        }
    }
}
