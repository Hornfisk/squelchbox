//! 16-step pattern model.
//!
//! A `Pattern` is a fixed array of 16 `Step`s plus an active length
//! (1..=16) and per-pattern swing. Each `Step` carries pitch + the three
//! classic 303 per-step toggles: accent, slide, and rest. Stepping the
//! pattern produces a `StepEvent` which the caller feeds to `Voice303`.
//!
//! The sequencer distinguishes two things the 303 conflates:
//!
//! * **Rest** — the step is silent. The voice is not triggered; a
//!   previously-triggered note's amp env continues to decay naturally.
//!   In the DSP path this becomes "no event at all" for that step.
//! * **Slide** — the *previous* step glides into this one instead of
//!   hard-retriggering. In the DSP path this becomes `StepEvent::Slide`
//!   so `Voice303::slide_to()` is called rather than `trigger()`.
//!
//! Gate length handling lives in the clock, not the pattern — the clock
//! knows "we are 30% through this step" and can emit a `GateOff` when
//! that crosses the gate-length threshold. The pattern is pure data.

/// Tiny LCG for the pattern randomizer. We don't want a `rand` dep just
/// for one button, and we don't need cryptographic quality — just
/// deterministic-from-seed and reasonably uncorrelated low bits.
struct LcgRng { state: u64 }

impl LcgRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(0x9E3779B97F4A7C15) }
    }
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / ((1u32 << 24) as f32)
    }
}

/// Maximum steps in a pattern. 16 matches the classic TB-303 / Roland
/// x0x layout; we keep it a compile-time constant so `Step` data can
/// live inline without allocation.
pub const MAX_PATTERN_LEN: usize = 16;

/// Per-step data. `semitone` is the MIDI note, `accent`/`slide`/`rest`
/// are the three toggles. `Default` is "C4, no flags set" — the same
/// rest a blank pattern slot would give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Step {
    /// MIDI note number (0..=127). Default 60 = C4.
    pub semitone: u8,
    /// Accent flag — voice is triggered with high velocity.
    pub accent: bool,
    /// Slide flag — the *previous* step glides into this one. If the
    /// previous step was a rest, the slide flag is ignored (nothing to
    /// slide from).
    pub slide: bool,
    /// Rest flag — the step is silent. When true, `semitone` is ignored.
    pub rest: bool,
}

impl Default for Step {
    fn default() -> Self {
        Self { semitone: 60, accent: false, slide: false, rest: false }
    }
}

/// What the pattern wants the voice to do at a given step boundary.
///
/// The ordering matters: the sequencer emits exactly one `StepEvent`
/// per step advance, based on the *current* step's data and whether the
/// *next* step has its slide flag set. See `Pattern::event_at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepEvent {
    /// Start this note fresh (hard retrigger).
    Trigger { semitone: u8, accent: bool },
    /// Glide the currently-ringing voice to this note. Caller should
    /// call `Voice303::slide_to()` rather than `trigger()`.
    Slide { semitone: u8, accent: bool },
    /// Nothing to do. Either a rest, or the playhead didn't cross a
    /// step boundary. The previously-triggered note (if any) continues
    /// to decay on its own envelopes.
    None,
}

/// A full pattern: 16 step slots + active length + swing.
///
/// `length` is the number of steps the playhead actually walks before
/// wrapping — 1..=16. Slots beyond `length` are ignored (but preserved
/// so shortening and re-lengthening a pattern is lossless).
///
/// `swing` is 0.0..=0.75 and shifts even-numbered steps (0-indexed:
/// 1, 3, 5, ...) later in the step period. 0.0 = straight, 0.5 =
/// triplet feel, 0.75 = extreme shuffle. Classic 303 uses 0.0.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Pattern {
    pub steps: [Step; MAX_PATTERN_LEN],
    pub length: u8,
    pub swing: f32,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            steps: [Step::default(); MAX_PATTERN_LEN],
            length: 16,
            swing: 0.0,
        }
    }
}

/// Four-slot pattern bank, classic Roland-style I/II/III/IV. Only the
/// `active` slot drives the audio thread; switching is instant (no bar
/// quantize for now). Stored alongside the patterns themselves so DAW
/// state restore brings back which slot was selected.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PatternBank {
    pub patterns: [Pattern; 4],
    pub active: u8,
}

impl Default for PatternBank {
    fn default() -> Self {
        // Slot I gets the classic riff so a fresh plugin still has
        // something to listen to; II/III/IV start blank-ish (default
        // C4 row, no rests) ready to be written into.
        let mut patterns: [Pattern; 4] = Default::default();
        patterns[0] = Pattern::default_classic_riff();
        Self { patterns, active: 0 }
    }
}

impl PatternBank {
    pub fn active(&self) -> &Pattern {
        &self.patterns[self.active as usize % 4]
    }
    pub fn active_mut(&mut self) -> &mut Pattern {
        let i = self.active as usize % 4;
        &mut self.patterns[i]
    }
    pub fn set_active(&mut self, i: u8) {
        self.active = i % 4;
    }
}

impl Pattern {
    /// Construct a pattern where every slot is a rest. Useful as a
    /// starting point for authoring via UI — the user can un-rest and
    /// edit individual slots.
    pub fn empty() -> Self {
        let mut p = Self::default();
        for s in p.steps.iter_mut() {
            s.rest = true;
        }
        p
    }

    /// A recognisable-from-the-first-bar default bassline: C minor
    /// acid riff at 16 steps, C2 root, with accent pulses and a
    /// slide-legato pair. Used as the M5 factory default so `acid`
    /// in the terminal plays something intelligible without any UI.
    pub fn default_classic_riff() -> Self {
        // MIDI: C2=36, D#2=39, G2=43, C3=48, A#1=34
        let mk = |semi, accent, slide| Step { semitone: semi, accent, slide, rest: false };
        let rest = Step { semitone: 36, accent: false, slide: false, rest: true };
        let steps = [
            mk(36, true,  false), // 1 — downbeat, accented
            rest,                 // 2
            mk(36, false, false), // 3
            mk(48, false, false), // 4  — upper octave
            mk(36, false, false), // 5
            rest,                 // 6
            mk(36, false, true),  // 7  — slide target
            mk(39, false, false), // 8  — slid-into note (D#)
            mk(36, true,  false), // 9  — downbeat, accented
            rest,                 // 10
            mk(36, false, false), // 11
            mk(34, false, false), // 12 — A#1 drop
            mk(36, false, false), // 13
            rest,                 // 14
            mk(36, false, true),  // 15 — slide
            mk(43, false, false), // 16 — G2
        ];
        Self { steps, length: 16, swing: 0.0 }
    }

    /// Rotate the active steps left by `n` (wrapping). Slots beyond
    /// `length` are left untouched so shortening + re-lengthening is
    /// still lossless.
    pub fn rotate_left(&mut self, n: usize) {
        let len = self.length.max(1) as usize;
        if len <= 1 { return; }
        let n = n % len;
        if n == 0 { return; }
        self.steps[..len].rotate_left(n);
    }

    /// Rotate the active steps right by `n` (wrapping).
    pub fn rotate_right(&mut self, n: usize) {
        let len = self.length.max(1) as usize;
        if len <= 1 { return; }
        let n = n % len;
        if n == 0 { return; }
        self.steps[..len].rotate_right(n);
    }

    /// Generate a fresh acid-flavoured random pattern, quantized to a
    /// minor pentatonic rooted at `root_semi` over two octaves.
    ///
    /// `density` is the probability that any given step is *not* a rest
    /// (0.0..=1.0). `accent_p` and `slide_p` are the per-non-rest-step
    /// probabilities for the accent and slide flags. `seed` drives an
    /// internal LCG so callers can reproduce a given pattern by reusing
    /// the seed (or pass a time-derived value for "fresh every click").
    pub fn random(
        seed: u64,
        density: f32,
        accent_p: f32,
        slide_p: f32,
        root_semi: u8,
    ) -> Self {
        const SCALE: [i32; 5] = [0, 3, 5, 7, 10];
        let mut palette = [0i32; 10];
        for (i, s) in SCALE.iter().enumerate() {
            palette[i] = *s;
            palette[i + 5] = s + 12;
        }
        let mut rng = LcgRng::new(seed);
        let density = density.clamp(0.0, 1.0);
        let accent_p = accent_p.clamp(0.0, 1.0);
        let slide_p = slide_p.clamp(0.0, 1.0);
        let mut steps = [Step::default(); MAX_PATTERN_LEN];
        for s in steps.iter_mut() {
            let rest = rng.next_f32() >= density;
            if rest {
                *s = Step { semitone: root_semi, accent: false, slide: false, rest: true };
                continue;
            }
            let pick = palette[(rng.next_u32() as usize) % palette.len()];
            let semi = (root_semi as i32 + pick).clamp(0, 127) as u8;
            let accent = rng.next_f32() < accent_p;
            let slide = rng.next_f32() < slide_p;
            *s = Step { semitone: semi, accent, slide, rest: false };
        }
        // Always make step 0 audible — a leading rest is the worst UX
        // for "I just clicked randomize and nothing happens".
        if steps[0].rest {
            let pick = palette[(rng.next_u32() as usize) % palette.len()];
            steps[0] = Step {
                semitone: (root_semi as i32 + pick).clamp(0, 127) as u8,
                accent: true,
                slide: false,
                rest: false,
            };
        }
        Self { steps, length: 16, swing: 0.0 }
    }

    /// Clamp `length` and `swing` to valid ranges. Call this any time
    /// the fields are mutated from outside.
    pub fn sanitize(&mut self) {
        self.length = self.length.clamp(1, MAX_PATTERN_LEN as u8);
        self.swing = self.swing.clamp(0.0, 0.75);
    }

    /// Look up step `i` modulo the active pattern length.
    pub fn step(&self, i: usize) -> Step {
        let len = self.length.max(1) as usize;
        self.steps[i % len]
    }

    /// Decide what event to emit when the playhead lands on step `i`.
    ///
    /// Rules, in order:
    ///   1. If step `i` is a rest → `None`.
    ///   2. If the *previous* step (i-1, wrapping) is a non-rest that
    ///      has its `slide` flag set → `Slide` (the voice glides from
    ///      the previous ringing note into this one).
    ///   3. Otherwise → `Trigger`.
    ///
    /// The slide flag on a step means "slide *out of* me into the next
    /// one". This matches the TB-303 front-panel semantic the user sees
    /// when they click the slide button — marking step N with slide
    /// couples N→N+1, so step N+1 is the one that glides in.
    ///
    /// Rests break slides: if the previous step is a rest, step `i` is
    /// always a hard trigger even if the step before the rest had slide
    /// set. The caller (runtime.rs) handles the first-run case where
    /// the voice hasn't been triggered yet by demoting leading Slides
    /// to Triggers.
    pub fn event_at(&self, i: usize) -> StepEvent {
        let s = self.step(i);
        if s.rest {
            return StepEvent::None;
        }

        let len = self.length.max(1) as usize;
        let prev_idx = (i + len - 1) % len;
        let prev = self.steps[prev_idx];
        if prev.slide && !prev.rest {
            StepEvent::Slide { semitone: s.semitone, accent: s.accent }
        } else {
            StepEvent::Trigger { semitone: s.semitone, accent: s.accent }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pattern_is_length_16_straight() {
        let p = Pattern::default();
        assert_eq!(p.length, 16);
        assert_eq!(p.swing, 0.0);
        for s in p.steps.iter() {
            assert_eq!(*s, Step::default());
            assert!(!s.rest);
        }
    }

    #[test]
    fn empty_pattern_is_all_rests() {
        let p = Pattern::empty();
        for s in p.steps.iter() {
            assert!(s.rest);
        }
    }

    #[test]
    fn sanitize_clamps_length_and_swing() {
        let mut p = Pattern::default();
        p.length = 99;
        p.swing = 5.0;
        p.sanitize();
        assert_eq!(p.length, MAX_PATTERN_LEN as u8);
        assert_eq!(p.swing, 0.75);

        p.length = 0;
        p.swing = -1.0;
        p.sanitize();
        assert_eq!(p.length, 1);
        assert_eq!(p.swing, 0.0);
    }

    #[test]
    fn rest_step_emits_none() {
        let mut p = Pattern::default();
        p.steps[0].rest = true;
        assert_eq!(p.event_at(0), StepEvent::None);
    }

    #[test]
    fn non_rest_step_with_only_rests_before_always_triggers() {
        // All earlier steps are rests → previous step (2) is a rest,
        // so step 3 hard-triggers regardless of its own flags.
        let mut p = Pattern::empty();
        p.steps[3].rest = false;
        p.steps[3].semitone = 48;
        p.steps[3].slide = true;
        assert_eq!(
            p.event_at(3),
            StepEvent::Trigger { semitone: 48, accent: false }
        );
    }

    #[test]
    fn slide_flag_on_prev_step_emits_slide_into_this_step() {
        // Step 0 has slide → step 1 should be slid *into*.
        let mut p = Pattern::default();
        p.steps[0] = Step { semitone: 48, accent: false, slide: true,  rest: false };
        p.steps[1] = Step { semitone: 50, accent: true,  slide: false, rest: false };
        assert_eq!(
            p.event_at(0),
            StepEvent::Trigger { semitone: 48, accent: false }
        );
        assert_eq!(
            p.event_at(1),
            StepEvent::Slide { semitone: 50, accent: true }
        );
    }

    #[test]
    fn slide_across_rest_is_broken() {
        // Step 0 has slide, step 1 is rest, step 2 is a normal note.
        // The rest in between kills the slide — step 2 hard triggers.
        let mut p = Pattern::default();
        p.steps[0] = Step { semitone: 48, accent: false, slide: true,  rest: false };
        p.steps[1].rest = true;
        p.steps[2] = Step { semitone: 50, accent: false, slide: false, rest: false };
        assert_eq!(p.event_at(1), StepEvent::None);
        assert_eq!(
            p.event_at(2),
            StepEvent::Trigger { semitone: 50, accent: false }
        );
    }

    #[test]
    fn step_wraps_on_pattern_length() {
        let mut p = Pattern::default();
        p.length = 4;
        p.steps[0].semitone = 10;
        p.steps[4].semitone = 99; // outside active length
        assert_eq!(p.step(0).semitone, 10);
        assert_eq!(p.step(4).semitone, 10); // wrapped back to step 0
        // step(5) → 5 % 4 = 1 → steps[1] which is still Default (60).
        assert_eq!(p.step(5).semitone, 60);
    }

    #[test]
    fn rotate_left_wraps_active_steps() {
        let mut p = Pattern::default();
        for i in 0..16 { p.steps[i].semitone = i as u8; }
        p.rotate_left(1);
        assert_eq!(p.steps[0].semitone, 1);
        assert_eq!(p.steps[15].semitone, 0);
    }

    #[test]
    fn rotate_right_wraps_active_steps() {
        let mut p = Pattern::default();
        for i in 0..16 { p.steps[i].semitone = i as u8; }
        p.rotate_right(1);
        assert_eq!(p.steps[0].semitone, 15);
        assert_eq!(p.steps[1].semitone, 0);
    }

    #[test]
    fn random_pattern_is_deterministic_per_seed() {
        let a = Pattern::random(42, 0.7, 0.3, 0.2, 36);
        let b = Pattern::random(42, 0.7, 0.3, 0.2, 36);
        assert_eq!(a.steps, b.steps);
    }

    #[test]
    fn random_pattern_step_zero_always_audible() {
        for seed in 0..16u64 {
            let p = Pattern::random(seed, 0.05, 0.5, 0.5, 36);
            assert!(!p.steps[0].rest, "step 0 must never be a rest (seed={seed})");
        }
    }

    #[test]
    fn accent_flag_propagates_through_event_at() {
        let mut p = Pattern::default();
        p.steps[0].accent = true;
        assert_eq!(
            p.event_at(0),
            StepEvent::Trigger { semitone: 60, accent: true }
        );
    }
}
