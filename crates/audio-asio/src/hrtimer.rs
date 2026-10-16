//! Per-thread high-resolution waits.
//!
//! `timeBeginPeriod(1)` raises the *global* timer interrupt rate, costs
//! power for the whole machine, and since Windows 11 is ignored for
//! processes whose windows are minimised or hidden -- which a DAW often is
//! while it records. A waitable timer created with
//! `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` (Windows 10 1803+) gives ~0.5 ms
//! wake-up precision to this thread only, independent of window state.
//! On older systems we fall back to `timeBeginPeriod` + `Sleep`.

use core::ffi::c_void;
use core::ptr::null_mut;

const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x0000_0002;
const TIMER_ALL_ACCESS: u32 = 0x001F_0003;
const INFINITE: u32 = 0xFFFF_FFFF;

#[link(name = "kernel32")]
extern "system" {
    fn CreateWaitableTimerExW(attr: *mut c_void, name: *const u16, flags: u32, access: u32) -> *mut c_void;
    fn SetWaitableTimer(
        h: *mut c_void,
        due: *const i64,
        period: i32,
        completion: *mut c_void,
        arg: *mut c_void,
        resume: i32,
    ) -> i32;
    fn WaitForSingleObject(h: *mut c_void, ms: u32) -> u32;
    fn CloseHandle(h: *mut c_void) -> i32;
    fn Sleep(ms: u32);
}

#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(p: u32) -> u32;
    fn timeEndPeriod(p: u32) -> u32;
}

/// One per thread (not shared): owns a high-resolution waitable timer, or
/// the legacy global 1 ms timer request when that is unavailable.
pub struct HrTimer {
    handle: *mut c_void,
    legacy_period: bool,
}

impl HrTimer {
    pub fn new() -> Self {
        let handle = unsafe {
            CreateWaitableTimerExW(null_mut(), core::ptr::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS)
        };
        let legacy_period = handle.is_null();
        if legacy_period {
            unsafe { timeBeginPeriod(1) };
        }
        Self { handle, legacy_period }
    }

    /// True when the per-thread high-resolution timer is in use.
    pub fn is_high_resolution(&self) -> bool {
        !self.handle.is_null()
    }

    /// Sleep for `us` microseconds (precision ~0.5 ms with the HR timer).
    pub fn sleep_us(&self, us: u64) {
        if us == 0 {
            return;
        }
        unsafe {
            if !self.handle.is_null() {
                // negative = relative, in 100 ns units
                let due: i64 = -((us as i64) * 10);
                if SetWaitableTimer(self.handle, &due, 0, null_mut(), null_mut(), 0) != 0 {
                    WaitForSingleObject(self.handle, INFINITE);
                    return;
                }
            }
            Sleep(((us + 999) / 1000) as u32);
        }
    }

    pub fn sleep_ms(&self, ms: u32) {
        self.sleep_us(ms as u64 * 1000);
    }
}

impl Default for HrTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HrTimer {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                CloseHandle(self.handle);
            }
            if self.legacy_period {
                timeEndPeriod(1);
            }
        }
    }
}
