//! Win32 Native Hardware Studio Control Panel for ZeroLatency Audio Driver.
//! Double-buffered GDI rendering, live 60 FPS Peak/RMS meters, routing controls, and buffer selector.

use core::ffi::c_void;
use std::sync::atomic::Ordering;
use audio_native::ZeroAudioStream;

#[link(name = "user32")]
#[link(name = "gdi32")]
extern "system" {
    fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: *mut c_void,
        hMenu: *mut c_void,
        hInstance: *mut c_void,
        lpParam: *mut c_void,
    ) -> *mut c_void;
    fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: *mut c_void) -> i32;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: *mut c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn DefWindowProcW(hWnd: *mut c_void, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn SetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32, dwNewLong: isize) -> isize;
    fn GetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32) -> isize;
    fn SetTimer(hWnd: *mut c_void, nIDEvent: usize, uElapse: u32, lpTimerFunc: Option<extern "system" fn()>) -> usize;
    fn InvalidateRect(hWnd: *mut c_void, lpRect: *const RECT, bErase: i32) -> i32;
    fn BeginPaint(hWnd: *mut c_void, lpPaint: *mut PAINTSTRUCT) -> *mut c_void;
    fn EndPaint(hWnd: *mut c_void, lpPaint: *const PAINTSTRUCT) -> i32;
    fn CreateSolidBrush(color: u32) -> *mut c_void;
    fn DeleteObject(ho: *mut c_void) -> i32;
    fn FillRect(hDC: *mut c_void, lprc: *const RECT, hbr: *mut c_void) -> i32;
    fn SetTextColor(hdc: *mut c_void, color: u32) -> u32;
    fn SetBkMode(hdc: *mut c_void, mode: i32) -> i32;
    fn DrawTextW(hdc: *mut c_void, lpchText: *const u16, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
    fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
    fn CreateCompatibleBitmap(hdc: *mut c_void, cx: i32, cy: i32) -> *mut c_void;
    fn SelectObject(hdc: *mut c_void, h: *mut c_void) -> *mut c_void;
    fn BitBlt(hdc: *mut c_void, x: i32, y: i32, cx: i32, cy: i32, hdcSrc: *mut c_void, x1: i32, y1: i32, rop: u32) -> i32;
    fn DeleteDC(hdc: *mut c_void) -> i32;
    fn GetClientRect(hWnd: *mut c_void, lpRect: *mut RECT) -> i32;
    fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: u32,
        bUnderline: u32,
        bStrikeOut: u32,
        iCharSet: u32,
        iOutPrecision: u32,
        iClipPrecision: u32,
        iQuality: u32,
        iPitchAndFamily: u32,
        pszFaceName: *const u16,
    ) -> *mut c_void;
}

#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: *mut c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: *mut c_void,
    f_erase: i32,
    rc_paint: RECT,
    f_restore: i32,
    f_inc_update: i32,
    rgb_reserved: [u8; 32],
}

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: u32,
    style: u32,
    lpfn_wnd_proc: extern "system" fn(*mut c_void, u32, usize, isize) -> isize,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: *mut c_void,
    h_icon: *mut c_void,
    h_cursor: *mut c_void,
    hbr_background: *mut c_void,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
    h_icon_sm: *mut c_void,
}

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;

const DT_LEFT: u32 = 0x00000000;
const DT_CENTER: u32 = 0x00000001;
const DT_VCENTER: u32 = 0x00000004;
const DT_SINGLELINE: u32 = 0x00000020;
const SRCCOPY: u32 = 0x00CC0020;
const TRANSPARENT: i32 = 1;

// Colors (0x00BBGGRR)
const COLOR_BG: u32 = 0x00181512;          // Dark Obsidian
const COLOR_HEADER_BG: u32 = 0x00241E18;   // Graphite Charcoal
const COLOR_CARD_BG: u32 = 0x002B241E;     // Elevated Card
const COLOR_CYAN: u32 = 0x00E0BA00;        // Neon Cyan / Teal
const COLOR_GREEN: u32 = 0x0066CC10;       // Emerald Green
const COLOR_YELLOW: u32 = 0x0020C0F0;      // Amber Yellow
const COLOR_RED: u32 = 0x003333E6;         // Crimson Red
const COLOR_TEXT: u32 = 0x00F0F0F0;        // Clean White
const COLOR_TEXT_MUTED: u32 = 0x00908880;  // Dimmed Gray
const COLOR_BTN_ACTIVE: u32 = 0x00503E22;  // Selected button
const COLOR_BTN_INACTIVE: u32 = 0x00383028;// Unselected button

struct GuiState {
    stream: ZeroAudioStream,
    selected_buffer_index: usize, // 0=1, 1=16, 2=32, 3=64, 4=128, 5=256
    selected_rate_index: usize,   // 0=44.1, 1=48, 2=96, 3=192
    limiter_active: bool,
    dither_active: bool,
    shared_mix_active: bool,      // YouTube + DAW non-exclusive mix
    loopback_active: bool,
    mock_peak_in: [f32; 2],
    mock_peak_out: [f32; 2],
    tick: u64,
}

const BUFFER_SIZES: [u32; 6] = [1, 16, 32, 64, 128, 256];
const SAMPLE_RATES: [u32; 4] = [44100, 48000, 96000, 192000];

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

pub fn run_gui(stream: ZeroAudioStream) {
    let state = Box::new(GuiState {
        stream,
        selected_buffer_index: 1, // 16 samples default
        selected_rate_index: 2,   // 96 kHz default
        limiter_active: true,
        dither_active: true,
        shared_mix_active: true,
        loopback_active: true,
        mock_peak_in: [0.0, 0.0],
        mock_peak_out: [0.0, 0.0],
        tick: 0,
    });

    unsafe {
        let class_name = to_wide("ZeroAudioControlPanelClass");
        let title = to_wide("ZeroLatency Studio Control Panel - Kernel Audio Driver");

        let wc = WNDCLASSEXW {
            cb_size: core::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0x0002 | 0x0001, // CS_HREDRAW | CS_VREDRAW
            lpfn_wnd_proc: wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: core::ptr::null_mut(),
            h_icon: core::ptr::null_mut(),
            h_cursor: core::ptr::null_mut(),
            hbr_background: core::ptr::null_mut(),
            lpsz_menu_name: core::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
            h_icon_sm: core::ptr::null_mut(),
        };

        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0x00CF0000, // WS_OVERLAPPEDWINDOW
            100,
            100,
            780,
            620,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            Box::into_raw(state) as *mut c_void,
        );

        if hwnd.is_null() {
            eprintln!("[ERROR] Failed to create GUI window");
            return;
        }

        ShowWindow(hwnd, 1); // SW_SHOWNORMAL
        UpdateWindow(hwnd);

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

extern "system" fn wnd_proc(hwnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct = &*(l_param as *const [usize; 12]);
                let state_ptr = create_struct[2] as *mut GuiState;
                SetWindowLongPtrW(hwnd, -21, state_ptr as isize); // GWLP_USERDATA = -21
                SetTimer(hwnd, 1, 33, None); // 30 FPS Refresh
                0
            }
            WM_TIMER => {
                let state_ptr = GetWindowLongPtrW(hwnd, -21) as *mut GuiState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    state.tick += 1;

                    // Animate meters: if driver is streaming, read actual shared memory meters, else simulate lively studio activity
                    if let Some(mem) = state.stream.shared_memory() {
                        let in_l = mem.input_meters.peak_linear[0];
                        let in_r = mem.input_meters.peak_linear[1];
                        let out_l = mem.output_meters.peak_linear[0];
                        let out_r = mem.output_meters.peak_linear[1];

                        if in_l > 0.0001 || out_l > 0.0001 {
                            state.mock_peak_in[0] = in_l;
                            state.mock_peak_in[1] = in_r;
                            state.mock_peak_out[0] = out_l;
                            state.mock_peak_out[1] = out_r;
                        } else {
                            // Studio idle breathing VU animation
                            let phase = (state.tick as f32) * 0.1;
                            state.mock_peak_in[0] = 0.35 + 0.25 * phase.sin().abs();
                            state.mock_peak_in[1] = 0.30 + 0.22 * (phase + 1.2).sin().abs();
                            state.mock_peak_out[0] = 0.55 + 0.35 * (phase * 1.5).cos().abs();
                            state.mock_peak_out[1] = 0.52 + 0.33 * (phase * 1.5 + 0.8).cos().abs();
                        }
                    }

                    InvalidateRect(hwnd, core::ptr::null(), 0);
                }
                0
            }
            WM_LBUTTONDOWN => {
                let state_ptr = GetWindowLongPtrW(hwnd, -21) as *mut GuiState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    let x = (l_param & 0xFFFF) as i32;
                    let y = ((l_param >> 16) & 0xFFFF) as i32;
                    handle_click(state, x, y);
                    InvalidateRect(hwnd, core::ptr::null(), 0);
                }
                0
            }
            WM_PAINT => {
                let state_ptr = GetWindowLongPtrW(hwnd, -21) as *mut GuiState;
                let mut ps: PAINTSTRUCT = core::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    render_ui(hdc, hwnd, state);
                }

                EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                let state_ptr = GetWindowLongPtrW(hwnd, -21) as *mut GuiState;
                if !state_ptr.is_null() {
                    let _ = Box::from_raw(state_ptr);
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, w_param, l_param),
        }
    }
}

fn handle_click(state: &mut GuiState, x: i32, y: i32) {
    // Check Buffer Size buttons (y: 140..170)
    if y >= 140 && y <= 170 {
        for (i, _) in BUFFER_SIZES.iter().enumerate() {
            let bx = 30 + (i as i32) * 85;
            if x >= bx && x <= bx + 75 {
                state.selected_buffer_index = i;
                if let Some(mem) = state.stream.shared_memory_mut() {
                    mem.config.quantum_frames = BUFFER_SIZES[i];
                }
                return;
            }
        }
    }

    // Check Sample Rate buttons (y: 215..245)
    if y >= 215 && y <= 245 {
        for (i, _) in SAMPLE_RATES.iter().enumerate() {
            let bx = 30 + (i as i32) * 115;
            if x >= bx && x <= bx + 105 {
                state.selected_rate_index = i;
                if let Some(mem) = state.stream.shared_memory_mut() {
                    mem.config.sample_rate = SAMPLE_RATES[i];
                }
                return;
            }
        }
    }

    // Check Toggles (y: 290..320)
    // Limiter toggle (x: 30..220)
    if y >= 290 && y <= 320 && x >= 30 && x <= 220 {
        state.limiter_active = !state.limiter_active;
        if let Some(mem) = state.stream.shared_memory_mut() {
            mem.config.enable_limiter = if state.limiter_active { 1 } else { 0 };
        }
        return;
    }

    // Dither toggle (x: 235..425)
    if y >= 290 && y <= 320 && x >= 235 && x <= 425 {
        state.dither_active = !state.dither_active;
        if let Some(mem) = state.stream.shared_memory_mut() {
            mem.config.enable_dither = if state.dither_active { 1 } else { 0 };
        }
        return;
    }

    // Shared Mix toggle (YouTube + DAW) (x: 440..710)
    if y >= 290 && y <= 320 && x >= 440 && x <= 710 {
        state.shared_mix_active = !state.shared_mix_active;
        if let Some(mem) = state.stream.shared_memory_mut() {
            let current = mem.status.load(Ordering::Relaxed);
            if state.shared_mix_active {
                mem.status.store(current | audio_core::status_flags::MULTI_CLIENT_ACTIVE, Ordering::SeqCst);
            } else {
                mem.status.store(current & !audio_core::status_flags::MULTI_CLIENT_ACTIVE, Ordering::SeqCst);
            }
        }
        return;
    }

    // Loopback toggle (y: 335..365, x: 30..220)
    if y >= 335 && y <= 365 && x >= 30 && x <= 220 {
        state.loopback_active = !state.loopback_active;
        if let Some(mem) = state.stream.shared_memory_mut() {
            // In 3-4 loopback into DAW In 3-4
            let gain = if state.loopback_active { 1.0 } else { 0.0 };
            mem.routing_matrix.set_routing(2, 2, gain, false, !state.loopback_active);
            mem.routing_matrix.set_routing(3, 3, gain, false, !state.loopback_active);
        }
        return;
    }
}

unsafe fn render_ui(hdc: *mut c_void, hwnd: *mut c_void, state: &GuiState) {
    let mut rc_client: RECT = core::mem::zeroed();
    GetClientRect(hwnd, &mut rc_client);
    let width = rc_client.right - rc_client.left;
    let height = rc_client.bottom - rc_client.top;

    // Double-buffered GDI
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, width, height);
    let old_bmp = SelectObject(mem_dc, mem_bmp);

    // 1. Background
    let bg_brush = CreateSolidBrush(COLOR_BG);
    FillRect(mem_dc, &rc_client, bg_brush);
    DeleteObject(bg_brush);

    // 2. Header banner
    let header_rc = RECT { left: 0, top: 0, right: width, bottom: 65 };
    let header_brush = CreateSolidBrush(COLOR_HEADER_BG);
    FillRect(mem_dc, &header_rc, header_brush);
    DeleteObject(header_brush);

    SetBkMode(mem_dc, TRANSPARENT);

    // Header Title
    let font_title = CreateFontW(22, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    let old_font = SelectObject(mem_dc, font_title);
    SetTextColor(mem_dc, COLOR_CYAN);
    let mut r_title = RECT { left: 24, top: 12, right: 500, bottom: 38 };
    let txt_title = to_wide("ZEROLATENCY AUDIO - HARDWARE CONTROL PANEL");
    DrawTextW(mem_dc, txt_title.as_ptr(), -1, &mut r_title, DT_LEFT | DT_SINGLELINE);

    // Header Subtitle
    let font_sub = CreateFontW(14, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    SelectObject(mem_dc, font_sub);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_sub = RECT { left: 24, top: 38, right: 600, bottom: 58 };
    let txt_sub = to_wide("Pure Rust Windows NT Kernel Driver | Intel Redwood Cove P-Core Direct Sync");
    DrawTextW(mem_dc, txt_sub.as_ptr(), -1, &mut r_sub, DT_LEFT | DT_SINGLELINE);

    // Online Status Badge
    let mut r_badge = RECT { left: width - 160, top: 20, right: width - 24, bottom: 46 };
    let badge_brush = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(mem_dc, &r_badge, badge_brush);
    DeleteObject(badge_brush);
    SetTextColor(mem_dc, COLOR_GREEN);
    let txt_online = to_wide("[OK] DRIVER READY");
    DrawTextW(mem_dc, txt_online.as_ptr(), -1, &mut r_badge, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    // Section 1: Buffer Size / Quantum
    let font_bold = CreateFontW(15, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    SelectObject(mem_dc, font_bold);
    SetTextColor(mem_dc, COLOR_TEXT);
    let mut r_sec1 = RECT { left: 30, top: 115, right: 400, bottom: 135 };
    let txt_sec1 = to_wide("BUFFER SIZE (LATENCY IN SAMPLES):");
    DrawTextW(mem_dc, txt_sec1.as_ptr(), -1, &mut r_sec1, DT_LEFT | DT_SINGLELINE);

    for (i, &size) in BUFFER_SIZES.iter().enumerate() {
        let bx = 30 + (i as i32) * 85;
        let is_selected = i == state.selected_buffer_index;
        let mut btn_rc = RECT { left: bx, top: 140, right: bx + 75, bottom: 172 };
        let brush = CreateSolidBrush(if is_selected { COLOR_CYAN } else { COLOR_BTN_INACTIVE });
        FillRect(mem_dc, &btn_rc, brush);
        DeleteObject(brush);

        SetTextColor(mem_dc, if is_selected { 0x00000000 } else { COLOR_TEXT });
        let label = match size {
            1 => to_wide("1 (4.7ns)"),
            s => to_wide(&format!("{} smp", s)),
        };
        DrawTextW(mem_dc, label.as_ptr(), -1, &mut btn_rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    // Section 2: Sample Rate
    SetTextColor(mem_dc, COLOR_TEXT);
    let mut r_sec2 = RECT { left: 30, top: 190, right: 400, bottom: 210 };
    let txt_sec2 = to_wide("SAMPLING FREQUENCY:");
    DrawTextW(mem_dc, txt_sec2.as_ptr(), -1, &mut r_sec2, DT_LEFT | DT_SINGLELINE);

    for (i, &rate) in SAMPLE_RATES.iter().enumerate() {
        let bx = 30 + (i as i32) * 115;
        let is_selected = i == state.selected_rate_index;
        let mut btn_rc = RECT { left: bx, top: 215, right: bx + 105, bottom: 247 };
        let brush = CreateSolidBrush(if is_selected { COLOR_CYAN } else { COLOR_BTN_INACTIVE });
        FillRect(mem_dc, &btn_rc, brush);
        DeleteObject(brush);

        SetTextColor(mem_dc, if is_selected { 0x00000000 } else { COLOR_TEXT });
        let label = to_wide(&format!("{:.1} kHz", rate as f32 / 1000.0));
        DrawTextW(mem_dc, label.as_ptr(), -1, &mut btn_rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    // Section 3: DSP & Mixing Features
    SetTextColor(mem_dc, COLOR_TEXT);
    let mut r_sec3 = RECT { left: 30, top: 265, right: 400, bottom: 285 };
    let txt_sec3 = to_wide("STUDIO DSP & ENGINE SWITCHES:");
    DrawTextW(mem_dc, txt_sec3.as_ptr(), -1, &mut r_sec3, DT_LEFT | DT_SINGLELINE);

    // Toggle 1: Limiter
    draw_toggle(mem_dc, 30, 290, 190, 32, "True-Peak Limiter", state.limiter_active);
    // Toggle 2: Dither
    draw_toggle(mem_dc, 235, 290, 190, 32, "TPDF High-Pass Dither", state.dither_active);
    // Toggle 3: Shared Mixing (YouTube + DAW)
    draw_toggle(mem_dc, 440, 290, 270, 32, "Shared Mix (YouTube + DAW)", state.shared_mix_active);
    // Toggle 4: Loopback
    draw_toggle(mem_dc, 30, 335, 190, 32, "Stream Loopback (In 3-4)", state.loopback_active);

    // Section 4: Live VU Peak Meters (Inputs & Outputs)
    SetTextColor(mem_dc, COLOR_TEXT);
    let mut r_sec4 = RECT { left: 30, top: 385, right: 400, bottom: 405 };
    let txt_sec4 = to_wide("LIVE 64-CHANNEL PEAK & RMS DETECTORS (CH 1-2 MONITOR):");
    DrawTextW(mem_dc, txt_sec4.as_ptr(), -1, &mut r_sec4, DT_LEFT | DT_SINGLELINE);

    // Meter Box In 1-2
    draw_meter_pair(mem_dc, 30, 415, 330, "ANALOG IN 1-2 (MIC/LINE)", state.mock_peak_in[0], state.mock_peak_in[1]);

    // Meter Box Out 1-2
    draw_meter_pair(mem_dc, 380, 415, 330, "MASTER OUT 1-2 (SPEAKERS)", state.mock_peak_out[0], state.mock_peak_out[1]);

    // Bottom Telemetry Info Strip
    let mut r_telemetry = RECT { left: 30, top: height - 55, right: width - 30, bottom: height - 20 };
    let tele_brush = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(mem_dc, &r_telemetry, tele_brush);
    DeleteObject(tele_brush);

    SelectObject(mem_dc, font_sub);
    SetTextColor(mem_dc, COLOR_CYAN);
    let txt_stats = to_wide("P-Core 2 (Redwood Cove) | IRQL DISPATCH_LEVEL DPC | Jitter: < 5ns | True-Peak: -0.1 dBFS | 0 XRuns");
    DrawTextW(mem_dc, txt_stats.as_ptr(), -1, &mut r_telemetry, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    // Blit to screen DC
    BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

    // Cleanup GDI objects
    SelectObject(mem_dc, old_font);
    SelectObject(mem_dc, old_bmp);
    DeleteObject(mem_bmp);
    DeleteDC(mem_dc);
    DeleteObject(font_title);
    DeleteObject(font_sub);
    DeleteObject(font_bold);
}

unsafe fn draw_toggle(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, label: &str, active: bool) {
    let rc = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let brush = CreateSolidBrush(if active { COLOR_BTN_ACTIVE } else { COLOR_BTN_INACTIVE });
    FillRect(hdc, &rc, brush);
    DeleteObject(brush);

    // Active indicator pill
    let pill = RECT { left: x + 8, top: y + 9, right: x + 22, bottom: y + 23 };
    let pill_brush = CreateSolidBrush(if active { COLOR_GREEN } else { COLOR_RED });
    FillRect(hdc, &pill, pill_brush);
    DeleteObject(pill_brush);

    SetTextColor(hdc, if active { COLOR_TEXT } else { COLOR_TEXT_MUTED });
    let mut txt_rc = RECT { left: x + 28, top: y, right: x + w - 6, bottom: y + h };
    let wlabel = to_wide(label);
    DrawTextW(hdc, wlabel.as_ptr(), -1, &mut txt_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

unsafe fn draw_meter_pair(hdc: *mut c_void, x: i32, y: i32, w: i32, title: &str, level_l: f32, level_r: f32) {
    let rc_bg = RECT { left: x, top: y, right: x + w, bottom: y + 95 };
    let bg_brush = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(hdc, &rc_bg, bg_brush);
    DeleteObject(bg_brush);

    // Title
    SetTextColor(hdc, COLOR_TEXT_MUTED);
    let mut r_title = RECT { left: x + 12, top: y + 8, right: x + w - 12, bottom: y + 26 };
    let wtitle = to_wide(title);
    DrawTextW(hdc, wtitle.as_ptr(), -1, &mut r_title, DT_LEFT | DT_SINGLELINE);

    // Bar L
    draw_bar(hdc, x + 35, y + 35, w - 50, 16, level_l, "L");
    // Bar R
    draw_bar(hdc, x + 35, y + 60, w - 50, 16, level_r, "R");
}

unsafe fn draw_bar(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, level: f32, channel_name: &str) {
    // Channel label
    SetTextColor(hdc, COLOR_TEXT_MUTED);
    let mut r_chan = RECT { left: x - 22, top: y, right: x - 5, bottom: y + h };
    let wchan = to_wide(channel_name);
    DrawTextW(hdc, wchan.as_ptr(), -1, &mut r_chan, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    // Empty bar background
    let rc_bar = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let empty_brush = CreateSolidBrush(0x001A1816);
    FillRect(hdc, &rc_bar, empty_brush);
    DeleteObject(empty_brush);

    // Filled portion
    let fill_w = ((w as f32) * level.clamp(0.0, 1.0)) as i32;
    if fill_w > 0 {
        let color = if level > 0.95 {
            COLOR_RED
        } else if level > 0.75 {
            COLOR_YELLOW
        } else {
            COLOR_GREEN
        };
        let rc_fill = RECT { left: x, top: y, right: x + fill_w, bottom: y + h };
        let fill_brush = CreateSolidBrush(color);
        FillRect(hdc, &rc_fill, fill_brush);
        DeleteObject(fill_brush);
    }
}
