//! 16-step bassline sequencer — data model and clock.
//!
//! M5 drop: pure DSP, no UI and no plugin wiring yet. The sequencer owns
//! a `Pattern` (16 `Step`s + length + swing) and a `Clock` that advances
//! per-sample driven by an external tempo. On each step boundary it
//! emits a `StepEvent` telling the caller what the voice should do —
//! trigger, slide, gate-off, or nothing.
//!
//! Transport decisions (play/stop) are NOT made here — the caller owns
//! an explicit play flag. This is deliberate: nih-plug standalone reports
//! `transport.playing = true` permanently (see
//! `feedback_nihplug_standalone_transport.md`), so we can't trust the
//! host to drive start/stop. The sequencer just runs when told to.

pub mod clock;
pub mod pattern;
pub mod runtime;

pub use clock::{Clock, ClockTick};
pub use pattern::{Pattern, PatternBank, Step, StepEvent, MAX_PATTERN_LEN};
pub use runtime::{SeqTick, SeqTrigger, Sequencer};
