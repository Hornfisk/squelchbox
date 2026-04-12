# SquelchBox

A FOSS TB-303-style bassline synthesizer. VST3 + CLAP + standalone.
Rust + nih-plug + nih_plug_egui.

**Status:** M0 scaffolding — plugin loads, logs, passes audio silently.
DSP, sequencer, and UI land in subsequent milestones (see
`~/.claude/plans/serialized-booping-ritchie.md`).

## Goals

- Faithful TB-303-style monosynth with 3-pole diode-ladder filter
- 16-step sequencer (host-synced + standalone)
- Overdrive → Delay → Reverb FX chain
- Normal / High / Ultra quality modes for CPU/authenticity trade-off
- Cross-platform (Linux / macOS / Windows) — VST3 + CLAP, no AU

## Build

```bash
cargo check
cargo test
cargo xtask bundle squelchbox --release
```

## License

GPL-3.0-or-later.
