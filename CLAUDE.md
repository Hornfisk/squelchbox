# SquelchBox

FOSS TB-303-style acid bassline synth. Rust + nih-plug + egui. VST3/CLAP/Standalone.

## Quick start

```bash
cargo build --release
# Standalone:
cargo run --release --bin squelchbox-standalone
# Plugin artefacts land in target/release/ (VST3 + CLAP bundles)
```

## Architecture

- `src/plugin.rs` — nih-plug Plugin impl, audio process loop, sequencer/voice wiring.
- `src/params.rs` — all nih-plug FloatParam/BoolParam/EnumParam definitions + snapshot helpers.
- `src/kbd.rs` — KbdQueue: lock-free event bridge between GUI and audio thread. Pattern bank, step selection, seq run/stop atomics.
- `src/midi_cc.rs` — fixed CC→param routing (CC 74=cutoff, 71=reso, etc.). Drives smoothers, not raw values.
- `src/dsp/envelope.rs` — AmpEnv (authentic 303 VEG: gate-driven, fixed shape), FilterEnv (decay-only power curve), AccentEnv (300 ms RC, charge-accumulation).
- `src/dsp/voice.rs` — Voice303: osc + diode ladder + envelopes + drift LFO + 2x oversampling.
- `src/dsp/filter_diode.rs` — 3-pole diode ladder (self-oscillating, bilinear transform).
- `src/dsp/fx/` — FxChain: Distortion → Delay → Reverb → LoudnessComp → Limiter.
- `src/sequencer/` — 16-step × 4-bank sequencer. Clock (sample-accurate), Pattern, Runtime (SeqTick events).
- `src/ui/` — egui panels. `step_area.rs` = 16-step grid with click-drag draw. `keyboard.rs` = global key handling (T-preview, A/S/R toggles).

## Key design decisions

- **AmpEnv is gate-driven** (authentic 303 VEG). The DECAY knob only drives FilterEnv. AmpEnv holds at unity until gate_off, then does a 16 ms two-segment release (8 ms flat + 8 ms linear fall).
- **AccentEnv does NOT reset on retrigger**. Successive accents accumulate charge on the RC cap, producing the build-up wobble. Accent modulates amp + cutoff only, not resonance.
- **LoudnessComp is always-on** — soft-knee compressor post-reverb that tames level swings without adding a user-facing toggle.
- **Gate-off on stop**: when the sequencer transitions from running to stopped, the voice gets an explicit gate_off so notes don't ring past transport stop.
- **Pattern sync**: GUI edits the pattern via `kbd.edit_pattern()` which bumps a revision counter. Audio thread polls `pattern_rev()` each block and pulls a fresh snapshot if changed. Lock-free, non-blocking.
- **Click-drag draw**: dragging across step cell boundaries paints pitches onto every traversed cell. State tracked via egui temp data (`draw_active` ID).

## Testing

```bash
cargo test          # 104 unit tests across DSP, sequencer, FX
cargo build --release  # full plugin build
```

## Packaging (AUR)

See `squelchbox-aur/` sister repo for PKGBUILD. Build from source tarball, installs VST3 + CLAP + standalone.

## Branches

- `main` — stable releases.
- `feature/authentic-303-dsp` — current: authentic envelope rework + loudness comp + click-drag draw.
