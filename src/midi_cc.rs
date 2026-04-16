//! MIDI CC → parameter routing.
//!
//! Applies incoming MIDI Control Change messages to plugin parameters so
//! DAWs (Renoise, Reaper, Bitwig, etc.) can automate SquelchBox via CC
//! envelopes or hardware controllers. Uses a fixed CC map following MIDI
//! Association conventions where possible (CC 74 = cutoff, 71 = resonance,
//! 91 = reverb send, 93 = delay send, ...).
//!
//! # Why smoother targets and not `set_plain_value`
//!
//! nih-plug's `ParamMut` trait — which exposes `set_plain_value` /
//! `set_normalized_value` — is `pub(crate)` and unreachable from plugin
//! code. The publicly reachable knob is `FloatParam::smoothed`, whose
//! `Smoother::set_target` takes `&self` and is safe to call from the
//! audio thread. Every DSP read in this codebase pulls through
//! `.smoothed.next()` (see `snapshot_voice_params` / `snapshot_fx_params`
//! in `params.rs`), so driving the smoother target is equivalent to
//! driving the parameter for audio purposes.
//!
//! The underlying `value()` atomic stays stale after a CC update, which
//! means the editor UI won't show live CC modulation in v1 — acceptable
//! trade for keeping the implementation under ~60 lines. Polished
//! MIDI-learn + live UI feedback live in the commercial JUCE edition.

use crate::params::SquelchBoxParams;
use nih_plug::params::Param;

/// Apply a MIDI CC to the relevant parameter, if any.
///
/// `normalized` is the CC value mapped to `[0.0, 1.0]` (nih-plug already
/// divides the raw 0..127 byte by 127 before producing `NoteEvent::MidiCC`).
///
/// CCs outside the map are silently ignored. Enum/Bool params are not
/// mapped — they need `ParamMut` access we don't have.
pub fn apply_cc(params: &SquelchBoxParams, sample_rate: f32, cc: u8, normalized: f32) {
    let n = normalized.clamp(0.0, 1.0);
    match cc {
        // ─── 303 core ────────────────────────────────────────────────
        5 => set_float(&params.slide_ms, sample_rate, n),       // Portamento Time
        7 => set_float(&params.master_volume, sample_rate, n),  // Channel Volume
        12 => set_float(&params.tuning, sample_rate, n),        // Effect control 1
        16 => set_float(&params.accent, sample_rate, n),        // General purpose 1
        71 => set_float(&params.resonance, sample_rate, n),     // Harmonic Content
        72 => set_float(&params.decay_ms, sample_rate, n),      // Release Time
        73 => set_float(&params.env_mod, sample_rate, n),       // Attack Time (reused)
        74 => set_float(&params.cutoff, sample_rate, n),        // Brightness

        // ─── FX ──────────────────────────────────────────────────────
        13 => set_float(&params.dist_drive, sample_rate, n),    // Effect control 2
        91 => set_float(&params.reverb_mix, sample_rate, n),    // Reverb Send
        93 => set_float(&params.delay_mix, sample_rate, n),     // Chorus/Delay Send

        _ => {}
    }
}

fn set_float(param: &nih_plug::prelude::FloatParam, sample_rate: f32, normalized: f32) {
    let target = param.preview_plain(normalized);
    param.smoothed.set_target(sample_rate, target);
}
