//! ZeroAudio Native Rust Audio SDK.
//! Provides a safe, modern, ultra-low-latency interface that outperforms legacy ASIO COM.

pub mod telemetry;
pub mod stream;

pub use telemetry::*;
pub use stream::*;
pub use audio_core::{AudioFrame, AudioStreamConfig, MAX_AUDIO_CHANNELS};
