## DolomiteAudioRT 2.2 — adaptive input clock sync, per-thread high-resolution timer

**Download:** `DolomiteAudioRT-Setup-*.exe` below → run it → in your DAW's audio settings pick **DolomiteAudioRT** as the ASIO device (Studio One, Cubase, Reaper, Ableton Live, FL Studio, Bitwig…). The panel lives in the tray icon next to the clock.

### New
- **Adaptive resampling on the inputs** (F. Adriaensen, *Controlling adaptive resampling*, LAC 2012). The capture device always runs on its own clock, and a USB mic or a second card can drift by tens of ppm. Until 2.1 the driver dropped samples now and then to catch up. Now the conversion ratio is servo-controlled by a 2nd-order loop:
  - nothing is ever dropped or repeated, and input latency stays constant;
  - the FIFO level is time-corrected using the device's capture timestamps, so the loop sees no sawtooth;
  - the loop locks in a few seconds (1 Hz bandwidth) and then narrows to 0.05 Hz.
  
  In a simulation with a 120 ppm offset it estimates the drift to within 0.01 ppm, and a 1 kHz sine stays at THD+N ≈ −101 dB in every 100 ms window. The panel shows the measured offset as **Sync in**.
- **Per-thread high-resolution timer** (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`). It replaces the global `timeBeginPeriod(1)`, which costs power system-wide and which Windows 11 ignores while the DAW window is minimised. The auto-exclusive watcher now ticks every 20 ms and stays precise in the background, as does the timer clock used when no sound card is available. Windows versions without it automatically use the old method.

### Fixed
- **Sample-rate converter, near 1:1 ratios**: the first coefficient of the minimum-phase filter carried an artefact from the cepstral transform. It caused a tiny gain step (0.1 %) once per fractional cycle, which is audible as modulation when the ratio sits very close to 1 (≈ −89 dB → −98 dB worst case). All polyphase rows are now checked for consistency in the tests.

---

## DolomiteAudioRT 2.1 — exclusive input, auto-release, tray panel

**Download:** `DolomiteAudioRT-Setup-*.exe` below → run it → in your DAW's audio settings pick **DolomiteAudioRT** as the ASIO device (Studio One, Cubase, Reaper, Ableton Live, FL Studio, Bitwig…). The panel lives in the tray icon next to the clock.

### New
- **Exclusive capture**: inputs now record from your sound card (they were silent before). In *Exclusive* mode input and output both run at the minimum hardware period, so round-trip latency drops the same way.
- **Auto-exclusive**: exclusive mode is released within ~150 ms when you switch to another program or another app starts playing, and re-acquired ~1 s after you return to the DAW. Switching happens live inside the driver (no ASIO reset, works with any ASIO host), and if the card is busy the driver backs off and retries.
- **Tray panel**: compact flyout that opens and closes from the notification-area icon (it hides itself when you click elsewhere, or pin it). The icon colour shows the state: green = exclusive, blue = shared, red = dropout. Optional *Start with Windows*.
- **Minimum-phase sample-rate converter**: the converter's own delay drops from 16 to ~2.4 frames at the same quality (THD+N ≈ −102 dB). It's only used when the DAW and sound card rates differ.
- **Recommended buffer**: the panel marks the smallest DAW buffer that matches your card's period.

### Reliability
- The audio thread never writes to disk: events are logged lock-free and written by a background thread.
- Denormals are flushed to zero on the audio thread, which avoids CPU spikes in decaying reverbs and filters.
- Self-healing: if a device is unplugged or errors, the driver reopens it (or falls back to the default device) without a DAW reset.
- Changes to the output or input device in the panel apply live.

### Fixed (2.0)
- Distorted or chopped audio, clicks at 44.1 kHz, and wrong latency reporting. See the 2.0 notes.
