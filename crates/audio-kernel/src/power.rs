//! Windows Kernel Power Management Handler.
//! Handles sleep, hibernation, and instant D0 wake-up without audio glitch or pop.

use crate::nt::*;
use crate::device::AudioDeviceExtension;

pub extern "system" fn dispatch_power(device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    unsafe {
        let irp_ref = &mut *irp;
        let stack = &*irp_ref.Tail.Overlay.CurrentStackLocation;
        let minor = stack.MinorFunction;

        let ext_ptr = (*device_object).DeviceExtension as *mut AudioDeviceExtension;
        if ext_ptr.is_null() {
            PoStartNextPowerIrp(irp);
            irp_ref.IoStatus.Status = STATUS_UNSUCCESSFUL;
            IoCompleteRequest(irp, IO_NO_INCREMENT);
            return STATUS_UNSUCCESSFUL;
        }
        let ext = &mut *ext_ptr;

        let status = match minor {
            IRP_MN_SET_POWER => {
                let target_state = stack.Parameters.Power.State;
                if target_state == DEVICE_POWER_STATE::PowerDeviceD0 as u32 {
                    // System / Device resuming to D0 (Full Power)
                    ext.power_state = DEVICE_POWER_STATE::PowerDeviceD0;
                    if ext.was_streaming_before_sleep {
                        ext.start_stream();
                        ext.was_streaming_before_sleep = false;
                    }
                } else {
                    // Entering low power (D1, D2, D3 Sleep/Hibernation)
                    if ext.is_streaming {
                        ext.was_streaming_before_sleep = true;
                        ext.stop_stream();
                    }
                    ext.power_state = DEVICE_POWER_STATE::PowerDeviceD3;
                }
                PoSetPowerState(device_object, 1, target_state); // 1 = DevicePowerState
                STATUS_SUCCESS
            }
            IRP_MN_QUERY_POWER => STATUS_SUCCESS,
            _ => STATUS_SUCCESS,
        };

        PoStartNextPowerIrp(irp);
        irp_ref.IoStatus.Status = status;
        IoCompleteRequest(irp, IO_NO_INCREMENT);
        status
    }
}
