//! Security and Input Validation for User-Mode Buffers.
//! Prevents privilege escalation and arbitrary kernel memory corruption.

use crate::nt::*;

/// Maximum valid 64-bit user-mode address on Windows x64 (Canonical user space limit)
pub const MM_HIGHEST_USER_ADDRESS: usize = 0x00007FFFFFFEFFFF;

/// Validate that a buffer pointer and its span reside entirely within user-mode address space
#[inline(always)]
pub fn probe_user_buffer(ptr: *const core::ffi::c_void, len: usize) -> Result<(), NTSTATUS> {
    if ptr.is_null() {
        return Err(STATUS_INVALID_PARAMETER);
    }

    let addr = ptr as usize;

    // Check pointer overflow
    let end_addr = match addr.checked_add(len) {
        Some(end) => end,
        None => return Err(STATUS_ACCESS_DENIED),
    };

    // Ensure entire range is in user space
    if end_addr > MM_HIGHEST_USER_ADDRESS {
        return Err(STATUS_ACCESS_DENIED);
    }

    Ok(())
}

/// Verify alignment requirement
#[inline(always)]
pub fn probe_alignment(ptr: *const core::ffi::c_void, alignment: usize) -> Result<(), NTSTATUS> {
    if ((ptr as usize) & (alignment - 1)) != 0 {
        return Err(STATUS_INVALID_PARAMETER);
    }
    Ok(())
}
