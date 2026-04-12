# Changelog

All notable changes to SquelchBox are documented here.

## [0.1.0] — 2026-04-13

First public release.

### Added
- **Oscillator** — bandlimited saw + square (BLIT/polyBLEP) with drift LFO for analog warmth
- **3-pole diode ladder filter** — self-oscillating, with 2x oversampling via half-band polyphase
- **Envelopes** — exponential amp, one-shot filter (attack-decay), dedicated accent envelope
- **16-step sequencer** — per-step pitch/accent/slide/rest, pattern length 1-16, swing, 4-bank pattern memory
- **Sync modes** — Internal (free-run), Host (DAW transport slave), MIDI (keyboard only)
- **FX chain** — Diode distortion, tempo-synced delay (analog/clean), Schroeder reverb, brickwall limiter
- **Slide/glide** — portamento between legato steps, authentic 303 slide behavior
- **Computer keyboard** — chromatic note input, step editing, tap tempo, pattern randomizer
- **MIDI export** — dump patterns as .mid files
- **egui GUI** — brushed-metal faceplate, interactive knobs (drag/shift-drag/ctrl-click), animated FX compartments, 16-step pitch slider grid with A/S/R toggles
- **Cross-platform** — VST3 + CLAP + standalone (Linux, macOS, Windows)

### Notes
- **macOS standalone**: CoreAudio may deliver larger buffers than requested, causing nih-plug's CPAL backend to panic. Use the included `squelchbox-macos.sh` launcher which passes `--period-size 4096`. See [nih-plug#266](https://github.com/robbert-vdh/nih-plug/issues/266).
- **Windows standalone**: WASAPI in shared mode delivers buffers in the device's native period (often 1056-1266 samples), exceeding nih-plug's default 512. Use the included `SquelchBox.bat` launcher or pass `--period-size 2048` manually.
- macOS binaries are unsigned/unnotarized. On first launch: right-click > Open, or `xattr -dr com.apple.quarantine squelchbox-standalone`.
