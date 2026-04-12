//! Sample-accurate step clock.
//!
//! Drives a 16-step sequencer at a given BPM, with swing and gate
//! length. Called once per sample from the plugin's audio loop; returns
//! a `ClockTick` that tells the caller whether a step boundary was
//! crossed (trigger a new note) and/or whether the gate should be
//! released (apply `gate_off` to the voice).
//!
//! The clock only knows about *samples* and *steps*. It does not know
//! about the pattern's length — the caller turns the absolute step
//! index (`u64`) into a pattern index with `step % pattern.length`.
//!
//! Tempo/swing/gate updates are allowed at any time and take effect on
//! the next sample without disturbing the current playhead position.

/// Per-sample clock result.
///
/// On most samples both fields are their "nothing happens" value
/// (`step = None`, `gate_off = false`). Both can fire on the same
/// sample (step boundary + immediate gate release for a very short
/// gate-length setting), so this is a struct not an enum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockTick {
    /// `Some(step_index)` when the playhead has just crossed into a
    /// new step. Monotonically increasing; never wraps — caller does
    /// `step % pattern.length` to index into the pattern.
    pub step: Option<u64>,
    /// `true` on exactly the sample the gate-off boundary is crossed
    /// inside the current step. Fires at most once per step.
    pub gate_off: bool,
}

/// Sample-accurate 16th-note clock with swing and gate length.
///
/// `samples_per_step` is recomputed lazily from `bpm` + `sr` (16th
/// notes = 4 per quarter note). Pair-level swing: on each pair of
/// steps, the downbeat is lengthened and the upbeat shortened by
/// `swing * samples_per_step` (so pair duration stays exactly
/// `2 * samples_per_step` and BPM is preserved).
#[derive(Clone, Debug)]
pub struct Clock {
    sr: f32,
    bpm: f32,
    swing: f32,
    gate_length: f32,
    running: bool,

    samples_per_step: f32,
    /// Position within the current step, in samples. Fractional so
    /// non-integer `samples_per_step` at arbitrary BPM doesn't drift.
    pos_in_step: f32,
    /// Absolute step counter from the start of playback. Never wraps.
    current_step: u64,
    /// `true` once we've already fired the gate-off event for the
    /// current step. Cleared when the step advances.
    gate_fired: bool,
}

impl Clock {
    pub fn new(sample_rate: f32) -> Self {
        let mut c = Self {
            sr: sample_rate,
            bpm: 120.0,
            swing: 0.0,
            gate_length: 0.5,
            running: false,
            samples_per_step: 0.0,
            pos_in_step: 0.0,
            current_step: 0,
            gate_fired: false,
        };
        c.recompute_step_length();
        c
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.recompute_step_length();
    }

    /// Set tempo in BPM (quarter-note tempo). Takes effect immediately;
    /// the current step's remaining time is scaled proportionally so
    /// the playhead doesn't jump.
    pub fn set_bpm(&mut self, bpm: f32) {
        let new_bpm = bpm.clamp(20.0, 300.0);
        if (new_bpm - self.bpm).abs() < f32::EPSILON {
            return;
        }
        let old_len = self.samples_per_step.max(f32::EPSILON);
        let frac = self.pos_in_step / old_len;
        self.bpm = new_bpm;
        self.recompute_step_length();
        self.pos_in_step = frac * self.samples_per_step;
    }

    /// Swing amount `0.0..=0.75`. 0.0 = straight sixteenths, 0.5 =
    /// triplet feel, 0.75 = heavy shuffle. See module docs.
    pub fn set_swing(&mut self, swing: f32) {
        self.swing = swing.clamp(0.0, 0.75);
    }

    /// Gate length as a fraction of the current step, `0.0..=1.0`.
    /// 0.5 = half-length, 1.0 = tied (gate never releases before the
    /// next step boundary — i.e. no separate `gate_off` event).
    pub fn set_gate_length(&mut self, g: f32) {
        self.gate_length = g.clamp(0.0, 1.0);
    }

    /// Start/stop transport. Stopping resets nothing — resume continues
    /// at the current playhead position. Use `rewind` to jump back to
    /// step 0.
    pub fn set_running(&mut self, run: bool) {
        self.running = run;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Jump back to step 0, sample 0. Does not change `running`.
    pub fn rewind(&mut self) {
        self.pos_in_step = 0.0;
        self.current_step = 0;
        self.gate_fired = false;
    }

    /// Absolute step index of the step currently under the playhead.
    pub fn current_step(&self) -> u64 {
        self.current_step
    }

    /// Normalized position 0..1 within the current step. Used by the UI
    /// to interpolate the playhead between step boundaries (e.g. for
    /// the beat-indicator LEDs in the transpose section, which need to
    /// look slightly ahead of the published step to compensate for
    /// audio-buffer + GUI-frame latency).
    pub fn step_phase(&self) -> f32 {
        if self.samples_per_step <= 0.0 { 0.0 }
        else { (self.pos_in_step / self.samples_per_step).clamp(0.0, 1.0) }
    }

    /// Length in samples of step index `i`, accounting for swing.
    /// Pair-level: downbeats (even absolute index) are longer, upbeats
    /// (odd) are shorter.
    fn step_length(&self, i: u64) -> f32 {
        let swing_samples = self.swing * self.samples_per_step;
        if i % 2 == 0 {
            self.samples_per_step + swing_samples
        } else {
            self.samples_per_step - swing_samples
        }
    }

    fn recompute_step_length(&mut self) {
        // 16th notes: 4 per beat, bpm beats per minute.
        self.samples_per_step = 60.0 * self.sr / (self.bpm * 4.0);
    }

    /// Advance the clock by one sample. Returns a `ClockTick` telling
    /// the caller whether a step boundary and/or gate-off fired this
    /// sample.
    ///
    /// **Ordering within the sample:** we check the gate-off threshold
    /// first (against the current step's length), then advance
    /// `pos_in_step`, then check whether we crossed into the next
    /// step. If both fire on the same sample, the returned tick will
    /// carry `gate_off = true` AND `step = Some(next)`. Callers should
    /// process gate-off *before* applying a new trigger so they don't
    /// accidentally kill the fresh note.
    pub fn tick(&mut self) -> ClockTick {
        if !self.running {
            return ClockTick::default();
        }

        let mut out = ClockTick::default();
        let step_len = self.step_length(self.current_step);

        // Gate-off check: fire once per step, at the threshold. A
        // `gate_length` of 1.0 means the gate stays open for the full
        // step — no explicit gate_off event is emitted (the next
        // trigger/slide takes over seamlessly).
        if !self.gate_fired && self.gate_length < 1.0 {
            let gate_off_at = step_len * self.gate_length;
            if self.pos_in_step >= gate_off_at {
                out.gate_off = true;
                self.gate_fired = true;
            }
        }

        // Advance the playhead.
        self.pos_in_step += 1.0;

        // Step boundary check.
        if self.pos_in_step >= step_len {
            self.pos_in_step -= step_len;
            self.current_step += 1;
            self.gate_fired = false;
            out.step = Some(self.current_step);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count step boundaries emitted across `n` samples.
    fn run_and_count_steps(clock: &mut Clock, n: usize) -> usize {
        let mut steps = 0;
        for _ in 0..n {
            if clock.tick().step.is_some() {
                steps += 1;
            }
        }
        steps
    }

    #[test]
    fn stopped_clock_emits_nothing() {
        let mut c = Clock::new(48_000.0);
        // Not running → no events even across a long window.
        for _ in 0..10_000 {
            assert_eq!(c.tick(), ClockTick::default());
        }
    }

    #[test]
    fn straight_120bpm_emits_eight_sixteenths_per_second() {
        // 120 BPM → 2 quarter notes/sec → 8 sixteenths/sec.
        let mut c = Clock::new(48_000.0);
        c.set_bpm(120.0);
        c.set_gate_length(1.0); // disable gate_off so we only count steps
        c.set_running(true);
        let steps = run_and_count_steps(&mut c, 48_000);
        assert_eq!(steps, 8, "expected 8 sixteenths/sec at 120 BPM");
    }

    #[test]
    fn gate_off_fires_mid_step_at_half_length() {
        let mut c = Clock::new(48_000.0);
        c.set_bpm(120.0);
        c.set_gate_length(0.5);
        c.set_running(true);
        // At 120 BPM, one sixteenth = 6000 samples. Gate off at 3000.
        let mut gate_off_sample = None;
        for i in 0..6_000 {
            if c.tick().gate_off {
                gate_off_sample = Some(i);
                break;
            }
        }
        let s = gate_off_sample.expect("gate_off should have fired");
        assert!(
            (2995..=3005).contains(&s),
            "gate_off at sample {s}, expected ~3000"
        );
    }

    #[test]
    fn gate_length_one_never_emits_gate_off() {
        let mut c = Clock::new(48_000.0);
        c.set_bpm(120.0);
        c.set_gate_length(1.0);
        c.set_running(true);
        for _ in 0..48_000 {
            assert!(!c.tick().gate_off);
        }
    }

    #[test]
    fn swing_preserves_pair_length_so_bpm_stays_accurate() {
        // At any swing in [0, 0.75], a full pair of steps must be
        // exactly 2 * samples_per_step → BPM unchanged over the long
        // haul. Count pair completions in a 1-second window.
        for &swing in &[0.0_f32, 0.25, 0.5, 0.75] {
            let mut c = Clock::new(48_000.0);
            c.set_bpm(120.0);
            c.set_swing(swing);
            c.set_gate_length(1.0);
            c.set_running(true);
            let steps = run_and_count_steps(&mut c, 48_000);
            // 8 sixteenths/sec at 120 BPM regardless of swing.
            assert!(
                (7..=9).contains(&steps),
                "swing={swing} emitted {steps} steps, expected ~8"
            );
        }
    }

    #[test]
    fn swing_delays_the_first_upbeat() {
        // With swing > 0, step 1 (the first upbeat) must start *later*
        // than it would at straight time.
        let mut straight = Clock::new(48_000.0);
        straight.set_bpm(120.0);
        straight.set_swing(0.0);
        straight.set_gate_length(1.0);
        straight.set_running(true);

        let mut swung = Clock::new(48_000.0);
        swung.set_bpm(120.0);
        swung.set_swing(0.5);
        swung.set_gate_length(1.0);
        swung.set_running(true);

        let first_boundary = |c: &mut Clock| -> usize {
            for i in 0..20_000 {
                if c.tick().step.is_some() {
                    return i;
                }
            }
            panic!("no step boundary in 20k samples");
        };

        let s0 = first_boundary(&mut straight);
        let s1 = first_boundary(&mut swung);
        assert!(
            s1 > s0,
            "swung first boundary {s1} should be later than straight {s0}"
        );
    }

    #[test]
    fn rewind_resets_playhead_without_stopping() {
        let mut c = Clock::new(48_000.0);
        c.set_bpm(120.0);
        c.set_running(true);
        for _ in 0..12_345 {
            c.tick();
        }
        assert!(c.current_step() > 0);
        c.rewind();
        assert_eq!(c.current_step(), 0);
        assert!(c.is_running());
    }

    #[test]
    fn set_bpm_preserves_fractional_position() {
        // Advance part-way through a step, then change BPM. The
        // fractional progress through the step must be preserved, so
        // no audible jump in the playhead.
        let mut c = Clock::new(48_000.0);
        c.set_bpm(120.0);
        c.set_running(true);
        for _ in 0..1_500 {
            c.tick();
        }
        // At 120bpm, samples_per_step = 6000; we're at 1500/6000 = 0.25.
        c.set_bpm(140.0);
        // New samples_per_step ≈ 48000 * 60 / (140*4) ≈ 5142.857.
        // Fractional position should still be ≈ 0.25 (≈ 1285 samples).
        let new_len = 60.0 * 48_000.0 / (140.0 * 4.0);
        let expected = 0.25 * new_len;
        // We can't read pos_in_step directly, so count samples until
        // the next step boundary and check it matches ~(new_len - expected).
        let mut to_boundary = 0usize;
        loop {
            to_boundary += 1;
            if c.tick().step.is_some() {
                break;
            }
        }
        let got = to_boundary as f32;
        let want = new_len - expected;
        assert!(
            (got - want).abs() < 5.0,
            "expected {want} samples to boundary, got {got}"
        );
    }
}
