//! Real-Time Control Panel and Telemetry Monitor for ZeroLatency Audio Driver.

use std::env;
use std::thread;
use std::time::Duration;
use audio_native::ZeroAudioStream;

mod gui;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("gui");

    let stream = match ZeroAudioStream::open() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ERROR] Failed to connect to ZeroAudio driver: {}", e);
            eprintln!("Ensure the kernel driver is loaded or running with admin rights.");
            return;
        }
    };

    match command {
        "gui" => {
            println!("Launching ZeroLatency Studio Control Panel GUI...");
            gui::run_gui(stream);
        }
        "status" => {
            println!("============================================================");
            println!("     ZEROLATENCY KERNEL AUDIO: CONTROL PANEL & TELEMETRY    ");
            println!("============================================================\n");
            show_status(&stream);
        }
        "monitor" => {
            println!("============================================================");
            println!("     ZEROLATENCY KERNEL AUDIO: CONTROL PANEL & TELEMETRY    ");
            println!("============================================================\n");
            run_live_monitor(stream);
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
    println!("  gui        (default) Launch the Native Win32 Studio Control Panel GUI");
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

    // Optional: start a minimal silent pump to drive clocks if not streaming
    let _ = stream.start(|_in_frames, out_frames| {
        for frame in out_frames.iter_mut() {
            *frame = audio_native::AudioFrame::EMPTY;
        }
    });

    let mut iteration = 0;

    loop {
        if let Some(t) = stream.telemetry() {
            // ANSI escape sequence to move cursor to start of block
            print!("\r[ITER {:04}] Frames: {:>10} | InXRun: {:>3} | OutXRun: {:>3} | Jitter: {:>6.1}ns | DPC: {:>5.1}ns",
                iteration, t.total_frames, t.input_xruns, t.output_xruns, t.max_latency_nanos, t.dpc_duration_nanos
            );
            iteration += 1;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
