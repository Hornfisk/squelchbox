# M6 FX Chain — Design Spec

## Overview

Post-voice effects chain for SquelchBox: stomp-box distortion, tempo-synced delay, ambient room reverb, and a transparent brickwall limiter. All FX are off by default; the UI reveals controls progressively via animated compartments so the default view is unchanged.

## Signal Chain

```
Voice303 → Distortion → Delay → Reverb → Brickwall Limiter → Master Volume → Output
```

Distortion before time-based effects so repeats and reverb process already-distorted signal. Limiter is always-on safety net. Master volume is the final gain stage (existing param, default bumped from -6 dB to -3 dB).

## Modules

### 1. Distortion

**Algorithm**: Asymmetric diode-pair waveshaper. Positive lobe: `1 - exp(-x * drive_gain)`. Negative lobe: scaled by asymmetry factor (~0.8) for even+odd harmonics. Drive knob maps to input gain via `1 + DRIVE_GAIN_RANGE * drive` (~+26 dB at max). Output makeup gain normalizes perceived loudness across drive range.

Single mode tuned for acid grit. Code structured with an enum so additional modes (clip, tape) can slot in later without API changes.

**Parameters**:

| ID | Name | Range | Default | Smoothing | Notes |
|----|------|-------|---------|-----------|-------|
| `dist_enable` | Dist Enable | bool | false | — | Bypass: passthrough when off |
| `dist_drive` | Drive | 0.0–1.0 | 0.5 | Linear 20ms | Maps to input gain |
| `dist_mix` | Dist Mix | 0.0–1.0 | 1.0 | Linear 20ms | Dry/wet blend |

### 2. Delay

**Algorithm**: Mono-in stereo-out delay line. Delay time derived from tempo + sync subdivision. Circular buffer sized for ~2 seconds at max sample rate (96 kHz → 192k samples). Feedback loop with optional one-pole LP filter for analog mode.

**Modes**:
- **Clean**: pristine repeats, no filtering in feedback path.
- **Analog** (default): one-pole LP at ~3 kHz in feedback path. Each repeat loses HF, sits behind the dry signal.

**Tempo source**: Reads BPM from the same value the sequencer clock uses — `seq_bpm` smoothed value in Internal mode, `transport.tempo` in Host mode, `seq_bpm` fallback in MIDI mode. Delay time recalculated per block (not per sample — tempo doesn't change that fast).

**Sync subdivisions** (enum):
- `Quarter` — 1 beat
- `Eighth` — 1/2 beat (default)
- `DottedEighth` — 3/4 beat
- `Sixteenth` — 1/4 beat
- `TripletEighth` — 1/3 beat

Delay time (seconds) = `60.0 / bpm * subdivision_factor`.

Feedback capped at 0.9 to prevent runaway. When delay is toggled off, the feedback tail is allowed to ring out naturally (wet signal fades while dry passes through), then the buffer is cleared once silent.

**Parameters**:

| ID | Name | Range | Default | Smoothing | Notes |
|----|------|-------|---------|-----------|-------|
| `delay_enable` | Delay Enable | bool | false | — | Bypass when off (tail rings out) |
| `delay_mode` | Delay Mode | Clean/Analog | Analog | — | Enum, no smoothing |
| `delay_sync` | Delay Sync | subdivision enum | Eighth | — | Enum selector |
| `delay_feedback` | Feedback | 0.0–0.9 | 0.4 | Linear 20ms | Repeat intensity |
| `delay_mix` | Delay Mix | 0.0–1.0 | 0.3 | Linear 20ms | Dry/wet blend |

### 3. Reverb

**Algorithm**: Schroeder reverb network. 4 parallel comb filters (mutually prime delay lengths, scaled by sample rate) feeding 2 series allpass diffusers. Fixed pre-delay of ~10ms. Comb feedback coefficients derived from the Decay parameter. Character: short, dense ambient room — tight reflections, no metallic ringing, no cathedral tails.

Comb delay lengths (at 48 kHz, scaled proportionally for other rates): 1117, 1188, 1277, 1356 samples. Allpass delays: 556, 441 samples. These are mutually prime to avoid coloring.

Decay parameter maps to comb feedback via `g = 0.7 * (0.3 + 0.7 * decay)` — range ~0.21 (tiny closet) to 0.70 (medium room). Allpass coefficient fixed at 0.5.

**Parameters**:

| ID | Name | Range | Default | Smoothing | Notes |
|----|------|-------|---------|-----------|-------|
| `reverb_enable` | Reverb Enable | bool | false | — | Bypass when off |
| `reverb_decay` | Decay | 0.0–1.0 | 0.4 | Linear 20ms | Room size / tail length |
| `reverb_mix` | Reverb Mix | 0.0–1.0 | 0.2 | Linear 20ms | Dry/wet blend |

### 4. Brickwall Limiter

**Algorithm**: Peak-detecting limiter. Ceiling at -0.3 dBFS. Attack 0.1ms, release 50ms. One-pole envelope follower in dB domain. Always on, no user-facing parameters.

Gain reduction = `min(0, ceiling_db - peak_db)`. Applied as linear gain multiplier. Envelope follows peaks with fast attack / moderate release so transients are caught without pumping.

### 5. FxChain Wrapper

A single `FxChain` struct owns all four modules. Exposes:
- `new(sample_rate) -> Self`
- `set_sample_rate(sr)` — propagates to all modules, resizes delay buffer
- `reset()` — clears all internal state (delay buffer, reverb combs, limiter envelope)
- `process(sample: f32, params: &FxParams) -> f32` — runs the full chain per sample

`FxParams` is a snapshot struct populated once per sample in `plugin.rs::process()` from the smoothed param values — same pattern as `VoiceLiveParams`.

## UI Layout

### Left Zone: Distortion Compartment

**Position**: Below "SQUELCHBOX / COMPUTER CONTROLLED BASS LINE" text, x ~28..310, y ~45..108.

**Default state (dist off)**:
- Toggle switch (iOS-style, 28×14px): dark background (#343438), muted circle, left-positioned
- "DIST" label (7.5px monospace, SILVER_SHADOW color) to right of toggle
- Thin silver separator line extending to right edge of zone
- Total height: ~18px. Unobtrusive, almost decorative.

**Enabled state (dist on)**:
- Toggle turns red (#c42a2a), circle slides right, turns white
- "DIST" label turns red, bold
- Below the toggle row: a recessed silver inset panel slides down (~150ms ease-out animation)
- Panel contains DRIVE + MIX knobs (16px radius, same style as main knobs)
- Knob labels below each knob, centered
- Total revealed height: ~50px

**Animation**: egui doesn't have native spring animations. Implement as a linear interpolation on a `0.0..1.0` progress float, advanced by `dt / 0.15` each frame. The compartment height lerps from 0 to full. `ctx.request_repaint()` while animating.

### Right Zone: Delay + Reverb Panel

**Position**: Replaces "SB-303 / Computer Controlled" branding area, x ~370..530, y ~108..220.

**Default state (both off)**:
- SB-303 branding text (28px proportional) centered in zone
- "Computer Controlled" subtitle below
- Two small toggle switches below branding, horizontally centered with ~16px gap:
  - DELAY toggle + label
  - REVERB toggle + label
- LED readout anchored at bottom of zone (unchanged position)

**One or both enabled**:
- SB-303 text and subtitle fade out (opacity lerp, ~200ms)
- Control rows fade in, stacked vertically:
  - **Delay row**: toggle + ANA/CLN mode button (small, styled like SYNC mode buttons) + SYNC knob + FDBK knob + MIX knob
  - **Reverb row**: toggle + DECAY knob + MIX knob
- Thin silver separator line between delay and reverb rows
- LED readout stays at bottom
- Knobs: 16px radius, consistent with distortion zone
- Mode button: 7.5px monospace, dark background, red when active (ANA), toggles on click

**Alignment**:
- Toggle switches left-aligned at x offset 0 within zone
- Knob centers vertically centered within their row (row height ~36px)
- Knob labels centered below each knob
- Mode button vertically centered in delay row, between toggle and first knob
- All elements on whole-pixel coordinates (no subpixel rendering issues)

### Animation State

Both zones track their animation with a simple struct:

```rust
struct FxPanelAnim {
    dist_open: f32,    // 0.0 = closed, 1.0 = open
    time_fx_open: f32, // 0.0 = branding, 1.0 = controls
}
```

Advanced each frame based on whether the corresponding enable param is on. Stored in egui temp data (keyed by `Id`).

## New Files

| Path | Purpose |
|------|---------|
| `src/dsp/fx/mod.rs` | FX module tree: `pub mod distortion, delay, reverb, limiter, fx_chain;` |
| `src/dsp/fx/distortion.rs` | Diode waveshaper with drive/mix/bypass |
| `src/dsp/fx/delay.rs` | Tempo-synced delay with feedback LP, clean/analog modes |
| `src/dsp/fx/reverb.rs` | Schroeder room reverb (4 comb + 2 allpass) |
| `src/dsp/fx/limiter.rs` | Brickwall peak limiter at -0.3 dBFS |
| `src/dsp/fx/fx_chain.rs` | Chains all four, exposes single `process()` entry point |

## Modified Files

| Path | Changes |
|------|---------|
| `src/dsp/mod.rs` | Add `pub mod fx;` |
| `src/params.rs` | Add 11 FX params (3 dist + 5 delay + 3 reverb), `DelayMode` enum, `DelaySyncDiv` enum, `FxParams` snapshot struct, update `seed_smoothers()` |
| `src/plugin.rs` | Add `fx_chain: FxChain` field, wire into `process()` after voice output, pass BPM to delay, call in `initialize()` / `reset()` |
| `src/ui.rs` | Add `draw_fx_dist()` (left zone) and `draw_fx_time()` (right zone), animation state, new knobs + toggles. Adjust Band 2 layout constants. |

## Master Volume Change

Default `master_volume` bumped from `db_to_gain(-6.0)` to `db_to_gain(-3.0)`. The FX chain adds gain (distortion drive, delay feedback stacking), but the limiter catches peaks. -3 dB gives a louder dry signal while leaving headroom for the limiter.

## Testing Strategy

**Per-module unit tests** (in each DSP file):
- Silence in → silence out (no DC offset, no noise floor)
- Impulse response: non-zero output after impulse, decays to silence
- No NaN/inf under extreme parameter values (drive=1.0, feedback=0.9, decay=1.0)
- Distortion: output bounded, asymmetry produces even harmonics
- Delay: correct delay time for known BPM + subdivision
- Reverb: output decays to silence within expected time
- Limiter: output never exceeds -0.3 dBFS with +20 dB input

**FxChain integration tests**:
- All bypassed → output equals input (clean passthrough)
- Each module enabled independently → produces expected effect
- Full chain → no NaN, no runaway, output bounded by limiter

**UI**: Manual verification in standalone — toggles animate, knobs respond, nothing overlaps at any combination of on/off states, LED readout visible in all states.

## Future Extensibility

- **Distortion modes**: Add variants to a `DistMode` enum + match arm in the waveshaper. UI gets a mode button like the delay's ANA/CLN toggle.
- **Plate reverb**: Second reverb algorithm behind a mode switch. Different delay line network, longer tails, brighter diffusion.
- **Delay tap patterns**: Ping-pong or multi-tap — extends the delay buffer logic, adds a mode variant.

All future additions are enum variants + match arms. No architectural changes needed.
