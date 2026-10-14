//! Windows Plug and Play (PNP) State Machine Handler.
//! Manages device lifecycle transitions (Start, Stop, QueryRemove, SurpriseRemoval).

use crate::nt::*;
use crate::device::AudioDeviceExtension;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DevicePnpState {
    NotStarted,
    Starting,
    Started,
    StopPending,
    Stopped,
    RemovePending,
    SurpriseRemoved,
    Deleted,
}

pub extern "system" fn dispatch_pnp(device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    unsafe {
        let irp_ref = &mut *irp;
        let stack = &*irp_ref.Tail.Overlay.CurrentStackLocation;
        let minor = stack.MinorFunction;

        let ext_ptr = (*device_object).DeviceExtension as *mut AudioDeviceExtension;
        if ext_ptr.is_null() {
            irp_ref.IoStatus.Status = STATUS_UNSUCCESSFUL;
            IoCompleteRequest(irp, IO_NO_INCREMENT);
            return STATUS_UNSUCCESSFUL;
        }
        let ext = &mut *ext_ptr;

        let status = match minor {
            IRP_MN_START_DEVICE => {
                ext.pnp_state = DevicePnpState::Started;
                STATUS_SUCCESS
            }
            IRP_MN_QUERY_STOP_DEVICE => {
                ext.pnp_state = DevicePnpState::StopPending;
                STATUS_SUCCESS
            }
            IRP_MN_CANCEL_STOP_DEVICE => {
                ext.pnp_state = DevicePnpState::Started;
                STATUS_SUCCESS
            }
            IRP_MN_STOP_DEVICE => {
                ext.stop_stream();
                ext.pnp_state = DevicePnpState::Stopped;
                STATUS_SUCCESS
            }
            IRP_MN_QUERY_REMOVE_DEVICE => {
                ext.pnp_state = DevicePnpState::RemovePending;
                STATUS_SUCCESS
            }
            IRP_MN_CANCEL_REMOVE_DEVICE => {
                ext.pnp_state = DevicePnpState::Started;
                STATUS_SUCCESS
            }
            IRP_MN_SURPRISE_REMOVAL => {
                ext.stop_stream();
                ext.pnp_state = DevicePnpState::SurpriseRemoved;
                STATUS_SUCCESS
            }
            IRP_MN_REMOVE_DEVICE => {
                ext.stop_stream();
                ext.destroy();
                ext.pnp_state = DevicePnpState::Deleted;
                IoDeleteDevice(device_object);
                STATUS_SUCCESS
            }
            _ => STATUS_SUCCESS,
        };

        irp_ref.IoStatus.Status = status;
        IoCompleteRequest(irp, IO_NO_INCREMENT);
        status
    }
}
