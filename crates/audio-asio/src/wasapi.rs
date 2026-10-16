//! WASAPI (Windows Audio Session API) Low-Latency Hardware Output & Capture Bridge.
//! Routes audio directly between DolomiteAudioRT ASIO and real physical soundcards (Speakers/Headphones/Microphones).

use core::ffi::c_void;
use std::ptr::{null, null_mut};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID {
    data1: 0xBCDE0395,
    data2: 0xE52F,
    data3: 0x467C,
    data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
};

pub const IID_IMMDEVICE_ENUMERATOR: GUID = GUID {
    data1: 0xA95664D2,
    data2: 0x9614,
    data3: 0x4F35,
    data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
};

pub const IID_IAUDIO_CLIENT: GUID = GUID {
    data1: 0x1CB9AD4C,
    data2: 0xDBFA,
    data3: 0x4C32,
    data4: [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2],
};

// Official Windows SDK Audioclient.h: F294ACFC-3146-4483-A7BF-ADDCA7C260E2
pub const IID_IAUDIO_RENDER_CLIENT: GUID = GUID {
    data1: 0xF294ACFC,
    data2: 0x3146,
    data3: 0x4483,
    data4: [0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2],
};

// Official Windows SDK Audioclient.h: C8ADBD64-E71E-48a0-A4DE-185C395CD317
pub const IID_IAUDIO_CAPTURE_CLIENT: GUID = GUID {
    data1: 0xC8ADBD64,
    data2: 0xE71E,
    data3: 0x48A0,
    data4: [0xA4, 0xDE, 0x18, 0x5C, 0x39, 0x5C, 0xD3, 0x17],
};

// Official Windows SDK Endpointvolume.h: 5BC63DB8-8859-470A-9076-18604A166648
pub const IID_IAUDIO_ENDPOINT_VOLUME: GUID = GUID {
    data1: 0x5BC63DB8,
    data2: 0x8859,
    data3: 0x470A,
    data4: [0x90, 0x76, 0x18, 0x60, 0x4A, 0x16, 0x66, 0x48],
};

#[repr(C)]
pub struct IAudioEndpointVolumeVtbl {
    pub query_interface: usize,
    pub add_ref: usize,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub register_control_change_notify: usize,
    pub unregister_control_change_notify: usize,
    pub get_channel_count: usize,
    pub set_master_volume_level: usize,
    pub set_master_volume_level_scalar: extern "system" fn(*mut c_void, f32, *const GUID) -> i32,
    pub get_master_volume_level: usize,
    pub get_master_volume_level_scalar: extern "system" fn(*mut c_void, *mut f32) -> i32,
    pub set_channel_volume_level: usize,
    pub set_channel_volume_level_scalar: usize,
    pub get_channel_volume_level: usize,
    pub get_channel_volume_level_scalar: usize,
    pub set_mute: extern "system" fn(*mut c_void, i32, *const GUID) -> i32,
    pub get_mute: extern "system" fn(*mut c_void, *mut i32) -> i32,
}

#[repr(C)]
pub struct WAVEFORMATEX {
    pub w_format_tag: u16,
    pub n_channels: u16,
    pub n_samples_per_sec: u32,
    pub n_avg_bytes_per_sec: u32,
    pub n_block_align: u16,
    pub w_bits_per_sample: u16,
    pub cb_size: u16,
}

#[repr(C)]
pub struct PROPERTYKEY {
    pub fmtid: GUID,
    pub pid: u32,
}

pub const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID {
        data1: 0xA45C254E,
        data2: 0xDF1C,
        data3: 0x4EFD,
        data4: [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
    },
    pid: 14,
};

#[repr(C)]
pub struct PROPVARIANT {
    pub vt: u16,
    pub w_reserved1: u16,
    pub w_reserved2: u16,
    pub w_reserved3: u16,
    pub pwsz_val: *mut u16,
}

#[repr(C)]
pub struct IMMDeviceEnumeratorVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub enum_audio_endpoints: extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32,
    pub get_default_audio_endpoint: extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32,
    pub get_device: extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> i32,
    pub register_endpoint_notification_callback: extern "system" fn(*mut c_void, *mut c_void) -> i32,
    pub unregister_endpoint_notification_callback: extern "system" fn(*mut c_void, *mut c_void) -> i32,
}

#[repr(C)]
pub struct IMMDeviceVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub activate: extern "system" fn(*mut c_void, *const GUID, u32, *mut c_void, *mut *mut c_void) -> i32,
    pub open_property_store: extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    pub get_id: extern "system" fn(*mut c_void, *mut *mut u16) -> i32,
    pub get_state: extern "system" fn(*mut c_void, *mut u32) -> i32,
}

#[repr(C)]
pub struct IMMDeviceCollectionVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub get_count: extern "system" fn(*mut c_void, *mut u32) -> i32,
    pub item: extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
}

#[repr(C)]
pub struct IPropertyStoreVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub get_count: extern "system" fn(*mut c_void, *mut u32) -> i32,
    pub get_at: extern "system" fn(*mut c_void, u32, *mut PROPERTYKEY) -> i32,
    pub get_value: extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> i32,
    pub set_value: extern "system" fn(*mut c_void, *const PROPERTYKEY, *const PROPVARIANT) -> i32,
    pub commit: extern "system" fn(*mut c_void) -> i32,
}

#[repr(C)]
pub struct IAudioClientVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub initialize: extern "system" fn(*mut c_void, i32, u32, i64, i64, *const c_void, *const GUID) -> i32,
    pub get_buffer_size: extern "system" fn(*mut c_void, *mut u32) -> i32,
    pub get_stream_latency: extern "system" fn(*mut c_void, *mut i64) -> i32,
    pub get_current_padding: extern "system" fn(*mut c_void, *mut u32) -> i32,
    pub is_format_supported: extern "system" fn(*mut c_void, i32, *const c_void, *mut *mut c_void) -> i32,
    pub get_mix_format: extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub get_device_period: extern "system" fn(*mut c_void, *mut i64, *mut i64) -> i32,
    pub start: extern "system" fn(*mut c_void) -> i32,
    pub stop: extern "system" fn(*mut c_void) -> i32,
    pub reset: extern "system" fn(*mut c_void) -> i32,
    pub set_event_handle: extern "system" fn(*mut c_void, *mut c_void) -> i32,
    pub get_service: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
}

#[repr(C)]
pub struct IAudioRenderClientVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub get_buffer: extern "system" fn(*mut c_void, u32, *mut *mut u8) -> i32,
    pub release_buffer: extern "system" fn(*mut c_void, u32, u32) -> i32,
}

#[repr(C)]
pub struct IAudioCaptureClientVtbl {
    pub query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    pub add_ref: extern "system" fn(*mut c_void) -> u32,
    pub release: extern "system" fn(*mut c_void) -> u32,
    pub get_buffer: extern "system" fn(*mut c_void, *mut *mut u8, *mut u32, *mut u32, *mut u64, *mut u64) -> i32,
    pub release_buffer: extern "system" fn(*mut c_void, u32) -> i32,
    pub get_next_packet_size: extern "system" fn(*mut c_void, *mut u32) -> i32,
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitialize(pvReserved: *mut c_void) -> i32;
    fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> i32;
    fn CoTaskMemFree(pv: *mut c_void);
}

#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
}

/// Helper to enumerate available audio endpoints
pub fn enumerate_devices(is_capture: bool) -> Vec<AudioDeviceInfo> {
    let mut devices = Vec::new();
    unsafe {
        let _ = CoInitialize(null_mut());
        let mut enumerator: *mut c_void = null_mut();
        let hr = CoCreateInstance(
            &CLSID_MMDEVICE_ENUMERATOR,
            null_mut(),
            1, // CLSCTX_INPROC_SERVER
            &IID_IMMDEVICE_ENUMERATOR,
            &mut enumerator,
        );
        if hr != 0 || enumerator.is_null() {
            return devices;
        }

        let enum_vtbl = &**(enumerator as *mut *const IMMDeviceEnumeratorVtbl);
        let mut collection: *mut c_void = null_mut();
        let flow = if is_capture { 1 } else { 0 }; // eRender=0, eCapture=1
        let hr = (enum_vtbl.enum_audio_endpoints)(enumerator, flow, 1, &mut collection); // DEVICE_STATE_ACTIVE = 1
        if hr == 0 && !collection.is_null() {
            let coll_vtbl = &**(collection as *mut *const IMMDeviceCollectionVtbl);
            let mut count = 0u32;
            let _ = (coll_vtbl.get_count)(collection, &mut count);

            for i in 0..count {
                let mut dev: *mut c_void = null_mut();
                if (coll_vtbl.item)(collection, i, &mut dev) == 0 && !dev.is_null() {
                    let dev_vtbl = &**(dev as *mut *const IMMDeviceVtbl);
                    let mut id_ptr: *mut u16 = null_mut();
                    let _ = (dev_vtbl.get_id)(dev, &mut id_ptr);

                    let mut dev_id = String::new();
                    if !id_ptr.is_null() {
                        let mut len = 0;
                        while *id_ptr.add(len) != 0 {
                            len += 1;
                        }
                        let slice = core::slice::from_raw_parts(id_ptr, len);
                        dev_id = String::from_utf16_lossy(slice);
                        CoTaskMemFree(id_ptr as _);
                    }

                    let mut friendly_name = format!("Device {}", i);
                    let mut prop_store: *mut c_void = null_mut();
                    if (dev_vtbl.open_property_store)(dev, 0, &mut prop_store) == 0 && !prop_store.is_null() {
                        let prop_vtbl = &**(prop_store as *mut *const IPropertyStoreVtbl);
                        let mut pv: PROPVARIANT = core::mem::zeroed();
                        if (prop_vtbl.get_value)(prop_store, &PKEY_DEVICE_FRIENDLY_NAME, &mut pv) == 0 {
                            if pv.vt == 31 && !pv.pwsz_val.is_null() {
                                let mut len = 0;
                                while *pv.pwsz_val.add(len) != 0 {
                                    len += 1;
                                }
                                let slice = core::slice::from_raw_parts(pv.pwsz_val, len);
                                friendly_name = String::from_utf16_lossy(slice);
                            }
                        }
                        let _ = (prop_vtbl.release)(prop_store);
                    }

                    devices.push(AudioDeviceInfo {
                        id: dev_id,
                        name: friendly_name,
                    });

                    let _ = (dev_vtbl.release)(dev);
                }
            }
            let _ = (coll_vtbl.release)(collection);
        }
        let _ = (enum_vtbl.release)(enumerator);
    }
    devices
}

// ---------------------------------------------------------------------------
// Render engine
// ---------------------------------------------------------------------------

// Official Windows SDK Audioclient.h: 7ED4EE07-8E67-4CD4-8C1A-2B7A5987AD42
pub const IID_IAUDIO_CLIENT3: GUID = GUID {
    data1: 0x7ED4EE07,
    data2: 0x8E67,
    data3: 0x4CD4,
    data4: [0x8C, 0x1A, 0x2B, 0x7A, 0x59, 0x87, 0xAD, 0x42],
};

/// IAudioClient3 = IAudioClient + IAudioClient2 (3 methods) + 3 new methods.
#[repr(C)]
pub struct IAudioClient3Vtbl {
    pub base: IAudioClientVtbl,
    pub is_offload_capable: usize,
    pub set_client_properties: usize,
    pub get_buffer_size_limits: usize,
    pub get_shared_mode_engine_period: extern "system" fn(*mut c_void, *const c_void, *mut u32, *mut u32, *mut u32, *mut u32) -> i32,
    pub get_current_shared_mode_engine_period: extern "system" fn(*mut c_void, *mut *mut c_void, *mut u32) -> i32,
    pub initialize_shared_audio_stream: extern "system" fn(*mut c_void, u32, u32, *const c_void, *const GUID) -> i32,
}

const AUDCLNT_STREAMFLAGS_EVENTCALLBACK: u32 = 0x0004_0000;
pub const AUDCLNT_E_DEVICE_INVALIDATED: i32 = 0x8889_0004u32 as i32;
const E_ROLE_CONSOLE: i32 = 0;

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> i32;
    fn CoUninitialize();
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateEventW(sa: *mut c_void, manual_reset: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn WaitForSingleObject(h: *mut c_void, ms: u32) -> u32;
    fn CloseHandle(h: *mut c_void) -> i32;
}

/// Calls CoInitializeEx(MTA) for the current thread and balances it on drop.
pub struct ComGuard(bool);
impl ComGuard {
    pub fn mta() -> Self {
        let hr = unsafe { CoInitializeEx(null_mut(), 0 /* COINIT_MULTITHREADED */) };
        ComGuard(hr >= 0) // S_OK or S_FALSE need balancing; RPC_E_CHANGED_MODE does not
    }
}
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    F32,
    I16,
    I24In32, // 24 valid bits left-justified in 32
    I32,
    I24Packed, // 3 bytes per sample (exclusive mode on many USB/HDA devices)
}

impl SampleFormat {
    #[allow(dead_code)]
    fn bytes(self) -> usize {
        match self {
            SampleFormat::I16 => 2,
            SampleFormat::I24Packed => 3,
            _ => 4,
        }
    }
}

/// Requested engine. Falls back Exclusive -> LowLatency -> Shared automatically.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Engine {
    Shared,
    LowLatency,
    Exclusive,
}

const AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED: i32 = 0x8889_0019u32 as i32;
pub const AUDCLNT_E_DEVICE_IN_USE: i32 = 0x8889_000Au32 as i32;

// KSDATAFORMAT_SUBTYPE_PCM / IEEE_FLOAT share this tail: -0000-0010-8000-00AA00389B71
const KS_SUBTYPE_TAIL: [u8; 12] = [0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71];

/// Build a WAVEFORMATEXTENSIBLE (40 bytes) for exclusive-mode negotiation.
fn make_wfx(rate: u32, channels: u16, fmt: SampleFormat) -> Vec<u8> {
    let (container, valid, float) = match fmt {
        SampleFormat::F32 => (32u16, 32u16, true),
        SampleFormat::I32 => (32, 32, false),
        SampleFormat::I24In32 => (32, 24, false),
        SampleFormat::I24Packed => (24, 24, false),
        SampleFormat::I16 => (16, 16, false),
    };
    let block = channels * container / 8;
    let mask: u32 = match channels {
        1 => 0x4,
        2 => 0x3,
        _ => (1u32 << channels.min(18)) - 1,
    };
    let mut b = Vec::with_capacity(40);
    b.extend_from_slice(&0xFFFEu16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * block as u32).to_le_bytes());
    b.extend_from_slice(&block.to_le_bytes());
    b.extend_from_slice(&container.to_le_bytes());
    b.extend_from_slice(&22u16.to_le_bytes());
    b.extend_from_slice(&valid.to_le_bytes());
    b.extend_from_slice(&mask.to_le_bytes());
    b.extend_from_slice(&(if float { 3u32 } else { 1u32 }).to_le_bytes());
    b.extend_from_slice(&KS_SUBTYPE_TAIL);
    debug_assert_eq!(b.len(), 40);
    b
}

/// Inspect a WAVEFORMATEX / WAVEFORMATEXTENSIBLE mix format.
unsafe fn parse_format(p: *const c_void) -> Option<SampleFormat> {
    let wfx = &*(p as *const WAVEFORMATEX);
    let mut tag = wfx.w_format_tag as u32;
    let mut valid_bits = wfx.w_bits_per_sample;
    if tag == 0xFFFE && wfx.cb_size >= 22 {
        let bytes = p as *const u8;
        valid_bits = core::ptr::read_unaligned(bytes.add(18) as *const u16);
        tag = core::ptr::read_unaligned(bytes.add(24) as *const u32); // SubFormat.Data1
        if valid_bits == 0 {
            valid_bits = wfx.w_bits_per_sample;
        }
    }
    match (tag, wfx.w_bits_per_sample) {
        (3, 32) => Some(SampleFormat::F32),
        (1, 16) => Some(SampleFormat::I16),
        (1, 32) if valid_bits == 24 => Some(SampleFormat::I24In32),
        (1, 32) => Some(SampleFormat::I32),
        _ => None,
    }
}

unsafe fn com_release(p: *mut c_void) {
    if !p.is_null() {
        let vt = &**(p as *mut *const IAudioClientVtbl);
        (vt.release)(p);
    }
}

unsafe fn read_friendly_name(dev: *mut c_void) -> Option<String> {
    let dev_vtbl = &**(dev as *mut *const IMMDeviceVtbl);
    let mut store: *mut c_void = null_mut();
    if (dev_vtbl.open_property_store)(dev, 0, &mut store) != 0 || store.is_null() {
        return None;
    }
    let vt = &**(store as *mut *const IPropertyStoreVtbl);
    let mut pv: PROPVARIANT = core::mem::zeroed();
    let mut out = None;
    if (vt.get_value)(store, &PKEY_DEVICE_FRIENDLY_NAME, &mut pv) == 0 && pv.vt == 31 && !pv.pwsz_val.is_null() {
        let mut len = 0;
        while *pv.pwsz_val.add(len) != 0 {
            len += 1;
        }
        out = Some(String::from_utf16_lossy(core::slice::from_raw_parts(pv.pwsz_val, len)));
        CoTaskMemFree(pv.pwsz_val as _);
    }
    (vt.release)(store);
    out
}

/// One WASAPI shared-mode render stream, event driven.
///
/// The engine's event is the master clock for the whole ASIO driver: the
/// pump only asks the DAW for a new block when the device has actually
/// consumed audio. That makes drift, FIFO overflow and "too fast" playback
/// impossible by construction.
pub struct WasapiRender {
    audio_client: *mut c_void,
    render_client: *mut c_void,
    event: *mut c_void,
    mix_format: *mut c_void,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
    pub buffer_frames: u32,
    pub period_frames: u32,
    pub low_latency: bool,
    pub exclusive: bool,
    pub name: String,
    /// Owned WAVEFORMATEXTENSIBLE when exclusive mode negotiated its own format.
    _own_format: Vec<u8>,
}

unsafe impl Send for WasapiRender {}

impl WasapiRender {
    /// Open `device_id` (or the default render device) in event-driven shared mode.
    /// The caller must have initialised COM on this thread (see `ComGuard`).
    pub fn open(device_id: Option<&str>, engine: Engine, daw_rate: u32, block_hint_secs: f64) -> Result<Self, String> {
        unsafe {
            let mut enumerator: *mut c_void = null_mut();
            let hr = CoCreateInstance(&CLSID_MMDEVICE_ENUMERATOR, null_mut(), 1, &IID_IMMDEVICE_ENUMERATOR, &mut enumerator);
            if hr != 0 || enumerator.is_null() {
                return Err(format!("MMDeviceEnumerator failed: 0x{:08X}", hr));
            }
            let enum_vtbl = &**(enumerator as *mut *const IMMDeviceEnumeratorVtbl);

            let mut device: *mut c_void = null_mut();
            if let Some(id) = device_id.filter(|s| !s.is_empty()) {
                let w: Vec<u16> = id.encode_utf16().chain(core::iter::once(0)).collect();
                if (enum_vtbl.get_device)(enumerator, w.as_ptr(), &mut device) != 0 {
                    device = null_mut();
                }
            }
            if device.is_null() {
                let hr = (enum_vtbl.get_default_audio_endpoint)(enumerator, 0 /* eRender */, E_ROLE_CONSOLE, &mut device);
                if hr != 0 || device.is_null() {
                    (enum_vtbl.release)(enumerator);
                    return Err(format!("no default render device: 0x{:08X}", hr));
                }
            }
            (enum_vtbl.release)(enumerator);

            let name = read_friendly_name(device).unwrap_or_else(|| "Windows audio device".into());
            let mut result = Err(String::new());
            if engine == Engine::Exclusive {
                result = Self::open_exclusive(device, &name, daw_rate);
                if let Err(e) = &result {
                    crate::asio_driver::log_debug(&format!("Exclusive mode unavailable on '{}' ({}), falling back to low-latency shared", name, e));
                }
            }
            if result.is_err() {
                result = Self::open_on_device(device, &name, engine != Engine::Shared, block_hint_secs);
            }
            let dev_vtbl = &**(device as *mut *const IMMDeviceVtbl);
            (dev_vtbl.release)(device);
            result
        }
    }

    /// WASAPI exclusive, event-driven, minimum hardware period (MS "Exclusive-Mode
    /// Streams" pattern): two ping-pong buffers of one period each.
    unsafe fn open_exclusive(device: *mut c_void, name: &str, daw_rate: u32) -> Result<Self, String> {
        let x = open_exclusive_client(device, daw_rate)?;
        let vt = &**(x.ac as *mut *const IAudioClientVtbl);
        let mut rc: *mut c_void = null_mut();
        if (vt.get_service)(x.ac, &IID_IAUDIO_RENDER_CLIENT, &mut rc) != 0 || rc.is_null() {
            CloseHandle(x.event);
            com_release(x.ac);
            return Err("GetService(IAudioRenderClient) failed".into());
        }
        crate::asio_driver::log_debug(&format!(
            "WASAPI render '{}' ready: EXCLUSIVE {} Hz, {} ch, {:?}, period/buffer={} frames ({:.2} ms)",
            name, x.rate, x.channels, x.fmt, x.buffer_frames, x.buffer_frames as f64 * 1000.0 / x.rate as f64
        ));
        Ok(Self {
            audio_client: x.ac,
            render_client: rc,
            event: x.event,
            mix_format: null_mut(),
            sample_rate: x.rate,
            channels: x.channels,
            format: x.fmt,
            buffer_frames: x.buffer_frames,
            period_frames: x.buffer_frames.max(1),
            low_latency: true,
            exclusive: true,
            name: name.to_string(),
            _own_format: x.wfx,
        })
    }

    unsafe fn activate(device: *mut c_void) -> Result<*mut c_void, String> {
        activate_client(device)
    }

    unsafe fn open_on_device(device: *mut c_void, name: &str, want_low_latency: bool, block_hint_secs: f64) -> Result<Self, String> {
        let mut audio_client = Self::activate(device)?;
        let vt = &**(audio_client as *mut *const IAudioClientVtbl);

        let mut mix: *mut c_void = null_mut();
        let hr = (vt.get_mix_format)(audio_client, &mut mix);
        if hr != 0 || mix.is_null() {
            com_release(audio_client);
            return Err(format!("GetMixFormat failed: 0x{:08X}", hr));
        }
        let fmt = match parse_format(mix) {
            Some(f) => f,
            None => {
                CoTaskMemFree(mix);
                com_release(audio_client);
                return Err("unsupported mix format".into());
            }
        };
        let wfx = &*(mix as *const WAVEFORMATEX);
        let sample_rate = wfx.n_samples_per_sec;
        let channels = wfx.n_channels;

        // --- 1) Try Windows 10+ low-latency shared mode (IAudioClient3) ---
        let mut low_latency = false;
        let mut period_frames = 0u32;
        let mut initialized = false;
        if want_low_latency {
            let mut ac3: *mut c_void = null_mut();
            if (vt.query_interface)(audio_client, &IID_IAUDIO_CLIENT3, &mut ac3) == 0 && !ac3.is_null() {
                let vt3 = &**(ac3 as *mut *const IAudioClient3Vtbl);
                let (mut def, mut fund, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
                if (vt3.get_shared_mode_engine_period)(ac3, mix, &mut def, &mut fund, &mut min, &mut max) == 0 && min > 0 && min < def {
                    let hr = (vt3.initialize_shared_audio_stream)(ac3, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, min, mix, null());
                    if hr == 0 {
                        initialized = true;
                        low_latency = true;
                        period_frames = min;
                    } else {
                        crate::asio_driver::log_debug(&format!("IAudioClient3 low-latency init failed (0x{:08X}), using classic shared mode", hr));
                    }
                }
                com_release(ac3);
            }
            if !initialized {
                // A failed Initialize can leave the client unusable: start from a fresh one.
                com_release(audio_client);
                audio_client = Self::activate(device)?;
            }
        }

        let vt = &**(audio_client as *mut *const IAudioClientVtbl);

        // --- 2) Classic event-driven shared mode ---
        if !initialized {
            let mut def_period: i64 = 0;
            let mut min_period: i64 = 0;
            let _ = (vt.get_device_period)(audio_client, &mut def_period, &mut min_period);
            if def_period <= 0 {
                def_period = 100_000; // 10 ms
            }
            // Buffer: at least 3 engine periods and 2 DAW blocks.
            let hns_block = (block_hint_secs * 1e7) as i64;
            let hns_buffer = (def_period * 3).max(hns_block * 2 + def_period).max(300_000);
            let hr = (vt.initialize)(audio_client, 0, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, hns_buffer, 0, mix, null());
            if hr != 0 {
                CoTaskMemFree(mix);
                com_release(audio_client);
                return Err(format!("IAudioClient::Initialize failed: 0x{:08X}", hr));
            }
            period_frames = ((def_period as f64 * sample_rate as f64) / 1e7).round() as u32;
        }

        let event = CreateEventW(null_mut(), 0, 0, null());
        let fail = |msg: String, ac: *mut c_void, mix: *mut c_void, ev: *mut c_void| -> Result<Self, String> {
            if !ev.is_null() {
                CloseHandle(ev);
            }
            CoTaskMemFree(mix);
            com_release(ac);
            Err(msg)
        };
        if event.is_null() {
            return fail("CreateEvent failed".into(), audio_client, mix, event);
        }
        let hr = (vt.set_event_handle)(audio_client, event);
        if hr != 0 {
            return fail(format!("SetEventHandle failed: 0x{:08X}", hr), audio_client, mix, event);
        }

        let mut buffer_frames = 0u32;
        let _ = (vt.get_buffer_size)(audio_client, &mut buffer_frames);

        let mut render_client: *mut c_void = null_mut();
        let hr = (vt.get_service)(audio_client, &IID_IAUDIO_RENDER_CLIENT, &mut render_client);
        if hr != 0 || render_client.is_null() {
            return fail(format!("GetService(IAudioRenderClient) failed: 0x{:08X}", hr), audio_client, mix, event);
        }

        crate::asio_driver::log_debug(&format!(
            "WASAPI render '{}' ready: {} Hz, {} ch, {:?}, period={} frames, buffer={} frames, mode={}",
            name, sample_rate, channels, fmt, period_frames, buffer_frames,
            if low_latency { "low-latency (IAudioClient3)" } else { "shared (event)" }
        ));

        Ok(Self {
            audio_client,
            render_client,
            event,
            mix_format: mix,
            sample_rate,
            channels,
            format: fmt,
            buffer_frames,
            period_frames: period_frames.max(1),
            low_latency,
            exclusive: false,
            name: name.to_string(),
            _own_format: Vec::new(),
        })
    }

    pub fn start(&self) -> i32 {
        unsafe { ((&**(self.audio_client as *mut *const IAudioClientVtbl)).start)(self.audio_client) }
    }

    pub fn stop(&self) {
        unsafe {
            let vt = &**(self.audio_client as *mut *const IAudioClientVtbl);
            let _ = (vt.stop)(self.audio_client);
        }
    }

    /// Frames currently queued in the device buffer (Err = HRESULT, e.g. device removed).
    pub fn padding(&self) -> Result<u32, i32> {
        let mut p = 0u32;
        let hr = unsafe { ((&**(self.audio_client as *mut *const IAudioClientVtbl)).get_current_padding)(self.audio_client, &mut p) };
        if hr == 0 { Ok(p) } else { Err(hr) }
    }

    /// Block until the engine asks for more data. Returns false on timeout.
    pub fn wait(&self, timeout_ms: u32) -> bool {
        unsafe { WaitForSingleObject(self.event, timeout_ms) == 0 }
    }

    /// Move as many frames as fit from `fifo` into the device buffer.
    pub fn write_from(&mut self, fifo: &mut std::collections::VecDeque<(f32, f32)>) -> Result<usize, i32> {
        let space = if self.exclusive {
            // event-driven exclusive: each event hands us exactly one whole buffer
            self.buffer_frames as usize
        } else {
            let padding = self.padding()?;
            self.buffer_frames.saturating_sub(padding) as usize
        };
        if self.exclusive && fifo.len() < space {
            return Ok(0);
        }
        let n = space.min(fifo.len());
        if n == 0 {
            return Ok(0);
        }
        unsafe {
            let rvt = &**(self.render_client as *mut *const IAudioRenderClientVtbl);
            let mut data: *mut u8 = null_mut();
            let hr = (rvt.get_buffer)(self.render_client, n as u32, &mut data);
            if hr != 0 || data.is_null() {
                return Err(hr);
            }
            let ch = self.channels as usize;
            for i in 0..n {
                let (l, r) = fifo.pop_front().unwrap_or((0.0, 0.0));
                // Map stereo onto the device layout (mono devices get a downmix).
                for c in 0..ch {
                    let v = match (ch, c) {
                        (1, _) => 0.5 * (l + r),
                        (_, 0) => l,
                        (_, 1) => r,
                        _ => 0.0,
                    };
                    let idx = i * ch + c;
                    if self.format == SampleFormat::I24Packed {
                        let sv = (v.clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32;
                        let p = data.add(idx * 3);
                        *p = sv as u8;
                        *p.add(1) = (sv >> 8) as u8;
                        *p.add(2) = (sv >> 16) as u8;
                        continue;
                    }
                    match self.format {
                        SampleFormat::F32 => *(data as *mut f32).add(idx) = v,
                        SampleFormat::I16 => *(data as *mut i16).add(idx) = (v.clamp(-1.0, 1.0) * 32767.0).round() as i16,
                        SampleFormat::I24In32 => {
                            let s = (v.clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32;
                            *(data as *mut i32).add(idx) = s << 8;
                        }
                        SampleFormat::I32 => *(data as *mut i32).add(idx) = (v.clamp(-1.0, 1.0) as f64 * 2_147_483_647.0) as i32,
                        SampleFormat::I24Packed => {}
                    }
                }
            }
            let _ = (rvt.release_buffer)(self.render_client, n as u32, 0);
        }
        Ok(n)
    }
}

impl Drop for WasapiRender {
    fn drop(&mut self) {
        unsafe {
            self.stop();
            if !self.render_client.is_null() {
                let rvt = &**(self.render_client as *mut *const IAudioRenderClientVtbl);
                (rvt.release)(self.render_client);
            }
            com_release(self.audio_client);
            if !self.event.is_null() {
                CloseHandle(self.event);
            }
            if !self.mix_format.is_null() {
                CoTaskMemFree(self.mix_format);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (render + capture)
// ---------------------------------------------------------------------------

unsafe fn activate_client(device: *mut c_void) -> Result<*mut c_void, String> {
    let dev_vtbl = &**(device as *mut *const IMMDeviceVtbl);
    let mut ac: *mut c_void = null_mut();
    let hr = (dev_vtbl.activate)(device, &IID_IAUDIO_CLIENT, 1 /* CLSCTX_INPROC_SERVER */, null_mut(), &mut ac);
    if hr != 0 || ac.is_null() {
        return Err(format!("IMMDevice::Activate failed: 0x{:08X}", hr));
    }
    Ok(ac)
}

/// Resolve an endpoint by id, or the default console endpoint for `flow`
/// (0 = render, 1 = capture). Returns an AddRef'd IMMDevice.
unsafe fn get_endpoint(device_id: Option<&str>, flow: i32) -> Result<*mut c_void, String> {
    let mut enumerator: *mut c_void = null_mut();
    let hr = CoCreateInstance(&CLSID_MMDEVICE_ENUMERATOR, null_mut(), 1, &IID_IMMDEVICE_ENUMERATOR, &mut enumerator);
    if hr != 0 || enumerator.is_null() {
        return Err(format!("MMDeviceEnumerator failed: 0x{:08X}", hr));
    }
    let enum_vtbl = &**(enumerator as *mut *const IMMDeviceEnumeratorVtbl);
    let mut device: *mut c_void = null_mut();
    if let Some(id) = device_id.filter(|s| !s.is_empty()) {
        let w: Vec<u16> = id.encode_utf16().chain(core::iter::once(0)).collect();
        if (enum_vtbl.get_device)(enumerator, w.as_ptr(), &mut device) != 0 {
            device = null_mut();
        }
    }
    if device.is_null() {
        let hr = (enum_vtbl.get_default_audio_endpoint)(enumerator, flow, E_ROLE_CONSOLE, &mut device);
        if hr != 0 || device.is_null() {
            (enum_vtbl.release)(enumerator);
            return Err(format!("no default {} device: 0x{:08X}", if flow == 0 { "render" } else { "capture" }, hr));
        }
    }
    (enum_vtbl.release)(enumerator);
    Ok(device)
}

unsafe fn release_device(device: *mut c_void) {
    if !device.is_null() {
        let dev_vtbl = &**(device as *mut *const IMMDeviceVtbl);
        (dev_vtbl.release)(device);
    }
}

struct ExclusiveClient {
    ac: *mut c_void,
    event: *mut c_void,
    wfx: Vec<u8>,
    rate: u32,
    fmt: SampleFormat,
    channels: u16,
    buffer_frames: u32,
}

/// Negotiate an exclusive, event-driven stream at the smallest hardware
/// period. Works for both render and capture endpoints.
unsafe fn open_exclusive_client(device: *mut c_void, daw_rate: u32) -> Result<ExclusiveClient, String> {
    let mut ac = activate_client(device)?;
    let vt = &**(ac as *mut *const IAudioClientVtbl);
    let mut mix: *mut c_void = null_mut();
    if (vt.get_mix_format)(ac, &mut mix) != 0 || mix.is_null() {
        com_release(ac);
        return Err("GetMixFormat failed".into());
    }
    let mix_rate = (*(mix as *const WAVEFORMATEX)).n_samples_per_sec;
    let mix_ch = (*(mix as *const WAVEFORMATEX)).n_channels.max(1);
    CoTaskMemFree(mix);

    // Prefer the DAW rate (no resampling, bit-perfect), then the mix rate.
    let mut rates = vec![daw_rate];
    if mix_rate != daw_rate {
        rates.push(mix_rate);
    }
    let fmts = [SampleFormat::F32, SampleFormat::I24In32, SampleFormat::I32, SampleFormat::I24Packed, SampleFormat::I16];
    let mut chosen: Option<(Vec<u8>, u32, SampleFormat, u16)> = None;
    'outer: for &r in &rates {
        for &ch in &[2u16, mix_ch, 1] {
            for &f in &fmts {
                let w = make_wfx(r, ch, f);
                if (vt.is_format_supported)(ac, 1 /* EXCLUSIVE */, w.as_ptr() as *const c_void, null_mut()) == 0 {
                    chosen = Some((w, r, f, ch));
                    break 'outer;
                }
            }
        }
    }
    let (wfx, rate, fmt, channels) = match chosen {
        Some(c) => c,
        None => {
            com_release(ac);
            return Err("no exclusive format supported".into());
        }
    };

    let (mut def_p, mut min_p) = (0i64, 0i64);
    let _ = (vt.get_device_period)(ac, &mut def_p, &mut min_p);
    let mut period = if min_p > 0 { min_p } else { 30_000 };
    let mut hr = (vt.initialize)(ac, 1, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, period, period, wfx.as_ptr() as *const c_void, null());
    if hr == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED {
        let mut n = 0u32;
        let _ = (vt.get_buffer_size)(ac, &mut n);
        period = (1e7 / rate as f64 * n as f64 + 0.5) as i64;
        com_release(ac);
        ac = activate_client(device)?;
        let vt2 = &**(ac as *mut *const IAudioClientVtbl);
        hr = (vt2.initialize)(ac, 1, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, period, period, wfx.as_ptr() as *const c_void, null());
    }
    if hr != 0 {
        com_release(ac);
        return Err(format!("exclusive Initialize failed: 0x{:08X}{}", hr, if hr == AUDCLNT_E_DEVICE_IN_USE { " (device in use)" } else { "" }));
    }
    let vt = &**(ac as *mut *const IAudioClientVtbl);
    let event = CreateEventW(null_mut(), 0, 0, null());
    if event.is_null() || (vt.set_event_handle)(ac, event) != 0 {
        if !event.is_null() {
            CloseHandle(event);
        }
        com_release(ac);
        return Err("SetEventHandle failed".into());
    }
    let mut buffer_frames = 0u32;
    let _ = (vt.get_buffer_size)(ac, &mut buffer_frames);
    Ok(ExclusiveClient { ac, event, wfx, rate, fmt, channels, buffer_frames })
}

/// Read one interleaved sample of `fmt` at sample index `idx`.
#[inline(always)]
unsafe fn read_sample(data: *const u8, idx: usize, fmt: SampleFormat) -> f32 {
    match fmt {
        SampleFormat::F32 => *(data as *const f32).add(idx),
        SampleFormat::I16 => *(data as *const i16).add(idx) as f32 * (1.0 / 32768.0),
        SampleFormat::I24In32 | SampleFormat::I32 => *(data as *const i32).add(idx) as f32 * (1.0 / 2_147_483_648.0),
        SampleFormat::I24Packed => {
            let p = data.add(idx * 3);
            let v = ((*p as i32) << 8) | ((*p.add(1) as i32) << 16) | ((*p.add(2) as i32) << 24);
            v as f32 * (1.0 / 2_147_483_648.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Capture engine
// ---------------------------------------------------------------------------

/// WASAPI capture stream (exclusive / low-latency shared / shared). It is
/// polled from the render pump, so it needs no thread of its own.
pub struct WasapiCapture {
    audio_client: *mut c_void,
    capture_client: *mut c_void,
    event: *mut c_void,
    mix_format: *mut c_void,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
    pub period_frames: u32,
    pub exclusive: bool,
    pub low_latency: bool,
    pub name: String,
    _own_format: Vec<u8>,
    /// QPC time (100 ns units) at which the last captured frame was sampled
    /// by the device; 0 = unknown. Time-corrects the FIFO level for the
    /// adaptive resampling loop.
    pub last_frame_100ns: u64,
}

unsafe impl Send for WasapiCapture {}

impl WasapiCapture {
    /// Open the capture endpoint. Caller must hold COM on this thread.
    pub fn open(device_id: Option<&str>, engine: Engine, daw_rate: u32) -> Result<Self, String> {
        unsafe {
            let device = get_endpoint(device_id, 1)?;
            let name = read_friendly_name(device).unwrap_or_else(|| "Windows input".into());
            let mut result = Err(String::new());
            if engine == Engine::Exclusive {
                result = Self::open_exclusive(device, &name, daw_rate);
                if let Err(e) = &result {
                    crate::asio_driver::log_debug(&format!("Exclusive capture unavailable on '{}' ({}), using shared", name, e));
                }
            }
            if result.is_err() {
                result = Self::open_shared(device, &name, engine != Engine::Shared);
            }
            release_device(device);
            result
        }
    }

    unsafe fn open_exclusive(device: *mut c_void, name: &str, daw_rate: u32) -> Result<Self, String> {
        let x = open_exclusive_client(device, daw_rate)?;
        let vt = &**(x.ac as *mut *const IAudioClientVtbl);
        let mut cc: *mut c_void = null_mut();
        if (vt.get_service)(x.ac, &IID_IAUDIO_CAPTURE_CLIENT, &mut cc) != 0 || cc.is_null() {
            CloseHandle(x.event);
            com_release(x.ac);
            return Err("GetService(IAudioCaptureClient) failed".into());
        }
        crate::asio_driver::log_debug(&format!(
            "WASAPI capture '{}' ready: EXCLUSIVE {} Hz, {} ch, {:?}, period={} frames ({:.2} ms)",
            name, x.rate, x.channels, x.fmt, x.buffer_frames, x.buffer_frames as f64 * 1000.0 / x.rate as f64
        ));
        Ok(Self {
            audio_client: x.ac,
            capture_client: cc,
            event: x.event,
            mix_format: null_mut(),
            sample_rate: x.rate,
            channels: x.channels,
            format: x.fmt,
            period_frames: x.buffer_frames.max(1),
            exclusive: true,
            low_latency: true,
            name: name.to_string(),
            _own_format: x.wfx,
            last_frame_100ns: 0,
        })
    }

    unsafe fn open_shared(device: *mut c_void, name: &str, want_low_latency: bool) -> Result<Self, String> {
        let mut ac = activate_client(device)?;
        let vt = &**(ac as *mut *const IAudioClientVtbl);
        let mut mix: *mut c_void = null_mut();
        if (vt.get_mix_format)(ac, &mut mix) != 0 || mix.is_null() {
            com_release(ac);
            return Err("GetMixFormat failed".into());
        }
        let fmt = match parse_format(mix) {
            Some(f) => f,
            None => {
                CoTaskMemFree(mix);
                com_release(ac);
                return Err("unsupported capture mix format".into());
            }
        };
        let wfx = &*(mix as *const WAVEFORMATEX);
        let (rate, channels) = (wfx.n_samples_per_sec, wfx.n_channels);

        let mut initialized = false;
        let mut low_latency = false;
        let mut period = 0u32;
        if want_low_latency {
            let mut ac3: *mut c_void = null_mut();
            if (vt.query_interface)(ac, &IID_IAUDIO_CLIENT3, &mut ac3) == 0 && !ac3.is_null() {
                let vt3 = &**(ac3 as *mut *const IAudioClient3Vtbl);
                let (mut def, mut fund, mut min, mut max) = (0u32, 0u32, 0u32, 0u32);
                if (vt3.get_shared_mode_engine_period)(ac3, mix, &mut def, &mut fund, &mut min, &mut max) == 0 && min > 0 && min < def {
                    if (vt3.initialize_shared_audio_stream)(ac3, 0, min, mix, null()) == 0 {
                        initialized = true;
                        low_latency = true;
                        period = min;
                    }
                }
                com_release(ac3);
            }
            if !initialized {
                com_release(ac);
                ac = activate_client(device)?;
            }
        }
        let vt = &**(ac as *mut *const IAudioClientVtbl);
        if !initialized {
            let (mut def_p, mut min_p) = (0i64, 0i64);
            let _ = (vt.get_device_period)(ac, &mut def_p, &mut min_p);
            if def_p <= 0 {
                def_p = 100_000;
            }
            let hr = (vt.initialize)(ac, 0, 0, (def_p * 4).max(400_000), 0, mix, null());
            if hr != 0 {
                CoTaskMemFree(mix);
                com_release(ac);
                return Err(format!("capture Initialize failed: 0x{:08X}", hr));
            }
            period = ((def_p as f64 * rate as f64) / 1e7).round() as u32;
        }
        let mut cc: *mut c_void = null_mut();
        if (vt.get_service)(ac, &IID_IAUDIO_CAPTURE_CLIENT, &mut cc) != 0 || cc.is_null() {
            CoTaskMemFree(mix);
            com_release(ac);
            return Err("GetService(IAudioCaptureClient) failed".into());
        }
        crate::asio_driver::log_debug(&format!(
            "WASAPI capture '{}' ready: {} Hz, {} ch, {:?}, period={} frames, {}",
            name, rate, channels, fmt, period, if low_latency { "low-latency shared" } else { "shared" }
        ));
        Ok(Self {
            audio_client: ac,
            capture_client: cc,
            event: null_mut(),
            mix_format: mix,
            sample_rate: rate,
            channels,
            format: fmt,
            period_frames: period.max(1),
            exclusive: false,
            low_latency,
            name: name.to_string(),
            _own_format: Vec::new(),
            last_frame_100ns: 0,
        })
    }

    pub fn start(&self) -> i32 {
        unsafe { ((&**(self.audio_client as *mut *const IAudioClientVtbl)).start)(self.audio_client) }
    }

    pub fn stop(&self) {
        unsafe {
            let vt = &**(self.audio_client as *mut *const IAudioClientVtbl);
            let _ = (vt.stop)(self.audio_client);
        }
    }

    /// Drain every captured packet into `left`/`right` (appending, stereo).
    /// Never blocks. Returns frames read, Err(HRESULT) if the device failed.
    pub fn read_into(&mut self, left: &mut Vec<f32>, right: &mut Vec<f32>, max_frames: usize) -> Result<usize, i32> {
        let mut total = 0usize;
        unsafe {
            let vt = &**(self.capture_client as *mut *const IAudioCaptureClientVtbl);
            loop {
                let mut packet = 0u32;
                let hr = (vt.get_next_packet_size)(self.capture_client, &mut packet);
                if hr != 0 {
                    return Err(hr);
                }
                if packet == 0 {
                    break;
                }
                let mut data: *mut u8 = null_mut();
                let (mut n, mut flags, mut dev_pos, mut qpc_pos) = (0u32, 0u32, 0u64, 0u64);
                let hr = (vt.get_buffer)(self.capture_client, &mut data, &mut n, &mut flags, &mut dev_pos, &mut qpc_pos);
                if hr != 0 {
                    return Err(hr);
                }
                let ch = self.channels.max(1) as usize;
                let silent = flags & 0x2 != 0 || data.is_null();
                // Never grow past the caller's preallocated capacity (no RT allocation).
                let room = max_frames.saturating_sub(left.len());
                for i in 0..(n as usize).min(room) {
                    let (l, r) = if silent {
                        (0.0, 0.0)
                    } else if ch == 1 {
                        let m = read_sample(data, i, self.format);
                        (m, m)
                    } else {
                        (read_sample(data, i * ch, self.format), read_sample(data, i * ch + 1, self.format))
                    };
                    left.push(l);
                    right.push(r);
                }
                let _ = (vt.release_buffer)(self.capture_client, n);
                // qpc_pos = when the packet's first frame was captured.
                // flag 0x4 = AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR
                self.last_frame_100ns = if qpc_pos != 0 && flags & 0x4 == 0 {
                    qpc_pos + (n as u64).saturating_sub(1) * 10_000_000 / self.sample_rate.max(1) as u64
                } else {
                    0
                };
                total += n as usize;
            }
        }
        Ok(total)
    }
}

impl Drop for WasapiCapture {
    fn drop(&mut self) {
        unsafe {
            self.stop();
            if !self.capture_client.is_null() {
                let vt = &**(self.capture_client as *mut *const IAudioCaptureClientVtbl);
                (vt.release)(self.capture_client);
            }
            com_release(self.audio_client);
            if !self.event.is_null() {
                CloseHandle(self.event);
            }
            if !self.mix_format.is_null() {
                CoTaskMemFree(self.mix_format);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Session probe: is any *other* application currently making sound on an
// endpoint? Used to drop exclusive mode when two programs need audio.
// ---------------------------------------------------------------------------

const IID_IAUDIO_SESSION_MANAGER2: GUID = GUID { data1: 0x77AA99A0, data2: 0x1BD6, data3: 0x484F, data4: [0x8B, 0xC7, 0x2C, 0x65, 0x4C, 0x9A, 0x9B, 0x6F] };
const IID_IAUDIO_SESSION_CONTROL2: GUID = GUID { data1: 0xBFB7FF88, data2: 0x7239, data3: 0x4FC9, data4: [0x8F, 0xA2, 0x07, 0xC9, 0x50, 0xBE, 0x9C, 0x6D] };
const IID_IAUDIO_METER_INFORMATION: GUID = GUID { data1: 0xC02216F6, data2: 0x8C67, data3: 0x4B5B, data4: [0x9D, 0x00, 0xD0, 0x08, 0xE7, 0x3E, 0x00, 0x64] };

#[repr(C)]
struct IUnknownVtbl {
    query_interface: extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: extern "system" fn(*mut c_void) -> u32,
    release: extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct IAudioSessionManager2Vtbl {
    base: IUnknownVtbl,
    get_audio_session_control: usize,
    get_simple_audio_volume: usize,
    get_session_enumerator: extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct IAudioSessionEnumeratorVtbl {
    base: IUnknownVtbl,
    get_count: extern "system" fn(*mut c_void, *mut i32) -> i32,
    get_session: extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct IAudioSessionControl2Vtbl {
    base: IUnknownVtbl,
    get_state: extern "system" fn(*mut c_void, *mut i32) -> i32,
    get_display_name: usize,
    set_display_name: usize,
    get_icon_path: usize,
    set_icon_path: usize,
    get_grouping_param: usize,
    set_grouping_param: usize,
    register_notification: usize,
    unregister_notification: usize,
    get_session_identifier: usize,
    get_session_instance_identifier: usize,
    get_process_id: extern "system" fn(*mut c_void, *mut u32) -> i32,
    is_system_sounds_session: extern "system" fn(*mut c_void) -> i32,
}

#[repr(C)]
struct IAudioMeterInformationVtbl {
    base: IUnknownVtbl,
    get_peak_value: extern "system" fn(*mut c_void, *mut f32) -> i32,
}

unsafe fn unk_release(p: *mut c_void) {
    if !p.is_null() {
        ((&**(p as *mut *const IUnknownVtbl)).release)(p);
    }
}

/// One audible session of another process on an endpoint.
#[derive(Clone, Debug)]
pub struct OtherSession {
    pub pid: u32,
    pub peak: f32,
}

/// Active, audible (> -70 dBFS) sessions on `device_id`/flow owned by a
/// process other than `own_pid` (system sounds excluded). Caller holds COM.
pub fn audible_sessions(device_id: Option<&str>, flow: i32, own_pid: u32) -> Vec<OtherSession> {
    let mut out = Vec::new();
    unsafe {
        let device = match get_endpoint(device_id, flow) {
            Ok(d) => d,
            Err(_) => return out,
        };
        let dev_vtbl = &**(device as *mut *const IMMDeviceVtbl);
        let mut mgr: *mut c_void = null_mut();
        let hr = (dev_vtbl.activate)(device, &IID_IAUDIO_SESSION_MANAGER2, 1, null_mut(), &mut mgr);
        release_device(device);
        if hr != 0 || mgr.is_null() {
            return out;
        }
        let mvt = &**(mgr as *mut *const IAudioSessionManager2Vtbl);
        let mut en: *mut c_void = null_mut();
        if (mvt.get_session_enumerator)(mgr, &mut en) == 0 && !en.is_null() {
            let evt = &**(en as *mut *const IAudioSessionEnumeratorVtbl);
            let mut count = 0i32;
            let _ = (evt.get_count)(en, &mut count);
            for i in 0..count {
                let mut ctl: *mut c_void = null_mut();
                if (evt.get_session)(en, i, &mut ctl) != 0 || ctl.is_null() {
                    continue;
                }
                let mut ctl2: *mut c_void = null_mut();
                let uvt = &**(ctl as *mut *const IUnknownVtbl);
                if (uvt.query_interface)(ctl, &IID_IAUDIO_SESSION_CONTROL2, &mut ctl2) == 0 && !ctl2.is_null() {
                    let cvt = &**(ctl2 as *mut *const IAudioSessionControl2Vtbl);
                    let (mut state, mut pid) = (0i32, 0u32);
                    let _ = (cvt.get_state)(ctl2, &mut state);
                    let _ = (cvt.get_process_id)(ctl2, &mut pid);
                    let system = (cvt.is_system_sounds_session)(ctl2) == 0;
                    if state == 1 /* Active */ && pid != own_pid && pid != 0 && !system {
                        let mut meter: *mut c_void = null_mut();
                        if (uvt.query_interface)(ctl, &IID_IAUDIO_METER_INFORMATION, &mut meter) == 0 && !meter.is_null() {
                            let mtv = &**(meter as *mut *const IAudioMeterInformationVtbl);
                            let mut peak = 0.0f32;
                            let _ = (mtv.get_peak_value)(meter, &mut peak);
                            if peak > 0.000_316 {
                                out.push(OtherSession { pid, peak });
                            }
                            unk_release(meter);
                        }
                    }
                    unk_release(ctl2);
                }
                unk_release(ctl);
            }
            unk_release(en);
        }
        unk_release(mgr);
    }
    out
}

/// Simple blocking player used by the control panel's "Test sound" button.
/// Generates audio through `gen(frame_index, sample_rate) -> (l, r)` for `secs`.
pub fn play_blocking(device_id: Option<&str>, secs: f64, mut gen: impl FnMut(usize, f64) -> (f32, f32)) -> Result<(), String> {
    let _com = ComGuard::mta();
    let mut out = WasapiRender::open(device_id, Engine::Shared, 48000, 0.01)?;
    let rate = out.sample_rate as f64;
    let total = (secs * rate) as usize;
    let mut fifo = std::collections::VecDeque::with_capacity(out.buffer_frames as usize);
    let mut n = 0usize;
    let mut started = false;
    while n < total {
        let pad = out.padding().map_err(|e| format!("device error 0x{:08X}", e))? as usize;
        let space = (out.buffer_frames as usize).saturating_sub(pad + fifo.len());
        for _ in 0..space.min(total - n) {
            fifo.push_back(gen(n, rate));
            n += 1;
        }
        out.write_from(&mut fifo).map_err(|e| format!("write failed 0x{:08X}", e))?;
        if !started {
            out.start();
            started = true;
        }
        out.wait(200);
    }
    // let the tail drain
    for _ in 0..50 {
        if out.padding().unwrap_or(0) == 0 {
            break;
        }
        out.wait(50);
    }
    Ok(())
}


/// Ultra-low latency Hardware WASAPI Input Capture
pub struct WasapiHardwareInput {
    audio_client: *mut c_void,
    capture_client: *mut c_void,
    pub buffer_frames: u32,
    pub sample_rate: u32,
    pub channels: u16,
    mix_format: *mut c_void,
}

unsafe impl Send for WasapiHardwareInput {}

impl WasapiHardwareInput {
    pub fn open(preferred_device_id: Option<&str>) -> Option<Self> {
        unsafe {
            let _ = CoInitialize(null_mut());
            let mut enumerator: *mut c_void = null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                null_mut(),
                1,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if hr != 0 || enumerator.is_null() {
                return None;
            }

            let enum_vtbl = &**(enumerator as *mut *const IMMDeviceEnumeratorVtbl);
            let mut device: *mut c_void = null_mut();

            // Try to open preferred device or fall back to system default capture endpoint
            let mut opened = false;
            if let Some(id) = preferred_device_id {
                if !id.is_empty() {
                    let wide_id: Vec<u16> = id.encode_utf16().chain(core::iter::once(0)).collect();
                    if (enum_vtbl.get_device)(enumerator, wide_id.as_ptr(), &mut device) == 0 && !device.is_null() {
                        opened = true;
                    }
                }
            }

            if !opened {
                let hr = (enum_vtbl.get_default_audio_endpoint)(enumerator, 1, 1, &mut device); // eCapture=1, eMultimedia=1
                if hr != 0 || device.is_null() {
                    let _ = (enum_vtbl.release)(enumerator);
                    return None;
                }
            }

            let dev_vtbl = &**(device as *mut *const IMMDeviceVtbl);
            let mut audio_client: *mut c_void = null_mut();
            let hr = (dev_vtbl.activate)(device, &IID_IAUDIO_CLIENT, 1, null_mut(), &mut audio_client);
            let _ = (dev_vtbl.release)(device);
            let _ = (enum_vtbl.release)(enumerator);

            if hr != 0 || audio_client.is_null() {
                return None;
            }

            let client_vtbl = &**(audio_client as *mut *const IAudioClientVtbl);
            let mut mix_fmt_ptr: *mut c_void = null_mut();
            let hr = (client_vtbl.get_mix_format)(audio_client, &mut mix_fmt_ptr);
            if hr != 0 || mix_fmt_ptr.is_null() {
                let _ = (client_vtbl.release)(audio_client);
                return None;
            }

            let wfx = &*(mix_fmt_ptr as *const WAVEFORMATEX);
            let sample_rate = wfx.n_samples_per_sec;
            let channels = wfx.n_channels;

            let hns_buffer_duration = 200_000i64; // 20ms
            let hr = (client_vtbl.initialize)(
                audio_client,
                0, // AUDCLNT_SHAREMODE_SHARED
                0,
                hns_buffer_duration,
                0,
                mix_fmt_ptr,
                null_mut(),
            );

            if hr != 0 {
                CoTaskMemFree(mix_fmt_ptr);
                let _ = (client_vtbl.release)(audio_client);
                return None;
            }

            let mut buf_frames = 0u32;
            let _ = (client_vtbl.get_buffer_size)(audio_client, &mut buf_frames);

            let mut capture_client: *mut c_void = null_mut();
            let hr = (client_vtbl.get_service)(audio_client, &IID_IAUDIO_CAPTURE_CLIENT, &mut capture_client);
            if hr != 0 || capture_client.is_null() {
                CoTaskMemFree(mix_fmt_ptr);
                let _ = (client_vtbl.release)(audio_client);
                return None;
            }

            let _ = (client_vtbl.start)(audio_client);

            Some(Self {
                audio_client,
                capture_client,
                buffer_frames: buf_frames,
                sample_rate,
                channels,
                mix_format: mix_fmt_ptr,
            })
        }
    }

    /// Read stereo floating point frames from physical microphone/capture device
    #[inline(always)]
    pub fn read_stereo(&mut self, left: *mut f32, right: *mut f32, num_frames: usize, _daw_sample_rate: u32) {
        if self.capture_client.is_null() || self.audio_client.is_null() {
            return;
        }

        unsafe {
            let capture_vtbl = &**(self.capture_client as *mut *const IAudioCaptureClientVtbl);
            let mut packet_size = 0u32;
            while (capture_vtbl.get_next_packet_size)(self.capture_client, &mut packet_size) == 0 && packet_size > 0 {
                let mut buf_ptr: *mut u8 = null_mut();
                let mut num_frames_read = 0u32;
                let mut flags = 0u32;
                let mut dev_pos = 0u64;
                let mut qpc_pos = 0u64;
                if (capture_vtbl.get_buffer)(self.capture_client, &mut buf_ptr, &mut num_frames_read, &mut flags, &mut dev_pos, &mut qpc_pos) == 0 {
                    if !buf_ptr.is_null() && num_frames_read > 0 {
                        let in_slice = buf_ptr as *const f32;
                        let to_copy = (num_frames_read as usize).min(num_frames);
                        for i in 0..to_copy {
                            if self.channels >= 2 {
                                *left.add(i) = *in_slice.add(i * 2);
                                *right.add(i) = *in_slice.add(i * 2 + 1);
                            } else {
                                let mono = *in_slice.add(i);
                                *left.add(i) = mono;
                                *right.add(i) = mono;
                            }
                        }
                    }
                    let _ = (capture_vtbl.release_buffer)(self.capture_client, num_frames_read);
                }
            }
        }
    }
}

impl Drop for WasapiHardwareInput {
    fn drop(&mut self) {
        unsafe {
            if !self.audio_client.is_null() {
                let client_vtbl = &**(self.audio_client as *mut *const IAudioClientVtbl);
                let _ = (client_vtbl.stop)(self.audio_client);
            }
            if !self.capture_client.is_null() {
                let capture_vtbl = &**(self.capture_client as *mut *const IAudioCaptureClientVtbl);
                let _ = (capture_vtbl.release)(self.capture_client);
            }
            if !self.audio_client.is_null() {
                let client_vtbl = &**(self.audio_client as *mut *const IAudioClientVtbl);
                let _ = (client_vtbl.release)(self.audio_client);
            }
            if !self.mix_format.is_null() {
                CoTaskMemFree(self.mix_format);
            }
        }
    }
}

/// Simple Configuration Persisted in %APPDATA%\DolomiteAudio\config.json
#[derive(Clone, Debug, Default)]
pub struct DolomiteConfig {
    pub output_device_id: String,
    pub input_device_id: String,
    pub buffer_size: u32,
    pub sample_rate: u32,
    /// Use Windows 10+ low-latency shared engine periods when the driver supports it.
    pub low_latency: bool,
    /// Take the device in WASAPI exclusive mode (lowest latency, silences other apps).
    pub exclusive: bool,
    /// Exclusive only while the DAW is in front and no other app is playing;
    /// otherwise fall back to shared on the fly.
    pub auto_release: bool,
    /// Record from the input device (ASIO inputs); off = silent inputs, mic untouched.
    pub input_enabled: bool,
}

impl DolomiteConfig {
    pub fn engine(&self) -> Engine {
        if self.exclusive {
            Engine::Exclusive
        } else if self.low_latency {
            Engine::LowLatency
        } else {
            Engine::Shared
        }
    }

    pub fn config_path() -> std::path::PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".into());
        let dir = std::path::PathBuf::from(appdata).join("DolomiteAudio");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut cfg = Self {
                output_device_id: String::new(),
                input_device_id: String::new(),
                buffer_size: 256,
                sample_rate: 48000,
                low_latency: true,
                exclusive: false,
                auto_release: true,
                input_enabled: true,
            };
            for line in content.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("\"output_device\":") {
                    cfg.output_device_id = val.trim().trim_matches('"').trim_matches(',').trim().to_string();
                } else if let Some(val) = line.strip_prefix("\"input_device\":") {
                    cfg.input_device_id = val.trim().trim_matches('"').trim_matches(',').trim().to_string();
                } else if let Some(val) = line.strip_prefix("\"buffer_size\":") {
                    if let Ok(sz) = val.trim().trim_matches(',').parse::<u32>() {
                        cfg.buffer_size = sz;
                    }
                } else if let Some(val) = line.strip_prefix("\"exclusive\":") {
                    cfg.exclusive = val.trim().trim_matches(',').trim().eq_ignore_ascii_case("true");
                } else if let Some(val) = line.strip_prefix("\"auto_release\":") {
                    cfg.auto_release = !val.trim().trim_matches(',').trim().eq_ignore_ascii_case("false");
                } else if let Some(val) = line.strip_prefix("\"input_enabled\":") {
                    cfg.input_enabled = !val.trim().trim_matches(',').trim().eq_ignore_ascii_case("false");
                } else if let Some(val) = line.strip_prefix("\"low_latency\":") {
                    cfg.low_latency = !val.trim().trim_matches(',').trim().eq_ignore_ascii_case("false");
                } else if let Some(val) = line.strip_prefix("\"sample_rate\":") {
                    if let Ok(sr) = val.trim().trim_matches(',').parse::<u32>() {
                        cfg.sample_rate = sr;
                    }
                }
            }
            cfg
        } else {
            Self {
                output_device_id: String::new(),
                input_device_id: String::new(),
                buffer_size: 256,
                sample_rate: 48000,
                low_latency: true,
                exclusive: false,
                auto_release: true,
                input_enabled: true,
            }
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        let content = format!(
            "{{\n  \"output_device\": \"{}\",\n  \"input_device\": \"{}\",\n  \"buffer_size\": {},\n  \"sample_rate\": {},\n  \"low_latency\": {},\n  \"exclusive\": {},\n  \"auto_release\": {},\n  \"input_enabled\": {}\n}}\n",
            self.output_device_id, self.input_device_id, self.buffer_size, self.sample_rate, self.low_latency, self.exclusive, self.auto_release, self.input_enabled
        );
        let _ = std::fs::write(path, content);
    }
}

