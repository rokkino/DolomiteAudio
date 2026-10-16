//! Embedded HTTP server + API for the DolomiteAudioRT control panel.
//!
//! * `GET  /`              – single-page UI
//! * `GET  /api/devices`   – WASAPI render/capture endpoints (enumerated on demand)
//! * `GET  /api/config`    – persisted settings
//! * `POST /api/config`    – save settings (the running driver picks up device /
//!                           engine-mode changes and asks the DAW to reset)
//! * `GET  /api/events`    – Server-Sent Events stream of live engine telemetry
//!                           (~30 fps), read from the driver's shared memory
//! * `POST /api/test_sound`– short chime on the selected output
//! * `GET/POST /api/pin`   – keep the tray flyout open when it loses focus
//! * `POST /api/hide`      – hide the flyout (it stays in the tray)

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use audio_asio::live_status::{LiveStatus, LiveStatusMap};
use audio_asio::wasapi::{enumerate_devices, play_blocking, DolomiteConfig};
use audio_native::ZeroAudioStream;

type Shared = Arc<Mutex<Option<ZeroAudioStream>>>;

/// Flyout behaviour, shared with the tray window.
pub static PINNED: AtomicBool = AtomicBool::new(false);
pub static HIDE_REQUEST: AtomicBool = AtomicBool::new(false);

pub fn start_control_server(stream_opt: Option<ZeroAudioStream>, port: u16, last_hb: Arc<AtomicI64>) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
    let stream_arc: Shared = Arc::new(Mutex::new(stream_opt));

    thread::spawn(move || {
        for conn in listener.incoming() {
            if let Ok(mut socket) = conn {
                let s = stream_arc.clone();
                let hb = last_hb.clone();
                thread::spawn(move || handle_client(&mut socket, &s, &hb));
            }
        }
    });
    Ok(())
}

fn now_ns() -> i64 {
    audio_core::current_time_nanos()
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        ctype,
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn read_request(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(hdr_end) = text.find("\r\n\r\n") {
            let len = text[..hdr_end]
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if buf.len() >= hdr_end + 4 + len {
                break;
            }
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn handle_client(stream: &mut TcpStream, shared: &Shared, last_hb: &Arc<AtomicI64>) {
    last_hb.store(now_ns(), Ordering::Relaxed);
    let req = match read_request(stream) {
        Some(r) => r,
        None => return,
    };
    let mut parts = req.lines().next().unwrap_or("").split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let body = req.find("\r\n\r\n").map(|i| &req[i + 4..]).unwrap_or("");

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(stream, "200 OK", "text/html; charset=utf-8", crate::html::INDEX_HTML.as_bytes());
        }
        ("GET", "/api/devices") => {
            let list = |cap: bool| {
                enumerate_devices(cap)
                    .iter()
                    .map(|d| format!("{{\"id\":\"{}\",\"name\":\"{}\"}}", esc(&d.id), esc(&d.name)))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let json = format!("{{\"outputs\":[{}],\"inputs\":[{}]}}", list(false), list(true));
            respond(stream, "200 OK", "application/json", json.as_bytes());
        }
        ("GET", "/api/config") => {
            let c = DolomiteConfig::load();
            let json = format!(
                "{{\"output_device\":\"{}\",\"input_device\":\"{}\",\"buffer_size\":{},\"sample_rate\":{},\"low_latency\":{},\"exclusive\":{},\"auto_release\":{},\"input_enabled\":{}}}",
                esc(&c.output_device_id),
                esc(&c.input_device_id),
                c.buffer_size,
                c.sample_rate,
                c.low_latency,
                c.exclusive,
                c.auto_release,
                c.input_enabled
            );
            respond(stream, "200 OK", "application/json", json.as_bytes());
        }
        ("POST", "/api/config") => {
            let mut cfg = DolomiteConfig::load();
            if let Some(v) = json_str(body, "output_device") {
                cfg.output_device_id = v;
            }
            if let Some(v) = json_str(body, "input_device") {
                cfg.input_device_id = v;
            }
            if let Some(v) = json_u32(body, "buffer_size") {
                cfg.buffer_size = v.clamp(32, 2048);
            }
            if let Some(v) = json_u32(body, "sample_rate") {
                cfg.sample_rate = v;
            }
            if let Some(v) = json_bool(body, "low_latency") {
                cfg.low_latency = v;
            }
            if let Some(v) = json_bool(body, "exclusive") {
                cfg.exclusive = v;
            }
            if let Some(v) = json_bool(body, "auto_release") {
                cfg.auto_release = v;
            }
            if let Some(v) = json_bool(body, "input_enabled") {
                cfg.input_enabled = v;
            }
            cfg.save();
            if let Ok(mut guard) = shared.lock() {
                if let Some(mem) = guard.as_mut().and_then(|s| s.shared_memory_mut()) {
                    mem.config.quantum_frames = cfg.buffer_size;
                    mem.config.sample_rate = cfg.sample_rate;
                }
            }
            respond(stream, "200 OK", "application/json", b"{\"status\":\"ok\"}");
        }
        ("POST", "/api/test_sound") => {
            thread::spawn(play_test_chime);
            respond(stream, "200 OK", "application/json", b"{\"status\":\"ok\"}");
        }
        ("GET", "/api/pin") => {
            let json = format!("{{\"pinned\":{}}}", PINNED.load(Ordering::Relaxed));
            respond(stream, "200 OK", "application/json", json.as_bytes());
        }
        ("POST", "/api/pin") => {
            PINNED.store(json_bool(body, "pinned").unwrap_or(false), Ordering::Relaxed);
            respond(stream, "200 OK", "application/json", b"{\"status\":\"ok\"}");
        }
        ("POST", "/api/hide") => {
            HIDE_REQUEST.store(true, Ordering::Relaxed);
            respond(stream, "200 OK", "application/json", b"{\"status\":\"ok\"}");
        }
        ("GET", "/api/events") => serve_events(stream, shared, last_hb),
        _ => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

/// Server-Sent Events: push a telemetry snapshot ~30 times per second until
/// the window closes (write fails).
fn serve_events(stream: &mut TcpStream, shared: &Shared, last_hb: &Arc<AtomicI64>) {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.set_nodelay(true);
    let is_kernel = shared.lock().map(|g| g.is_some()).unwrap_or(false);
    let mut map: Option<LiveStatusMap> = None;
    let mut host = HostName::default();
    let mut tick = 0u32;

    loop {
        // (Re)attach to the driver's shared block when a DAW opens the driver.
        if map.is_none() && tick % 15 == 0 {
            map = LiveStatusMap::open_read();
        }
        let json = match &map {
            Some(m) => snapshot_json(m.get(), is_kernel, &mut host),
            None => format!("{{\"alive\":false,\"kernel\":{}}}", is_kernel),
        };
        if stream.write_all(format!("data: {}\n\n", json).as_bytes()).is_err() {
            return;
        }
        last_hb.store(now_ns(), Ordering::Relaxed);
        tick = tick.wrapping_add(1);
        thread::sleep(Duration::from_millis(33));
    }
}

#[derive(Default)]
struct HostName {
    pid: u32,
    name: String,
}

fn snapshot_json(s: &LiveStatus, is_kernel: bool, host: &mut HostName) -> String {
    let ld = |a: &std::sync::atomic::AtomicU32| a.load(Ordering::Relaxed);
    let f = LiveStatus::get_f32;
    let pid = ld(&s.host_pid);
    if pid != host.pid {
        host.pid = pid;
        host.name = process_name(pid).unwrap_or_default();
    }
    let v3 = ld(&s.version) >= 3;
    let v3u = |a: &std::sync::atomic::AtomicU32| if v3 { a.load(Ordering::Relaxed) } else { 0 };
    format!(
        concat!(
            "{{\"alive\":{},\"kernel\":{},\"host\":\"{}\",\"device\":\"{}\",\"mode\":{},",
            "\"daw_rate\":{},\"device_rate\":{},\"buffer\":{},\"period\":{},\"queue\":{},",
            "\"out_latency\":{},\"in_latency\":{},\"resampling\":{},\"rs_latency\":{},",
            "\"in_mode\":{},\"in_device\":\"{}\",\"in_rate\":{},\"in_underruns\":{},\"auto_state\":{},\"switches\":{},",
            "\"callbacks\":{},\"underruns\":{},\"late\":{},\"load\":{},\"load_peak\":{},",
            "\"peak\":[{:.5},{:.5},{:.5},{:.5}],\"rms\":[{:.5},{:.5},{:.5},{:.5}],\"clip\":[{},{},{},{}]}}"
        ),
        s.is_alive(),
        is_kernel,
        esc(&host.name),
        esc(&s.device_name()),
        ld(&s.engine_mode),
        ld(&s.daw_rate),
        ld(&s.device_rate),
        ld(&s.buffer_frames),
        ld(&s.device_period_frames),
        ld(&s.queue_target_frames),
        ld(&s.output_latency_frames),
        ld(&s.input_latency_frames),
        ld(&s.resampling) != 0,
        v3u(&s.resampler_latency_frames),
        v3u(&s.input_mode),
        esc(&if v3 { s.input_device_name() } else { String::new() }),
        v3u(&s.input_rate),
        v3u(&s.input_underruns),
        v3u(&s.auto_state),
        v3u(&s.engine_switches),
        s.callbacks.load(Ordering::Relaxed),
        ld(&s.underruns),
        ld(&s.late_wakeups),
        ld(&s.dsp_load_permille),
        ld(&s.dsp_load_peak_permille),
        f(&s.peak[0]),
        f(&s.peak[1]),
        f(&s.peak[2]),
        f(&s.peak[3]),
        f(&s.rms[0]),
        f(&s.rms[1]),
        f(&s.rms[2]),
        f(&s.rms[3]),
        ld(&s.clip[0]),
        ld(&s.clip[1]),
        ld(&s.clip[2]),
        ld(&s.clip[3]),
    )
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
    fn QueryFullProcessImageNameW(h: *mut core::ffi::c_void, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(h: *mut core::ffi::c_void) -> i32;
}

/// "Studio One.exe" -> "Studio One"
fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(0x1000 /* PROCESS_QUERY_LIMITED_INFORMATION */, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        if !ok {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let file = full.rsplit(['\\', '/']).next().unwrap_or(&full);
        Some(file.trim_end_matches(".exe").trim_end_matches(".EXE").to_string())
    }
}

fn json_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\"", key);
    let after = &json[json.find(&pat)? + pat.len()..];
    Some(after[after.find(':')? + 1..].trim_start())
}

fn json_str(json: &str, key: &str) -> Option<String> {
    let v = json_value(json, key)?;
    let mut chars = v.strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

fn json_u32(json: &str, key: &str) -> Option<u32> {
    let v = json_value(json, key)?;
    v.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

fn json_bool(json: &str, key: &str) -> Option<bool> {
    let v = json_value(json, key)?;
    if v.starts_with("true") {
        Some(true)
    } else if v.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// C-major chime (C5-E5-G5) with exponential decay, 0.6 s.
fn play_test_chime() {
    let cfg = DolomiteConfig::load();
    let id = (!cfg.output_device_id.is_empty()).then_some(cfg.output_device_id.as_str());
    let _ = play_blocking(id, 0.6, |n, rate| {
        let t = n as f64 / rate;
        let env = (-5.0 * t).exp() * (t * 400.0).min(1.0);
        let tau = std::f64::consts::TAU;
        let s = ((tau * 523.25 * t).sin() * 0.12 + (tau * 659.25 * t).sin() * 0.10 + (tau * 783.99 * t).sin() * 0.08) * env;
        (s as f32, s as f32)
    });
}
