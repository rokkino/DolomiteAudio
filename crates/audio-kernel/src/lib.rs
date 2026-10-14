//! Windows Kernel-Level Zero-Latency Audio Driver in Rust (#![no_std]).
//! Exposes physical memory mapping, zero-copy wait-free audio streaming, and P-Core affinity.

#![no_std]
#![allow(non_snake_case)]

pub mod nt;
pub mod device;
pub mod pnp;
pub mod power;
pub mod security;

#[cfg(not(feature = "std"))]
use core::panic::PanicInfo;
use audio_core::ioctl::*;
use crate::nt::*;
use crate::device::AudioDeviceExtension;

static DEVICE_NAME: [u16; 23] = [
    '\\' as u16, 'D' as u16, 'e' as u16, 'v' as u16, 'i' as u16, 'c' as u16, 'e' as u16, '\\' as u16,
    'Z' as u16, 'e' as u16, 'r' as u16, 'o' as u16, 'L' as u16, 'a' as u16, 't' as u16, 'e' as u16,
    'n' as u16, 'c' as u16, 'y' as u16, 'A' as u16, 'u' as u16, 'd' as u16, 0,
];

static DOS_DEVICE_NAME: [u16; 28] = [
    '\\' as u16, 'D' as u16, 'o' as u16, 's' as u16, 'D' as u16, 'e' as u16, 'v' as u16, 'i' as u16,
    'c' as u16, 'e' as u16, 's' as u16, '\\' as u16, 'Z' as u16, 'e' as u16, 'r' as u16, 'o' as u16,
    'L' as u16, 'a' as u16, 't' as u16, 'e' as u16, 'n' as u16, 'c' as u16, 'y' as u16, 'A' as u16,
    'u' as u16, 'd' as u16, 0, 0,
];

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "system" fn DriverEntry(
    driver_object: *mut DRIVER_OBJECT,
    _registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    if driver_object.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    unsafe {
        let mut dev_name = UNICODE_STRING {
            Length: (DEVICE_NAME.len() * 2 - 2) as u16,
            MaximumLength: (DEVICE_NAME.len() * 2) as u16,
            Buffer: DEVICE_NAME.as_ptr() as *mut u16,
        };

        let mut dos_name = UNICODE_STRING {
            Length: (DOS_DEVICE_NAME.len() * 2 - 2) as u16,
            MaximumLength: (DOS_DEVICE_NAME.len() * 2) as u16,
            Buffer: DOS_DEVICE_NAME.as_ptr() as *mut u16,
        };

        let mut device_object: *mut DEVICE_OBJECT = core::ptr::null_mut();

        let status = IoCreateDevice(
            driver_object,
            0,
            &mut dev_name,
            FILE_DEVICE_SOUND,
            0,
            0,
            &mut device_object,
        );

        if status != STATUS_SUCCESS {
            return status;
        }

        let link_status = IoCreateSymbolicLink(&mut dos_name, &mut dev_name);
        if link_status != STATUS_SUCCESS {
            IoDeleteDevice(device_object);
            return link_status;
        }

        // Initialize Audio Device Extension
        match AudioDeviceExtension::new() {
            Ok(ext_ptr) => {
                (*device_object).DeviceExtension = ext_ptr as _;
            }
            Err(err) => {
                IoDeleteSymbolicLink(&mut dos_name);
                IoDeleteDevice(device_object);
                return err;
            }
        }

        (*device_object).Flags |= DO_BUFFERED_IO | DO_POWER_PAGABLE;
        (*device_object).Flags &= !0x00000080; // Clear DO_DEVICE_INITIALIZING

        // Setup driver dispatch table for all major Windows kernel events
        let driver = &mut *driver_object;
        driver.DriverUnload = Some(driver_unload);
        driver.MajorFunction[IRP_MJ_CREATE as usize] = Some(dispatch_create);
        driver.MajorFunction[IRP_MJ_CLOSE as usize] = Some(dispatch_close);
        driver.MajorFunction[IRP_MJ_CLEANUP as usize] = Some(dispatch_cleanup);
        driver.MajorFunction[IRP_MJ_DEVICE_CONTROL as usize] = Some(dispatch_device_control);
        driver.MajorFunction[IRP_MJ_PNP as usize] = Some(pnp::dispatch_pnp);
        driver.MajorFunction[IRP_MJ_POWER as usize] = Some(power::dispatch_power);

        STATUS_SUCCESS
    }
}

extern "system" fn driver_unload(driver_object: *mut DRIVER_OBJECT) {
    unsafe {
        let mut dos_name = UNICODE_STRING {
            Length: (DOS_DEVICE_NAME.len() * 2 - 2) as u16,
            MaximumLength: (DOS_DEVICE_NAME.len() * 2) as u16,
            Buffer: DOS_DEVICE_NAME.as_ptr() as *mut u16,
        };

        let _ = IoDeleteSymbolicLink(&mut dos_name);

        if let Some(driver) = driver_object.as_ref() {
            if let Some(device) = driver.DeviceObject.as_ref() {
                if !device.DeviceExtension.is_null() {
                    let ext = &mut *(device.DeviceExtension as *mut AudioDeviceExtension);
                    ext.destroy();
                }
                IoDeleteDevice(driver.DeviceObject);
            }
        }
    }
}

extern "system" fn dispatch_create(_device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    complete_irp(irp, STATUS_SUCCESS, 0)
}

extern "system" fn dispatch_close(device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    unsafe {
        if let Some(device) = device_object.as_ref() {
            if !device.DeviceExtension.is_null() {
                let ext = &mut *(device.DeviceExtension as *mut AudioDeviceExtension);
                ext.stop_stream();
                ext.unmap_from_user();
            }
        }
    }
    complete_irp(irp, STATUS_SUCCESS, 0)
}

extern "system" fn dispatch_cleanup(_device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    complete_irp(irp, STATUS_SUCCESS, 0)
}

extern "system" fn dispatch_device_control(device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    unsafe {
        let irp_ref = &mut *irp;
        let stack = &*irp_ref.Tail.Overlay.CurrentStackLocation;
        let params = stack.Parameters.DeviceIoControl;
        let ioctl = params.IoControlCode;

        let ext_ptr = (*device_object).DeviceExtension as *mut AudioDeviceExtension;
        if ext_ptr.is_null() {
            return complete_irp(irp, STATUS_UNSUCCESSFUL, 0);
        }
        let ext = &mut *ext_ptr;

        let system_buffer = irp_ref.AssociatedIrp.SystemBuffer;

        match ioctl {
            IOCTL_AUDIO_MAP_SHARED_MEMORY => {
                if params.OutputBufferLength < core::mem::size_of::<MapMemoryResponse>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                match ext.map_to_user() {
                    Ok(va) => {
                        let response = system_buffer as *mut MapMemoryResponse;
                        (*response).user_address = va as u64;
                        (*response).size_bytes = core::mem::size_of::<audio_core::SharedAudioMemory>() as u64;
                        (*response).version = audio_core::PROTOCOL_VERSION;
                        (*response)._reserved = 0;

                        complete_irp(irp, STATUS_SUCCESS, core::mem::size_of::<MapMemoryResponse>())
                    }
                    Err(err) => complete_irp(irp, err, 0),
                }
            }

            IOCTL_AUDIO_UNMAP_SHARED_MEMORY => {
                ext.unmap_from_user();
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            IOCTL_AUDIO_START_STREAMING => {
                let status = ext.start_stream();
                complete_irp(irp, status, 0)
            }

            IOCTL_AUDIO_STOP_STREAMING => {
                let status = ext.stop_stream();
                complete_irp(irp, status, 0)
            }

            IOCTL_AUDIO_SET_CORE_AFFINITY => {
                if params.InputBufferLength < core::mem::size_of::<SetAffinityRequest>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                let req = &*(system_buffer as *const SetAffinityRequest);
                let core_num = req.affinity_mask.trailing_zeros() as u8;
                ext.set_core_affinity(core_num);
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            IOCTL_AUDIO_REGISTER_EVENT => {
                if params.InputBufferLength < core::mem::size_of::<RegisterEventRequest>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                let req = &*(system_buffer as *const RegisterEventRequest);
                let handle = req.event_handle as *mut core::ffi::c_void;

                let mut event_obj: *mut core::ffi::c_void = core::ptr::null_mut();
                let status = ObReferenceObjectByHandle(
                    handle,
                    0x001F0003, // EVENT_ALL_ACCESS
                    core::ptr::null_mut(),
                    UserMode as i8,
                    &mut event_obj,
                    core::ptr::null_mut(),
                );

                if status == STATUS_SUCCESS {
                    if !ext.user_event.is_null() {
                        ObDereferenceObject(ext.user_event);
                    }
                    ext.user_event = event_obj;
                }

                complete_irp(irp, status, 0)
            }

            IOCTL_AUDIO_SET_ROUTING => {
                if params.InputBufferLength < core::mem::size_of::<SetRoutingRequest>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                let req = &*(system_buffer as *const SetRoutingRequest);
                if let Some(shared_mem) = ext.shared_memory.as_mut() {
                    shared_mem.routing_matrix.set_routing(
                        req.input_channel as usize,
                        req.output_channel as usize,
                        req.gain,
                        req.phase_inverted != 0,
                        req.is_muted != 0,
                    );
                }
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            IOCTL_AUDIO_SET_SAMPLE_RATE => {
                if params.InputBufferLength < core::mem::size_of::<SetSampleRateRequest>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                let req = &*(system_buffer as *const SetSampleRateRequest);
                if let Some(shared_mem) = ext.shared_memory.as_mut() {
                    shared_mem.config.sample_rate = req.sample_rate;
                }
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            IOCTL_AUDIO_SET_QUANTUM => {
                if params.InputBufferLength < core::mem::size_of::<SetQuantumRequest>() as u32 {
                    return complete_irp(irp, STATUS_BUFFER_TOO_SMALL, 0);
                }

                let req = &*(system_buffer as *const SetQuantumRequest);
                if let Some(shared_mem) = ext.shared_memory.as_mut() {
                    shared_mem.config.quantum_frames = req.quantum_frames;
                }
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            IOCTL_AUDIO_RESET_TELEMETRY => {
                if let Some(shared_mem) = ext.shared_memory.as_mut() {
                    shared_mem.telemetry = audio_core::DriverTelemetry::new();
                }
                complete_irp(irp, STATUS_SUCCESS, 0)
            }

            _ => complete_irp(irp, STATUS_NOT_IMPLEMENTED, 0),
        }
    }
}

#[inline(always)]
fn complete_irp(irp: *mut IRP, status: NTSTATUS, info: usize) -> NTSTATUS {
    unsafe {
        (*irp).IoStatus.Status = status;
        (*irp).IoStatus.Information = info;
        IoCompleteRequest(irp, IO_NO_INCREMENT);
    }
    status
}
