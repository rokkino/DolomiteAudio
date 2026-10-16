//! Steinberg ASIO 2.3 Standard Types and C ABI Definitions.
#![allow(non_upper_case_globals, dead_code)]

use core::ffi::c_void;

pub type ASIOBool = i32;
pub const ASIOFalse: ASIOBool = 0;
pub const ASIOTrue: ASIOBool = 1;

pub type ASIOError = i32;
pub const ASE_OK: ASIOError = 0;
pub const ASE_SUCCESS: ASIOError = 0x3f484c27;
pub const ASE_NotPresent: ASIOError = -1000;
pub const ASE_HWMalfunction: ASIOError = -999;
pub const ASE_InvalidParameter: ASIOError = -998;
pub const ASE_InvalidMode: ASIOError = -997;
pub const ASE_SPNotAdvancing: ASIOError = -996;
pub const ASE_NoClock: ASIOError = -995;
pub const ASE_NoMemory: ASIOError = -994;

pub type ASIOSampleRate = f64;
/// ASIO SDK on Windows is built with NATIVE_INT64 = 0, so 64-bit sample
/// positions and timestamps are passed as `struct { unsigned long hi, lo; }`
/// (high word FIRST). Writing a native little-endian i64 here swaps the
/// words and hands the host garbage positions.
#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct ASIO64 {
    pub hi: u32,
    pub lo: u32,
}

impl ASIO64 {
    #[inline]
    pub const fn from_i64(v: i64) -> Self {
        let u = v as u64;
        Self { hi: (u >> 32) as u32, lo: u as u32 }
    }
    #[inline]
    pub const fn to_i64(self) -> i64 {
        (((self.hi as u64) << 32) | self.lo as u64) as i64
    }
}

pub type ASIOSamples = ASIO64;
pub type ASIOTimeStamp = ASIO64;

// ASIOSampleType constants according to Steinberg ASIO 2.3 SDK
pub const ASIOSTInt16MSB: i32 = 0;
pub const ASIOSTInt24MSB: i32 = 1;
pub const ASIOSTInt32MSB: i32 = 2;
pub const ASIOSTFloat32MSB: i32 = 3;
pub const ASIOSTFloat64MSB: i32 = 4;

pub const ASIOSTInt32MSB16: i32 = 8;
pub const ASIOSTInt32MSB18: i32 = 9;
pub const ASIOSTInt32MSB20: i32 = 10;
pub const ASIOSTInt32MSB24: i32 = 11;

pub const ASIOSTInt16LSB: i32 = 16;
pub const ASIOSTInt24LSB: i32 = 17;
pub const ASIOSTInt32LSB: i32 = 18;
pub const ASIOSTFloat32LSB: i32 = 19;
pub const ASIOSTFloat64LSB: i32 = 20;

pub const ASIOSTInt32LSB16: i32 = 24;
pub const ASIOSTInt32LSB18: i32 = 25;
pub const ASIOSTInt32LSB20: i32 = 26;
pub const ASIOSTInt32LSB24: i32 = 27;

// asioMessage selectors (host callbacks)
pub const kAsioSelectorSupported: i32 = 1;
pub const kAsioEngineVersion: i32 = 2;
pub const kAsioResetRequest: i32 = 3;
pub const kAsioBufferSizeChange: i32 = 4;
pub const kAsioResyncRequest: i32 = 5;
pub const kAsioLatenciesChanged: i32 = 6;
pub const kAsioSupportsTimeInfo: i32 = 7;
pub const kAsioSupportsTimeCode: i32 = 8;
pub const kAsioOverload: i32 = 12;

// ASIOFuture selectors according to Steinberg ASIO 2.3 SDK
pub const kAsioEnableTimeCodeRead: i32 = 1;
pub const kAsioDisableTimeCodeRead: i32 = 2;
pub const kAsioSetInputMonitor: i32 = 3;
pub const kAsioTransport: i32 = 4;
pub const kAsioSetInputGain: i32 = 5;
pub const kAsioGetInputMeter: i32 = 6;
pub const kAsioSetOutputGain: i32 = 7;
pub const kAsioGetOutputMeter: i32 = 8;
pub const kAsioCanInputMonitor: i32 = 9;
pub const kAsioCanTimeInfo: i32 = 10;
pub const kAsioCanTimeCode: i32 = 11;
pub const kAsioCanTransport: i32 = 12;
pub const kAsioCanInputGain: i32 = 13;
pub const kAsioCanInputMeter: i32 = 14;
pub const kAsioCanOutputGain: i32 = 15;
pub const kAsioCanOutputMeter: i32 = 16;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOChannelInfo {
    pub channel: i32,
    pub is_input: ASIOBool,
    pub is_active: ASIOBool,
    pub channel_group: i32,
    pub sample_type: i32,
    pub name: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOBufferInfo {
    pub is_input: ASIOBool,
    pub channel_num: i32,
    pub buffers: [*mut c_void; 2],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOCallbacks {
    pub buffer_switch: Option<extern "C" fn(double_buffer_index: i32, direct_process: ASIOBool)>,
    pub sample_rate_did_change: Option<extern "C" fn(s_rate: ASIOSampleRate)>,
    pub asio_message: Option<extern "C" fn(selector: i32, value: i32, message: *mut c_void, opt: *mut f64) -> i32>,
    pub buffer_switch_time_info: Option<extern "C" fn(params: *mut c_void, double_buffer_index: i32, direct_process: ASIOBool) -> *mut c_void>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOClockSource {
    pub index: i32,
    pub associated_channel: i32,
    pub associated_group: i32,
    pub is_current_source: ASIOBool,
    pub name: [u8; 32],
}

pub const kSystemTimeValid: u32 = 1 << 0;
pub const kSamplePositionValid: u32 = 1 << 1;
pub const kSampleRateValid: u32 = 1 << 2;
pub const kSpeedValid: u32 = 1 << 3;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOTimeInfo {
    pub speed: f64,
    pub system_time: ASIOTimeStamp,      // In nanoseconds
    pub sample_position: ASIOSamples,    // Sample frames
    pub sample_rate: ASIOSampleRate,
    pub flags: u32,
    pub reserved: [u8; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOTimeCode {
    pub speed: f64,
    pub time_code_samples: ASIOSamples,
    pub flags: u32,
    pub future: [u8; 64],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ASIOTime {
    pub reserved: [i32; 4],
    pub time_info: ASIOTimeInfo,
    pub time_code: ASIOTimeCode,
}

impl ASIOTime {
    pub fn new(sample_pos: i64, sample_rate: f64, system_time_ns: i64) -> Self {
        Self {
            reserved: [0; 4],
            time_info: ASIOTimeInfo {
                speed: 1.0,
                system_time: ASIO64::from_i64(system_time_ns),
                sample_position: ASIO64::from_i64(sample_pos),
                sample_rate,
                flags: kSystemTimeValid | kSamplePositionValid | kSampleRateValid | kSpeedValid,
                reserved: [0; 12],
            },
            time_code: ASIOTimeCode {
                speed: 1.0,
                time_code_samples: ASIO64::from_i64(sample_pos),
                flags: 0,
                future: [0; 64],
            },
        }
    }
}

const _: () = assert!(core::mem::size_of::<ASIOTimeInfo>() == 48);
const _: () = assert!(core::mem::size_of::<ASIOTime>() == 16 + 48 + 88);
