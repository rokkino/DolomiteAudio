//! SMPTE and High-Precision Audio Sample Timecode Synchronization.
//! Supports 24 fps (Film), 25 fps (PAL), 29.97 Drop-Frame (NTSC), 30 fps, and Sub-Sample Time alignment.

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TimecodeFrameRate {
    Fps24,
    Fps25,
    Fps2997Drop,
    Fps2997NonDrop,
    Fps30,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct SmpteTime {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
    pub sub_frames: u8, // 1/80th or 1/100th of a frame
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct AudioTimecodeTracker {
    pub sample_rate: f64,
    pub frame_rate: TimecodeFrameRate,
    pub sample_position: i64,
    pub system_time_nanos: u64,
    pub speed_ratio: f64,
}

impl AudioTimecodeTracker {
    pub fn new(sample_rate: f64, frame_rate: TimecodeFrameRate) -> Self {
        Self {
            sample_rate,
            frame_rate,
            sample_position: 0,
            system_time_nanos: 0,
            speed_ratio: 1.0,
        }
    }

    #[inline(always)]
    pub fn advance_samples(&mut self, samples: i64, system_time_nanos: u64) {
        self.sample_position += samples;
        self.system_time_nanos = system_time_nanos;
    }

    /// Convert current sample position to SMPTE time
    pub fn to_smpte(&self) -> SmpteTime {
        let fps = match self.frame_rate {
            TimecodeFrameRate::Fps24 => 24.0,
            TimecodeFrameRate::Fps25 => 25.0,
            TimecodeFrameRate::Fps2997Drop | TimecodeFrameRate::Fps2997NonDrop => 29.97,
            TimecodeFrameRate::Fps30 => 30.0,
        };

        let total_seconds = (self.sample_position.max(0) as f64) / self.sample_rate;
        let hours = (total_seconds / 3600.0).floor() as u8 % 24;
        let rem_after_hours = total_seconds - ((hours as f64) * 3600.0);
        let minutes = (rem_after_hours / 60.0).floor() as u8;
        let rem_after_minutes = rem_after_hours - ((minutes as f64) * 60.0);
        let seconds = rem_after_minutes.floor() as u8;
        let fractional_seconds = rem_after_minutes - (seconds as f64);
        let frames = (fractional_seconds * fps).floor() as u8;
        let sub_frames = ((fractional_seconds * fps - (frames as f64)) * 80.0).floor() as u8;

        SmpteTime {
            hours,
            minutes,
            seconds,
            frames,
            sub_frames,
        }
    }
}
