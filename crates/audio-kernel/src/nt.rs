//! Complete Windows Kernel NT Definitions and Extern FFI Bindings.
//! Provides pure no_std direct bindings to ntoskrnl.exe and hal.dll for high-performance driver operation.

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

pub type NTSTATUS = i32;
pub const STATUS_SUCCESS: NTSTATUS = 0x00000000;
pub const STATUS_UNSUCCESSFUL: NTSTATUS = -1073741823; // 0xC0000001
pub const STATUS_NOT_IMPLEMENTED: NTSTATUS = -1073741822; // 0xC0000002
pub const STATUS_INVALID_PARAMETER: NTSTATUS = -1073741811; // 0xC000000D
pub const STATUS_INSUFFICIENT_RESOURCES: NTSTATUS = -1073741670; // 0xC000009A
pub const STATUS_BUFFER_TOO_SMALL: NTSTATUS = -1073741789; // 0xC0000023
pub const STATUS_ACCESS_DENIED: NTSTATUS = -1073741790; // 0xC0000022
pub const STATUS_DELETE_PENDING: NTSTATUS = -1073741738; // 0xC0000056

// Major Function Codes
pub const IRP_MJ_CREATE: u8 = 0x00;
pub const IRP_MJ_CLOSE: u8 = 0x02;
pub const IRP_MJ_DEVICE_CONTROL: u8 = 0x0e;
pub const IRP_MJ_CLEANUP: u8 = 0x12;
pub const IRP_MJ_POWER: u8 = 0x16;
pub const IRP_MJ_PNP: u8 = 0x1b;

// PNP Minor Function Codes
pub const IRP_MN_START_DEVICE: u8 = 0x00;
pub const IRP_MN_QUERY_REMOVE_DEVICE: u8 = 0x01;
pub const IRP_MN_REMOVE_DEVICE: u8 = 0x02;
pub const IRP_MN_CANCEL_REMOVE_DEVICE: u8 = 0x03;
pub const IRP_MN_STOP_DEVICE: u8 = 0x04;
pub const IRP_MN_QUERY_STOP_DEVICE: u8 = 0x05;
pub const IRP_MN_CANCEL_STOP_DEVICE: u8 = 0x06;
pub const IRP_MN_SURPRISE_REMOVAL: u8 = 0x17;

// Power Minor Function Codes
pub const IRP_MN_WAIT_WAKE: u8 = 0x00;
pub const IRP_MN_POWER_SEQUENCE: u8 = 0x01;
pub const IRP_MN_SET_POWER: u8 = 0x02;
pub const IRP_MN_QUERY_POWER: u8 = 0x03;

pub const FILE_DEVICE_UNKNOWN: u32 = 0x00000022;
pub const FILE_DEVICE_SOUND: u32 = 0x0000001d;
pub const DO_BUFFERED_IO: u32 = 0x00000004;
pub const DO_DIRECT_IO: u32 = 0x00000010;
pub const DO_POWER_PAGABLE: u32 = 0x00002000;
pub const IO_NO_INCREMENT: i8 = 0;

pub const POOL_FLAG_NON_PAGED: u64 = 0x0000000000000040;

pub const UserMode: u8 = 1;
pub const KernelMode: u8 = 0;

pub type KSPIN_LOCK = usize;
pub type KIRQL = u8;

#[repr(C)]
pub enum MEMORY_CACHING_TYPE {
    MmNonCached = 0,
    MmCached = 1,
    MmWriteCombined = 2,
    MmHardwareCoherentCached = 3,
    MmNonCachedUnordered = 4,
    MmUSWCCached = 5,
    MmMaximumCacheType = 6,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DEVICE_POWER_STATE {
    PowerDeviceUnspecified = 0,
    PowerDeviceD0 = 1,
    PowerDeviceD1 = 2,
    PowerDeviceD2 = 3,
    PowerDeviceD3 = 4,
    PowerDeviceMaximum = 5,
}

#[repr(C)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *mut u16,
}

#[repr(C)]
pub struct DRIVER_OBJECT {
    pub Type: i16,
    pub Size: i16,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub Flags: u32,
    pub DriverStart: *mut core::ffi::c_void,
    pub DriverSize: u32,
    pub DriverSection: *mut core::ffi::c_void,
    pub DriverExtension: *mut core::ffi::c_void,
    pub DriverName: UNICODE_STRING,
    pub HardwareDatabase: *mut UNICODE_STRING,
    pub FastIoDispatch: *mut core::ffi::c_void,
    pub DriverInit: *mut core::ffi::c_void,
    pub DriverStartIo: *mut core::ffi::c_void,
    pub DriverUnload: Option<extern "system" fn(*mut DRIVER_OBJECT)>,
    pub MajorFunction: [Option<extern "system" fn(*mut DEVICE_OBJECT, *mut IRP) -> NTSTATUS>; 28],
}

#[repr(C)]
pub struct DEVICE_OBJECT {
    pub Type: i16,
    pub Size: u16,
    pub ReferenceCount: i32,
    pub DriverObject: *mut DRIVER_OBJECT,
    pub NextDevice: *mut DEVICE_OBJECT,
    pub AttachedDevice: *mut DEVICE_OBJECT,
    pub CurrentIrp: *mut IRP,
    pub Timer: *mut core::ffi::c_void,
    pub Flags: u32,
    pub Characteristics: u32,
    pub Vpb: *mut core::ffi::c_void,
    pub DeviceExtension: *mut core::ffi::c_void,
    pub DeviceType: u32,
    pub StackSize: i8,
    pub Queue: [u8; 40],
    pub AlignmentRequirement: u32,
    pub DeviceQueue: [u8; 32],
    pub Dpc: [u8; 64],
    pub ActiveThreadCount: u32,
    pub SecurityDescriptor: *mut core::ffi::c_void,
    pub DeviceLock: [u8; 24],
    pub SectorSize: u16,
    pub Spare1: u16,
    pub DeviceObjectExtension: *mut core::ffi::c_void,
    pub Reserved: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct IO_STATUS_BLOCK {
    pub Status: NTSTATUS,
    pub Information: usize,
}

#[repr(C)]
pub struct IRP {
    pub Type: i16,
    pub Size: u16,
    pub MdlAddress: *mut MDL,
    pub Flags: u32,
    pub AssociatedIrp: AssociatedIrpUnion,
    pub ThreadListEntry: [usize; 2],
    pub IoStatus: IO_STATUS_BLOCK,
    pub RequestorMode: i8,
    pub PendingReturned: u8,
    pub StackCount: i8,
    pub CurrentLocation: i8,
    pub Cancel: u8,
    pub CancelIrql: u8,
    pub ApcEnvironment: i8,
    pub AllocationFlags: u8,
    pub UserIosb: *mut IO_STATUS_BLOCK,
    pub UserEvent: *mut core::ffi::c_void,
    pub Overlay: [usize; 2],
    pub CancelRoutine: *mut core::ffi::c_void,
    pub UserBuffer: *mut core::ffi::c_void,
    pub Tail: IrpTail,
}

#[repr(C)]
pub union AssociatedIrpUnion {
    pub MasterIrp: *mut IRP,
    pub IrpCount: i32,
    pub SystemBuffer: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct IrpTail {
    pub Overlay: IrpTailOverlay,
}

#[repr(C)]
pub struct IrpTailOverlay {
    pub DriverContext: [*mut core::ffi::c_void; 4],
    pub Thread: *mut core::ffi::c_void,
    pub AuxBuffer: *mut core::ffi::c_void,
    pub ListEntry: [usize; 2],
    pub CurrentStackLocation: *mut IO_STACK_LOCATION,
    pub OriginalFileObject: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct IO_STACK_LOCATION {
    pub MajorFunction: u8,
    pub MinorFunction: u8,
    pub Flags: u8,
    pub Control: u8,
    pub Parameters: IO_STACK_LOCATION_PARAMETERS,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub FileObject: *mut core::ffi::c_void,
    pub CompletionRoutine: *mut core::ffi::c_void,
    pub Context: *mut core::ffi::c_void,
}

#[repr(C)]
pub union IO_STACK_LOCATION_PARAMETERS {
    pub DeviceIoControl: DeviceIoControlParameters,
    pub Power: PowerParameters,
    pub Others: [usize; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DeviceIoControlParameters {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub Type3InputBuffer: *mut core::ffi::c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PowerParameters {
    pub SystemContext: u32,
    pub Type: u32,
    pub State: u32,
    pub ShutdownType: u32,
}

#[repr(C)]
pub struct MDL {
    pub Next: *mut MDL,
    pub Size: i16,
    pub MdlFlags: i16,
    pub Process: *mut core::ffi::c_void,
    pub MappedSystemVa: *mut core::ffi::c_void,
    pub StartVa: *mut core::ffi::c_void,
    pub ByteOffset: u32,
    pub ByteCount: u32,
}

#[repr(C)]
pub struct KDPC {
    pub Type: u8,
    pub Importance: u8,
    pub Number: u16,
    pub DpcListEntry: [usize; 2],
    pub DeferredRoutine: Option<extern "system" fn(*mut KDPC, *mut core::ffi::c_void, *mut core::ffi::c_void, *mut core::ffi::c_void)>,
    pub DeferredContext: *mut core::ffi::c_void,
    pub SystemArgument1: *mut core::ffi::c_void,
    pub SystemArgument2: *mut core::ffi::c_void,
    pub DpcData: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct KTIMER {
    pub Header: [u8; 24],
    pub DueTime: i64,
    pub TimerListEntry: [usize; 2],
    pub Dpc: *mut KDPC,
    pub Period: u32,
}

extern "system" {
    pub fn IoCreateDevice(
        DriverObject: *mut DRIVER_OBJECT,
        DeviceExtensionSize: u32,
        DeviceName: *mut UNICODE_STRING,
        DeviceType: u32,
        DeviceCharacteristics: u32,
        Exclusive: u8,
        DeviceObject: *mut *mut DEVICE_OBJECT,
    ) -> NTSTATUS;

    pub fn IoDeleteDevice(DeviceObject: *mut DEVICE_OBJECT);

    pub fn IoCreateSymbolicLink(
        SymbolicLinkName: *mut UNICODE_STRING,
        DeviceName: *mut UNICODE_STRING,
    ) -> NTSTATUS;

    pub fn IoDeleteSymbolicLink(SymbolicLinkName: *mut UNICODE_STRING) -> NTSTATUS;

    pub fn IoCompleteRequest(Irp: *mut IRP, PriorityBoost: i8);

    pub fn ExAllocatePool2(Flags: u64, NumberOfBytes: usize, Tag: u32) -> *mut core::ffi::c_void;
    pub fn ExFreePoolWithTag(P: *mut core::ffi::c_void, Tag: u32);

    pub fn IoAllocateMdl(
        VirtualAddress: *mut core::ffi::c_void,
        Length: u32,
        SecondaryBuffer: u8,
        ChargeQuota: u8,
        Irp: *mut IRP,
    ) -> *mut MDL;

    pub fn IoFreeMdl(Mdl: *mut MDL);

    pub fn MmBuildMdlForNonPagedPool(MemoryDescriptorList: *mut MDL);

    pub fn MmMapLockedPagesSpecifyCache(
        MemoryDescriptorList: *mut MDL,
        AccessMode: u8,
        CacheType: MEMORY_CACHING_TYPE,
        RequestedAddress: *mut core::ffi::c_void,
        BugCheckOnFailure: u32,
        Priority: u32,
    ) -> *mut core::ffi::c_void;

    pub fn MmUnmapLockedPages(BaseAddress: *mut core::ffi::c_void, MemoryDescriptorList: *mut MDL);

    pub fn KeInitializeDpc(
        Dpc: *mut KDPC,
        DeferredRoutine: Option<extern "system" fn(*mut KDPC, *mut core::ffi::c_void, *mut core::ffi::c_void, *mut core::ffi::c_void)>,
        DeferredContext: *mut core::ffi::c_void,
    );

    pub fn KeSetTargetProcessorDpc(Dpc: *mut KDPC, Number: u8);

    pub fn KeInitializeTimerEx(Timer: *mut KTIMER, Type: u32);

    pub fn KeSetTimerEx(
        Timer: *mut KTIMER,
        DueTime: i64,
        Period: i32,
        Dpc: *mut KDPC,
    ) -> u8;

    pub fn KeCancelTimer(Timer: *mut KTIMER) -> u8;

    pub fn KeInitializeSpinLock(SpinLock: *mut KSPIN_LOCK);
    pub fn KeAcquireSpinLockRaiseToDpc(SpinLock: *mut KSPIN_LOCK) -> KIRQL;
    pub fn KeReleaseSpinLock(SpinLock: *mut KSPIN_LOCK, NewIrql: KIRQL);

    pub fn KeQueryPerformanceCounter(PerformanceFreq: *mut i64) -> i64;

    pub fn ObReferenceObjectByHandle(
        Handle: *mut core::ffi::c_void,
        DesiredAccess: u32,
        ObjectType: *mut core::ffi::c_void,
        AccessMode: i8,
        Object: *mut *mut core::ffi::c_void,
        HandleInformation: *mut core::ffi::c_void,
    ) -> NTSTATUS;

    pub fn ObDereferenceObject(Object: *mut core::ffi::c_void);

    pub fn KeSetEvent(
        Event: *mut core::ffi::c_void,
        Increment: i8,
        Wait: u8,
    ) -> i32;

    pub fn PoSetPowerState(
        DeviceObject: *mut DEVICE_OBJECT,
        StateType: u32,
        State: u32,
    ) -> u32;

    pub fn PoStartNextPowerIrp(Irp: *mut IRP);

    pub fn DbgPrint(Format: *const u8, ...) -> u32;
}
