//! Stomp-box distortion — warm overdrive waveshaper.
//!
//! Tanh-based soft clipper with asymmetric bias for even+odd harmonics.
//! Shaped to complement the 303 voice: adds grit and body without
//! destroying the acid character. Single mode tuned for classic overdrive;
//! structured so additional modes can slot in as enum variants.

use crate::dsp::flush_denormal;

/// Drive knob at 1.0 maps to `1 + DRIVE_GAIN_RANGE`× input gain (~+24 dB).
const DRIVE_GAIN_RANGE: f32 = 15.0;

/// DC bias added before the waveshaper to create asymmetry (even harmonics).
/// Small enough to avoid audible DC offset after makeup gain.
const ASYM_BIAS: f32 = 0.15;

/// One-pole DC blocker coefficient (fc ≈ 5 Hz at 48 kHz).
const DC_BLOCK_R: f32 = 0.9993;

pub struct Distortion {
    // DC blocker state to remove any offset introduced by the asymmetric bias.
    dc_x1: f32,
    dc_y1: f32,
}

impl Distortion {
    pub fn new() -> Self {
        Self {
            dc_x1: 0.0,
            dc_y1: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.dc_x1 = 0.0;
        self.dc_y1 = 0.0;
    }

    /// Prime the DC-blocker to its silence-in steady state so the first
    /// sample after enabling doesn't produce a DC step click.
    pub fn prime_for_silence(&mut self) {
        self.dc_x1 = fast_tanh(ASYM_BIAS);
        self.dc_y1 = 0.0;
    }

    /// Process a single sample. `drive`: 0.0–1.0, `mix`: 0.0–1.0.
    #[inline]
    pub fn process(&mut self, input: f32, drive: f32, mix: f32) -> f32 {
        if mix <= 0.0 {
            return input;
        }

        let gain = 1.0 + drive * DRIVE_GAIN_RANGE;
        let driven = input * gain + ASYM_BIAS;

        // Warm tanh saturation — gradual, musical soft-clip.
        let saturated = fast_tanh(driven);

        // DC blocker: remove offset from asymmetric bias.
        let dc_blocked = saturated - self.dc_x1 + DC_BLOCK_R * self.dc_y1;
        self.dc_x1 = flush_denormal(saturated);
        self.dc_y1 = flush_denormal(dc_blocked);

        // Makeup gain: keep perceived loudness roughly constant across drive range.
        // tanh output is [-1, 1], so we scale to match the input level.
        let makeup = 1.0 / (1.0 + drive * 2.0);
        let wet = dc_blocked * makeup;

        input * (1.0 - mix) + wet * mix
    }
}

/// Fast tanh approximation (Pade 3/3). Accurate to ~0.001 across [-4, 4],
/// clips cleanly outside that range. Much cheaper than libm tanh.
#[inline]
fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-4.0, 4.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_in_silence_out() {
        let mut dist = Distortion::new();
        // Run enough samples for DC blocker to settle (~5 time constants).
        for _ in 0..10_000 {
            dist.process(0.0, 0.5, 1.0);
        }
        // After settling, output should be near zero.
        let out = dist.process(0.0, 0.5, 1.0);
        assert!(out.abs() < 0.005, "should be near silence after settling, got {out}");
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
        // Settle DC blocker
        for _ in 0..500 { dist.process(0.3, 1.0, 1.0); }
        let out = dist.process(0.5, 1.0, 1.0);
        assert!(out.abs() > 0.01, "should produce nonzero output, got {out}");
        assert!(out.abs() < 2.0, "should be bounded, got {out}");
    }

    #[test]
    fn asymmetric_waveshaper() {
        // At moderate levels the bias creates asymmetry:
        // positive input gets biased further positive (more saturation),
        // negative input gets biased toward zero (less saturation).
        let drive = 0.3;
        let gain = 1.0 + drive * DRIVE_GAIN_RANGE;
        let input = 0.3;
        let pos = fast_tanh(input * gain + ASYM_BIAS);
        let neg = fast_tanh(-input * gain + ASYM_BIAS);
        // pos should be larger magnitude than neg due to bias
        assert!(
            pos.abs() > neg.abs(),
            "positive lobe should saturate harder: pos={pos}, neg={neg}"
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
        for _ in 0..500 { dist.process(0.5, 1.0, 1.0); }
        let out = dist.process(1.0, 1.0, 1.0);
        assert!(out.abs() < 5.0, "output should be bounded, got {out}");
    }
}
