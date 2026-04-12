//! DSP module tree for SquelchBox.
//!
//! Organised as small, testable units that each own a clear slice of the
//! synthesis chain. `voice.rs` assembles them into a single `Voice303` that
//! the plugin drives per sample.
//!
//! The real 3-pole diode ladder lives in `filter_diode.rs` (M2).
//! M1 uses `filter_placeholder::OnePoleLp` to prove the voice pipeline
//! end-to-end with an audible (if unremarkable) lowpass.

pub mod envelope;
pub mod filter_diode;
pub mod filter_placeholder;
pub mod oscillator;
pub mod oversampler;
pub mod voice;
pub mod fx;
