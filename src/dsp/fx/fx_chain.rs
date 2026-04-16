//! FX chain wrapper: Distortion → Delay → Reverb → LoudnessComp → Limiter.
//!
//! Single `process()` entry point driven per-sample from `plugin.rs`.

use super::delay::{Delay, DelayMode, SyncDiv};
use super::distortion::Distortion;
use super::limiter::Limiter;
use super::loudness_comp::LoudnessComp;
use super::reverb::Reverb;

/// Per-sample FX snapshot, populated from smoothed params in `plugin.rs`.
#[derive(Clone, Copy, Debug)]
pub struct FxParams {
    pub dist_enable: bool,
    pub dist_drive: f32,
    pub dist_mix: f32,

    pub delay_enable: bool,
    pub delay_mode: DelayMode,
    pub delay_feedback: f32,
    pub delay_mix: f32,

    pub reverb_enable: bool,
    pub reverb_decay: f32,
    pub reverb_mix: f32,
}

impl Default for FxParams {
    fn default() -> Self {
        Self {
            dist_enable: false,
            dist_drive: 0.5,
            dist_mix: 1.0,
            delay_enable: false,
            delay_mode: DelayMode::Analog,
            delay_feedback: 0.4,
            delay_mix: 0.3,
            reverb_enable: false,
            reverb_decay: 0.4,
            reverb_mix: 0.2,
        }
    }
}

pub struct FxChain {
    pub distortion: Distortion,
    pub delay: Delay,
    pub reverb: Reverb,
    pub loudness_comp: LoudnessComp,
    pub limiter: Limiter,
}

impl FxChain {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            distortion: Distortion::new(),
            delay: Delay::new(sample_rate),
            reverb: Reverb::new(sample_rate),
            loudness_comp: LoudnessComp::new(sample_rate),
            limiter: Limiter::new(sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.delay.set_sample_rate(sr);
        self.reverb.set_sample_rate(sr);
        self.loudness_comp.set_sample_rate(sr);
        self.limiter.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.distortion.reset();
        self.delay.reset();
        self.reverb.reset();
        self.loudness_comp.reset();
        self.limiter.reset();
    }

    /// Update delay tempo. Call once per block from `plugin.rs`.
    pub fn set_delay_tempo(&mut self, bpm: f32, div: SyncDiv) {
        self.delay.set_tempo(bpm, div);
    }

    /// Process a single sample through the full chain.
    #[inline]
    pub fn process(&mut self, input: f32, params: &FxParams) -> f32 {
        let mut s = input;

        // Stage 1: Distortion
        if params.dist_enable {
            s = self.distortion.process(s, params.dist_drive, params.dist_mix);
        }

        // Stage 2: Delay
        if params.delay_enable {
            s = self.delay.process(s, params.delay_feedback, params.delay_mix, params.delay_mode);
        }

        // Stage 3: Reverb
        if params.reverb_enable {
            self.reverb.set_decay(params.reverb_decay);
            s = self.reverb.process(s, params.reverb_mix);
        }

        // Stage 4: Loudness comp (always on, tames post-reverb swings)
        s = self.loudness_comp.process(s);

        // Stage 5: Limiter (always on)
        self.limiter.process(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn all_bypassed_preserves_signal() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Eighth);
        let params = FxParams::default(); // all off
        let input = 0.3;
        let mut out = 0.0;
        for _ in 0..100 {
            out = chain.process(input, &params);
        }
        // Loudness comp adds ~2 dB makeup; signal should be present
        // and slightly boosted, not attenuated or zeroed.
        assert!(
            out > input * 0.9 && out < input * 2.0,
            "bypassed chain should preserve signal, got {out}"
        );
    }

    #[test]
    fn distortion_only() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Eighth);
        let params = FxParams {
            dist_enable: true,
            dist_drive: 0.8,
            dist_mix: 1.0,
            ..Default::default()
        };
        let out = chain.process(0.5, &params);
        assert!(out.abs() > 0.01, "distortion should produce output");
        assert!(out.is_finite());
    }

    #[test]
    fn delay_only() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Eighth);
        let params = FxParams {
            delay_enable: true,
            delay_feedback: 0.5,
            delay_mix: 0.5,
            ..Default::default()
        };
        chain.process(1.0, &params);
        let mut found_repeat = false;
        for _ in 1..24_000 {
            let out = chain.process(0.0, &params);
            if out.abs() > 0.1 {
                found_repeat = true;
                break;
            }
        }
        assert!(found_repeat, "delay should produce a repeat");
    }

    #[test]
    fn reverb_only() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Eighth);
        let params = FxParams {
            reverb_enable: true,
            reverb_decay: 0.5,
            reverb_mix: 0.5,
            ..Default::default()
        };
        chain.process(1.0, &params);
        let mut found_tail = false;
        for _ in 1..24_000 {
            let out = chain.process(0.0, &params);
            if out.abs() > 0.001 {
                found_tail = true;
                break;
            }
        }
        assert!(found_tail, "reverb should produce a tail");
    }

    #[test]
    fn full_chain_no_nan() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Eighth);
        let params = FxParams {
            dist_enable: true,
            dist_drive: 1.0,
            dist_mix: 1.0,
            delay_enable: true,
            delay_feedback: 0.9,
            delay_mix: 0.5,
            delay_mode: DelayMode::Analog,
            reverb_enable: true,
            reverb_decay: 1.0,
            reverb_mix: 0.5,
        };
        for _ in 0..96_000 {
            let out = chain.process(1.0, &params);
            assert!(out.is_finite(), "full chain produced NaN/inf: {out}");
        }
    }

    #[test]
    fn limiter_catches_stacked_gain() {
        let mut chain = FxChain::new(SR);
        chain.set_delay_tempo(120.0, SyncDiv::Sixteenth);
        let params = FxParams {
            dist_enable: true,
            dist_drive: 1.0,
            dist_mix: 1.0,
            delay_enable: true,
            delay_feedback: 0.9,
            delay_mix: 1.0,
            delay_mode: DelayMode::Clean,
            reverb_enable: true,
            reverb_decay: 1.0,
            reverb_mix: 1.0,
        };
        let ceiling = nih_plug::util::db_to_gain(-0.3);
        for _ in 0..100 {
            chain.process(1.0, &params);
        }
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let out = chain.process(0.0, &params);
            peak = peak.max(out.abs());
        }
        assert!(
            peak < ceiling * 3.0,
            "limiter should catch stacked gain: peak={peak}, ceiling={ceiling}"
        );
    }
}
