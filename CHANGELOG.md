# Changelog

All notable changes to SquelchBox are documented here.

## [0.1.1] — 2026-04-16

UI layout refinements and two small DSP fixes.

### Changed
- **Upper-panel layout** — SB-303 branding stays centered; REVERB toggle + knobs relocated to a dedicated far-right zone so they no longer overlap the branding; BANK row + LEN spinner shifted up; DIST controls sit bare on the faceplate (no tray); logo and subtitle removed; TEMPO knob sized to match VOLUME.
- **Readouts** — BPM and LED readout now share the inset LED styling and are aligned at the same vertical position; BPM box narrowed to 3-digit comfy width.
- **DIST knob sizing** — DRIVE + MIX knobs 10% smaller and lifted slightly off the separator.
- **FX labels** — delay/reverb knob labels match DIST label styling (size, spacing, weight).

### Fixed
- **DIST enable click** — priming the distortion DC-blocker to its silence steady-state removes the subtle click heard when toggling DIST on with no input.
- **Delay re-enable tail artifact** — delay buffer is now cleared on off→on transition, preventing stale residue from the previous run bleeding back in.

### Notes
- Same nih-plug buffer-size caveats apply as in 0.1.0 (see below).

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
