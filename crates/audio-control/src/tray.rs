//! Notification-area (system tray) icon and flyout panel.
//!
//! * One resident `audio-control.exe` per user session (named mutex). Running
//!   it again - from Studio One's "Control Panel" button, the Start menu or the
//!   tray autostart - just shows the existing panel.
//! * Left click toggles a small panel docked above the clock; it hides itself
//!   when you click elsewhere (unless pinned). Right click: menu.
//! * The panel is an Edge `--app` window that is *hidden*, not closed, so it
//!   re-opens instantly; while hidden the page stops its live stream.
//! * The icon and its tooltip show the live engine state (exclusive / shared /
//!   idle) and the current latency.
//! * Re-adds itself if Explorer restarts (TaskbarCreated).

#![allow(clashing_extern_declarations)]

use core::ffi::c_void;
use core::ptr::{null, null_mut};
use std::cell::RefCell;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use audio_asio::live_status::LiveStatusMap;

type HWND = *mut c_void;

#[repr(C)]
#[derive(Copy, Clone)]
struct GUID {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

#[repr(C)]
struct NOTIFYICONDATAW {
    cb_size: u32,
    hwnd: HWND,
    uid: u32,
    flags: u32,
    callback_message: u32,
    icon: *mut c_void,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    timeout_or_version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid_item: GUID,
    balloon_icon: *mut c_void,
}

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: u32,
    style: u32,
    wnd_proc: extern "system" fn(HWND, u32, usize, isize) -> isize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: *mut c_void,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu_name: *const u16,
    class_name: *const u16,
    icon_sm: *mut c_void,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: POINT,
    private: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MONITORINFO {
    cb_size: u32,
    monitor: RECT,
    work: RECT,
    flags: u32,
}

#[repr(C)]
struct ICONINFO {
    is_icon: i32,
    x_hotspot: u32,
    y_hotspot: u32,
    mask: *mut c_void,
    color: *mut c_void,
}

#[repr(C)]
struct BITMAPINFOHEADER {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(c: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: *mut c_void, inst: *mut c_void, param: *mut c_void) -> HWND;
    fn DefWindowProcW(h: HWND, m: u32, w: usize, l: isize) -> isize;
    fn GetMessageW(msg: *mut MSG, h: HWND, a: u32, b: u32) -> i32;
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> isize;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(h: HWND, m: u32, w: usize, l: isize) -> i32;
    fn FindWindowW(class: *const u16, title: *const u16) -> HWND;
    fn IsWindow(h: HWND) -> i32;
    fn IsWindowVisible(h: HWND) -> i32;
    fn ShowWindow(h: HWND, cmd: i32) -> i32;
    fn SetForegroundWindow(h: HWND) -> i32;
    fn GetForegroundWindow() -> HWND;
    fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetWindowLongPtrW(h: HWND, idx: i32) -> isize;
    fn SetWindowLongPtrW(h: HWND, idx: i32, v: isize) -> isize;
    fn SetTimer(h: HWND, id: usize, ms: u32, f: *const c_void) -> usize;
    fn RegisterWindowMessageW(s: *const u16) -> u32;
    fn CreatePopupMenu() -> *mut c_void;
    fn AppendMenuW(m: *mut c_void, flags: u32, id: usize, text: *const u16) -> i32;
    fn TrackPopupMenu(m: *mut c_void, flags: u32, x: i32, y: i32, r: i32, h: HWND, rc: *const RECT) -> i32;
    fn DestroyMenu(m: *mut c_void) -> i32;
    fn GetCursorPos(p: *mut POINT) -> i32;
    fn MonitorFromPoint(p: POINT, flags: u32) -> *mut c_void;
    fn GetMonitorInfoW(m: *mut c_void, mi: *mut MONITORINFO) -> i32;
    fn GetDpiForWindow(h: HWND) -> u32;
    fn CreateIconIndirect(ii: *const ICONINFO) -> *mut c_void;
    fn SetProcessDPIAware() -> i32;
    fn AllowSetForegroundWindow(pid: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateDIBSection(hdc: *mut c_void, bmi: *const BITMAPINFOHEADER, usage: u32, bits: *mut *mut c_void, section: *mut c_void, offset: u32) -> *mut c_void;
    fn CreateBitmap(w: i32, h: i32, planes: u32, bpp: u32, bits: *const c_void) -> *mut c_void;
    fn DeleteObject(o: *mut c_void) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(msg: u32, data: *mut NOTIFYICONDATAW) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(sa: *mut c_void, owner: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(m: *mut c_void, name: *const u8) -> *mut c_void;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(key: isize, sub: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
    fn RegSetValueExW(key: isize, name: *const u16, res: u32, ty: u32, data: *const u8, len: u32) -> i32;
    fn RegDeleteValueW(key: isize, name: *const u16) -> i32;
    fn RegQueryValueExW(key: isize, name: *const u16, res: *mut u32, ty: *mut u32, data: *mut u8, len: *mut u32) -> i32;
    fn RegCloseKey(key: isize) -> i32;
}

const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_NULL: u32 = 0x0000;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_APP: u32 = 0x8000;
const WM_TRAY: u32 = WM_APP + 1;
const WM_SHOW_PANEL: u32 = WM_APP + 2;

const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 1;
const NIF_ICON: u32 = 2;
const NIF_TIP: u32 = 4;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x80;
const WS_EX_APPWINDOW: isize = 0x40000;
const HWND_TOPMOST: HWND = -1isize as HWND;
const SWP_FRAMECHANGED: u32 = 0x20;

const MF_STRING: u32 = 0;
const MF_CHECKED: u32 = 8;
const MF_SEPARATOR: u32 = 0x800;
const TPM_RIGHTBUTTON: u32 = 0x2;
const TPM_BOTTOMALIGN: u32 = 0x20;
const TPM_RETURNCMD: u32 = 0x100;

const CMD_OPEN: usize = 1;
const CMD_AUTOSTART: usize = 2;
const CMD_TEST: usize = 3;
const CMD_EXIT: usize = 9;

const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "DolomiteAudio";

pub const TRAY_CLASS: &str = "DolomiteTrayWnd";
const PANEL_TITLE: &str = "Dolomite Audio";
const PANEL_W: i32 = 372; // DIPs
const PANEL_H: i32 = 640;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

fn tip_text(s: &str) -> [u16; 128] {
    let mut t = [0u16; 128];
    for (i, c) in s.encode_utf16().take(127).enumerate() {
        t[i] = c;
    }
    t
}

// ---------------------------------------------------------------------------
// Single instance
// ---------------------------------------------------------------------------

/// Returns false if another tray instance already runs (and pokes it to
/// show its panel when `show` is set).
pub fn claim_single_instance(show: bool) -> bool {
    unsafe {
        let _m = CreateMutexW(null_mut(), 0, wide("Local\\DolomiteAudioTray").as_ptr());
        if GetLastError() == 183 /* ERROR_ALREADY_EXISTS */ {
            if show {
                let h = FindWindowW(wide(TRAY_CLASS).as_ptr(), null());
                if !h.is_null() {
                    AllowSetForegroundWindow(u32::MAX /* ASFW_ANY */);
                    PostMessageW(h, WM_SHOW_PANEL, 0, 0);
                }
            }
            return false;
        }
        // keep the mutex for the life of the process (handle intentionally leaked)
        true
    }
}

// ---------------------------------------------------------------------------
// Autostart (HKCU\...\Run)
// ---------------------------------------------------------------------------

fn exe_path() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
}

pub fn autostart_enabled() -> bool {
    unsafe {
        let mut key = 0isize;
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, 0x1 /* QUERY */, &mut key) != 0 {
            return false;
        }
        let mut len = 0u32;
        let ok = RegQueryValueExW(key, wide(RUN_VALUE).as_ptr(), null_mut(), null_mut(), null_mut(), &mut len) == 0;
        RegCloseKey(key);
        ok
    }
}

pub fn set_autostart(on: bool) {
    unsafe {
        let mut key = 0isize;
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, 0x2 /* SET_VALUE */, &mut key) != 0 {
            return;
        }
        if on {
            let cmd = wide(&format!("\"{}\" tray", exe_path()));
            RegSetValueExW(key, wide(RUN_VALUE).as_ptr(), 0, 1 /* REG_SZ */, cmd.as_ptr() as *const u8, (cmd.len() * 2) as u32);
        } else {
            RegDeleteValueW(key, wide(RUN_VALUE).as_ptr());
        }
        RegCloseKey(key);
    }
}

// ---------------------------------------------------------------------------
// Icon (drawn in code: no resource file needed)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum IconState {
    Idle,
    Shared,
    Exclusive,
    Trouble,
}

fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// 32x32 BGRA: the Dolomite peak (as in the panel logo) + a status dot.
fn make_icon(state: IconState) -> *mut c_void {
    const N: usize = 32;
    let k = 32.0 / 34.0;
    let sc = |p: &[(f32, f32)]| p.iter().map(|&(x, y)| (x * k, y * k)).collect::<Vec<_>>();
    let peak = sc(&[(2.0, 30.0), (11.0, 12.0), (15.0, 19.0), (21.0, 6.0), (32.0, 30.0)]);
    let shade = sc(&[(2.0, 30.0), (11.0, 12.0), (15.0, 19.0), (13.0, 30.0)]);
    let (main, dark): ([u8; 3], [u8; 3]) = if state == IconState::Idle {
        ([0xA0, 0xA6, 0xAC], [0x78, 0x7E, 0x84])
    } else {
        ([0xF2, 0xA1, 0xB5], [0xC9, 0x7F, 0x93])
    };
    let dot: Option<[u8; 3]> = match state {
        IconState::Idle => None,
        IconState::Shared => Some([0x7F, 0xB4, 0xE6]),
        IconState::Exclusive => Some([0x6C, 0xCB, 0x8A]),
        IconState::Trouble => Some([0xEF, 0x5B, 0x5B]),
    };
    let (dcx, dcy, dr) = (25.5f32, 25.5f32, 5.5f32);
    let mut px = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let (mut r, mut g, mut b, mut a) = (0f32, 0f32, 0f32, 0f32);
            for sy in 0..4 {
                for sx in 0..4 {
                    let fx = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let fy = y as f32 + (sy as f32 + 0.5) / 4.0;
                    let d = ((fx - dcx).powi(2) + (fy - dcy).powi(2)).sqrt();
                    let col: Option<[u8; 3]> = if let (Some(dc), true) = (dot, d <= dr) {
                        Some(dc)
                    } else if dot.is_some() && d <= dr + 1.6 {
                        None // transparent ring separates the dot from the peak
                    } else if inside(&shade, fx, fy) {
                        Some(dark)
                    } else if inside(&peak, fx, fy) {
                        Some(main)
                    } else {
                        None
                    };
                    if let Some(c) = col {
                        r += c[0] as f32;
                        g += c[1] as f32;
                        b += c[2] as f32;
                        a += 1.0;
                    }
                }
            }
            let i = (y * N + x) * 4;
            if a > 0.0 {
                px[i] = (b / a) as u8;
                px[i + 1] = (g / a) as u8;
                px[i + 2] = (r / a) as u8;
                px[i + 3] = (a / 16.0 * 255.0).round() as u8;
            }
        }
    }
    unsafe {
        let bmi = BITMAPINFOHEADER {
            size: 40,
            width: N as i32,
            height: -(N as i32),
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut bits: *mut c_void = null_mut();
        let color = CreateDIBSection(null_mut(), &bmi, 0, &mut bits, null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return null_mut();
        }
        core::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u8, px.len());
        let zeros = vec![0u8; N * N / 8];
        let mask = CreateBitmap(N as i32, N as i32, 1, 1, zeros.as_ptr() as *const c_void);
        let ii = ICONINFO { is_icon: 1, x_hotspot: 0, y_hotspot: 0, mask, color };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}

// ---------------------------------------------------------------------------
// Tray state (main thread only)
// ---------------------------------------------------------------------------

struct Tray {
    hwnd: HWND,
    port: u16,
    taskbar_created: u32,
    icons: [*mut c_void; 4],
    shown_state: Option<IconState>,
    tip: String,
    panel: HWND,
    launching: Option<Instant>,
    show_when_found: bool,
    was_foreground: bool,
    away_since: Option<Instant>,
    last_auto_hide: Instant,
    live: Option<LiveStatusMap>,
    last_live_try: Instant,
    last_update: Instant,
    last_underruns: u32,
    trouble_until: Instant,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with_tray<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    TRAY.with(|t| t.borrow_mut().as_mut().map(f))
}

impl Tray {
    fn nid(&self, flags: u32) -> NOTIFYICONDATAW {
        let mut n: NOTIFYICONDATAW = unsafe { core::mem::zeroed() };
        n.cb_size = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
        n.hwnd = self.hwnd;
        n.uid = 1;
        n.flags = flags;
        n.callback_message = WM_TRAY;
        n.icon = self.icons[self.shown_state.unwrap_or(IconState::Idle) as usize];
        n.tip = tip_text(&self.tip);
        n
    }

    fn add_icon(&mut self) {
        let mut n = self.nid(NIF_MESSAGE | NIF_ICON | NIF_TIP);
        unsafe {
            if Shell_NotifyIconW(NIM_ADD, &mut n) == 0 {
                Shell_NotifyIconW(NIM_MODIFY, &mut n);
            }
        }
    }

    fn remove_icon(&mut self) {
        let mut n = self.nid(0);
        unsafe { Shell_NotifyIconW(NIM_DELETE, &mut n) };
    }

    /// Refresh icon + tooltip from the driver's live status (1 Hz).
    fn update_status(&mut self) {
        if self.live.is_none() && self.last_live_try.elapsed() > Duration::from_secs(2) {
            self.last_live_try = Instant::now();
            self.live = LiveStatusMap::open_read();
        }
        let mut state = IconState::Idle;
        let mut tip = "Dolomite Audio — non in uso".to_string();
        if let Some(m) = &self.live {
            let s = m.get();
            if s.is_alive() {
                let ld = |a: &std::sync::atomic::AtomicU32| a.load(Ordering::Relaxed);
                let rate = ld(&s.daw_rate).max(1) as f64;
                let out_ms = ld(&s.output_latency_frames) as f64 * 1000.0 / rate;
                let in_ms = ld(&s.input_latency_frames) as f64 * 1000.0 / rate;
                let mode = ld(&s.engine_mode);
                let under = ld(&s.underruns);
                if under > self.last_underruns {
                    self.trouble_until = Instant::now() + Duration::from_secs(3);
                }
                self.last_underruns = under;
                state = if Instant::now() < self.trouble_until {
                    IconState::Trouble
                } else if mode == 3 {
                    IconState::Exclusive
                } else {
                    IconState::Shared
                };
                let mode_s = ["Timer", "Compatibile", "Bassa latenza", "Esclusivo"].get(mode as usize).copied().unwrap_or("-");
                tip = if ld(&s.input_mode) != 0 {
                    format!("Dolomite Audio — {}\nUscita {:.1} ms · Ingresso {:.1} ms\n{}", mode_s, out_ms, in_ms, s.device_name())
                } else {
                    format!("Dolomite Audio — {}\nUscita {:.1} ms\n{}", mode_s, out_ms, s.device_name())
                };
            } else {
                self.last_underruns = 0;
            }
        }
        if Some(state) != self.shown_state || tip != self.tip {
            self.shown_state = Some(state);
            self.tip = tip;
            let mut n = self.nid(NIF_ICON | NIF_TIP);
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &mut n) };
        }
    }

    // ---- panel -------------------------------------------------------------

    fn find_panel(&mut self) -> HWND {
        unsafe {
            if !self.panel.is_null() && IsWindow(self.panel) != 0 {
                return self.panel;
            }
            self.panel = FindWindowW(wide("Chrome_WidgetWin_1").as_ptr(), wide(PANEL_TITLE).as_ptr());
            self.panel
        }
    }

    fn panel_visible(&mut self) -> bool {
        let p = self.find_panel();
        !p.is_null() && unsafe { IsWindowVisible(p) } != 0
    }

    /// Bottom-right of the work area of the monitor under the cursor.
    fn place(&self, panel: HWND) {
        unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            let mon = MonitorFromPoint(pt, 2 /* NEAREST */);
            let mut mi = MONITORINFO { cb_size: core::mem::size_of::<MONITORINFO>() as u32, monitor: RECT::default(), work: RECT::default(), flags: 0 };
            GetMonitorInfoW(mon, &mut mi);
            let dpi = GetDpiForWindow(panel).max(96) as i32;
            let s = |v: i32| v * dpi / 96;
            let wa = mi.work;
            let w = s(PANEL_W);
            let h = s(PANEL_H).min(wa.bottom - wa.top - s(24));
            let x = wa.right - w - s(12);
            let y = wa.bottom - h - s(12);
            SetWindowPos(panel, HWND_TOPMOST, x, y, w, h, SWP_FRAMECHANGED);
        }
    }

    fn show_panel(&mut self) {
        let p = self.find_panel();
        if p.is_null() {
            if self.launching.map(|t| t.elapsed() < Duration::from_secs(15)).unwrap_or(false) {
                return; // already starting
            }
            if launch_edge(self.port) {
                self.launching = Some(Instant::now());
                self.show_when_found = true;
            } else {
                // No Edge: classic native window
                std::thread::spawn(|| crate::gui::run_gui(None));
            }
            return;
        }
        unsafe {
            self.place(p);
            ShowWindow(p, SW_SHOW);
            SetForegroundWindow(p);
        }
        // only auto-hide after the panel actually had focus once
        self.was_foreground = false;
        self.away_since = None;
    }

    fn hide_panel(&mut self) {
        let p = self.find_panel();
        if !p.is_null() {
            unsafe { ShowWindow(p, SW_HIDE) };
        }
        self.was_foreground = false;
        self.away_since = None;
    }

    fn toggle_panel(&mut self) {
        if self.panel_visible() {
            self.hide_panel();
        } else if self.last_auto_hide.elapsed() < Duration::from_millis(400) {
            // this very click on the tray icon is what made the panel lose
            // focus and auto-hide: treat it as "close", not "open again"
        } else {
            self.show_panel();
        }
    }

    /// 10 Hz housekeeping.
    fn tick(&mut self) {
        if crate::server::HIDE_REQUEST.swap(false, Ordering::Relaxed) {
            self.hide_panel();
        }

        // Edge window appeared after launch: make it a flyout.
        if let Some(t) = self.launching {
            let p = self.find_panel();
            if !p.is_null() {
                self.launching = None;
                unsafe {
                    ShowWindow(p, SW_HIDE);
                    let ex = GetWindowLongPtrW(p, GWL_EXSTYLE);
                    SetWindowLongPtrW(p, GWL_EXSTYLE, (ex | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW);
                }
                if self.show_when_found {
                    self.show_panel();
                }
            } else if t.elapsed() > Duration::from_secs(15) {
                self.launching = None;
            }
        }

        // Auto-hide when focus moves elsewhere (unless pinned).
        if !crate::server::PINNED.load(Ordering::Relaxed) && self.panel_visible() {
            let fg = unsafe { GetForegroundWindow() };
            if fg == self.panel {
                self.was_foreground = true;
                self.away_since = None;
            } else if self.was_foreground {
                let since = *self.away_since.get_or_insert_with(Instant::now);
                if since.elapsed() > Duration::from_millis(150) {
                    self.hide_panel();
                    self.last_auto_hide = Instant::now();
                }
            }
        }

        if self.last_update.elapsed() > Duration::from_millis(900) {
            self.last_update = Instant::now();
            self.update_status();
        }
    }

    fn menu(&mut self) {
        unsafe {
            let m = CreatePopupMenu();
            AppendMenuW(m, MF_STRING, CMD_OPEN, wide("Apri pannello").as_ptr());
            AppendMenuW(m, MF_STRING, CMD_TEST, wide("Suono di prova").as_ptr());
            AppendMenuW(m, MF_SEPARATOR, 0, null());
            AppendMenuW(m, MF_STRING | if autostart_enabled() { MF_CHECKED } else { 0 }, CMD_AUTOSTART, wide("Avvia con Windows").as_ptr());
            AppendMenuW(m, MF_SEPARATOR, 0, null());
            AppendMenuW(m, MF_STRING, CMD_EXIT, wide("Esci").as_ptr());
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(m, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD, pt.x, pt.y, 0, self.hwnd, null());
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            DestroyMenu(m);
            match cmd as usize {
                CMD_OPEN => self.show_panel(),
                CMD_TEST => {
                    let port = self.port;
                    std::thread::spawn(move || {
                        use std::io::Write;
                        if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                            let _ = s.write_all(b"POST /api/test_sound HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n");
                        }
                    });
                }
                CMD_AUTOSTART => set_autostart(!autostart_enabled()),
                CMD_EXIT => {
                    let p = self.find_panel();
                    if !p.is_null() {
                        PostMessageW(p, WM_CLOSE, 0, 0);
                    }
                    self.remove_icon();
                    PostQuitMessage(0);
                }
                _ => {}
            }
        }
    }
}

fn edge_path() -> Option<&'static str> {
    [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists())
}

pub fn edge_available() -> bool {
    edge_path().is_some()
}

fn launch_edge(port: u16) -> bool {
    let Some(path) = edge_path() else { return false };
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".into());
    Command::new(path)
        .arg(format!("--app=http://127.0.0.1:{}", port))
        .arg(format!("--user-data-dir={}\\DolomiteAudio\\ui-profile", appdata))
        .arg(format!("--window-size={},{}", PANEL_W, PANEL_H))
        .arg("--window-position=-32000,-32000") // off-screen until docked
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .spawn()
        .is_ok()
}

extern "system" fn wnd_proc(h: HWND, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_TRAY => {
            match l as u32 {
                WM_LBUTTONUP => {
                    with_tray(|t| t.toggle_panel());
                }
                WM_RBUTTONUP => {
                    with_tray(|t| t.menu());
                }
                _ => {}
            }
            0
        }
        WM_SHOW_PANEL => {
            with_tray(|t| {
                t.last_auto_hide = Instant::now() - Duration::from_secs(1);
                t.show_panel()
            });
            0
        }
        WM_TIMER => {
            with_tray(|t| t.tick());
            0
        }
        WM_DESTROY => {
            with_tray(|t| t.remove_icon());
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            let tc = with_tray(|t| t.taskbar_created).unwrap_or(0);
            if tc != 0 && msg == tc {
                // Explorer restarted: our icon vanished with it.
                with_tray(|t| t.add_icon());
                return 0;
            }
            unsafe { DefWindowProcW(h, msg, w, l) }
        }
    }
}

fn enable_dpi_awareness() {
    unsafe {
        let user32 = LoadLibraryW(wide("user32.dll").as_ptr());
        let f = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr());
        if !f.is_null() {
            let f: extern "system" fn(isize) -> i32 = core::mem::transmute(f);
            if f(-4 /* PER_MONITOR_AWARE_V2 */) != 0 {
                return;
            }
        }
        SetProcessDPIAware();
    }
}

/// Run the tray icon message loop (blocks until "Esci").
pub fn run(port: u16, show_panel_now: bool) {
    enable_dpi_awareness();
    unsafe {
        let inst = GetModuleHandleW(null());
        let class = wide(TRAY_CLASS);
        let wc = WNDCLASSEXW {
            cb_size: core::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            wnd_proc,
            cls_extra: 0,
            wnd_extra: 0,
            instance: inst,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: class.as_ptr(),
            icon_sm: null_mut(),
        };
        RegisterClassExW(&wc);
        // Hidden top-level window (not message-only: it must receive the
        // TaskbarCreated broadcast).
        let hwnd = CreateWindowExW(0, class.as_ptr(), wide("Dolomite Audio Tray").as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), inst, null_mut());
        if hwnd.is_null() {
            return;
        }
        let icons = [
            make_icon(IconState::Idle),
            make_icon(IconState::Shared),
            make_icon(IconState::Exclusive),
            make_icon(IconState::Trouble),
        ];
        let now = Instant::now();
        let mut tray = Tray {
            hwnd,
            port,
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            icons,
            shown_state: Some(IconState::Idle),
            tip: "Dolomite Audio".into(),
            panel: null_mut(),
            launching: None,
            show_when_found: false,
            was_foreground: false,
            away_since: None,
            last_auto_hide: now - Duration::from_secs(5),
            live: None,
            last_live_try: now - Duration::from_secs(5),
            last_update: now - Duration::from_secs(5),
            last_underruns: 0,
            trouble_until: now,
        };
        tray.add_icon();
        tray.update_status();
        TRAY.with(|t| *t.borrow_mut() = Some(tray));
        SetTimer(hwnd, 1, 100, null());

        if show_panel_now {
            with_tray(|t| t.show_panel());
        } else if edge_available() {
            // Warm the panel up hidden, so the first click opens instantly.
            with_tray(|t| {
                if launch_edge(t.port) {
                    t.launching = Some(Instant::now());
                    t.show_when_found = false;
                }
            });
        }

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        with_tray(|t| t.remove_icon());
    }
}
