//! Steinberg ASIO COM DLL In-Process Server.
//! Exposes DllGetClassObject, DllRegisterServer, and DllUnregisterServer for Windows DAWs.

pub mod asio_types;
pub mod kernel_client;
pub mod asio_driver;
pub mod com_interface;
pub mod registry;
pub mod wasapi;
pub mod live_status;
pub mod hrtimer;

use core::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};
use crate::com_interface::{IASIO, CLSID_ZERO_LATENCY_AUDIO};

type HRESULT = i32;
const S_OK: HRESULT = 0;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231;
const E_POINTER: HRESULT = -2147467261;

static MODULE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(core::ptr::null_mut());

#[no_mangle]
pub extern "system" fn DllMain(
    h_inst_dll: *mut c_void,
    fdw_reason: u32,
    _lpv_reserved: *mut c_void,
) -> i32 {
    if fdw_reason == 1 {
        // DLL_PROCESS_ATTACH
        MODULE_HANDLE.store(h_inst_dll, Ordering::SeqCst);
    }
    1 // TRUE
}

// IClassFactory COM Interface
#[repr(C)]
pub struct IClassFactoryVtbl {
    pub query_interface: extern "system" fn(*mut IClassFactory, *const [u8; 16], *mut *mut c_void) -> HRESULT,
    pub add_ref: extern "system" fn(*mut IClassFactory) -> u32,
    pub release: extern "system" fn(*mut IClassFactory) -> u32,
    pub create_instance: extern "system" fn(*mut IClassFactory, *mut c_void, *const [u8; 16], *mut *mut c_void) -> HRESULT,
    pub lock_server: extern "system" fn(*mut IClassFactory, i32) -> HRESULT,
}

#[repr(C)]
pub struct IClassFactory {
    pub vtbl: *const IClassFactoryVtbl,
}

static FACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    query_interface: factory_query_interface,
    add_ref: factory_add_ref,
    release: factory_release,
    create_instance: factory_create_instance,
    lock_server: factory_lock_server,
};

static mut GLOBAL_FACTORY: IClassFactory = IClassFactory {
    vtbl: &FACTORY_VTBL,
};

extern "system" fn factory_query_interface(
    this: *mut IClassFactory,
    _riid: *const [u8; 16],
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    unsafe {
        *ppv = this as *mut c_void;
    }
    S_OK
}

extern "system" fn factory_add_ref(_this: *mut IClassFactory) -> u32 {
    2
}

extern "system" fn factory_release(_this: *mut IClassFactory) -> u32 {
    1
}

extern "system" fn factory_create_instance(
    _this: *mut IClassFactory,
    _p_unk_outer: *mut c_void,
    _riid: *const [u8; 16],
    ppv_object: *mut *mut c_void,
) -> HRESULT {
    if ppv_object.is_null() {
        return E_POINTER;
    }

    if !_p_unk_outer.is_null() {
        return -2147221232; // CLASS_E_NOAGGREGATION
    }
    let asio_instance = IASIO::new_boxed();

    unsafe {
        *ppv_object = Box::into_raw(asio_instance) as *mut c_void;
    }
    S_OK
}

extern "system" fn factory_lock_server(_this: *mut IClassFactory, _f_lock: i32) -> HRESULT {
    S_OK
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleFileNameW(h: *mut c_void, buf: *mut u16, size: u32) -> u32;
}

/// Directory containing audio_asio.dll (used to find audio-control.exe).
pub fn module_dir() -> Option<std::path::PathBuf> {
    let h = MODULE_HANDLE.load(Ordering::SeqCst);
    if h.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameW(h, buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    let path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    path.parent().map(|p| p.to_path_buf())
}

#[no_mangle]
pub extern "system" fn DllGetClassObject(
    rclsid: *const [u8; 16],
    _riid: *const [u8; 16],
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }

    unsafe {
        if *rclsid == CLSID_ZERO_LATENCY_AUDIO {
            *ppv = &raw mut GLOBAL_FACTORY as *mut _ as *mut c_void;
            S_OK
        } else {
            CLASS_E_CLASSNOTAVAILABLE
        }
    }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_OK
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    let h_mod = MODULE_HANDLE.load(Ordering::SeqCst);
    unsafe { registry::register_com_server(h_mod) }
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    unsafe { registry::unregister_com_server() }
}
