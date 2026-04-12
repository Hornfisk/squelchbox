//! Stomp-box distortion — asymmetric diode-pair waveshaper.
//!
//! Single acid-tuned mode. Structured so additional modes (clip, tape)
//! can be added as enum variants without API changes.

/// Drive knob at 1.0 maps to `1 + DRIVE_GAIN_RANGE`× input gain.
const DRIVE_GAIN_RANGE: f32 = 19.0;

/// Asymmetry factor for the negative lobe of the diode waveshaper.
/// Values < 1.0 make the negative half clip harder, producing even harmonics.
const DIODE_NEG_ASYM: f32 = 0.8;

/// Clamp input to the diode `.exp()` to prevent overflow.
const DIODE_EXP_CLAMP: f32 = 20.0;

pub struct Distortion;

impl Distortion {
    pub fn new() -> Self {
        Self
    }

    pub fn reset(&mut self) {
        // Stateless waveshaper — nothing to reset.
    }

    /// Process a single sample. `drive`: 0.0–1.0, `mix`: 0.0–1.0.
    #[inline]
    pub fn process(&mut self, input: f32, drive: f32, mix: f32) -> f32 {
        if mix <= 0.0 || drive <= 1e-4 {
            return input;
        }
        let gain = 1.0 + drive * DRIVE_GAIN_RANGE;
        let driven = (input * gain).clamp(-DIODE_EXP_CLAMP, DIODE_EXP_CLAMP);

        let saturated = if driven >= 0.0 {
            1.0 - (-driven).exp()
        } else {
            -(1.0 - driven.exp()) * DIODE_NEG_ASYM
        };

        // Makeup gain: compensate for level loss from clipping.
        let makeup = 1.0 / gain.sqrt();
        let wet = saturated * makeup;

        input * (1.0 - mix) + wet * mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_in_silence_out() {
        let mut dist = Distortion::new();
        for _ in 0..1000 {
            let out = dist.process(0.0, 0.5, 1.0);
            assert_eq!(out, 0.0, "distortion should output silence for silent input");
        }
    }

    #[test]
    fn zero_mix_is_dry_passthrough() {
        let mut dist = Distortion::new();
        let input = 0.6;
        let out = dist.process(input, 1.0, 0.0);
        assert!((out - input).abs() < 1e-6, "mix=0 should be dry, got {out}");
    }

    #[test]
    fn full_mix_produces_saturated_output() {
        let mut dist = Distortion::new();
        let out = dist.process(0.5, 1.0, 1.0);
        assert!(out.abs() > 0.01, "should produce nonzero output, got {out}");
        assert!(out.abs() < 2.0, "should be bounded, got {out}");
    }

    #[test]
    fn asymmetric_produces_even_harmonics() {
        let mut dist = Distortion::new();
        let pos = dist.process(0.5, 0.7, 1.0).abs();
        dist.reset();
        let neg = dist.process(-0.5, 0.7, 1.0).abs();
        assert!(
            (pos - neg).abs() > 0.001,
            "diode should be asymmetric: pos={pos}, neg={neg}"
        );
    }

    #[test]
    fn no_nan_at_extreme_drive() {
        let mut dist = Distortion::new();
        for &drive in &[0.0, 0.5, 1.0] {
            for &input in &[-10.0, -1.0, 0.0, 1.0, 10.0] {
                let out = dist.process(input, drive, 1.0);
                assert!(out.is_finite(), "NaN/inf at input={input}, drive={drive}");
            }
        }
    }

    #[test]
    fn output_bounded_under_heavy_drive() {
        let mut dist = Distortion::new();
        let out = dist.process(1.0, 1.0, 1.0);
        assert!(out.abs() < 5.0, "output should be bounded, got {out}");
    }
}
