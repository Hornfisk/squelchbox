# Changelog

All notable changes to SquelchBox are documented here.

## [0.2.0] — 2026-04-19

UX polish + DSP authenticity pass.

### Added
- **FX panel reorg** — DIST, DELAY, and REVERB now sit in a single unified strip in the middle band, reading left-to-right as one signal-chain section instead of being scattered between the top-left brand area and the mid-strip.
- **Per-step note labels in sequencer** — every active step shows its pitch as a small note name (`C3`, `F#4`, …) below the slider, accent-colored.
- **Tuning knob microtuning** — Tuning snaps to whole semitones by default; hold Shift while dragging for continuous (microtuning) motion. Suits both step-sequence acid use and detune/expressive use.
- **Auto-default SYNC mode** — standalone defaults to `INTERNAL` (free-run sequencer), plugin defaults to `HOST` (DAW transport). Persisted state in DAW projects still wins on subsequent loads.

### Changed
- **Accent envelope** — rewritten as a cap-discharge source (80 ms RC) into a 1-pole LP follower (4 ms). Trigger snaps the cap to 1.0 without resetting the LP, preserving the cap-accumulation behavior on rapid successive accents while smoothing the attack curve.
- **Resonance bass-strip** — frequency-dependent post-VCF HPF (30 Hz at reso=0 → ~250 Hz at reso=1, quadratic taper) strips the bass swell that builds up at high Q without thinning low-Q passages.
- **SYNC selector** — three-button stack collapsed into a single cycling button (`INT` → `HOST` → `MIDI`), freeing horizontal space in the middle band for the FX reorg.

### Fixed
- **Sequencer drag-paint latching** — dragging horizontally across step cells while drawing pitches now paints the cell the pointer is actually over, instead of latching to the source cell. Click-and-drag pattern entry works as expected.

## [0.1.3] — 2026-04-17

Bugfix release.

### Fixed
- **Keyboard preview sustained forever** — pressing a note key (`Z`/`X`/`C`/…) or `T` to audition the selected step pushed a gate-on with no corresponding gate-off, so the voice's amp env held at unity indefinitely. Keyboard input now tracks which keys are gating the voice and pushes a single gate-off once the last note key is released, matching monosynth key-release semantics.

## [0.1.2] — 2026-04-17

Authentic TB-303 DSP pass based on hardware-level expert feedback.

### Changed
- **Filter: 4-pole diode ladder** (was 3-pole). First-stage coefficient uses `2·fc` to model the real 303's half-value first capacitor, putting the first pole an octave above the other three. Slope is now a true 24 dB/oct.
- **Filter: DC blocker in the feedback path** — a 1-pole 30 Hz HPF on the resonance feedback signal gives the filter a mild bandpass character at very low cutoffs, matching the coupling-capacitor losses in the real circuit.
- **Filter: no self-oscillation** — `K_MAX = 4.0` sits at 93% of the analytical `K_crit ≈ 4.3` for this topology. High-res settings produce a screamy peak with a long ring-down but the filter always decays on its own, matching the TT-303 and real TB-303 hardware. HF resonance taper widened (3 kHz → 10 kHz) with a higher floor (0.60) so high-cutoff screams still work.
- **Oscillator: 30% duty-cycle pulse** — the "square" waveform is now a narrow PolyBLEP pulse, matching the TB-303's actual waveshape rather than a textbook 50% square.
- **Oscillator: AC-coupling HPF** — a 30 Hz 1-pole HPF between the oscillator and the filter models the coupling capacitor in the real circuit, adding the characteristic thin/buzzy droop to both saw and square.
- **Voice: accent cap bleeds into every note** — the accent envelope's residual charge now always modulates cutoff and amplitude, not just on accented steps. The envelope still only re-triggers on accented steps. This matches C13's actual behavior: charging is gated by the accent switch, the remaining voltage is not.
- **Voice: resonance-to-VCA compensation** — output is scaled by `1.0 + 0.35·resonance` after the VCA saturation, compensating the perceived volume drop at high resonance. Mirrors the real 303's resonance-pot → VCA feed.

### Fixed
- **UI: SLIDE / OCT labels** — the big SLIDE label no longer overlaps the SLIDE knob; OCT sits level with the up-arrow button, with the same inter-element gap as between the two arrows.

### Notes
- Same nih-plug buffer-size caveats apply as in 0.1.0 (see below). macOS and Windows launch scripts bundled with the release apply the necessary `--period-size` workaround.
- DSP changes documented in detail for the future SquelchPro (JUCE/C++) port.

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
