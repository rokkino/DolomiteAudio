//! End-to-end ASIO driver self test ("fake DAW").
//!
//! Loads an `audio_asio.dll` by path exactly like a DAW does (DllGetClassObject
//! -> IClassFactory -> IASIO), renders a pure 1 kHz sine through it, and at the
//! same time records what really reaches the sound card with WASAPI loopback.
//! The recording is analysed for glitches (dropouts / skips / clicks) and
//! THD+N, and the callback rate is compared with real time.
//!
//! Usage: asio_selftest <path\to\audio_asio.dll> <daw_rate> <buffer> [seconds]

use audio_asio::asio_types::*;
use audio_asio::com_interface::{IASIOVtbl, CLSID_ZERO_LATENCY_AUDIO};
use audio_asio::wasapi::*;
use core::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(p: *const u16) -> *mut c_void;
    fn GetProcAddress(h: *mut c_void, n: *const u8) -> *mut c_void;
    fn Sleep(ms: u32);
}
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(r: *mut c_void, m: u32) -> i32;
    fn CoCreateInstance(c: *const GUID, o: *mut c_void, ctx: u32, i: *const GUID, p: *mut *mut c_void) -> i32;
}

#[repr(C)]
struct IClassFactoryVtbl {
    qi: usize,
    add_ref: usize,
    release: usize,
    create_instance: extern "system" fn(*mut c_void, *mut c_void, *const [u8; 16], *mut *mut c_void) -> i32,
    lock_server: usize,
}

// ---------------- fake DAW state (callbacks are plain C functions) -------------
static OUT_L: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static OUT_R: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static BLOCK: AtomicUsize = AtomicUsize::new(0);
static RATE_BITS: AtomicU64 = AtomicU64::new(0);
static PHASE_N: AtomicU64 = AtomicU64::new(0);
static CALLBACKS: AtomicU64 = AtomicU64::new(0);
static LOAD_NS: AtomicU64 = AtomicU64::new(0); // simulated DAW DSP time per callback
const FREQ: f64 = 1000.0;
const AMP: f64 = 0.1; // -20 dBFS: audible but quiet

fn render(index: i32) {
    let block = BLOCK.load(Ordering::Relaxed);
    let rate = f64::from_bits(RATE_BITS.load(Ordering::Relaxed));
    let n0 = PHASE_N.fetch_add(block as u64, Ordering::Relaxed);
    let i = (index & 1) as usize;
    let l = OUT_L[i].load(Ordering::Relaxed) as *mut f32;
    let r = OUT_R[i].load(Ordering::Relaxed) as *mut f32;
    for k in 0..block {
        let t = (n0 + k as u64) as f64 / rate;
        let v = (AMP * (std::f64::consts::TAU * FREQ * t).sin()) as f32;
        unsafe {
            *l.add(k) = v;
            *r.add(k) = v;
        }
    }
    let busy = LOAD_NS.load(Ordering::Relaxed);
    if busy > 0 {
        let t = std::time::Instant::now();
        while (t.elapsed().as_nanos() as u64) < busy {
            core::hint::spin_loop();
        }
    }
    CALLBACKS.fetch_add(1, Ordering::Relaxed);
}
extern "C" fn cb_switch(index: i32, _direct: ASIOBool) {
    render(index);
}
extern "C" fn cb_switch_ti(_p: *mut c_void, index: i32, _direct: ASIOBool) -> *mut c_void {
    render(index);
    null_mut()
}
extern "C" fn cb_rate(_r: f64) {}
extern "C" fn cb_msg(sel: i32, val: i32, _m: *mut c_void, _o: *mut f64) -> i32 {
    match (sel, val) {
        (1, 7) | (1, 6) | (1, 3) | (1, 2) => 1, // selector supported?
        (2, _) => 2,                              // engine version: ASIO 2
        (7, _) => 1,                              // supports time info
        _ => 0,
    }
}

// ---------------- WASAPI loopback recorder -----------------------------------
#[allow(dead_code)]
struct Loopback {
    client: *mut c_void,
    capture: *mut c_void,
    rate: u32,
    channels: usize,
}

unsafe fn open_loopback(device_id: &str) -> Result<Loopback, String> {
    let mut en: *mut c_void = null_mut();
    let hr = CoCreateInstance(&CLSID_MMDEVICE_ENUMERATOR, null_mut(), 1, &IID_IMMDEVICE_ENUMERATOR, &mut en);
    if hr != 0 {
        return Err(format!("enumerator 0x{:08X}", hr));
    }
    let ev = &**(en as *mut *const IMMDeviceEnumeratorVtbl);
    let mut dev: *mut c_void = null_mut();
    if !device_id.is_empty() {
        let w: Vec<u16> = device_id.encode_utf16().chain(Some(0)).collect();
        (ev.get_device)(en, w.as_ptr(), &mut dev);
    }
    if dev.is_null() && (ev.get_default_audio_endpoint)(en, 0, 0, &mut dev) != 0 {
        return Err("no render device".into());
    }
    let dv = &**(dev as *mut *const IMMDeviceVtbl);
    let mut ac: *mut c_void = null_mut();
    let hr = (dv.activate)(dev, &IID_IAUDIO_CLIENT, 1, null_mut(), &mut ac);
    if hr != 0 {
        return Err(format!("activate 0x{:08X}", hr));
    }
    let av = &**(ac as *mut *const IAudioClientVtbl);
    let mut fmt: *mut c_void = null_mut();
    (av.get_mix_format)(ac, &mut fmt);
    let wfx = &*(fmt as *const WAVEFORMATEX);
    if wfx.w_bits_per_sample != 32 {
        return Err(format!("loopback format {} bit not supported by the test", wfx.w_bits_per_sample));
    }
    let (rate, channels) = (wfx.n_samples_per_sec, wfx.n_channels as usize);
    let hr = (av.initialize)(ac, 0, 0x0002_0000 /* LOOPBACK */, 20_000_000, 0, fmt, std::ptr::null());
    if hr != 0 {
        return Err(format!("loopback init 0x{:08X}", hr));
    }
    let mut cap: *mut c_void = null_mut();
    let hr = (av.get_service)(ac, &IID_IAUDIO_CAPTURE_CLIENT, &mut cap);
    if hr != 0 {
        return Err(format!("capture service 0x{:08X}", hr));
    }
    (av.start)(ac);
    Ok(Loopback { client: ac, capture: cap, rate, channels })
}

unsafe fn drain(lb: &Loopback, out: &mut Vec<f32>) {
    let cv = &**(lb.capture as *mut *const IAudioCaptureClientVtbl);
    let mut pkt = 0u32;
    while (cv.get_next_packet_size)(lb.capture, &mut pkt) == 0 && pkt > 0 {
        let (mut data, mut n, mut flags, mut a, mut b) = (null_mut(), 0u32, 0u32, 0u64, 0u64);
        if (cv.get_buffer)(lb.capture, &mut data, &mut n, &mut flags, &mut a, &mut b) != 0 {
            break;
        }
        let silent = flags & 0x2 != 0;
        for i in 0..n as usize {
            out.push(if silent || data.is_null() { 0.0 } else { *(data as *const f32).add(i * lb.channels) });
        }
        (cv.release_buffer)(lb.capture, n);
    }
}

// ---------------- analysis ----------------------------------------------------
struct Analysis {
    windows: usize,
    glitch_windows: usize,
    thdn_db: f64,
    level_db: f64,
}

fn analyse(sig: &[f32], rate: f64) -> Option<Analysis> {
    let start = sig.iter().position(|v| v.abs() > (AMP as f32) * 0.3)?;
    let end = sig.iter().rposition(|v| v.abs() > (AMP as f32) * 0.3)?;
    let skip = (rate * 0.3) as usize;
    if end < start + 3 * skip {
        return None;
    }
    let s = &sig[start + skip..end - skip];
    let win = (rate / FREQ * 10.0).round() as usize; // 10 periods ~ 10 ms
    let w = std::f64::consts::TAU * FREQ / rate;
    let (mut glitches, mut windows, mut err_tot, mut pow_tot) = (0, 0, 0.0, 0.0);
    for chunk in s.chunks_exact(win) {
        // least-squares fit of a*sin + b*cos + dc per window
        let (mut ss, mut cc, mut sc, mut ys, mut yc) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (i, &y) in chunk.iter().enumerate() {
            let (si, co) = (w * i as f64).sin_cos();
            ss += si * si;
            cc += co * co;
            sc += si * co;
            ys += y as f64 * si;
            yc += y as f64 * co;
        }
        let det = ss * cc - sc * sc;
        let a = (ys * cc - yc * sc) / det;
        let b = (yc * ss - ys * sc) / det;
        let (mut err, mut pow, mut peak_err) = (0.0f64, 0.0f64, 0.0f64);
        for (i, &y) in chunk.iter().enumerate() {
            let (si, co) = (w * i as f64).sin_cos();
            let ideal = a * si + b * co;
            let e = y as f64 - ideal;
            err += e * e;
            pow += ideal * ideal;
            peak_err = peak_err.max(e.abs());
        }
        windows += 1;
        err_tot += err;
        pow_tot += pow;
        // a dropout, skipped sample or click shows up as a large local error
        if peak_err > AMP * 0.05 || pow < 1e-9 {
            glitches += 1;
        }
    }
    Some(Analysis {
        windows,
        glitch_windows: glitches,
        thdn_db: 10.0 * (err_tot / pow_tot.max(1e-12)).log10(),
        level_db: 10.0 * (pow_tot / (windows * win) as f64 * 2.0).log10(),
    })
}

fn write_wav(path: &str, data: &[f32], rate: u32) {
    let mut b = Vec::with_capacity(44 + data.len() * 4);
    let n = (data.len() * 4) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // float
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&n.to_le_bytes());
    for v in data {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let _ = std::fs::write(path, b);
}

fn report(rec: &[f32], rate: u32, speed: f64, wav: &str) {
    write_wav(wav, rec, rate);
    println!("recording saved: {}", wav);
    match analyse(rec, rate as f64) {
        Some(r) => {
            let pct = 100.0 * r.glitch_windows as f64 / r.windows as f64;
            println!("recorded: {} samples, level {:.1} dBFS", rec.len(), r.level_db);
            println!("glitchy 10 ms windows: {} of {} ({:.1} %)", r.glitch_windows, r.windows, pct);
            println!("THD+N: {:.1} dB", r.thdn_db);
            let pass = r.glitch_windows == 0 && r.thdn_db < -60.0 && (speed - 1.0).abs() < 0.02;
            println!("RESULT: {}", if pass { "PASS (clean)" } else { "FAIL (distorted)" });
        }
        None => println!("RESULT: FAIL (no signal reached the sound card; recorded {} samples)", rec.len()),
    }
}

/// Reference: same sine played straight through WASAPI, no ASIO driver at all.
unsafe fn reference(secs: f64, wav: &str) {
    println!("=== REFERENCE: sine via plain WASAPI (no ASIO driver) | {} s ===", secs);
    let cfg = DolomiteConfig::load();
    let lb = match open_loopback(&cfg.output_device_id) {
        Ok(l) => l,
        Err(e) => {
            println!("RESULT: ERROR loopback: {}", e);
            return;
        }
    };
    let id = cfg.output_device_id.clone();
    let player = std::thread::spawn(move || {
        let idr = (!id.is_empty()).then_some(id.as_str());
        play_blocking(idr, secs, |n, rate| {
            let v = (AMP * (std::f64::consts::TAU * FREQ * n as f64 / rate).sin()) as f32;
            (v, v)
        })
    });
    let mut rec = Vec::new();
    while !player.is_finished() {
        drain(&lb, &mut rec);
        Sleep(5);
    }
    for _ in 0..40 {
        drain(&lb, &mut rec);
        Sleep(10);
    }
    if let Ok(Err(e)) = player.join() {
        println!("player error: {}", e);
    }
    report(&rec, lb.rate, 1.0, wav);
}

/// Temporarily applies device/engine settings to the driver config, restoring on drop.
struct ConfigOverride(Option<String>);
impl Drop for ConfigOverride {
    fn drop(&mut self) {
        let path = DolomiteConfig::config_path();
        match &self.0 {
            Some(orig) => {
                let _ = std::fs::write(path, orig);
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn apply_overrides(device: Option<&str>, engine: Option<&str>) -> Option<ConfigOverride> {
    if device.is_none() && engine.is_none() {
        return None;
    }
    let path = DolomiteConfig::config_path();
    let guard = ConfigOverride(std::fs::read_to_string(&path).ok());
    let mut cfg = DolomiteConfig::load();
    if let Some(d) = device {
        let devs = enumerate_devices(false);
        match devs.iter().find(|x| x.name.to_lowercase().contains(&d.to_lowercase())) {
            Some(x) => cfg.output_device_id = x.id.clone(),
            None => println!("WARNING: no output device matching '{}'", d),
        }
    }
    if let Some(e) = engine {
        // "exclusive" = fixed exclusive (a console test is never the
        // foreground app); "auto" = exclusive with live auto-release.
        cfg.low_latency = e != "shared";
        cfg.exclusive = e == "exclusive" || e == "auto";
        cfg.auto_release = e == "auto";
    }
    cfg.save();
    Some(guard)
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    // pull out --device X / --engine Y / --list, keep the positional args
    let (mut args, mut device, mut engine, mut list) = (Vec::new(), None::<String>, None::<String>, false);
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--device" => device = it.next(),
            "--engine" => engine = it.next(),
            "--list" => list = true,
            _ => args.push(a),
        }
    }
    if list {
        unsafe { CoInitializeEx(null_mut(), 0) };
        for d in enumerate_devices(false) {
            println!("output: {}  [{}]", d.name, d.id);
        }
        return;
    }
    let _cfg_guard = apply_overrides(device.as_deref(), engine.as_deref());
    let is_exclusive = matches!(engine.as_deref(), Some("exclusive") | Some("auto"));
    if args.len() >= 2 && args[1] == "--reference" {
        unsafe {
            CoInitializeEx(null_mut(), 0);
            reference(4.0, args.get(2).map(|s| s.as_str()).unwrap_or("selftest_reference.wav"));
        }
        return;
    }
    if args.len() < 4 {
        eprintln!("usage: asio_selftest <audio_asio.dll> <daw_rate> <buffer> [seconds]");
        std::process::exit(2);
    }
    let dll = &args[1];
    let rate: f64 = args[2].parse().unwrap();
    let block: usize = args[3].parse().unwrap();
    let secs: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let load: f64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    LOAD_NS.store((load * block as f64 / rate * 1e9) as u64, Ordering::Relaxed);
    println!(
        "=== ASIO self test: {} | DAW {} Hz | buffer {} | {} s | DAW load {:.0} % | device {} | engine {} ===",
        dll, rate, block, secs, load * 100.0, device.as_deref().unwrap_or("(config)"), engine.as_deref().unwrap_or("(config)")
    );

    unsafe {
        CoInitializeEx(null_mut(), 0);
        let cfg = DolomiteConfig::load();
        let lb = match open_loopback(&cfg.output_device_id) {
            Ok(l) => l,
            Err(e) => {
                println!("RESULT: ERROR loopback: {}", e);
                return;
            }
        };
        println!("loopback recorder: {} Hz, {} ch", lb.rate, lb.channels);

        let w: Vec<u16> = dll.encode_utf16().chain(Some(0)).collect();
        let h = LoadLibraryW(w.as_ptr());
        if h.is_null() {
            println!("RESULT: ERROR cannot load DLL");
            return;
        }
        type GetClassObject = extern "system" fn(*const [u8; 16], *const [u8; 16], *mut *mut c_void) -> i32;
        let gco: GetClassObject = core::mem::transmute(GetProcAddress(h, b"DllGetClassObject\0".as_ptr()));
        let mut factory: *mut c_void = null_mut();
        let iid_cf = [0x01, 0, 0, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46];
        gco(&CLSID_ZERO_LATENCY_AUDIO, &iid_cf, &mut factory);
        let fv = &**(factory as *mut *const IClassFactoryVtbl);
        let mut asio: *mut c_void = null_mut();
        (fv.create_instance)(factory, null_mut(), &CLSID_ZERO_LATENCY_AUDIO, &mut asio);
        let a = asio as *mut audio_asio::com_interface::IASIO;
        let v: &IASIOVtbl = &*(*(asio as *mut *const IASIOVtbl));

        (v.init)(a, null_mut());
        let mut name = [0u8; 32];
        (v.get_driver_name)(a, name.as_mut_ptr());
        println!("driver: {}", String::from_utf8_lossy(&name).trim_end_matches('\0'));
        let e = (v.set_sample_rate)(a, rate);
        println!("setSampleRate({}) -> {}", rate, e);

        let mut infos = [
            ASIOBufferInfo { is_input: ASIOFalse, channel_num: 0, buffers: [null_mut(); 2] },
            ASIOBufferInfo { is_input: ASIOFalse, channel_num: 1, buffers: [null_mut(); 2] },
        ];
        let mut cbs = ASIOCallbacks {
            buffer_switch: Some(cb_switch),
            sample_rate_did_change: Some(cb_rate),
            asio_message: Some(cb_msg),
            buffer_switch_time_info: Some(cb_switch_ti),
        };
        BLOCK.store(block, Ordering::Relaxed);
        RATE_BITS.store(rate.to_bits(), Ordering::Relaxed);
        let e = (v.create_buffers)(a, infos.as_mut_ptr(), 2, block as i32, &mut cbs);
        println!("createBuffers -> {}", e);
        for i in 0..2 {
            OUT_L[i].store(infos[0].buffers[i] as usize, Ordering::Relaxed);
            OUT_R[i].store(infos[1].buffers[i] as usize, Ordering::Relaxed);
        }
        let (mut il, mut ol) = (0, 0);
        (v.get_latencies)(a, &mut il, &mut ol);
        println!("reported latency: in {} / out {} samples ({:.1} ms out)", il, ol, ol as f64 * 1000.0 / rate);

        let mut rec = Vec::with_capacity((lb.rate as f64 * (secs + 2.0)) as usize);
        let t0 = std::time::Instant::now();
        (v.start)(a);
        while t0.elapsed().as_secs_f64() < secs {
            drain(&lb, &mut rec);
            // poll the sample position like a DAW does
            let (mut sp, mut ts) = (ASIO64::default(), ASIO64::default());
            (v.get_sample_position)(a, &mut sp as *mut _ as *mut _, &mut ts as *mut _ as *mut _);
            Sleep(5);
        }
        let elapsed = t0.elapsed().as_secs_f64();
        // driver-side counters (work in every engine mode, incl. exclusive)
        if let Some(m) = audio_asio::live_status::LiveStatusMap::open_read() {
            let st = m.get();
            let mode = ["timer", "compatible", "low-latency", "EXCLUSIVE"].get(st.engine_mode.load(Ordering::Relaxed) as usize).copied().unwrap_or("?");
            let lat = st.output_latency_frames.load(Ordering::Relaxed);
            println!(
                "driver: device '{}' | engine {} | device {} Hz | period {} | out latency {} smp = {:.2} ms | dropouts {} | late wakeups {} | load {:.0}% (peak {:.0}%)",
                st.device_name(), mode, st.device_rate.load(Ordering::Relaxed), st.device_period_frames.load(Ordering::Relaxed),
                lat, lat as f64 * 1000.0 / rate, st.underruns.load(Ordering::Relaxed), st.late_wakeups.load(Ordering::Relaxed),
                st.dsp_load_permille.load(Ordering::Relaxed) as f64 / 10.0, st.dsp_load_peak_permille.load(Ordering::Relaxed) as f64 / 10.0
            );
            let in_mode = ["off", "shared", "low-latency", "EXCLUSIVE"].get(st.input_mode.load(Ordering::Relaxed) as usize).copied().unwrap_or("?");
            let in_lat = st.input_latency_frames.load(Ordering::Relaxed);
            println!(
                "input:  device '{}' | engine {} | {} Hz | in latency {} smp = {:.2} ms | round trip {:.2} ms | input underruns {} | engine switches {} | auto state {}",
                st.input_device_name(), in_mode, st.input_rate.load(Ordering::Relaxed), in_lat, in_lat as f64 * 1000.0 / rate,
                (lat + in_lat) as f64 * 1000.0 / rate, st.input_underruns.load(Ordering::Relaxed),
                st.engine_switches.load(Ordering::Relaxed), st.auto_state.load(Ordering::Relaxed)
            );
        }
        (v.stop)(a);
        let cb = CALLBACKS.load(Ordering::Relaxed);
        for _ in 0..40 {
            drain(&lb, &mut rec);
            Sleep(10);
        }
        (v.dispose_buffers)(a);
        (v.release)(a);

        let expected = elapsed * rate / block as f64;
        let speed = cb as f64 / expected;
        println!("callbacks: {} in {:.2} s (expected {:.0}) -> speed {:.3}x real time", cb, elapsed, expected, speed);
        if is_exclusive {
            println!("(exclusive mode: Windows loopback cannot record exclusive streams -> judged on driver counters)");
            println!("RESULT: {}", if (speed - 1.0).abs() < 0.02 { "see dropouts above" } else { "FAIL (wrong speed)" });
            return;
        }
        let wav = format!("selftest_{}_{}_{}_load{}.wav", if dll.contains("before") { "before" } else { "after" }, rate as u32, block, (load * 100.0) as u32);
        report(&rec, lb.rate, speed, &wav);
    }
}
