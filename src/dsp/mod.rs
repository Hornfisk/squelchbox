//! DSP module tree for SquelchBox.
//!
//! Organised as small, testable units that each own a clear slice of the
//! synthesis chain. `voice.rs` assembles them into a single `Voice303` that
//! the plugin drives per sample.
//!
//! The real 3-pole diode ladder lives in `filter_diode.rs`.

pub mod envelope;
pub mod filter_diode;
mod filter_placeholder;
pub mod oscillator;
pub mod oversampler;
pub mod voice;
pub mod fx;

/// Flush subnormal floats to zero. On x86, denormals (~1e-38) fall back to
/// microcode handling, making each arithmetic op ~100× slower — enough to
/// cause buffer underruns and audible glitches. Apply to all IIR filter
/// state every sample.
#[inline(always)]
pub fn flush_denormal(x: f32) -> f32 {
    if x.abs() < 1.0e-15 { 0.0 } else { x }
}
