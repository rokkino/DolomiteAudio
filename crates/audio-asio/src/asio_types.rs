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
pub type ASIOSamples = i64;
pub type ASIOTimeStamp = i64;

pub const ASIOSTInt16LSB: i32 = 0;
pub const ASIOSTInt24LSB: i32 = 1;
pub const ASIOSTInt32LSB: i32 = 2;
pub const ASIOSTFloat32LSB: i32 = 3;

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
