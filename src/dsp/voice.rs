//! Monophonic 303 voice: oscillator + diode ladder filter + envelopes + pitch control.
//!
//! Assembles `BlepSaw`/`BlepSquare` → `DiodeLadder4Pole` (oversampled 2×)
//! → `AmpEnv`/`FilterEnv`/`AccentEnv` into a per-sample render pipeline.

use crate::dsp::envelope::{AccentEnv, AmpEnv, FilterEnv};
use crate::dsp::filter_diode::DiodeLadder4Pole;
use crate::dsp::oscillator::{BlepSaw, BlepSquare};
use crate::dsp::oversampler::Halfband2x;

/// Per-instance quality tier. Controls how aggressively the nonlinear
/// filter block is oversampled. `Normal` is bypass (base rate), `High`
/// adds 2× halfband oversampling, `Ultra` is reserved for a future 4×
/// mode and currently falls back to `High`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityMode {
    Normal,
    High,
    Ultra,
}

/// Oscillator selection. The real 303 only has saw and square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    Saw,
    Square,
}

/// Per-trigger voice parameters captured at note-on. These are sampled
/// from the smoothed plugin params once per step/MIDI event, then the
/// envelopes run freely until the next trigger or gate-off.
#[derive(Clone, Copy, Debug)]
pub struct VoiceParams {
    pub waveform: Waveform,
    /// Cutoff in Hz before env-mod is applied.
    pub base_cutoff_hz: f32,
    /// Resonance `0..1` where 1.0 is at or near self-oscillation.
    pub resonance: f32,
    /// Env Mod amount `0..1` — how much the filter env opens the cutoff.
    pub env_mod: f32,
    /// Amp/filter decay in ms (`Decay` knob — both envs share it in a 303).
    pub decay_ms: f32,
    /// Filter env curve steepness.
    pub filter_curve: f32,
    /// Global tuning offset in semitones.
    pub tuning_semitones: f32,
    /// Accent amount `0..1` — scales the accent env's effect if the step
    /// is accented.
    pub accent_amount: f32,
}

/// Per-sample live parameters pushed from `plugin.rs::process()` so that
/// knob sweeps are audible mid-note without retriggering envelopes. This
/// is a distinct type from [`VoiceParams`] (which is captured once at
/// note-on) because the live path runs in the hot loop and shouldn't
/// touch anything with per-trigger semantics (filter curve, etc.).
#[derive(Clone, Copy, Debug)]
pub struct VoiceLiveParams {
    pub waveform: Waveform,
    pub base_cutoff_hz: f32,
    pub resonance: f32,
    pub env_mod: f32,
    pub accent_amount: f32,
    pub tuning_semitones: f32,
    pub decay_ms: f32,
    /// Portamento glide time — seconds for the oscillator frequency to
    /// travel ≈99% of the way to a new target after a slide-legato
    /// note-on. Pulled from the front-panel Slide knob.
    pub slide_ms: f32,
}

/// Always-on drift LFO bank — two free-running sines at irrational-ratio
/// rates so their sum never repeats. The outputs modulate tuning and
/// cutoff by tiny amounts so held notes don't sit perfectly still.
///
/// Depth is deliberately subtle: ~3 cents of pitch wobble and ~1% cutoff
/// wobble at maximum, summed across both LFOs. You feel it, you don't
/// hear it as vibrato.
struct DriftBank {
    phase_a: f32,
    phase_b: f32,
}

impl DriftBank {
    fn new() -> Self {
        Self { phase_a: 0.0, phase_b: 0.37 }
    }

    /// Advance both LFOs and return `(pitch_cents, cutoff_ratio)` where
    /// `pitch_cents ∈ ~[-3, 3]` and `cutoff_ratio ∈ ~[-0.01, 0.01]`.
    #[inline]
    fn tick(&mut self, sr: f32) -> (f32, f32) {
        const RATE_A: f32 = 0.27; // Hz
        const RATE_B: f32 = 0.19; // Hz
        self.phase_a += RATE_A / sr;
        if self.phase_a >= 1.0 {
            self.phase_a -= 1.0;
        }
        self.phase_b += RATE_B / sr;
        if self.phase_b >= 1.0 {
            self.phase_b -= 1.0;
        }
        let a = (self.phase_a * std::f32::consts::TAU).sin();
        let b = (self.phase_b * std::f32::consts::TAU).sin();
        let pitch_cents = (a + 0.6 * b) * 1.9;
        let cutoff_ratio = (a * 0.4 + b) * 0.006;
        (pitch_cents, cutoff_ratio)
    }
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            waveform: Waveform::Saw,
            base_cutoff_hz: 500.0,
            resonance: 0.5,
            env_mod: 0.6,
            decay_ms: 200.0,
            filter_curve: 2.0,
            tuning_semitones: 0.0,
            accent_amount: 0.6,
        }
    }
}

pub struct Voice303 {
    saw: BlepSaw,
    square: BlepSquare,
    filter: DiodeLadder4Pole,
    amp_env: AmpEnv,
    filter_env: FilterEnv,
    accent_env: AccentEnv,
    oversampler: Halfband2x,
    quality: QualityMode,
    sample_rate: f32,
    /// Current oscillator frequency in semitones (note + tuning). Slides
    /// toward `target_freq_semitones` one-pole-style each sample.
    current_freq_semitones: f32,
    /// Target frequency the voice is gliding toward. On a fresh trigger
    /// the current value snaps to this; on slide-legato it chases it.
    target_freq_semitones: f32,
    /// Per-sample one-pole coefficient for the slide smoother. Cached
    /// from `slide_ms` so we don't recompute an exp() every sample.
    slide_coef: f32,
    /// Last `slide_ms` value seen on the live path — used to invalidate
    /// the cached `slide_coef` when the knob actually moves.
    last_slide_ms: f32,
    /// Cached hz value the filter + oscillator actually see, after
    /// drift and slide are applied. Kept as a field so tests can
    /// inspect it without re-deriving from semitones.
    current_freq_hz: f32,
    /// Last MIDI note number, retained so live tuning updates can
    /// recompute the frequency without a fresh trigger.
    current_midi_note: u8,
    /// Last tuning offset seen on the live path — skip recompute when
    /// the smoother is at rest.
    last_tuning_semitones: f32,
    drift: DriftBank,
    /// Active waveform for this note.
    waveform: Waveform,
    /// Cached per-note values so the per-sample loop stays fast.
    base_cutoff_hz: f32,
    resonance: f32,
    env_mod: f32,
    accent_amount: f32,
    accent_active: bool,
    /// AC-coupling HPF state for the oscillator output. Models the
    /// capacitor between the VCO and VCF in the real circuit — flat
    /// sections of the waveform droop exponentially, giving the 303
    /// its characteristic buzzy/thin square and slightly brightened saw.
    osc_ac_x1: f32,
    osc_ac_y1: f32,
    osc_ac_r: f32,
}

impl Voice303 {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            saw: BlepSaw::new(sample_rate),
            square: BlepSquare::new(sample_rate),
            filter: DiodeLadder4Pole::new(sample_rate),
            amp_env: AmpEnv::new(sample_rate),
            filter_env: FilterEnv::new(sample_rate),
            accent_env: AccentEnv::new(sample_rate),
            oversampler: Halfband2x::new(),
            // Default to Normal so unit tests see base-rate behavior.
            // The plugin bumps to High in `initialize()`.
            quality: QualityMode::Normal,
            sample_rate,
            current_freq_semitones: 57.0,
            target_freq_semitones: 57.0,
            slide_coef: 0.0,
            last_slide_ms: -1.0,
            current_freq_hz: 220.0,
            current_midi_note: 57,
            last_tuning_semitones: 0.0,
            drift: DriftBank::new(),
            waveform: Waveform::Saw,
            base_cutoff_hz: 500.0,
            resonance: 0.5,
            env_mod: 0.6,
            accent_amount: 0.6,
            accent_active: false,
            osc_ac_x1: 0.0,
            osc_ac_y1: 0.0,
            osc_ac_r: (-std::f32::consts::TAU * 30.0 / sample_rate).exp(),
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.osc_ac_r = (-std::f32::consts::TAU * 30.0 / sr).exp();
        // Envelopes always run at the base rate.
        self.amp_env.set_sample_rate(sr);
        self.filter_env.set_sample_rate(sr);
        self.accent_env.set_sample_rate(sr);
        // Oscillator + filter may run at 2× when the voice is in
        // oversampling mode — apply the right rate for the current
        // quality tier.
        self.apply_quality_rates();
    }

    pub fn reset(&mut self) {
        self.saw.reset();
        self.square.reset();
        self.filter.reset();
        self.oversampler.reset();
        self.osc_ac_x1 = 0.0;
        self.osc_ac_y1 = 0.0;
    }

    /// Select the oversampling tier. Called from `plugin.rs::initialize`
    /// — surfaces on the front panel in M7 full UI. When oversampling
    /// engages, the oscillator and filter are told they're running at
    /// 2× the voice's sample rate so their cutoff coefficients and BLEP
    /// corrections stay accurate.
    pub fn set_quality(&mut self, quality: QualityMode) {
        if self.quality != quality {
            self.oversampler.reset();
        }
        self.quality = quality;
        self.apply_quality_rates();
    }

    fn apply_quality_rates(&mut self) {
        let inner_sr = match self.quality {
            QualityMode::Normal => self.sample_rate,
            QualityMode::High | QualityMode::Ultra => self.sample_rate * 2.0,
        };
        self.saw.set_sample_rate(inner_sr);
        self.square.set_sample_rate(inner_sr);
        self.filter.set_sample_rate(inner_sr);
    }

    /// Trigger a new note. Retriggers amp and filter envelopes. For
    /// slide-legato behavior (no env retrigger), use [`slide_to`] instead.
    pub fn trigger(&mut self, midi_note: u8, accent: bool, params: &VoiceParams) {
        self.waveform = params.waveform;
        self.base_cutoff_hz = params.base_cutoff_hz;
        self.resonance = params.resonance;
        self.env_mod = params.env_mod;
        self.accent_amount = params.accent_amount;
        self.accent_active = accent;

        self.current_midi_note = midi_note;
        self.last_tuning_semitones = params.tuning_semitones;
        let target = midi_note as f32 + params.tuning_semitones;
        self.target_freq_semitones = target;
        // Fresh trigger → snap the smoother to the target so the
        // attack transient has a clean pitch.
        self.current_freq_semitones = target;
        self.current_freq_hz = midi_f_to_hz(target);

        // Authentic 303 VEG — fixed shape, DECAY knob only drives FilterEnv.
        self.amp_env.gate_on();

        let filter_env_dur_s = params.decay_ms * 0.75 / 1000.0;
        self.filter_env.trigger(filter_env_dur_s, params.filter_curve);

        if accent {
            self.accent_env.trigger();
        }
    }

    /// Legato slide: retarget oscillator frequency but do NOT retrigger
    /// the amp envelope. Accent behavior still fires if requested.
    pub fn slide_to(&mut self, midi_note: u8, accent: bool, params: &VoiceParams) {
        self.current_midi_note = midi_note;
        self.last_tuning_semitones = params.tuning_semitones;
        // Only the TARGET moves — `current_freq_semitones` glides
        // toward it in `tick()`, driving the classic 303 slide.
        self.target_freq_semitones = midi_note as f32 + params.tuning_semitones;
        self.accent_active = accent;
        if accent {
            self.accent_env.trigger();
            // Extend filter env so the slide has fresh filter modulation.
            let filter_env_dur_s = params.decay_ms * 0.75 / 1000.0;
            self.filter_env.trigger(filter_env_dur_s, params.filter_curve);
        }
    }

    /// Release the amp envelope (MIDI note-off, or rest step).
    pub fn gate_off(&mut self) {
        self.amp_env.gate_off();
    }

    /// Live knob updates that should take effect mid-note without
    /// retriggering envelopes. Called per-sample from `process()` so the
    /// user can sweep any of the front-panel knobs in real time.
    #[inline]
    pub fn set_live(&mut self, p: &VoiceLiveParams) {
        self.waveform = p.waveform;
        self.base_cutoff_hz = p.base_cutoff_hz;
        self.resonance = p.resonance;
        self.env_mod = p.env_mod;
        self.accent_amount = p.accent_amount;

        if (p.tuning_semitones - self.last_tuning_semitones).abs() > 1e-4 {
            self.last_tuning_semitones = p.tuning_semitones;
            // Tuning shifts the target — the glide smoother handles the
            // transition so a Tuning-knob sweep has a gentle slew.
            self.target_freq_semitones =
                self.current_midi_note as f32 + p.tuning_semitones;
        }

        // Cache the one-pole slide coefficient whenever `slide_ms`
        // actually moves. Coef is derived so ~99% of the gap is
        // covered after `slide_ms` milliseconds (≈ 4.6 time constants).
        if (p.slide_ms - self.last_slide_ms).abs() > 1e-4 {
            self.last_slide_ms = p.slide_ms;
            let tau_s = (p.slide_ms / 1000.0 / 4.6).max(1.0e-5);
            self.slide_coef = 1.0 - (-1.0 / (tau_s * self.sample_rate)).exp();
        }

        // Authentic 303: DECAY knob only drives the filter env, not the amp.
        self.filter_env
            .set_duration_s(p.decay_ms * 0.75 / 1000.0);
    }

    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Per-sample voice output. Call once per output sample.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        if !self.amp_env.is_active() {
            return 0.0;
        }

        let env_f = self.filter_env.tick();
        let env_a = self.amp_env.tick();
        let acc = self.accent_env.tick();

        // Slide: one-pole chase toward the target pitch. If the
        // coefficient is still zero (e.g. a voice instance that's
        // never had set_live called — unit tests, basically) just
        // snap instantly so those tests see deterministic pitch.
        if self.slide_coef > 0.0 {
            self.current_freq_semitones +=
                (self.target_freq_semitones - self.current_freq_semitones) * self.slide_coef;
        } else {
            self.current_freq_semitones = self.target_freq_semitones;
        }

        // Drift LFO — tiny pitch + cutoff wobble for analog character.
        let (pitch_cents, cutoff_drift) = self.drift.tick(self.sample_rate);
        let drifted_semitones = self.current_freq_semitones + pitch_cents * 0.01;
        self.current_freq_hz = midi_f_to_hz(drifted_semitones);

        // Cutoff modulation. Env Mod opens upward in octaves. Accent adds
        // a little more on top (residual cap charge affects all notes, not
        // just accented ones — authentic 303 D24/C13 behaviour). Drift
        // then multiplies the final cutoff by a small ± ratio.
        let accent_cutoff_octaves = self.accent_amount * acc * 2.0;
        let cutoff_octaves = self.env_mod * env_f * 4.0 + accent_cutoff_octaves;
        // Cap the post-modulation cutoff at ~8 kHz: the real 303's diode
        // ladder loses transistor bandwidth above this and the front-panel
        // CUTOFF knob doesn't reach higher anyway. Going further is also
        // where the unit-delay-feedback approximation in DiodeLadder4Pole
        // starts losing phase margin and detuning the resonance peak.
        const FC_CEILING: f32 = 8_000.0;
        let cutoff_hz = (self.base_cutoff_hz
            * 2f32.powf(cutoff_octaves)
            * (1.0 + cutoff_drift))
            .clamp(20.0, FC_CEILING.min(self.sample_rate * 0.45));
        self.filter.set_cutoff(cutoff_hz);

        // Authentic 303: accent CV is wired to VCA + VCF cutoff only, not
        // resonance. Modulating reso on accent gave a chirpy character;
        // removing it gives the authentic punchy weight.
        self.filter.set_resonance(self.resonance);

        let filtered = match self.quality {
            QualityMode::Normal => {
                let osc_sample = self.tick_osc();
                self.filter.process(osc_sample)
            }
            // Ultra falls back to High until the 4× path lands. The
            // match arm is listed explicitly so the compiler catches
            // any future enum extensions.
            QualityMode::High | QualityMode::Ultra => self.tick_oversampled_filter(),
        };

        // Output stage: models the 303's VCA transistor, which
        // compresses asymmetrically. The bias makes the positive half
        // clip slightly earlier than the negative, adding even
        // harmonics (warmth). At normal signal levels it's near-linear;
        // it only bites when resonance or accent drives the signal hot.
        let filtered = vca_sat(filtered);

        // Accent amplitude boost: residual cap charge affects all notes
        // (authentic 303 — cap only charges on accented steps but its
        // remaining voltage lifts both accented and unaccented notes).
        let accent_amp_boost = 1.0 + self.accent_amount * acc;

        // Resonance-to-VCA compensation: the real 303 feeds a fraction of
        // the resonance pot voltage to the VCA to offset the volume drop
        // that a high-Q filter produces.
        const RESO_VCA_SCALE: f32 = 0.35;
        let reso_comp = 1.0 + RESO_VCA_SCALE * self.resonance;

        filtered * env_a * accent_amp_boost * reso_comp
    }

    /// Single oscillator step at the base rate, with AC-coupling HPF.
    #[inline]
    fn tick_osc(&mut self) -> f32 {
        let raw = match self.waveform {
            Waveform::Saw => self.saw.tick(self.current_freq_hz),
            Waveform::Square => self.square.tick(self.current_freq_hz),
        };
        self.ac_couple(raw)
    }

    /// 1-pole HPF (~30 Hz) modelling the coupling capacitor between VCO
    /// and VCF. Causes flat waveform sections to droop exponentially —
    /// the source of the 303 square's distinctive thin/buzzy character.
    #[inline(always)]
    fn ac_couple(&mut self, x: f32) -> f32 {
        let y = x - self.osc_ac_x1 + self.osc_ac_r * self.osc_ac_y1;
        self.osc_ac_x1 = x;
        self.osc_ac_y1 = y;
        y
    }

    /// Run the oscillator → diode ladder block at 2× the base sample
    /// rate. The oscillator's internal sample rate is already set to
    /// `2 · base_sr` (see `apply_quality_rates`), so calling `tick` at
    /// the real note frequency produces two correct high-rate samples
    /// per base tick. Both are filtered, then the halfband FIR rejects
    /// everything above `fs/2` (the base Nyquist) and we drop every
    /// other output to land back at base rate.
    #[inline]
    fn tick_oversampled_filter(&mut self) -> f32 {
        let (s0, s1) = match self.waveform {
            Waveform::Saw => (
                self.saw.tick(self.current_freq_hz),
                self.saw.tick(self.current_freq_hz),
            ),
            Waveform::Square => (
                self.square.tick(self.current_freq_hz),
                self.square.tick(self.current_freq_hz),
            ),
        };
        // Apply AC coupling to each oversampled oscillator output sample.
        let s0 = self.ac_couple(s0);
        let s1 = self.ac_couple(s1);
        let y0 = self.filter.process(s0);
        let y1 = self.filter.process(s1);
        self.oversampler.downsample2([y0, y1])
    }
}

/// Asymmetric output-stage saturation modelling the 303's VCA
/// transistor. The positive bias makes compression kick in earlier on
/// positive half-cycles, generating even harmonics (2nd, 4th) that
/// give body and warmth. Near-unity at small amplitudes; compresses
/// gently past ±0.7 or so.
#[inline(always)]
fn vca_sat(x: f32) -> f32 {
    const BIAS: f32 = 0.12;
    let biased = x + BIAS;
    let sat = biased / (1.0 + biased * biased).sqrt();
    let dc = BIAS / (1.0 + BIAS * BIAS).sqrt();
    sat - dc
}

/// Convert a MIDI note number to frequency in Hz, with an optional
/// semitone tuning offset applied.
#[inline]
pub fn midi_to_hz(midi: u8, tuning_semitones: f32) -> f32 {
    440.0 * 2f32.powf(((midi as f32) - 69.0 + tuning_semitones) / 12.0)
}

/// Convert a fractional MIDI pitch (note + tuning + glide) directly to
/// Hz. Used by the portamento smoother in `Voice303::tick`.
#[inline]
pub fn midi_f_to_hz(semitones: f32) -> f32 {
    440.0 * 2f32.powf((semitones - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn midi_to_hz_a4() {
        let f = midi_to_hz(69, 0.0);
        assert!((f - 440.0).abs() < 0.001, "A4 should be 440, got {f}");
    }

    #[test]
    fn midi_to_hz_tuning_offset() {
        let f = midi_to_hz(69, 1.0); // +1 semitone
        assert!((f - 466.163_76).abs() < 0.01, "got {f}");
    }

    #[test]
    fn idle_voice_outputs_silence() {
        let mut v = Voice303::new(SR);
        for _ in 0..1_000 {
            assert_eq!(v.tick(), 0.0);
        }
    }

    #[test]
    fn trigger_produces_audible_output() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(57, false, &p); // A2
        let mut peak = 0.0f32;
        for _ in 0..4_000 {
            peak = peak.max(v.tick().abs());
        }
        assert!(peak > 0.01, "voice should produce audible output, got peak {peak}");
    }

    #[test]
    fn gate_off_releases_voice() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(57, false, &p);
        for _ in 0..500 {
            v.tick();
        }
        v.gate_off();
        // After 16 ms release + margin, voice should be silent and inactive.
        for _ in 0..((0.03 * SR) as usize) {
            v.tick();
        }
        assert!(!v.is_active(), "voice should be inactive after gate_off");
    }

    #[test]
    fn quality_high_produces_audio_comparable_to_normal() {
        // Both modes should produce audible output of similar peak
        // amplitude on the same note — High just rejects more aliasing
        // near Nyquist, it doesn't change RMS energy significantly.
        let mut v_normal = Voice303::new(SR);
        v_normal.set_quality(QualityMode::Normal);
        let mut v_high = Voice303::new(SR);
        v_high.set_quality(QualityMode::High);
        let p = VoiceParams::default();
        v_normal.trigger(57, false, &p);
        v_high.trigger(57, false, &p);
        let mut peak_n = 0.0f32;
        let mut peak_h = 0.0f32;
        for _ in 0..4_000 {
            peak_n = peak_n.max(v_normal.tick().abs());
            peak_h = peak_h.max(v_high.tick().abs());
        }
        assert!(peak_n > 0.01, "Normal should produce audio, got {peak_n}");
        assert!(peak_h > 0.01, "High should produce audio, got {peak_h}");
        // Loose equivalence — within a factor of 2 is fine for this
        // smoke test. The filter's inner SR differs between modes so
        // we don't expect bit-identical output.
        assert!(
            (peak_h / peak_n - 1.0).abs() < 1.0,
            "peaks should be comparable, normal={peak_n} high={peak_h}"
        );
    }

    #[test]
    fn set_quality_switches_cleanly() {
        // Switching quality mid-note should not explode or produce NaN.
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(57, false, &p);
        for _ in 0..1_000 {
            let s = v.tick();
            assert!(s.is_finite());
        }
        v.set_quality(QualityMode::High);
        for _ in 0..1_000 {
            let s = v.tick();
            assert!(s.is_finite());
        }
        v.set_quality(QualityMode::Normal);
        for _ in 0..1_000 {
            let s = v.tick();
            assert!(s.is_finite());
        }
    }

    fn live_template() -> VoiceLiveParams {
        VoiceLiveParams {
            waveform: Waveform::Saw,
            base_cutoff_hz: 500.0,
            resonance: 0.5,
            env_mod: 0.6,
            accent_amount: 0.6,
            tuning_semitones: 0.0,
            decay_ms: 200.0,
            slide_ms: 0.01, // effectively-instant glide for tests
        }
    }

    #[test]
    fn set_live_tuning_shifts_frequency_without_retrigger() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(69, false, &p); // A4 = 440 Hz
        // Advance the smoother so it lands on target before we read.
        let live = VoiceLiveParams { tuning_semitones: 12.0, ..live_template() };
        // First set_live caches slide_coef; then a few ticks walk the
        // smoother fully to target (0.01 ms glide is ~immediate).
        for _ in 0..16 {
            v.set_live(&live);
            v.tick();
        }
        // Compare against the target in semitones to avoid the drift
        // LFO's tiny cent-level wobble showing up as a false negative.
        let semi = v.current_freq_semitones;
        assert!(
            (semi - 81.0).abs() < 0.05,
            "+12 st should land at semitone 81, got {semi}"
        );
    }

    #[test]
    fn amp_env_holds_until_gate_off() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(57, false, &p);
        let live = live_template();
        // Tick for 200 ms — the authentic VEG should hold at ~unity
        // the entire time (no decay, it's gate-driven). Check RMS of
        // the last 50 ms (audio crosses zero each cycle, so min is
        // useless; RMS captures sustained energy).
        for _ in 0..((0.15 * SR) as usize) {
            v.set_live(&live);
            v.tick();
        }
        let n = (0.05 * SR) as usize;
        let mut sum_sq = 0.0f32;
        for _ in 0..n {
            v.set_live(&live);
            let s = v.tick();
            sum_sq += s * s;
        }
        let rms = (sum_sq / n as f32).sqrt();
        assert!(v.is_active(), "voice should still be active");
        assert!(
            rms > 0.05,
            "VEG should sustain until gate_off, got RMS {rms}"
        );
    }

    #[test]
    fn trigger_snaps_pitch_even_with_large_slide() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        let live = VoiceLiveParams { slide_ms: 500.0, ..live_template() };
        v.set_live(&live);
        v.trigger(72, false, &p);
        // Fresh trigger should snap instantly — slide only affects
        // subsequent slide_to() calls.
        assert!(
            (v.current_freq_semitones - 72.0).abs() < 0.001,
            "trigger should snap, got {}",
            v.current_freq_semitones
        );
    }

    #[test]
    fn slide_to_glides_toward_target_over_slide_ms() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        let live = VoiceLiveParams { slide_ms: 60.0, ..live_template() };
        v.set_live(&live);
        v.trigger(60, false, &p);
        v.slide_to(72, false, &p);
        // Before ticking, current should still be 60.
        assert!(
            (v.current_freq_semitones - 60.0).abs() < 0.001,
            "slide_to must not snap"
        );
        // Tick for ~80 ms with set_live each sample — more than enough
        // to land well inside the 60 ms glide window.
        let n = (0.08 * SR) as usize;
        for _ in 0..n {
            v.set_live(&live);
            v.tick();
        }
        assert!(
            (v.current_freq_semitones - 72.0).abs() < 0.5,
            "should reach target ±0.5 semi after slide, got {}",
            v.current_freq_semitones
        );
    }

    #[test]
    fn drift_lfo_wobbles_pitch_over_time() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(60, false, &p);
        let f0 = v.current_freq_hz;
        // Tick for a couple of seconds — the drift LFO rates are
        // sub-Hz so we need real time to accumulate a measurable
        // deviation.
        let mut peak_dev = 0.0f32;
        for _ in 0..((2.0 * SR) as usize) {
            v.tick();
            peak_dev = peak_dev.max((v.current_freq_hz - f0).abs() / f0);
        }
        // Peak drift should be small — ~3 cents ≈ 0.17% — but nonzero.
        assert!(
            peak_dev > 1e-5 && peak_dev < 0.01,
            "drift out of range: {peak_dev}"
        );
    }

    #[test]
    fn slide_does_not_retrigger_amp() {
        let mut v = Voice303::new(SR);
        let p = VoiceParams::default();
        v.trigger(57, false, &p);
        for _ in 0..200 {
            v.tick();
        } // into decay phase
        let gain_before = {
            let mut g = 0.0f32;
            for _ in 0..64 {
                g = g.max(v.tick().abs());
            }
            g
        };
        v.slide_to(60, false, &p);
        // Immediately after slide, amp env should still be in decay, not
        // attack. Gain envelope should be monotonically non-increasing
        // relative to pre-slide within reason.
        let mut g_after = 0.0f32;
        for _ in 0..64 {
            g_after = g_after.max(v.tick().abs());
        }
        assert!(
            g_after <= gain_before + 0.2,
            "slide should not retrigger attack: before={gain_before}, after={g_after}"
        );
    }

    /// Full-pipeline audio diagnostic at 44100 Hz (standalone rate).
    /// Renders voice + FX for 2 seconds and checks for common issues.
    #[test]
    fn full_pipeline_diagnostic_44100() {
        use crate::dsp::fx::fx_chain::{FxChain, FxParams};
        use crate::dsp::fx::delay::SyncDiv;

        let sr = 44_100.0;
        let mut voice = Voice303::new(sr);
        voice.set_quality(QualityMode::High);
        let mut fx = FxChain::new(sr);
        fx.set_delay_tempo(130.0, SyncDiv::Eighth);

        let vp = VoiceParams::default();
        let live = VoiceLiveParams {
            waveform: Waveform::Saw,
            base_cutoff_hz: 500.0,
            resonance: 0.6,
            env_mod: 0.6,
            accent_amount: 0.6,
            tuning_semitones: 0.0,
            decay_ms: 200.0,
            slide_ms: 60.0,
        };
        // All FX off (default plugin state)
        let fx_off = FxParams::default();
        // FX on (distortion + delay + reverb)
        let fx_on = FxParams {
            dist_enable: true,
            dist_drive: 0.5,
            dist_mix: 1.0,
            delay_enable: true,
            delay_feedback: 0.4,
            delay_mix: 0.3,
            delay_mode: crate::dsp::fx::delay::DelayMode::Analog,
            reverb_enable: true,
            reverb_decay: 0.4,
            reverb_mix: 0.2,
        };

        for (label, fx_params) in [("FX off", &fx_off), ("FX on", &fx_on)] {
            voice.reset();
            fx.reset();
            voice.set_quality(QualityMode::High);

            let total = (sr * 2.0) as usize;
            let mut buf = Vec::with_capacity(total);

            voice.trigger(36, true, &vp);
            for i in 0..total {
                voice.set_live(&live);
                let v = voice.tick();
                let out = fx.process(v, fx_params);
                buf.push(out);
                if i == (sr * 0.3) as usize { voice.gate_off(); }
                if i == (sr * 0.5) as usize { voice.trigger(48, false, &vp); }
                if i == (sr * 0.8) as usize { voice.slide_to(36, true, &vp); }
                if i == (sr * 1.2) as usize { voice.gate_off(); }
            }

            let nan_count = buf.iter().filter(|x| !x.is_finite()).count();
            let peak = buf.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
            let dc = buf.iter().sum::<f32>() / buf.len() as f32;
            let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();

            let mut max_jump = 0.0f32;
            let mut big_jumps = 0usize;
            for w in buf.windows(2) {
                let j = (w[1] - w[0]).abs();
                if j > max_jump { max_jump = j; }
                if j > 0.5 { big_jumps += 1; }
            }

            // Tail: last 0.5s should be near-silent
            let tail_start = (sr * 1.5) as usize;
            let tail_peak = buf[tail_start..].iter().fold(0.0f32, |a, &b| a.max(b.abs()));

            eprintln!("--- {label} @ {sr} Hz ---");
            eprintln!("  NaN/Inf: {nan_count}");
            eprintln!("  Peak: {peak:.4}");
            eprintln!("  DC: {dc:.6}");
            eprintln!("  RMS: {rms:.4}");
            eprintln!("  Max jump: {max_jump:.4}");
            eprintln!("  Jumps > 0.5: {big_jumps}");
            eprintln!("  Tail peak (last 0.5s): {tail_peak:.6}");

            assert_eq!(nan_count, 0, "{label}: NaN/Inf in output");
            assert!(peak < 5.0, "{label}: peak too high: {peak}");
            assert!(dc.abs() < 0.05, "{label}: DC offset: {dc}");
            assert!(big_jumps < 20, "{label}: too many discontinuities: {big_jumps}");
        }
    }
}
