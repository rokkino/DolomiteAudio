//! User-Mode Kernel Driver Client.
//! Communicates with \\.\ZeroLatencyAudio via direct IOCTL and zero-copy mapped memory.
#![allow(dead_code)]

use std::ptr::null_mut;
use audio_core::{
    SharedAudioMemory, AudioStreamConfig,
    IOCTL_AUDIO_MAP_SHARED_MEMORY, IOCTL_AUDIO_UNMAP_SHARED_MEMORY,
    IOCTL_AUDIO_START_STREAMING, IOCTL_AUDIO_STOP_STREAMING,
    IOCTL_AUDIO_SET_CORE_AFFINITY, IOCTL_AUDIO_REGISTER_EVENT,
    MapMemoryResponse, SetAffinityRequest, RegisterEventRequest,
};

pub struct KernelAudioClient {
    device_handle: *mut core::ffi::c_void,
    shared_memory: *mut SharedAudioMemory,
    mapped_size: usize,
    event_handle: *mut core::ffi::c_void,
    is_kernel_connected: bool,
    fallback_memory: Option<Box<SharedAudioMemory>>,
}

// Windows API types and externs
type HANDLE = *mut core::ffi::c_void;
type BOOL = i32;
type DWORD = u32;

const GENERIC_READ: DWORD = 0x80000000;
const GENERIC_WRITE: DWORD = 0x40000000;
const OPEN_EXISTING: DWORD = 3;
const FILE_ATTRIBUTE_NORMAL: DWORD = 0x00000080;
const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *mut core::ffi::c_void,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;

    fn DeviceIoControl(
        hDevice: HANDLE,
        dwIoControlCode: DWORD,
        lpInBuffer: *const core::ffi::c_void,
        nInBufferSize: DWORD,
        lpOutBuffer: *mut core::ffi::c_void,
        nOutBufferSize: DWORD,
        lpBytesReturned: *mut DWORD,
        lpOverlapped: *mut core::ffi::c_void,
    ) -> BOOL;

    fn CloseHandle(hObject: HANDLE) -> BOOL;
    fn CreateEventW(
        lpEventAttributes: *mut core::ffi::c_void,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
    fn SetEvent(hEvent: HANDLE) -> BOOL;
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
}

static DEVICE_PATH: [u16; 23] = [
    '\\' as u16, '\\' as u16, '.' as u16, '\\' as u16,
    'Z' as u16, 'e' as u16, 'r' as u16, 'o' as u16, 'L' as u16, 'a' as u16, 't' as u16, 'e' as u16,
    'n' as u16, 'c' as u16, 'y' as u16, 'A' as u16, 'u' as u16, 'd' as u16, 'i' as u16, 'o' as u16,
    0, 0, 0,
];

impl KernelAudioClient {
    pub fn new() -> Self {
        let mut client = Self {
            device_handle: INVALID_HANDLE_VALUE,
            shared_memory: null_mut(),
            mapped_size: 0,
            event_handle: null_mut(),
            is_kernel_connected: false,
            fallback_memory: None,
        };

        // Try to connect to kernel driver
        unsafe {
            let h_device = CreateFileW(
                DEVICE_PATH.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            );

            if h_device != INVALID_HANDLE_VALUE {
                client.device_handle = h_device;
                client.is_kernel_connected = true;

                // Map physical non-paged kernel pool directly to user virtual address
                let mut resp = MapMemoryResponse {
                    user_address: 0,
                    size_bytes: 0,
                    version: 0,
                    _reserved: 0,
                };
                let mut bytes_returned: DWORD = 0;

                let success = DeviceIoControl(
                    h_device,
                    IOCTL_AUDIO_MAP_SHARED_MEMORY,
                    null_mut(),
                    0,
                    &mut resp as *mut _ as _,
                    core::mem::size_of::<MapMemoryResponse>() as DWORD,
                    &mut bytes_returned,
                    null_mut(),
                );

                if success != 0 && resp.user_address != 0 {
                    client.shared_memory = resp.user_address as *mut SharedAudioMemory;
                    client.mapped_size = resp.size_bytes as usize;

                    // Create event and register with kernel
                    let ev = CreateEventW(null_mut(), 0, 0, null_mut());
                    if ev != null_mut() {
                        client.event_handle = ev;
                        let mut reg = RegisterEventRequest {
                            event_handle: ev as u64,
                        };
                        let mut ret: DWORD = 0;
                        let _ = DeviceIoControl(
                            h_device,
                            IOCTL_AUDIO_REGISTER_EVENT,
                            &mut reg as *mut _ as _,
                            core::mem::size_of::<RegisterEventRequest>() as DWORD,
                            null_mut(),
                            0,
                            &mut ret,
                            null_mut(),
                        );
                    }
                }
            } else {
                // Fallback zero-copy user-space memory engine
                let mem = SharedAudioMemory::heap_boxed(AudioStreamConfig::DEFAULT);
                let mem_ptr = Box::into_raw(mem);
                client.shared_memory = mem_ptr;
                client.event_handle = CreateEventW(null_mut(), 0, 0, null_mut());
            }
        }

        client
    }

    pub fn is_connected_to_kernel(&self) -> bool {
        self.is_kernel_connected
    }

    pub fn shared_memory(&self) -> Option<&SharedAudioMemory> {
        if self.shared_memory.is_null() {
            None
        } else {
            unsafe { Some(&*self.shared_memory) }
        }
    }

    pub fn shared_memory_mut(&mut self) -> Option<&mut SharedAudioMemory> {
        if self.shared_memory.is_null() {
            None
        } else {
            unsafe { Some(&mut *self.shared_memory) }
        }
    }

    pub fn event_handle(&self) -> *mut core::ffi::c_void {
        self.event_handle
    }

    pub fn start_streaming(&mut self) -> bool {
        if self.is_kernel_connected {
            unsafe {
                let mut ret: DWORD = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_START_STREAMING,
                    null_mut(),
                    0,
                    null_mut(),
                    0,
                    &mut ret,
                    null_mut(),
                ) != 0
            }
        } else {
            true
        }
    }

    pub fn stop_streaming(&mut self) -> bool {
        if self.is_kernel_connected {
            unsafe {
                let mut ret: DWORD = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_STOP_STREAMING,
                    null_mut(),
                    0,
                    null_mut(),
                    0,
                    &mut ret,
                    null_mut(),
                ) != 0
            }
        } else {
            true
        }
    }

    pub fn set_affinity(&mut self, mask: u64) -> bool {
        if self.is_kernel_connected {
            unsafe {
                let mut req = SetAffinityRequest { affinity_mask: mask };
                let mut ret: DWORD = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_SET_CORE_AFFINITY,
                    &mut req as *mut _ as _,
                    core::mem::size_of::<SetAffinityRequest>() as DWORD,
                    null_mut(),
                    0,
                    &mut ret,
                    null_mut(),
                ) != 0
            }
        } else {
            true
        }
    }
}

impl Drop for KernelAudioClient {
    fn drop(&mut self) {
        self.stop_streaming();
        unsafe {
            if self.is_kernel_connected {
                let mut ret: DWORD = 0;
                DeviceIoControl(
                    self.device_handle,
                    IOCTL_AUDIO_UNMAP_SHARED_MEMORY,
                    null_mut(),
                    0,
                    null_mut(),
                    0,
                    &mut ret,
                    null_mut(),
                );
                CloseHandle(self.device_handle);
            } else if !self.shared_memory.is_null() {
                let _ = Box::from_raw(self.shared_memory);
            }

            if !self.event_handle.is_null() {
                CloseHandle(self.event_handle);
            }
        }
    }
}
