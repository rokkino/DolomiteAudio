#![windows_subsystem = "windows"]
//! Real-Time Control Panel and Telemetry Monitor for DolomiteAudioRT Audio Driver.

use std::env;
use std::thread;
use std::time::Duration;
use audio_native::ZeroAudioStream;

mod gui;
mod server;
mod html;
mod tray;

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(dwProcessId: u32) -> i32;
}

fn log_control(msg: &str) {
    let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".into())).join("DolomiteAudio");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("control_debug.log")) {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {}", audio_core::current_time_nanos(), msg);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("gui");
    log_control(&format!("audio-control started with command: {}", command));

    if command == "status" || command == "monitor" || command == "help" || command == "--help" || command == "-h" {
        unsafe {
            AttachConsole(0xFFFFFFFF); // ATTACH_PARENT_PROCESS
        }
    }

    let stream_opt = ZeroAudioStream::open().ok();

    match command {
        "gui" | "tray" => {
            // gui  = open the panel (Studio One's "Control Panel" button, Start menu)
            // tray = start silently in the notification area (Windows autostart)
            let show = command == "gui";
            if !tray::claim_single_instance(show) {
                log_control("another instance is running: asked it to show the panel");
                return;
            }
            let port = 49215;
            let last_hb = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
            match server::start_control_server(stream_opt, port, last_hb) {
                Ok(_) => log_control("control server listening on 127.0.0.1:49215"),
                Err(e) => log_control(&format!("start_control_server error: {:?}", e)),
            }
            thread::sleep(Duration::from_millis(50));
            if !tray::edge_available() && show {
                log_control("Edge not found, using the native GDI panel");
                gui::run_gui(None);
                return;
            }
            tray::run(port, show);
            log_control("tray exited");
        }
        "status" => {
            println!("============================================================");
            println!("    DOLOMITE AUDIO RT: CONTROL PANEL & TELEMETRY            ");
            println!("============================================================\n");
            if let Some(stream) = &stream_opt {
                show_status(stream);
            } else {
                println!("[INFO] DolomiteAudio Kernel driver not loaded. Running in Standalone / WASAPI Bridge mode.");
                let cfg = audio_asio::wasapi::DolomiteConfig::load();
                println!("  Output Device ID  : {}", if cfg.output_device_id.is_empty() { "(Default Windows Endpoint)" } else { &cfg.output_device_id });
                println!("  Input Device ID   : {}", if cfg.input_device_id.is_empty() { "(Default Windows Endpoint)" } else { &cfg.input_device_id });
                println!("  Buffer Size       : {} frames", cfg.buffer_size);
                println!("  Sample Rate       : {} Hz\n", cfg.sample_rate);
            }
        }
        "monitor" => {
            println!("============================================================");
            println!("    DOLOMITE AUDIO RT: CONTROL PANEL & TELEMETRY            ");
            println!("============================================================\n");
            if let Some(stream) = stream_opt {
                run_live_monitor(stream);
            } else {
                eprintln!("[ERROR] Real-time kernel telemetry requires kernel driver.");
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: '{}'. Run 'audio-control help' for usage.", command);
        }
    }
}

fn print_usage() {
    println!("Usage: audio-control [command]");
    println!("Commands:");
    println!("  gui        (default) Show the control panel (starts the tray icon if needed)");
    println!("  tray       Start in the notification area only (used by Windows autostart)");
    println!("  monitor    Run live real-time ASCII peak meters and telemetry");
    println!("  status     Show current snapshot of driver configuration and xruns");
    println!("  help       Show this help dialog");
}

fn show_status(stream: &ZeroAudioStream) {
    if let Some(t) = stream.telemetry() {
        println!("Driver Status Snapshot:");
        println!("  Streaming Active  : {}", t.is_running);
        println!("  Sample Rate       : {} Hz", t.sample_rate);
        println!("  Quantum (Buffer)  : {} frames ({:.3} ms)", t.quantum_frames, (t.quantum_frames as f64 / t.sample_rate as f64) * 1000.0);
        println!("  Total Processed   : {} frames", t.total_frames);
        println!("  Input XRuns       : {}", t.input_xruns);
        println!("  Output XRuns      : {}", t.output_xruns);
        println!("  Min Latency       : {:.2} ns", t.min_latency_nanos);
        println!("  Max Latency       : {:.2} ns", t.max_latency_nanos);
        println!("  DPC Duration      : {:.2} ns\n", t.dpc_duration_nanos);
    }
}

fn run_live_monitor(mut stream: ZeroAudioStream) {
    println!("Running live monitor. Press Ctrl+C to exit.\n");

    let _ = stream.start(|_in_frames, out_frames| {
        for frame in out_frames.iter_mut() {
            *frame = audio_native::AudioFrame::EMPTY;
        }
    });

    let mut iteration = 0;

    loop {
        if let Some(t) = stream.telemetry() {
            print!("\r[ITER {:04}] Frames: {:>10} | InXRun: {:>3} | OutXRun: {:>3} | Jitter: {:>6.1}ns | DPC: {:>5.1}ns",
                iteration, t.total_frames, t.input_xruns, t.output_xruns, t.max_latency_nanos, t.dpc_duration_nanos
            );
            iteration += 1;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
