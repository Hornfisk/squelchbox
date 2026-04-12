//! Runtime wrapper that glues `Clock` + `Pattern` into one object the
//! plugin drives per-sample.
//!
//! Responsibilities:
//!   * Own a mutable `Pattern` and `Clock`.
//!   * Emit the *current* step immediately when playback first starts
//!     (the raw clock only fires on boundary crossings, which would
//!     skip step 0).
//!   * Suppress slides when the pattern says "slide" but the voice is
//!     known-dead (prev step was a rest and we never triggered).
//!   * Expose one per-sample `tick()` that returns a `SeqTick` carrying
//!     "gate off?" and "trigger this note?" as independent fields so
//!     both can fire on the same sample.

use super::{Clock, Pattern, StepEvent};

/// What the caller should do to the voice on this sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqTick {
    /// Release the amp env: `Voice303::gate_off()`.
    pub gate_off: bool,
    /// Retrigger or glide the voice. `None` = leave it alone.
    pub trigger: Option<SeqTrigger>,
    /// `Some(absolute_step)` on the sample where the playhead just
    /// crossed into a new step. The plugin uses this to apply
    /// pattern-loop-quantized bank switches.
    pub boundary: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqTrigger {
    /// Hard retrigger (new note): `Voice303::trigger(note, accent, …)`.
    Hard { semitone: u8, accent: bool },
    /// Slide-legato into the new note: `Voice303::slide_to(note, accent, …)`.
    Slide { semitone: u8, accent: bool },
}

pub struct Sequencer {
    pub clock: Clock,
    pub pattern: Pattern,
    /// `true` once we've emitted the step-0 bootstrap for the current
    /// running phase. Cleared whenever the clock stops.
    bootstrapped: bool,
    /// `true` if a non-rest step has been triggered during the current
    /// running phase. Used to decide whether a slide flag actually has
    /// something to slide from. (Pattern::has_prior_non_rest is a
    /// data-only check — it doesn't know the voice has been silent all
    /// along when playback just started.)
    has_triggered: bool,
}

impl Sequencer {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            clock: Clock::new(sample_rate),
            pattern: Pattern::default_classic_riff(),
            bootstrapped: false,
            has_triggered: false,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.clock.set_sample_rate(sr);
    }

    /// Full reset: rewind + forget bootstrap/trigger state. Call from
    /// `Plugin::reset()` so a fresh playback always starts at step 0
    /// with a clean slate.
    pub fn reset(&mut self) {
        self.clock.rewind();
        self.bootstrapped = false;
        self.has_triggered = false;
    }

    /// Advance one sample. Returns whether a gate-off and/or a new
    /// trigger should be applied to the voice on this sample.
    pub fn tick(&mut self) -> SeqTick {
        if !self.clock.is_running() {
            // When stopped, forget the bootstrap state so the next
            // start begins by emitting step 0 again.
            self.bootstrapped = false;
            return SeqTick::default();
        }

        // Bootstrap the very first sample of a running phase: the raw
        // Clock only emits step boundaries on crossings, so without
        // this we'd skip step 0 entirely.
        if !self.bootstrapped {
            self.bootstrapped = true;
            return self.emit_for_step(self.clock.current_step());
        }

        let t = self.clock.tick();
        let mut out = SeqTick::default();

        if t.gate_off {
            // The clock fires gate_off during the step that's about to
            // end. Figure out which absolute step that is: if this
            // same sample also crossed a boundary, it's the step we
            // just left (`current_step - 1` after the advance). On a
            // normal mid-step sample, it's the current step.
            let closing_abs = if t.step.is_some() {
                self.clock.current_step().saturating_sub(1)
            } else {
                self.clock.current_step()
            };
            let pat_len = self.pattern.length.max(1) as u64;
            let pat_idx = (closing_abs % pat_len) as usize;
            let ending = self.pattern.steps[pat_idx];
            // Slid steps bridge their envelope into the next step —
            // suppress the mid-step gate-off so the amp env keeps
            // running and the following `slide_to()` glides over a
            // still-ringing voice. A rest step never had a real note
            // to close anyway, so it also doesn't need a gate-off.
            let bridge = ending.slide && !ending.rest;
            if !bridge {
                out.gate_off = true;
            }
        }

        if let Some(new_step) = t.step {
            let step_tick = self.emit_for_step(new_step);
            out.trigger = step_tick.trigger;
            out.boundary = Some(new_step);
        }
        out
    }

    /// Build the `SeqTick` for landing on the given absolute step. Uses
    /// `Pattern::event_at` for base semantics, then clamps slides to
    /// hard-triggers if no prior note actually played during this run.
    pub fn emit_for_step(&mut self, absolute_step: u64) -> SeqTick {
        let pat_len = self.pattern.length.max(1) as u64;
        let pat_idx = (absolute_step % pat_len) as usize;

        let raw = self.pattern.event_at(pat_idx);
        let trigger = match raw {
            StepEvent::None => None,
            StepEvent::Trigger { semitone, accent } => {
                self.has_triggered = true;
                Some(SeqTrigger::Hard { semitone, accent })
            }
            StepEvent::Slide { semitone, accent } => {
                if self.has_triggered {
                    Some(SeqTrigger::Slide { semitone, accent })
                } else {
                    // Slide flag on a step that appears before any
                    // non-rest in the pattern (or we haven't triggered
                    // yet this run) — demote to hard trigger so the
                    // very first note is audible.
                    self.has_triggered = true;
                    Some(SeqTrigger::Hard { semitone, accent })
                }
            }
        };

        SeqTick { gate_off: false, trigger, boundary: Some(absolute_step) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequencer::Step;

    fn count_ticks(seq: &mut Sequencer, samples: usize) -> (usize, usize) {
        let mut triggers = 0;
        let mut gate_offs = 0;
        for _ in 0..samples {
            let t = seq.tick();
            if t.trigger.is_some() {
                triggers += 1;
            }
            if t.gate_off {
                gate_offs += 1;
            }
        }
        (triggers, gate_offs)
    }

    #[test]
    fn stopped_sequencer_emits_nothing() {
        let mut seq = Sequencer::new(48_000.0);
        let (trigs, gates) = count_ticks(&mut seq, 48_000);
        assert_eq!(trigs, 0);
        assert_eq!(gates, 0);
    }

    #[test]
    fn running_sequencer_emits_step_zero_on_first_sample() {
        // The raw clock only fires on boundary crossings, which would
        // skip step 0 at t=0. The runtime wrapper must bootstrap.
        let mut seq = Sequencer::new(48_000.0);
        // Force a known pattern: step 0 = C2 trigger, rest elsewhere.
        seq.pattern = crate::sequencer::Pattern::empty();
        seq.pattern.steps[0] = Step { semitone: 36, accent: false, slide: false, rest: false };
        seq.clock.set_bpm(120.0);
        seq.clock.set_gate_length(1.0);
        seq.clock.set_running(true);
        let t = seq.tick();
        assert_eq!(
            t.trigger,
            Some(SeqTrigger::Hard { semitone: 36, accent: false }),
            "step 0 must fire on the first running sample"
        );
        assert!(!t.gate_off);
    }

    #[test]
    fn all_rest_pattern_never_triggers() {
        let mut seq = Sequencer::new(48_000.0);
        seq.pattern = crate::sequencer::Pattern::empty();
        seq.clock.set_bpm(120.0);
        seq.clock.set_gate_length(1.0);
        seq.clock.set_running(true);
        let (trigs, _) = count_ticks(&mut seq, 48_000);
        assert_eq!(trigs, 0);
    }

    #[test]
    fn leading_slide_is_demoted_to_hard_trigger() {
        // Step 15 has slide set → step 0 would wrap-slide from it.
        // But on the very first run-through we haven't triggered
        // anything yet, so there's nothing to slide from. Runtime
        // must demote to a hard trigger.
        let mut seq = Sequencer::new(48_000.0);
        seq.pattern = crate::sequencer::Pattern::empty();
        seq.pattern.steps[15] = Step { semitone: 43, accent: false, slide: true,  rest: false };
        seq.pattern.steps[0]  = Step { semitone: 48, accent: false, slide: false, rest: false };
        seq.clock.set_bpm(120.0);
        seq.clock.set_gate_length(1.0);
        seq.clock.set_running(true);
        let t = seq.tick();
        assert_eq!(
            t.trigger,
            Some(SeqTrigger::Hard { semitone: 48, accent: false }),
        );
    }

    #[test]
    fn default_pattern_fires_on_step_zero_and_advances() {
        let mut seq = Sequencer::new(48_000.0);
        seq.clock.set_bpm(120.0);
        seq.clock.set_gate_length(0.5);
        seq.clock.set_running(true);
        // First tick must trigger something (step 0 of the default
        // riff is a non-rest).
        let first = seq.tick();
        assert!(first.trigger.is_some(), "default pattern step 0 should trigger");
        // After 1 second (120 BPM → 8 sixteenths) we should see
        // strictly more triggers (subsequent steps) and gate-offs.
        let (more_trigs, more_gates) = count_ticks(&mut seq, 48_000);
        assert!(more_trigs >= 1, "expected subsequent triggers in 1s");
        assert!(more_gates >= 1, "expected gate-off events at half gate length");
    }
}
