//! Win32 Self-Registration Engine for ASIO COM DLL.
//! Allows regsvr32.exe to register and unregister the ASIO driver directly.
#![allow(dead_code)]

use core::ffi::c_void;

type HKEY = *mut c_void;
type LSTATUS = i32;

const HKEY_LOCAL_MACHINE: HKEY = 0x80000002usize as HKEY;
const HKEY_CURRENT_USER: HKEY = 0x80000001usize as HKEY;
const HKEY_CLASSES_ROOT: HKEY = 0x80000000usize as HKEY;

const KEY_ALL_ACCESS: u32 = 0xF003F;
const REG_SZ: u32 = 1;

#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(
        hKey: HKEY,
        lpSubKey: *const u16,
        Reserved: u32,
        lpClass: *mut u16,
        dwOptions: u32,
        samDesired: u32,
        lpSecurityAttributes: *mut c_void,
        phkResult: *mut HKEY,
        lpdwDisposition: *mut u32,
    ) -> LSTATUS;

    fn RegSetValueExW(
        hKey: HKEY,
        lpValueName: *const u16,
        Reserved: u32,
        dwType: u32,
        lpData: *const u8,
        cbData: u32,
    ) -> LSTATUS;

    fn RegDeleteKeyW(hKey: HKEY, lpSubKey: *const u16) -> LSTATUS;
    fn RegCloseKey(hKey: HKEY) -> LSTATUS;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleFileNameW(hModule: *mut c_void, lpFilename: *mut u16, nSize: u32) -> u32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

pub unsafe fn register_com_server(h_module: *mut c_void) -> i32 {
    let clsid = "{3E8C1234-A789-4B56-9DEF-0123456789AB}";
    let driver_name = "ZeroLatency Rust Kernel ASIO";
    let desc = "ZeroLatency Rust Kernel Audio Driver";

    let mut dll_path = [0u16; 1024];
    let len = GetModuleFileNameW(h_module, dll_path.as_mut_ptr(), 1024);
    if len == 0 {
        return -2147467259; // E_FAIL
    }

    // Try HKLM, fallback to HKCU if not elevated
    let root_keys = [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER];

    for &root in &root_keys {
        // 1. Register ASIO Driver Entry: SOFTWARE\ASIO\<driver_name>
        let asio_sub_key = to_wide(&format!("SOFTWARE\\ASIO\\{}", driver_name));
        let mut h_key: HKEY = core::ptr::null_mut();
        let mut disp = 0u32;

        if RegCreateKeyExW(root, asio_sub_key.as_ptr(), 0, core::ptr::null_mut(), 0, KEY_ALL_ACCESS, core::ptr::null_mut(), &mut h_key, &mut disp) == 0 {
            let clsid_w = to_wide(clsid);
            let desc_w = to_wide(desc);

            let clsid_name = to_wide("CLSID");
            let desc_name = to_wide("Description");

            RegSetValueExW(h_key, clsid_name.as_ptr(), 0, REG_SZ, clsid_w.as_ptr() as _, (clsid_w.len() * 2) as u32);
            RegSetValueExW(h_key, desc_name.as_ptr(), 0, REG_SZ, desc_w.as_ptr() as _, (desc_w.len() * 2) as u32);
            RegCloseKey(h_key);
        }

        // 2. Register COM CLSID: Software\Classes\CLSID\<clsid>\InprocServer32
        let inproc_sub_key = to_wide(&format!("Software\\Classes\\CLSID\\{}\\InprocServer32", clsid));
        let mut h_inproc: HKEY = core::ptr::null_mut();

        if RegCreateKeyExW(root, inproc_sub_key.as_ptr(), 0, core::ptr::null_mut(), 0, KEY_ALL_ACCESS, core::ptr::null_mut(), &mut h_inproc, &mut disp) == 0 {
            let model = to_wide("Both");
            let model_name = to_wide("ThreadingModel");

            // Default value is the DLL path
            RegSetValueExW(h_inproc, core::ptr::null(), 0, REG_SZ, dll_path.as_ptr() as _, (len * 2 + 2) as u32);
            RegSetValueExW(h_inproc, model_name.as_ptr(), 0, REG_SZ, model.as_ptr() as _, (model.len() * 2) as u32);
            RegCloseKey(h_inproc);
        }
    }

    0 // S_OK
}

pub unsafe fn unregister_com_server() -> i32 {
    let clsid = "{3E8C1234-A789-4B56-9DEF-0123456789AB}";
    let driver_name = "ZeroLatency Rust Kernel ASIO";

    let root_keys = [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER];

    for &root in &root_keys {
        let asio_sub_key = to_wide(&format!("SOFTWARE\\ASIO\\{}", driver_name));
        RegDeleteKeyW(root, asio_sub_key.as_ptr());

        let inproc_sub_key = to_wide(&format!("Software\\Classes\\CLSID\\{}\\InprocServer32", clsid));
        RegDeleteKeyW(root, inproc_sub_key.as_ptr());

        let clsid_sub_key = to_wide(&format!("Software\\Classes\\CLSID\\{}", clsid));
        RegDeleteKeyW(root, clsid_sub_key.as_ptr());
    }

    0 // S_OK
}
