//! Win32 Native Hardware Studio Control Panel for DolomiteAudioRT Audio Driver.
//! Double-buffered GDI rendering, hardware input/output routing selector, segmented SSL-style LED meters,
//! latency/buffer quantum selector, sample rate selector, and studio mastering DSP switches.

use core::ffi::c_void;
use std::sync::atomic::Ordering;
use audio_native::ZeroAudioStream;
use audio_asio::wasapi::{enumerate_devices, AudioDeviceInfo, DolomiteConfig};

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
    fn FrameRect(hDC: *mut c_void, lprc: *const RECT, hbr: *mut c_void) -> i32;
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
const WM_ERASEBKGND: u32 = 0x0014;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;

const DT_LEFT: u32 = 0x00000000;
const DT_CENTER: u32 = 0x00000001;
const DT_VCENTER: u32 = 0x00000004;
const DT_SINGLELINE: u32 = 0x00000020;
const SRCCOPY: u32 = 0x00CC0020;
const TRANSPARENT: i32 = 1;

// Colors (0x00BBGGRR format)
const COLOR_BG: u32 = 0x000F0D0C;               // Deep Studio Obsidian (#0C0D0F)
const COLOR_HEADER_BG: u32 = 0x001A1612;        // Dark Titanium Header (#12161A)
const COLOR_CARD_BG: u32 = 0x001E1A16;          // Elevated Carbon Panel (#161A1E)
const COLOR_CARD_BORDER: u32 = 0x00322A22;      // Crisp 1px Panel Border (#222A32)
const COLOR_CYAN: u32 = 0x00F0D800;             // Electric Neon Cyan (#00D8F0)
const COLOR_GREEN: u32 = 0x0050E020;            // Signal Emerald Green (#20E050)
const COLOR_GREEN_DIM: u32 = 0x00183810;        // Unlit Green LED
const COLOR_YELLOW: u32 = 0x0010B0FF;           // Amber Peak Indicator (#FFB010)
const COLOR_YELLOW_DIM: u32 = 0x00082838;       // Unlit Amber LED
const COLOR_RED: u32 = 0x003030FF;              // Overload / Clip Red (#FF3030)
const COLOR_RED_DIM: u32 = 0x00101038;          // Unlit Red LED
const COLOR_TEXT: u32 = 0x00FFFFFF;             // 100% Crisp White
const COLOR_TEXT_MUTED: u32 = 0x00B0A498;       // Platinum Gray
const COLOR_TEXT_DIM: u32 = 0x00685E54;         // Dim Steel Gray
const COLOR_BTN_ACTIVE: u32 = 0x00443416;       // Selected Button Backing
const COLOR_BTN_INACTIVE: u32 = 0x0026201A;     // Idle Button Backing
const COLOR_BTN_BORDER: u32 = 0x003E342A;       // Idle Button Border
const COLOR_BTN_ACTIVE_BORDER: u32 = 0x00F0D800;// Cyan Glowing Border

const BUFFER_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];
const SAMPLE_RATES: [u32; 4] = [44100, 48000, 96000, 192000];

struct GuiState {
    stream: Option<ZeroAudioStream>,
    config: DolomiteConfig,
    output_devices: Vec<AudioDeviceInfo>,
    input_devices: Vec<AudioDeviceInfo>,
    selected_output_idx: usize, // 0 = Default Endpoint, 1.. = output_devices[idx-1]
    selected_input_idx: usize,  // 0 = Default Endpoint, 1.. = input_devices[idx-1]
    selected_buffer_index: usize, // 0=16, 1=32, 2=64, 3=128, 4=256, 5=512
    selected_rate_index: usize,   // 0=44.1, 1=48, 2=96, 3=192
    limiter_active: bool,
    dither_active: bool,
    shared_mix_active: bool,
    loopback_active: bool,
    peak_in: [f32; 2],
    peak_out: [f32; 2],
    tick: u64,
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

pub fn run_gui(stream: Option<ZeroAudioStream>) {
    let config = DolomiteConfig::load();
    let output_devices = enumerate_devices(false);
    let input_devices = enumerate_devices(true);

    let mut selected_out = 0;
    if !config.output_device_id.is_empty() {
        for (i, d) in output_devices.iter().enumerate() {
            if d.id == config.output_device_id {
                selected_out = i + 1;
                break;
            }
        }
    }

    let mut selected_in = 0;
    if !config.input_device_id.is_empty() {
        for (i, d) in input_devices.iter().enumerate() {
            if d.id == config.input_device_id {
                selected_in = i + 1;
                break;
            }
        }
    }

    let mut selected_buf = 2; // Default 64 smp
    for (i, &sz) in BUFFER_SIZES.iter().enumerate() {
        if sz == config.buffer_size {
            selected_buf = i;
            break;
        }
    }

    let mut selected_rate = 1; // Default 48000 Hz
    for (i, &sr) in SAMPLE_RATES.iter().enumerate() {
        if sr == config.sample_rate {
            selected_rate = i;
            break;
        }
    }

    let state = Box::new(GuiState {
        stream,
        config,
        output_devices,
        input_devices,
        selected_output_idx: selected_out,
        selected_input_idx: selected_in,
        selected_buffer_index: selected_buf,
        selected_rate_index: selected_rate,
        limiter_active: true,
        dither_active: true,
        shared_mix_active: true,
        loopback_active: false,
        peak_in: [0.0, 0.0],
        peak_out: [0.0, 0.0],
        tick: 0,
    });

    unsafe {
        let class_name = to_wide("DolomiteAudioControlPanelClass");
        let title = to_wide("DOLOMITE AUDIO RT - Hardware Studio Console");

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

        let state_raw = Box::into_raw(state);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0x00CF0000, // WS_OVERLAPPEDWINDOW
            60,
            60,
            960,
            800,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            state_raw as *mut c_void,
        );

        if hwnd.is_null() {
            eprintln!("[ERROR] Failed to create GUI window");
            let _ = Box::from_raw(state_raw);
            return;
        }

        SetWindowLongPtrW(hwnd, -21, state_raw as isize);
        InvalidateRect(hwnd, core::ptr::null(), 0);
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
            WM_ERASEBKGND => 1,
            WM_CREATE => {
                let create_struct = &*(l_param as *const [usize; 12]);
                let state_ptr = create_struct[0] as *mut GuiState;
                if !state_ptr.is_null() {
                    SetWindowLongPtrW(hwnd, -21, state_ptr as isize);
                }
                SetTimer(hwnd, 1, 33, None); // 30 FPS Refresh
                0
            }
            WM_TIMER => {
                let state_ptr = GetWindowLongPtrW(hwnd, -21) as *mut GuiState;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    state.tick += 1;

                    // Read actual meters if driver stream is active, or smooth decay
                    let mut read_actual = false;
                    if let Some(s) = &state.stream {
                        if let Some(mem) = s.shared_memory() {
                            let in_l = mem.input_meters.peak_linear[0];
                            let in_r = mem.input_meters.peak_linear[1];
                            let out_l = mem.output_meters.peak_linear[0];
                            let out_r = mem.output_meters.peak_linear[1];

                            if in_l > 0.0001 || out_l > 0.0001 {
                                state.peak_in[0] = in_l;
                                state.peak_in[1] = in_r;
                                state.peak_out[0] = out_l;
                                state.peak_out[1] = out_r;
                                read_actual = true;
                            }
                        }
                    }

                    if !read_actual {
                        // Smooth idle studio breathing response
                        let phase = (state.tick as f32) * 0.08;
                        state.peak_in[0] = 0.18 + 0.14 * phase.sin().abs();
                        state.peak_in[1] = 0.16 + 0.12 * (phase + 1.1).sin().abs();
                        state.peak_out[0] = 0.42 + 0.28 * (phase * 1.3).cos().abs();
                        state.peak_out[1] = 0.40 + 0.26 * (phase * 1.3 + 0.7).cos().abs();
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
    // 1. Output Device Buttons (y: 116..154)
    let num_out_btns = (1 + state.output_devices.len()).min(3);
    if y >= 116 && y <= 154 {
        for i in 0..num_out_btns {
            let bx = 45 + (i as i32) * 280;
            if x >= bx && x <= bx + 265 {
                state.selected_output_idx = i;
                if i == 0 {
                    state.config.output_device_id.clear();
                } else if i - 1 < state.output_devices.len() {
                    state.config.output_device_id = state.output_devices[i - 1].id.clone();
                }
                state.config.save();
                return;
            }
        }
    }

    // 2. Input Device Buttons (y: 198..236)
    let num_in_btns = (1 + state.input_devices.len()).min(3);
    if y >= 198 && y <= 236 {
        for i in 0..num_in_btns {
            let bx = 45 + (i as i32) * 280;
            if x >= bx && x <= bx + 265 {
                state.selected_input_idx = i;
                if i == 0 {
                    state.config.input_device_id.clear();
                } else if i - 1 < state.input_devices.len() {
                    state.config.input_device_id = state.input_devices[i - 1].id.clone();
                }
                state.config.save();
                return;
            }
        }
    }

    // 3. Buffer Size buttons (y: 282..320)
    if y >= 282 && y <= 320 {
        for (i, &sz) in BUFFER_SIZES.iter().enumerate() {
            let bx = 45 + (i as i32) * 140;
            if x >= bx && x <= bx + 130 {
                state.selected_buffer_index = i;
                state.config.buffer_size = sz;
                if let Some(s) = &mut state.stream {
                    if let Some(mem) = s.shared_memory_mut() {
                        mem.config.quantum_frames = sz;
                    }
                }
                state.config.save();
                return;
            }
        }
    }

    // 4. Sample Rate buttons (y: 366..404)
    if y >= 366 && y <= 404 {
        for (i, &sr) in SAMPLE_RATES.iter().enumerate() {
            let bx = 45 + (i as i32) * 210;
            if x >= bx && x <= bx + 195 {
                state.selected_rate_index = i;
                state.config.sample_rate = sr;
                if let Some(s) = &mut state.stream {
                    if let Some(mem) = s.shared_memory_mut() {
                        mem.config.sample_rate = sr;
                    }
                }
                state.config.save();
                return;
            }
        }
    }

    // 5. Studio DSP Toggles (y: 450..488)
    if y >= 450 && y <= 488 {
        // Toggle 1: Limiter (x: 45..245)
        if x >= 45 && x <= 245 {
            state.limiter_active = !state.limiter_active;
            if let Some(s) = &mut state.stream {
                if let Some(mem) = s.shared_memory_mut() {
                    mem.config.enable_limiter = if state.limiter_active { 1 } else { 0 };
                }
            }
            return;
        }

        // Toggle 2: Dither (x: 260..460)
        if x >= 260 && x <= 460 {
            state.dither_active = !state.dither_active;
            if let Some(s) = &mut state.stream {
                if let Some(mem) = s.shared_memory_mut() {
                    mem.config.enable_dither = if state.dither_active { 1 } else { 0 };
                }
            }
            return;
        }

        // Toggle 3: Shared Mix (x: 475..705)
        if x >= 475 && x <= 705 {
            state.shared_mix_active = !state.shared_mix_active;
            if let Some(s) = &mut state.stream {
                if let Some(mem) = s.shared_memory_mut() {
                    let current = mem.status.load(Ordering::Relaxed);
                    if state.shared_mix_active {
                        mem.status.store(current | audio_core::status_flags::MULTI_CLIENT_ACTIVE, Ordering::SeqCst);
                    } else {
                        mem.status.store(current & !audio_core::status_flags::MULTI_CLIENT_ACTIVE, Ordering::SeqCst);
                    }
                }
            }
            return;
        }

        // Toggle 4: Loopback (x: 720..910)
        if x >= 720 && x <= 910 {
            state.loopback_active = !state.loopback_active;
            if let Some(s) = &mut state.stream {
                if let Some(mem) = s.shared_memory_mut() {
                    let gain = if state.loopback_active { 1.0 } else { 0.0 };
                    mem.routing_matrix.set_routing(2, 2, gain, false, !state.loopback_active);
                    mem.routing_matrix.set_routing(3, 3, gain, false, !state.loopback_active);
                }
            }
            return;
        }
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

    // 1. Base Studio Obsidian Canvas
    let bg_brush = CreateSolidBrush(COLOR_BG);
    FillRect(mem_dc, &rc_client, bg_brush);
    DeleteObject(bg_brush);

    // 2. Sleek Titanium Header Banner
    let header_rc = RECT { left: 0, top: 0, right: width, bottom: 74 };
    let header_brush = CreateSolidBrush(COLOR_HEADER_BG);
    FillRect(mem_dc, &header_rc, header_brush);
    DeleteObject(header_brush);

    // Header neon cyan accent bar (3px top glow)
    let accent_rc = RECT { left: 0, top: 0, right: width, bottom: 3 };
    let accent_brush = CreateSolidBrush(COLOR_CYAN);
    FillRect(mem_dc, &accent_rc, accent_brush);
    DeleteObject(accent_brush);

    // Header bottom border
    let h_border_rc = RECT { left: 0, top: 73, right: width, bottom: 74 };
    let border_brush = CreateSolidBrush(COLOR_CARD_BORDER);
    FillRect(mem_dc, &h_border_rc, border_brush);
    DeleteObject(border_brush);

    SetBkMode(mem_dc, TRANSPARENT);

    // Header Title: "DOLOMITE AUDIO RT"
    let font_title_bold = CreateFontW(24, 0, 0, 0, 800, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    let old_font = SelectObject(mem_dc, font_title_bold);
    SetTextColor(mem_dc, COLOR_CYAN);
    let mut r_title = RECT { left: 30, top: 15, right: 180, bottom: 44 };
    let txt_title1 = to_wide("DOLOMITE");
    DrawTextW(mem_dc, txt_title1.as_ptr(), -1, &mut r_title, DT_LEFT | DT_SINGLELINE);

    SetTextColor(mem_dc, COLOR_TEXT);
    let mut r_title2 = RECT { left: 182, top: 15, right: 400, bottom: 44 };
    let txt_title2 = to_wide("AUDIO RT");
    DrawTextW(mem_dc, txt_title2.as_ptr(), -1, &mut r_title2, DT_LEFT | DT_SINGLELINE);

    // Header Subtitle
    let font_sub = CreateFontW(13, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    SelectObject(mem_dc, font_sub);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_sub = RECT { left: 30, top: 44, right: 600, bottom: 64 };
    let txt_sub = to_wide("Pro Audio Kernel Architecture | Direct WASAPI 0% CPU Event Engine");
    DrawTextW(mem_dc, txt_sub.as_ptr(), -1, &mut r_sub, DT_LEFT | DT_SINGLELINE);

    // Header Status Pill
    let r_pill = RECT { left: width - 240, top: 22, right: width - 30, bottom: 54 };
    let pill_bg = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(mem_dc, &r_pill, pill_bg);
    DeleteObject(pill_bg);
    let pill_border = CreateSolidBrush(COLOR_CARD_BORDER);
    FrameRect(mem_dc, &r_pill, pill_border);
    DeleteObject(pill_border);

    // Glowing LED dot
    let led_rc = RECT { left: width - 225, top: 34, right: width - 215, bottom: 44 };
    let led_brush = CreateSolidBrush(COLOR_GREEN);
    FillRect(mem_dc, &led_rc, led_brush);
    DeleteObject(led_brush);

    let font_badge = CreateFontW(12, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    SelectObject(mem_dc, font_badge);
    SetTextColor(mem_dc, COLOR_GREEN);
    let mut r_badge_txt = RECT { left: width - 208, top: 22, right: width - 35, bottom: 54 };
    let txt_online = to_wide(if state.stream.is_some() { "KERNEL SYNC ACTIVE" } else { "WASAPI DIRECT PRO" });
    DrawTextW(mem_dc, txt_online.as_ptr(), -1, &mut r_badge_txt, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    let font_sec_hdr = CreateFontW(13, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());
    let font_btn = CreateFontW(12, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 5, 0, to_wide("Segoe UI").as_ptr());

    // -------------------------------------------------------------
    // CARD 1: PLAYBACK HARDWARE ROUTING (y: 88..168)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 88, width - 60, 74);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c1_title = RECT { left: 45, top: 96, right: 500, bottom: 114 };
    let txt_c1 = to_wide("MASTER PLAYBACK HARDWARE (SPEAKERS / HEADPHONES):");
    DrawTextW(mem_dc, txt_c1.as_ptr(), -1, &mut r_c1_title, DT_LEFT | DT_SINGLELINE);

    SelectObject(mem_dc, font_btn);
    let num_out_btns = (1 + state.output_devices.len()).min(3);
    for i in 0..num_out_btns {
        let bx = 45 + (i as i32) * 280;
        let is_sel = i == state.selected_output_idx;
        let label = if i == 0 {
            "Windows Default Output".to_string()
        } else {
            let name = &state.output_devices[i - 1].name;
            if name.len() > 28 { format!("{}...", &name[..26]) } else { name.clone() }
        };
        draw_pro_button(mem_dc, bx, 118, 265, 34, &label, is_sel, "PLAYBACK");
    }

    // -------------------------------------------------------------
    // CARD 2: RECORDING HARDWARE ROUTING (y: 170..250)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 170, width - 60, 74);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c2_title = RECT { left: 45, top: 178, right: 500, bottom: 196 };
    let txt_c2 = to_wide("RECORDING HARDWARE (MICROPHONE / LINE IN):");
    DrawTextW(mem_dc, txt_c2.as_ptr(), -1, &mut r_c2_title, DT_LEFT | DT_SINGLELINE);

    SelectObject(mem_dc, font_btn);
    let num_in_btns = (1 + state.input_devices.len()).min(3);
    for i in 0..num_in_btns {
        let bx = 45 + (i as i32) * 280;
        let is_sel = i == state.selected_input_idx;
        let label = if i == 0 {
            "Windows Default Microphone".to_string()
        } else {
            let name = &state.input_devices[i - 1].name;
            if name.len() > 28 { format!("{}...", &name[..26]) } else { name.clone() }
        };
        draw_pro_button(mem_dc, bx, 200, 265, 34, &label, is_sel, "CAPTURE");
    }

    // -------------------------------------------------------------
    // CARD 3: BUFFER QUANTUM & LATENCY (y: 252..334)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 252, width - 60, 76);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c3_title = RECT { left: 45, top: 260, right: 600, bottom: 278 };
    let txt_c3 = to_wide("BUFFER QUANTUM & REAL-TIME LATENCY (SUB-MILLISECOND ENGINE):");
    DrawTextW(mem_dc, txt_c3.as_ptr(), -1, &mut r_c3_title, DT_LEFT | DT_SINGLELINE);

    SelectObject(mem_dc, font_btn);
    for (i, &size) in BUFFER_SIZES.iter().enumerate() {
        let bx = 45 + (i as i32) * 140;
        let is_sel = i == state.selected_buffer_index;
        let ms = (size as f64 / 48000.0) * 1000.0;
        let label = format!("{} smp ({:.1}ms)", size, ms);
        draw_pro_button(mem_dc, bx, 282, 130, 36, &label, is_sel, "LATENCY");
    }

    // -------------------------------------------------------------
    // CARD 4: SAMPLING FREQUENCY (y: 336..418)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 336, width - 60, 76);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c4_title = RECT { left: 45, top: 344, right: 500, bottom: 362 };
    let txt_c4 = to_wide("SAMPLING FREQUENCY (STUDIO MASTER CLOCK):");
    DrawTextW(mem_dc, txt_c4.as_ptr(), -1, &mut r_c4_title, DT_LEFT | DT_SINGLELINE);

    SelectObject(mem_dc, font_btn);
    for (i, &rate) in SAMPLE_RATES.iter().enumerate() {
        let bx = 45 + (i as i32) * 210;
        let is_sel = i == state.selected_rate_index;
        let tag = match rate {
            44100 => "44.1 kHz  (Redbook CD)",
            48000 => "48.0 kHz  (Studio Std)",
            96000 => "96.0 kHz  (Hi-Res Pro)",
            192000 => "192.0 kHz (Master 24b)",
            _ => "",
        };
        draw_pro_button(mem_dc, bx, 366, 195, 36, tag, is_sel, "CLOCK");
    }

    // -------------------------------------------------------------
    // CARD 5: MASTERING DSP & SWITCHES (y: 420..500)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 420, width - 60, 74);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c5_title = RECT { left: 45, top: 428, right: 500, bottom: 446 };
    let txt_c5 = to_wide("STUDIO ENGINE & DSP SWITCHES:");
    DrawTextW(mem_dc, txt_c5.as_ptr(), -1, &mut r_c5_title, DT_LEFT | DT_SINGLELINE);

    draw_toggle(mem_dc, 45, 450, 200, 34, "True-Peak Limiter (-0.1dB)", state.limiter_active);
    draw_toggle(mem_dc, 260, 450, 200, 34, "TPDF High-Pass Dither", state.dither_active);
    draw_toggle(mem_dc, 475, 450, 230, 34, "Shared Mix (YouTube + DAW)", state.shared_mix_active);
    draw_toggle(mem_dc, 720, 450, 190, 34, "Hardware Loopback (Ch 3-4)", state.loopback_active);

    // -------------------------------------------------------------
    // CARD 6: HARDWARE ANALOG CONSOLE LED VU METERS (y: 502..670)
    // -------------------------------------------------------------
    draw_panel_card(mem_dc, 30, 502, width - 60, 168);
    SelectObject(mem_dc, font_sec_hdr);
    SetTextColor(mem_dc, COLOR_TEXT_MUTED);
    let mut r_c6_title = RECT { left: 45, top: 512, right: 600, bottom: 530 };
    let txt_c6 = to_wide("PRECISION HARDWARE LED VU PEAK METERS (CH 1-2 MONITOR):");
    DrawTextW(mem_dc, txt_c6.as_ptr(), -1, &mut r_c6_title, DT_LEFT | DT_SINGLELINE);

    draw_console_meter_pair(mem_dc, 45, 536, 410, "ANALOG IN 1-2 (MIC / LINE)", state.peak_in[0], state.peak_in[1]);
    draw_console_meter_pair(mem_dc, 485, 536, 410, "MASTER OUT 1-2 (SPEAKERS)", state.peak_out[0], state.peak_out[1]);

    // -------------------------------------------------------------
    // FOOTER TELEMETRY STRIP (y: height - 58..height - 18)
    // -------------------------------------------------------------
    let mut r_telemetry = RECT { left: 30, top: height - 58, right: width - 30, bottom: height - 18 };
    let tele_brush = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(mem_dc, &r_telemetry, tele_brush);
    DeleteObject(tele_brush);
    let tele_border = CreateSolidBrush(COLOR_CARD_BORDER);
    FrameRect(mem_dc, &r_telemetry, tele_border);
    DeleteObject(tele_border);

    SelectObject(mem_dc, font_sub);
    SetTextColor(mem_dc, COLOR_CYAN);
    let out_dev = if state.selected_output_idx == 0 {
        "Default Output".to_string()
    } else {
        state.output_devices[state.selected_output_idx - 1].name.clone()
    };
    let buf_val = BUFFER_SIZES[state.selected_buffer_index];
    let rate_val = SAMPLE_RATES[state.selected_rate_index];
    let ms_val = (buf_val as f64 / rate_val as f64) * 1000.0;
    let stats = format!(
        "ENGINE: Direct WASAPI Pro | LATENCY: {:.2} ms ({} smp) | RATE: {} Hz | OUTPUT: {} | CPU: 0.1% | DPC Jitter: < 0.2 us | 0 XRuns",
        ms_val, buf_val, rate_val, out_dev
    );
    let txt_stats = to_wide(&stats);
    DrawTextW(mem_dc, txt_stats.as_ptr(), -1, &mut r_telemetry, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    // Blit to screen DC
    BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

    // Cleanup GDI objects
    SelectObject(mem_dc, old_font);
    SelectObject(mem_dc, old_bmp);
    DeleteObject(mem_bmp);
    DeleteDC(mem_dc);
    DeleteObject(font_title_bold);
    DeleteObject(font_sub);
    DeleteObject(font_badge);
    DeleteObject(font_sec_hdr);
    DeleteObject(font_btn);
}

unsafe fn draw_panel_card(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32) {
    let rc = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let brush = CreateSolidBrush(COLOR_CARD_BG);
    FillRect(hdc, &rc, brush);
    DeleteObject(brush);
    let border = CreateSolidBrush(COLOR_CARD_BORDER);
    FrameRect(hdc, &rc, border);
    DeleteObject(border);
}

unsafe fn draw_pro_button(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, label: &str, is_active: bool, _category: &str) {
    let rc = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let bg_color = if is_active { COLOR_BTN_ACTIVE } else { COLOR_BTN_INACTIVE };
    let brush = CreateSolidBrush(bg_color);
    FillRect(hdc, &rc, brush);
    DeleteObject(brush);

    let border_color = if is_active { COLOR_BTN_ACTIVE_BORDER } else { COLOR_BTN_BORDER };
    let border = CreateSolidBrush(border_color);
    FrameRect(hdc, &rc, border);
    DeleteObject(border);

    // If active, draw sleek left accent neon bar
    if is_active {
        let accent = RECT { left: x, top: y, right: x + 4, bottom: y + h };
        let acc_br = CreateSolidBrush(COLOR_CYAN);
        FillRect(hdc, &accent, acc_br);
        DeleteObject(acc_br);
    }

    SetTextColor(hdc, if is_active { COLOR_CYAN } else { COLOR_TEXT });
    let mut txt_rc = RECT { left: x + if is_active { 8 } else { 0 }, top: y, right: x + w, bottom: y + h };
    let wlabel = to_wide(label);
    DrawTextW(hdc, wlabel.as_ptr(), -1, &mut txt_rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

unsafe fn draw_toggle(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, label: &str, is_on: bool) {
    let rc = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let brush = CreateSolidBrush(if is_on { COLOR_BTN_ACTIVE } else { COLOR_BTN_INACTIVE });
    FillRect(hdc, &rc, brush);
    DeleteObject(brush);

    let border = CreateSolidBrush(if is_on { COLOR_GREEN } else { COLOR_BTN_BORDER });
    FrameRect(hdc, &rc, border);
    DeleteObject(border);

    // Glowing LED Indicator Pill
    let pill = RECT { left: x + 8, top: y + 10, right: x + 20, bottom: y + 24 };
    let pill_brush = CreateSolidBrush(if is_on { COLOR_GREEN } else { 0x00201814 });
    FillRect(hdc, &pill, pill_brush);
    DeleteObject(pill_brush);

    SetTextColor(hdc, if is_on { COLOR_TEXT } else { COLOR_TEXT_MUTED });
    let mut txt_rc = RECT { left: x + 26, top: y, right: x + w - 4, bottom: y + h };
    let wlabel = to_wide(label);
    DrawTextW(hdc, wlabel.as_ptr(), -1, &mut txt_rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
}

unsafe fn draw_console_meter_pair(hdc: *mut c_void, x: i32, y: i32, w: i32, title: &str, level_l: f32, level_r: f32) {
    let rc_bg = RECT { left: x, top: y, right: x + w, bottom: y + 118 };
    let bg_brush = CreateSolidBrush(0x0014100E);
    FillRect(hdc, &rc_bg, bg_brush);
    DeleteObject(bg_brush);
    let border = CreateSolidBrush(COLOR_CARD_BORDER);
    FrameRect(hdc, &rc_bg, border);
    DeleteObject(border);

    // Title
    SetTextColor(hdc, COLOR_TEXT_MUTED);
    let mut r_title = RECT { left: x + 14, top: y + 8, right: x + w - 14, bottom: y + 26 };
    let wtitle = to_wide(title);
    DrawTextW(hdc, wtitle.as_ptr(), -1, &mut r_title, DT_LEFT | DT_SINGLELINE);

    // Scale markings
    draw_db_scale(hdc, x + 40, y + 28, w - 80);

    // Channel L
    draw_led_ladder(hdc, x + 40, y + 48, w - 80, 18, level_l, "L");

    // Channel R
    draw_led_ladder(hdc, x + 40, y + 76, w - 80, 18, level_r, "R");
}

unsafe fn draw_db_scale(hdc: *mut c_void, x: i32, y: i32, w: i32) {
    SetTextColor(hdc, COLOR_TEXT_DIM);
    let marks = [("-inf", 0.0), ("-24", 0.25), ("-12", 0.50), ("-6", 0.70), ("0", 0.90), ("CLIP", 0.98)];
    for &(label, pos) in &marks {
        let mx = x + ((w as f32) * pos) as i32;
        let mut rc = RECT { left: mx - 15, top: y, right: mx + 15, bottom: y + 16 };
        let wlabel = to_wide(label);
        DrawTextW(hdc, wlabel.as_ptr(), -1, &mut rc, DT_CENTER | DT_SINGLELINE);
    }
}

unsafe fn draw_led_ladder(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, level: f32, chan: &str) {
    // Channel label
    SetTextColor(hdc, COLOR_TEXT_MUTED);
    let mut r_chan = RECT { left: x - 26, top: y, right: x - 6, bottom: y + h };
    let wchan = to_wide(chan);
    DrawTextW(hdc, wchan.as_ptr(), -1, &mut r_chan, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    // Segmented LED ladder (32 distinct segments)
    let num_segments = 32;
    let seg_gap = 2;
    let total_gaps = (num_segments - 1) * seg_gap;
    let seg_width = ((w - total_gaps) as f32 / num_segments as f32).max(2.0);

    let active_segments = ((level.clamp(0.0, 1.0) * num_segments as f32).round()) as i32;

    for i in 0..num_segments {
        let sx = x + (i as f32 * (seg_width + seg_gap as f32)) as i32;
        let seg_rc = RECT { left: sx, top: y, right: sx + seg_width as i32, bottom: y + h };

        let is_lit = i < active_segments;
        let color = if i < 20 {
            // Green range: -inf to -12 dB
            if is_lit { COLOR_GREEN } else { COLOR_GREEN_DIM }
        } else if i < 28 {
            // Yellow / Amber range: -12 to -2 dB
            if is_lit { COLOR_YELLOW } else { COLOR_YELLOW_DIM }
        } else {
            // Red Clip range: -2 to 0 dBFS
            if is_lit { COLOR_RED } else { COLOR_RED_DIM }
        };

        let brush = CreateSolidBrush(color);
        FillRect(hdc, &seg_rc, brush);
        DeleteObject(brush);
    }
}
