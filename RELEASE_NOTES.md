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
