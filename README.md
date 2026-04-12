# SquelchBox

A FOSS TB-303-style acid bassline synthesizer plugin. VST3 + CLAP + standalone.

Built with Rust, [nih-plug](https://github.com/robbert-vdh/nih-plug), and egui.

![SquelchBox UI](assets/screenshot.png)

## Features

- **Oscillator** -- bandlimited saw + square (BLIT/polyBLEP), drift LFO for analog warmth
- **3-pole diode ladder filter** -- self-oscillating, with 2x oversampling via half-band polyphase
- **Envelopes** -- exponential amp, one-shot filter (attack-decay), dedicated accent envelope
- **16-step sequencer** -- per-step pitch/accent/slide/rest, pattern length 1-16, swing, 4-bank pattern memory
- **Sync modes** -- Internal (free-run), Host (DAW transport slave), MIDI (keyboard only)
- **FX chain** -- Diode distortion, tempo-synced delay (analog/clean), Schroeder reverb, brickwall limiter
- **Slide/glide** -- portamento between legato steps, authentic 303 slide behavior
- **Computer keyboard** -- chromatic note input, step editing, tap tempo, pattern randomizer
- **MIDI export** -- dump patterns as .mid files

## Audio

<!-- TODO: Add audio samples / links here -->

Coming soon.

## Screenshot

The UI is a faithful recreation of the TB-303 front panel, rendered in egui with a brushed-metal faceplate, interactive knobs (drag to adjust, shift+drag for fine control, ctrl-click to reset), animated FX compartments, and a 16-step pitch slider grid with A/S/R (accent/slide/rest) toggles per step.

## Build

Requires Rust nightly or stable 1.75+.

```bash
# Check / test
cargo check
cargo test         # 101 tests

# Bundle VST3 + CLAP
cargo xtask bundle squelchbox --release

# Install (Linux)
rm -rf ~/.vst3/SquelchBox.vst3
cp -r target/bundled/SquelchBox.vst3 ~/.vst3/
cp -f target/bundled/squelchbox.clap ~/.clap/

# Standalone
cargo run --release -- --sample-rate 44100 --period-size 512
```

## Project structure

```
src/
  lib.rs              -- crate root
  plugin.rs           -- nih-plug Plugin impl, process loop
  params.rs           -- parameter definitions (knobs, toggles, FX)
  kbd.rs              -- keyboard/MIDI event queue (GUI <-> audio)
  main.rs             -- standalone entry point

  dsp/
    oscillator.rs     -- bandlimited saw + square
    envelope.rs       -- amp, filter, accent envelopes
    filter_diode.rs   -- 3-pole diode ladder (bilinear transform)
    oversampler.rs    -- 2x half-band polyphase up/downsample
    voice.rs          -- monophonic voice: osc + filter + envelopes
    fx/               -- distortion, delay, reverb, limiter, FxChain

  sequencer/
    clock.rs          -- tempo clock with swing
    pattern.rs        -- 16-step pattern + 4-slot bank
    runtime.rs        -- sequencer state machine

  ui/
    mod.rs            -- editor entry point (create())
    ids.rs            -- centralized egui ID registry
    palette.rs        -- colors + layout constants
    widgets.rs        -- param knob, button painters
    keyboard.rs       -- keyboard input, pattern persistence
    panels/           -- faceplate, band1, band2, fx_dist, fx_time, toast
      lower/          -- left strip, pitch row, step grid, transpose, right strip

  util/
    paths.rs          -- XDG data/config/preset directories
    midi_export.rs    -- pattern-to-MIDI file export
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [nih-plug](https://github.com/robbert-vdh/nih-plug) | Plugin framework (VST3/CLAP/standalone) |
| [nih_plug_egui](https://github.com/robbert-vdh/nih-plug) | egui integration for plugin GUIs |
| parking_lot | Fast mutexes for audio/GUI sync |
| rtrb | Lock-free SPSC ring buffer |
| serde + serde_json | Pattern bank persistence |
| tracing | Structured logging |

## License

[GPL-3.0-or-later](LICENSE) -- required by nih-plug's license.
