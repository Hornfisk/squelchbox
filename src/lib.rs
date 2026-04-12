//! SquelchBox — TB-303-style bassline synthesizer.
//!
//! Exported as a VST3 + CLAP plugin (via `cdylib`) and as a standalone binary
//! (`squelchbox-standalone`). Module tree is declared here; `main.rs` is a
//! thin wrapper that calls [`run_standalone`].
//!
//! This is the M0 scaffolding drop: the plugin loads, logs, and passes audio
//! through silently. DSP, sequencer, and UI modules land in subsequent
//! milestones.

use nih_plug::prelude::*;

mod logging;
mod params;
mod plugin;
mod util;

pub use plugin::SquelchBox;

nih_export_vst3!(plugin::SquelchBox);
nih_export_clap!(plugin::SquelchBox);

/// Entry point for the standalone binary. Called from `src/main.rs`.
pub fn run_standalone() {
    nih_export_standalone::<plugin::SquelchBox>();
}
